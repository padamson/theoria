//! theoria's own browser e2e: serve a gallery built from theoria's
//! components-as-fixtures and drive it with playwright-rs.
//!
//! The core check is generic — like Storybook's test-runner, it discovers every
//! story from the gallery's own sidebar at runtime and asserts each one renders
//! and selects. Add a story (a `*.stories.rs`) and it's covered automatically;
//! no compile-time list here. This is theoria dogfooding itself one level — never
//! a `Gallery` inside a `Gallery`.
//!
//! Every test needs the built gallery, and all but the lockfile check need
//! Chromium, so each is `#[ignore]`d and fails, naming the build command, when
//! run without the gallery. Install the browser once, build the gallery, then
//! run:
//!   cargo run --manifest-path crates/theoria-e2e/Cargo.toml --example install-browsers
//!   cargo run -p theoria-cli -- build --config theoria-e2e.toml
//!   cargo nextest run --manifest-path crates/theoria-e2e/Cargo.toml --config-file .config/nextest.toml --run-ignored all

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, ensure};
use axum::Router;
use playwright_rs::protocol::click::KeyboardModifier;
use playwright_rs::protocol::{Locator, Page, Playwright, SelectOption};
use playwright_rs::{ClickOptions, expect, expect_page};
use serde::Deserialize;
use tower_http::services::ServeDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gallery_dir() -> PathBuf {
    repo_root().join("target/theoria/gallery/theoria")
}

fn dist_dir() -> PathBuf {
    gallery_dir().join("dist")
}

/// A package as a lockfile pins it.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Locked {
    name: String,
    version: String,
    source: Option<String>,
}

#[derive(Deserialize)]
struct Lockfile {
    package: Vec<Locked>,
}

fn locked(lockfile: &Path) -> Result<BTreeSet<Locked>> {
    let text = std::fs::read_to_string(lockfile)
        .with_context(|| format!("read {}", lockfile.display()))?;
    let lock: Lockfile =
        toml::from_str(&text).with_context(|| format!("parse {}", lockfile.display()))?;
    Ok(lock.package.into_iter().collect())
}

async fn serve(dist: &PathBuf) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let app = Router::new().fallback_service(ServeDir::new(dist));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind gallery server")?;
    let addr = listener.local_addr().context("local addr")?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve gallery");
    });
    Ok((addr, handle))
}

/// Where the built gallery is served.
struct Site {
    base: String,
}

impl Site {
    /// The gallery's URL followed by `rest`, a query or nothing.
    fn at(&self, rest: &str) -> String {
        format!("{}/{rest}", self.base)
    }

    /// The gallery's URL with no story in it.
    fn bare(&self) -> String {
        self.at("")
    }

    /// The URL that opens the story named `name`.
    fn story(&self, name: &str) -> String {
        self.at(&story_query(name))
    }
}

/// The query that opens the story named `name`, built from the name as any
/// test that opens a story by URL builds it.
fn story_query(name: &str) -> String {
    let name: String = form_urlencoded::byte_serialize(name.as_bytes()).collect();
    format!("?story={}", name.replace("%2F", "/"))
}

/// Run `body` on a new page, with no URL opened yet, beside the built
/// gallery's address. Fails when the gallery is not built, and closes the
/// browser whether or not `body` passed.
async fn with_site(body: impl AsyncFnOnce(&Page, &Site) -> Result<()>) -> Result<()> {
    let dist = dist_dir();
    ensure!(
        dist.join("index.html").exists(),
        "{} not built; run cargo run -p theoria-cli -- build --config theoria-e2e.toml",
        dist.display()
    );

    let (addr, _server) = serve(&dist).await?;
    let site = Site {
        base: format!("http://{addr}"),
    };
    let pw = Playwright::launch().await.context("launch playwright")?;
    let browser = pw.chromium().launch().await.context("launch chromium")?;
    let result = async {
        let page = browser.new_page().await.context("new page")?;
        body(&page, &site).await
    }
    .await;
    let closed = browser.close().await.context("close browser");
    result.and(closed)
}

/// Run `body` on a page showing the built gallery at its bare URL.
async fn with_gallery(body: impl AsyncFnOnce(&Page) -> Result<()>) -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        body(page).await
    })
    .await
}

async fn open(page: &Page, url: &str) -> Result<()> {
    page.goto(url, None)
        .await
        .map(drop)
        .with_context(|| format!("open {url}"))
}

/// Every story's link in the gallery's own sidebar.
fn sidebar_links(page: &Page) -> Locator {
    page.locator(".theoria > .theoria-nav a")
}

/// The sidebar entry marked as the story on the stage.
fn current(page: &Page) -> Locator {
    page.locator(".theoria > .theoria-nav [aria-current=page]")
}

/// Waits for the sidebar to mark the story named `name`.
async fn expect_marked(page: &Page, name: &str) -> Result<()> {
    expect_marked_at(page, &story_query(name)).await
}

/// Waits for the sidebar to mark the story whose `href` is `query`.
async fn expect_marked_at(page: &Page, query: &str) -> Result<()> {
    expect(current(page))
        .to_have_attribute("href", query)
        .await
        .with_context(|| format!("the sidebar marks the story at {query:?}"))
}

/// The sidebar link to the story named `name`.
fn link(page: &Page, name: &str) -> Locator {
    link_to(page, &story_query(name))
}

/// The sidebar link whose `href` is `query`.
fn link_to(page: &Page, query: &str) -> Locator {
    page.locator(format!(r#".theoria > .theoria-nav a[href="{query}"]"#))
}

/// Selects the story named `name` from the sidebar and waits for it to be marked.
async fn click_story(page: &Page, name: &str) -> Result<()> {
    link(page, name)
        .click(None)
        .await
        .with_context(|| format!("click the story {name:?}"))?;
    expect_marked(page, name).await
}

/// Waits two animation frames (a second at most), so what a click set off has
/// been drawn.
async fn settle(page: &Page) -> Result<()> {
    page.evaluate::<(), ()>(
        "() => new Promise(r => { requestAnimationFrame(() => requestAnimationFrame(() => r(null))); setTimeout(() => r(null), 1000); })",
        None,
    )
    .await
    .context("wait two frames")
}

async fn select(page: &Page, story: &str) -> Result<()> {
    page.get_by_text(story, true)
        .click(None)
        .await
        .with_context(|| format!("select the story {story:?}"))
}

/// Pick the option labelled `label` from the panel's menu.
async fn choose(page: &Page, label: &str) -> Result<()> {
    page.locator(".theoria-panel select")
        .select_option(SelectOption::Label(label.to_string()), None)
        .await
        .map(drop)
        .with_context(|| format!("choose the option {label:?}"))
}

/// Fails when `what` needs more width than it shows.
async fn ensure_fits(locator: &Locator, what: &str) -> Result<()> {
    let (shown, needed): (f64, f64) = locator
        .evaluate("el => [el.clientWidth, el.scrollWidth]", None::<()>)
        .await
        .with_context(|| format!("measure {what}"))?;
    ensure!(
        needed <= shown,
        "{what} needs {needed}px and shows {shown}px"
    );
    Ok(())
}

/// The `href` of every story in the sidebar, once the gallery has mounted.
async fn every_story_query(page: &Page) -> Result<Vec<String>> {
    let links = sidebar_links(page);
    // `evaluate_all` does not wait, and the page loads before the app mounts,
    // so wait for the sidebar or the list reads empty.
    expect(links.first())
        .to_be_visible()
        .await
        .context("the sidebar lists a story")?;
    links
        .evaluate_all(
            "links => links.map(link => link.getAttribute('href'))",
            None::<()>,
        )
        .await
        .context("read every story's href")
}

async fn ensure_the_stage_shows_something(page: &Page, query: &str) -> Result<()> {
    let stage_children = page
        .locator(".theoria-stage *")
        .count()
        .await
        .context("count stage content")?;
    ensure!(
        stage_children > 0,
        "the story at {query:?} renders non-empty content"
    );
    Ok(())
}

/// Discover every story from the gallery sidebar and assert each one opens by
/// its URL and renders non-empty content on the stage. Generic over whatever
/// stories the gallery was built with.
async fn assert_every_story_opens_by_its_url(page: &Page, site: &Site) -> Result<()> {
    open(page, &site.bare()).await?;
    for query in &every_story_query(page).await? {
        open(page, &site.at(query)).await?;
        expect_marked_at(page, query).await?;
        ensure_the_stage_shows_something(page, query).await?;
    }
    Ok(())
}

/// Select every story from the sidebar in turn, on one page, and assert each
/// replaces the one before on the stage.
async fn assert_every_story_selects_from_the_sidebar(page: &Page) -> Result<()> {
    for query in &every_story_query(page).await? {
        link_to(page, query)
            .click(None)
            .await
            .with_context(|| format!("click {query:?}"))?;
        expect_marked_at(page, query).await?;
        ensure_the_stage_shows_something(page, query).await?;
    }
    Ok(())
}

/// `generate` puts a copy of the workspace's lockfile beside the app, and the
/// build has cargo rewrite it to what the app uses, the app itself included.
/// So the app's entry shows that a build happened, and anything else the
/// workspace's lockfile lacks was resolved without it.
///
/// A build that ignored the workspace's lockfile still passes while that
/// lockfile is level with the registry, so this fails only once it has
/// fallen behind. The CLI's own tests pin the copy.
#[test]
#[ignore = "needs the built gallery; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
fn the_gallery_is_built_from_what_the_workspace_lockfile_records() -> Result<()> {
    let gallery = gallery_dir().join("Cargo.lock");
    ensure!(
        gallery.exists(),
        "{} not built; run cargo run -p theoria-cli -- build --config theoria-e2e.toml",
        gallery.display()
    );
    let workspace = locked(&repo_root().join("Cargo.lock"))?;

    let unrecorded: Vec<Locked> = locked(&gallery)?.difference(&workspace).cloned().collect();

    let names: Vec<&str> = unrecorded.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["theoria-gallery"],
        "rebuild the gallery if the workspace's Cargo.lock has moved since; got {unrecorded:?}"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn every_story_opens_by_its_url_and_renders() -> Result<()> {
    with_site(assert_every_story_opens_by_its_url).await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn every_story_selects_from_the_sidebar_and_renders() -> Result<()> {
    with_gallery(assert_every_story_selects_from_the_sidebar).await
}

/// The controls panel drives the stage live: editing a knob re-renders the
/// story view, and the show-code block carries the captured source. This is what
/// leptotes's SSR snapshots can't prove — the interactive re-render.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn controls_drive_the_stage_live() -> Result<()> {
    with_gallery(async |page: &Page| {
        // Select the knobs demo (a `#[story]` with args → renders a controls panel).
        select(page, "Knobs · demo").await?;

        // Default `on = true` → the stage shows "ON".
        expect(page.locator(".theoria-stage .k-flag"))
            .to_have_text("ON")
            .await
            .context("flag starts ON")?;

        // Toggle the bool control off; the stage re-renders to "OFF".
        page.locator(".theoria-panel input[type=checkbox]")
            .click(None)
            .await
            .context("toggle the flag control")?;
        expect(page.locator(".theoria-stage .k-flag"))
            .to_have_text("OFF")
            .await
            .context("toggling the control re-renders the stage")?;

        // Show-code carries the captured source.
        expect(page.locator(".theoria-panel .theoria-code summary"))
            .to_have_text("Show code")
            .await
            .context("show-code toggle present")?;

        // Autodocs: the doc-comment renders as Markdown (bold + a list), and
        // each control row documents its arg's type.
        let strong = page
            .locator(".theoria-panel .theoria-md strong")
            .count()
            .await
            .context("count rendered <strong>")?;
        ensure!(strong > 0, "description renders Markdown emphasis");
        let items = page
            .locator(".theoria-panel .theoria-md li")
            .count()
            .await
            .context("count rendered <li>")?;
        ensure!(items > 0, "description renders a Markdown list");
        let types = page
            .locator(".theoria-panel .theoria-controls .control-type")
            .all_inner_texts()
            .await
            .context("read each arg's type")?;
        ensure!(
            types.iter().any(|t| t.trim() == "bool"),
            "the controls document the bool arg, got {types:?}"
        );

        Ok(())
    })
    .await
}

/// Leptos patches the stage's elements in place when a body reruns, so only
/// state the body made tells a rerun from none.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_edit_to_a_signal_arg_keeps_the_story_state() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · signals").await?;
        let clicks = page.locator(".theoria-stage .k-clicks");
        clicks
            .click(None)
            .await
            .context("click the story's own button")?;
        expect(clicks.clone())
            .to_have_text("clicked 1")
            .await
            .context("the story counts the click")?;

        page.locator(".theoria-panel input[type=checkbox]")
            .click(None)
            .await
            .context("toggle the flag control")?;
        expect(page.locator(".theoria-stage .k-flag"))
            .to_have_text("OFF")
            .await
            .context("the edit reaches the stage")?;
        expect(clicks)
            .to_have_text("clicked 1")
            .await
            .context("the story's count survives the edit")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn moving_a_bounded_slider_moves_its_number_box() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        page.locator(".theoria-panel input[type=range]")
            .fill("0.5", None)
            .await
            .context("move the slider to 0.5")?;

        expect(page.locator(".theoria-panel input[type=number]"))
            .to_have_value("0.5")
            .await
            .context("the number box follows the slider")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn moving_a_bounded_slider_reaches_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        page.locator(".theoria-panel input[type=range]")
            .fill("0.5", None)
            .await
            .context("move the slider to 0.5")?;

        expect(page.locator(".theoria-stage .k-radius"))
            .to_have_text("0.5")
            .await
            .context("the stage shows the slider's value")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_number_typed_out_of_range_shows_the_bound_when_it_loses_focus() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("7", None)
            .await
            .context("type 7, above the maximum")?;
        number.blur().await.context("leave the number box")?;

        expect(number)
            .to_have_value("1")
            .await
            .context("the number box shows the maximum")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_number_typed_out_of_range_moves_the_slider_to_the_bound() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("7", None)
            .await
            .context("type 7, above the maximum")?;

        expect(page.locator(".theoria-panel input[type=range]"))
            .to_have_value("1")
            .await
            .context("the slider shows the maximum")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_number_reaches_the_story_clamped_while_it_is_typed() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("7", None)
            .await
            .context("type 7, above the maximum")?;

        expect(page.locator(".theoria-stage .k-radius"))
            .to_have_text("1")
            .await
            .context("the story gets the maximum before the box loses focus")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_number_keeps_its_text_while_it_is_typed() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("7", None)
            .await
            .context("type 7, above the maximum")?;
        expect(page.locator(".theoria-stage .k-radius"))
            .to_have_text("1")
            .await
            .context("the story has taken the clamped value")?;

        expect(number)
            .to_have_value("7")
            .await
            .context("the typed text is left alone")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_number_typed_between_steps_reaches_the_story_on_a_step() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("0.3", None)
            .await
            .context("type 0.3, between steps of 0.0625")?;

        expect(page.locator(".theoria-stage .k-radius"))
            .to_have_text("0.3125")
            .await
            .context("the story gets the nearest step")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_emptied_bounded_number_shows_the_value_again_when_it_loses_focus() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("", None)
            .await
            .context("empty the number box")?;
        number.blur().await.context("leave the number box")?;

        expect(number)
            .to_have_value("0.25")
            .await
            .context("the box shows the arg's value again")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bounded_slider_is_whole_in_the_panel_and_clear_of_its_number_box() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let panel = page
            .locator(".theoria-panel")
            .bounding_box()
            .await?
            .context("the panel is laid out")?;
        let number = page
            .locator(".theoria-panel input[type=number]")
            .bounding_box()
            .await?
            .context("the number box is laid out")?;
        let slider = page
            .locator(".theoria-panel input[type=range]")
            .bounding_box()
            .await?
            .context("the slider is laid out")?;

        let inside = slider.x >= panel.x && slider.x + slider.width <= panel.x + panel.width;
        let beside = slider.x >= number.x + number.width;
        let below = slider.y >= number.y + number.height;
        ensure!(
            inside && (beside || below),
            "the slider runs out of the panel or overlaps the number box: \
             panel {panel:?}, number {number:?}, slider {slider:?}"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_emptied_number_shows_the_value_again_when_it_loses_focus() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .fill("", None)
            .await
            .context("empty the count control")?;
        number.blur().await.context("leave the count control")?;

        expect(number)
            .to_have_value("2")
            .await
            .context("the count control shows the arg's value again")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_number_box_shows_the_next_storys_value() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number.fill("5", None).await.context("type 5 into count")?;
        select(page, "Knobs · signals").await?;

        expect(number)
            .to_have_value("2")
            .await
            .context("the box shows the newly selected story's count")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn pressing_enter_part_way_through_a_number_keeps_what_is_typed() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number
            .clear(None)
            .await
            .context("clear the count control")?;
        number
            .press_sequentially("1.", None)
            .await
            .context("type 1.")?;
        number.press("Enter", None).await.context("press Enter")?;
        number
            .press_sequentially("5", None)
            .await
            .context("type 5")?;

        expect(number)
            .to_have_value("1.5")
            .await
            .context("the count control shows what was typed")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn picking_a_color_writes_its_hex_into_the_text() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let swatch = page.locator(".theoria-panel input[type=color]");
        let text = page.locator(".theoria-panel input[type=text]");
        swatch.fill("#ff8000", None).await.context("pick #ff8000")?;

        expect(text)
            .to_have_value("#ff8000")
            .await
            .context("the text field holds the picked color")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn picking_a_color_reaches_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let swatch = page.locator(".theoria-panel input[type=color]");
        swatch.fill("#ff8000", None).await.context("pick #ff8000")?;

        expect(page.locator(".theoria-stage .k-ink"))
            .to_have_text("#ff8000")
            .await
            .context("the story gets the picked color")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_color_typed_as_a_function_reaches_the_story_as_written() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let text = page.locator(".theoria-panel input[type=text]");
        text.fill("rgb(36 56 50 / 0.8)", None)
            .await
            .context("type an rgb() color")?;

        expect(page.locator(".theoria-stage .k-ink"))
            .to_have_text("rgb(36 56 50 / 0.8)")
            .await
            .context("the story gets the color as typed")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_color_typed_into_the_text_paints_the_swatch() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        page.locator(".theoria-panel input[type=text]")
            .fill("rgb(36 56 50 / 0.8)", None)
            .await
            .context("type an rgb() color")?;

        expect(page.locator(".theoria-panel .theoria-swatch span"))
            .to_have_css("background-color", "rgba(36, 56, 50, 0.8)")
            .await
            .context("the swatch shows the typed color")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn text_that_is_not_a_color_empties_the_swatch() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        page.locator(".theoria-panel input[type=text]")
            .fill("notacolor", None)
            .await
            .context("type text that is not a color")?;

        expect(page.locator(".theoria-panel .theoria-swatch span"))
            .to_have_css("background-color", "rgba(0, 0, 0, 0)")
            .await
            .context("the swatch shows no color")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn the_picker_opens_at_an_opaque_color() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let swatch = page.locator(".theoria-panel input[type=color]");
        page.locator(".theoria-panel input[type=text]")
            .fill("teal", None)
            .await
            .context("type teal")?;
        swatch
            .dispatch_event("click", None)
            .await
            .context("open the picker")?;

        expect(swatch)
            .to_have_value("#008080")
            .await
            .context("the picker starts at teal")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn the_picker_opens_one_step_off_a_color_with_alpha() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let swatch = page.locator(".theoria-panel input[type=color]");
        page.locator(".theoria-panel input[type=text]")
            .fill("rgb(36 56 50 / 0.8)", None)
            .await
            .context("type an rgb() color with alpha")?;
        swatch
            .dispatch_event("click", None)
            .await
            .context("open the picker")?;

        expect(swatch)
            .to_have_value("#243833")
            .await
            .context("the picker starts one step of blue off the color")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_color_the_picker_cannot_hold_logs_nothing_to_the_console() -> Result<()> {
    with_gallery(async |page: &Page| {
        let logged = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&logged);
        page.on_console(move |msg| {
            let log = Arc::clone(&log);
            async move {
                log.lock()
                    .expect("the console log")
                    .push(msg.text().to_string());
                Ok(())
            }
        })
        .await
        .context("listen to the console")?;
        select(page, "Knobs · color").await?;
        page.locator(".theoria-panel input[type=text]")
            .fill("teal", None)
            .await
            .context("type teal")?;
        expect(page.locator(".theoria-stage .k-ink"))
            .to_have_text("teal")
            .await
            .context("the story gets the color")?;

        let logged = logged.lock().expect("the console log").clone();
        ensure!(logged.is_empty(), "the console logged {logged:?}");

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_color_text_field_shows_the_whole_of_a_color_function() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · color").await?;
        let text = page.locator(".theoria-panel input[type=text]");
        text.fill("rgb(36 56 50 / 0.8)", None)
            .await
            .context("type an rgb() color")?;

        ensure_fits(&text, "the text field").await?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_control_sits_under_its_args_name_and_type() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · bounded").await?;
        let name = page
            .locator(".theoria-panel .control-name")
            .bounding_box()
            .await?
            .context("the arg's name is laid out")?;
        let ty = page
            .locator(".theoria-panel .control-type")
            .bounding_box()
            .await?
            .context("the arg's type is laid out")?;
        let control = page
            .locator(".theoria-panel .control-input")
            .bounding_box()
            .await?
            .context("the control is laid out")?;

        let head = (name.y + name.height).max(ty.y + ty.height);
        ensure!(
            control.y >= head,
            "the control is beside its name or type: \
             name {name:?}, type {ty:?}, control {control:?}"
        );

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_choice_control_starts_at_the_storys_selected_option() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · choice").await?;

        expect(page.locator(".theoria-panel select option:checked"))
            .to_have_text("0.13.0")
            .await
            .context("the menu shows the story's selected option")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn choosing_an_option_reaches_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · choice").await?;
        choose(page, "0.14.0").await?;

        expect(page.locator(".theoria-stage .k-build"))
            .to_have_text("0.14.0")
            .await
            .context("the story gets the chosen build")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn choosing_a_labelled_option_gives_the_story_the_value_under_it() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · labelled choice").await?;
        choose(page, "Serif").await?;

        expect(page.locator(".theoria-stage .k-font"))
            .to_have_text("Georgia, serif")
            .await
            .context("the story gets the font stack under the label")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_choice_control_shows_the_next_storys_selection() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · choice").await?;
        choose(page, "dev").await?;
        select(page, "Knobs · labelled choice").await?;

        expect(page.locator(".theoria-panel select option:checked"))
            .to_have_text("Sans")
            .await
            .context("the menu shows the newly selected story's option")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_choice_control_keeps_its_selection_when_its_story_is_selected_again() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · choice").await?;
        choose(page, "0.14.0").await?;
        select(page, "Knobs · labelled choice").await?;
        select(page, "Knobs · choice").await?;

        expect(page.locator(".theoria-panel select option:checked"))
            .to_have_text("0.14.0")
            .await
            .context("the menu shows the option chosen before")?;

        Ok(())
    })
    .await
}

/// The heights of the first and the last element `selector` finds on the
/// stage: a row with a short name and type, then one with long ones.
async fn first_and_last_heights(page: &Page, selector: &str) -> Result<(f64, f64)> {
    let found = page.locator(format!(".theoria-stage {selector}"));
    let short = found
        .first()
        .bounding_box()
        .await?
        .context("the short one is laid out")?;
    let long = found
        .last()
        .bounding_box()
        .await?
        .context("the long one is laid out")?;
    Ok((short.height, long.height))
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_arg_name_too_long_for_one_line_wraps_inside_the_controls() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Controls · panel").await?;

        let (short, long) = first_and_last_heights(page, ".control-name").await?;
        ensure!(
            long > short,
            "the long name is on one line: {long}px tall beside a short name's {short}px"
        );
        ensure_fits(
            &page.locator(".theoria-stage .theoria-controls"),
            "the list of controls",
        )
        .await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_arg_type_too_long_for_one_line_wraps_inside_the_controls() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Controls · panel").await?;

        let (short, long) = first_and_last_heights(page, ".control-type").await?;
        ensure!(
            long > short,
            "the long type is on one line: {long}px tall beside a short type's {short}px"
        );
        ensure_fits(
            &page.locator(".theoria-stage .theoria-controls"),
            "the list of controls",
        )
        .await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_control_is_found_by_its_args_name() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · choice").await?;

        expect(page.get_by_label("build", true))
            .to_have_value("1")
            .await
            .context("the menu labelled build holds the story's selection")?;

        Ok(())
    })
    .await
}

/// A decimal typed key by key into a number control stays as typed and
/// reaches the story. Each keystroke fires `input`, and Chromium reports a
/// typed `1.` as `1`, so a control that writes the parsed value back drops the
/// `.`, moves the caret to the start, and turns the next key into `51`.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_number_control_keeps_a_decimal_typed_key_by_key() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        let count = page.locator(".theoria-panel input[type=number]");
        count.clear(None).await.context("clear the count control")?;
        count
            .press_sequentially("1.5", None)
            .await
            .context("type 1.5 into the count control")?;

        expect(count)
            .to_have_value("1.5")
            .await
            .context("the count control shows what was typed")?;
        expect(page.locator(".theoria-stage .k-count"))
            .to_have_text("1.5")
            .await
            .context("the stage shows what was typed")?;

        Ok(())
    })
    .await
}

/// What keeps you on your story across Trunk's hot reload.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_reload_keeps_the_selected_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        click_story(page, "StoryNav/Flat/Single").await?;
        page.reload(None).await.context("reload the page")?;
        expect_marked(page, "StoryNav/Flat/Single").await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_url_opens_its_story_when_another_shares_its_last_segment() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.story("StoryNav/Grouped/Single")).await?;
        expect(page.locator(".theoria-stage [data-testid=fixture-grouped-single]"))
            .to_have_count(1)
            .await
            .context("the stage shows StoryNav/Grouped/Single")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_url_naming_a_story_wins_over_the_stored_one() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        click_story(page, "StoryNav/Flat/Single").await?;
        open(page, &site.story("StoryNav/Grouped/Single")).await?;
        expect_marked(page, "StoryNav/Grouped/Single").await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bare_url_opens_the_stored_story() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        click_story(page, "StoryNav/Flat/Single").await?;
        open(page, &site.bare()).await?;
        expect_marked(page, "StoryNav/Flat/Single").await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_bare_url_opens_the_first_story_when_nothing_is_stored() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        let first = sidebar_links(page)
            .first()
            .get_attribute("href")
            .await
            .context("read the first story's href")?
            .context("the first sidebar link has an href")?;
        expect_marked_at(page, &first).await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_url_naming_no_story_says_so_on_the_stage() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.story("Knobs · gone")).await?;
        expect(page.locator(".theoria-stage .theoria-missing"))
            .to_contain_text("Knobs · gone")
            .await
            .context("the stage names the story the URL asked for")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_url_naming_no_story_marks_nothing_in_the_sidebar() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.story("Knobs · gone")).await?;
        expect(page.locator(".theoria-stage .theoria-missing"))
            .to_have_count(1)
            .await
            .context("the gallery has mounted")?;
        expect(current(page))
            .to_have_count(0)
            .await
            .context("no sidebar entry is marked")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn selecting_a_story_points_the_url_at_it() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        click_story(page, "StoryNav/Grouped/Three").await?;
        expect_page(page)
            .to_have_url(&site.story("StoryNav/Grouped/Three"))
            .await
            .context("the URL names the selected story")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn selecting_a_story_adds_no_history_entry() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.bare()).await?;
        expect(current(page))
            .to_have_count(1)
            .await
            .context("the gallery has mounted")?;
        let history = async || -> Result<u32> {
            page.evaluate::<(), u32>("() => history.length", None)
                .await
                .context("read history.length")
        };
        let before = history().await?;
        click_story(page, "StoryNav/Grouped/Three").await?;
        click_story(page, "StoryNav/Flat/Single").await?;
        assert_eq!(
            history().await?,
            before,
            "history.length after two selections"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_shift_click_opens_the_story_in_a_new_window() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.story("StoryNav/Flat/Three")).await?;
        expect_marked(page, "StoryNav/Flat/Three").await?;
        let opened = page
            .context()
            .context("the page's context")?
            .expect_page(Some(5_000.0))
            .await
            .context("wait for a new page")?;
        link(page, "StoryNav/Flat/Single")
            .click(Some(
                ClickOptions::builder()
                    .modifiers(vec![KeyboardModifier::Shift])
                    .build(),
            ))
            .await
            .context("shift-click another story")?;
        let window = opened
            .wait()
            .await
            .context("the browser opens a new window")?;
        expect_page(&window)
            .to_have_url(&site.story("StoryNav/Flat/Single"))
            .await
            .context("the new window opens the story")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_click_with_a_modifier_key_leaves_the_selection() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.story("StoryNav/Flat/Three")).await?;
        expect_marked(page, "StoryNav/Flat/Three").await?;
        // Alt is left out: Chromium downloads the link on an Alt-click.
        for modifier in [
            KeyboardModifier::Shift,
            KeyboardModifier::Meta,
            KeyboardModifier::Control,
        ] {
            let click = ClickOptions::builder().modifiers(vec![modifier]).build();
            link(page, "StoryNav/Flat/Single")
                .click(Some(click))
                .await
                .with_context(|| format!("{modifier:?}-click another story"))?;
            settle(page).await?;
            expect_marked(page, "StoryNav/Flat/Three")
                .await
                .with_context(|| format!("after a {modifier:?}-click"))?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn selecting_a_story_keeps_the_rest_of_the_url() -> Result<()> {
    with_site(async |page: &Page, site: &Site| {
        open(page, &site.at("?theme=dark&story=Knobs+%C2%B7+demo#panel")).await?;
        click_story(page, "StoryNav/Flat/Single").await?;
        expect_page(page)
            .to_have_url(&site.at("?theme=dark&story=StoryNav/Flat/Single#panel"))
            .await
            .context("only the story changes in the URL")
    })
    .await
}
