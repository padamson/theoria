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

use anyhow::{Context, Result, ensure};
use axum::Router;
use playwright_rs::protocol::click::KeyboardModifier;
use playwright_rs::protocol::{
    AriaRole, BoundingBox, GetByRoleOptions, Locator, Page, Playwright, SelectOption, Viewport,
};
use playwright_rs::{ClickOptions, DragToOptions, Position, expect, expect_page};
use serde::Deserialize;
use tokio::sync::mpsc::UnboundedReceiver;
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

fn reset_button(page: &Page) -> Locator {
    page.locator(".theoria-panel .theoria-reset")
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn reset_puts_every_arg_of_the_story_back_to_its_default() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        page.locator(".theoria-panel input[type=checkbox]")
            .click(None)
            .await
            .context("toggle the flag control")?;
        page.locator(".theoria-panel input[type=text]")
            .fill("bye", None)
            .await
            .context("type a label")?;
        page.locator(".theoria-panel input[type=number]")
            .fill("7", None)
            .await
            .context("type a count")?;
        for (class, edited) in [("k-flag", "OFF"), ("k-label", "bye"), ("k-count", "7")] {
            expect(page.locator(format!(".theoria-stage .{class}")))
                .to_have_text(edited)
                .await
                .with_context(|| format!("the stage's .{class} shows the edit {edited:?}"))?;
        }

        reset_button(page)
            .click(None)
            .await
            .context("click Reset")?;

        for (class, default) in [("k-flag", "ON"), ("k-label", "hi"), ("k-count", "2")] {
            expect(page.locator(format!(".theoria-stage .{class}")))
                .to_have_text(default)
                .await
                .with_context(|| format!("the stage's .{class} shows {default:?} again"))?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn reset_shows_the_default_in_a_number_box_typed_in() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · demo").await?;
        let number = page.locator(".theoria-panel input[type=number]");
        number.fill("7", None).await.context("type a count")?;
        expect(page.locator(".theoria-stage .k-count"))
            .to_have_text("7")
            .await
            .context("the count reaches the stage")?;

        reset_button(page)
            .click(None)
            .await
            .context("click Reset")?;

        expect(number)
            .to_have_value("2")
            .await
            .context("the number box shows the default")?;
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

/// A message the page logged to its console.
#[derive(Debug)]
struct Logged {
    /// `log`, `warning`, `error`, and so on.
    kind: String,
    text: String,
}

/// Every message the page logs to its console from now on, in order.
async fn listen_to_console(page: &Page) -> Result<UnboundedReceiver<Logged>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    page.on_console(move |msg| {
        tx.send(Logged {
            kind: msg.type_().to_string(),
            text: msg.text().to_string(),
        })
        .ok();
        async { Ok(()) }
    })
    .await
    .context("listen to the console")?;
    Ok(rx)
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_color_the_picker_cannot_hold_logs_nothing_to_the_console() -> Result<()> {
    with_gallery(async |page: &Page| {
        let mut console = listen_to_console(page).await?;
        select(page, "Knobs · color").await?;
        page.locator(".theoria-panel input[type=text]")
            .fill("teal", None)
            .await
            .context("type teal")?;
        expect(page.locator(".theoria-stage .k-ink"))
            .to_have_text("teal")
            .await
            .context("the story gets the color")?;

        let logged: Vec<_> = std::iter::from_fn(|| console.try_recv().ok()).collect();
        ensure!(logged.is_empty(), "the console logged {logged:?}");

        Ok(())
    })
    .await
}

/// Clicks the panic fixture's button and returns the console error that reports
/// the panic, within five seconds. Without the panic hook `Stories::mount`
/// sets, the browser reports only `RuntimeError: unreachable`, as a page error
/// rather than a console message, so this fails.
async fn the_error_a_panic_logs(page: &Page) -> Result<String> {
    let mut console = listen_to_console(page).await?;
    select(page, "Gallery · panic on click").await?;
    page.locator(".theoria-stage .k-panic")
        .click(None)
        .await
        .context("click the story's button")?;

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(logged) = console.recv().await {
            if logged.kind == "error" && logged.text.starts_with("panicked at ") {
                return Some(logged.text);
            }
        }
        None
    })
    .await
    .context("the console logs no panic within 5s of the click")?
    .context("the console listener went away")
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_panic_in_a_story_logs_its_message_to_the_console() -> Result<()> {
    with_gallery(async |page: &Page| {
        let error = the_error_a_panic_logs(page).await?;
        ensure!(
            error.contains("the story panicked on purpose"),
            "the console logged {error:?}"
        );
        Ok(())
    })
    .await
}

/// A hardened page (SES lockdown, say) freezes `Error`, so the hook cannot
/// raise its stack trace limit there.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_panic_is_logged_on_a_page_that_froze_error() -> Result<()> {
    with_gallery(async |page: &Page| {
        page.evaluate::<(), ()>("() => { Object.freeze(Error); }", None)
            .await
            .context("freeze Error")?;
        let error = the_error_a_panic_logs(page).await?;
        ensure!(
            error.contains("the story panicked on purpose"),
            "the console logged {error:?}"
        );
        Ok(())
    })
    .await
}

/// The panic's location names only the line that panicked; the stack is what
/// traces it back through the story's code, here by the fixture's module name,
/// which the location (`gallery.stories.rs`) does not contain.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_panic_in_a_story_logs_the_stack_that_reached_it() -> Result<()> {
    with_gallery(async |page: &Page| {
        let error = the_error_a_panic_logs(page).await?;
        ensure!(
            error.contains("gallery_stories"),
            "the console logged {error:?}"
        );
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
async fn a_field_typed_into_under_its_structs_name_reaches_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        page.locator(r#".theoria-panel [role=group][aria-label="card"]"#)
            .get_by_label("title", true)
            .fill("Porch", None)
            .await
            .context("type a title into the card's title field")?;
        expect(page.locator(".theoria-stage .k-title"))
            .to_have_text("Porch")
            .await
            .context("the title reaches the stage")?;

        Ok(())
    })
    .await
}

/// The group of the `Knobs · struct` story's `card` arg called `name`.
fn card_group(page: &Page, name: &str) -> Locator {
    page.locator(format!(
        r#".theoria-panel [role=group][aria-label="card"] [role=group][aria-label="{name}"]"#
    ))
}

/// The input in `within` whose role is `role` and whose name is `name`.
fn by_role(within: &Locator, role: AriaRole, name: &str) -> Locator {
    within.get_by_role(
        role,
        Some(GetByRoleOptions::default().name(name).exact(true)),
    )
}

/// The switch in the card's `shadow` group, which an `Option` gives one.
fn shadow_switch(page: &Page) -> Locator {
    by_role(&card_group(page, "shadow"), AriaRole::Checkbox, "shadow")
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn switching_a_group_off_takes_it_from_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        let card = page.locator(".theoria-stage .k-card");
        expect(card.clone())
            .to_have_css("box-shadow", "rgba(0, 0, 0, 0.25) 0px 2px 6px 0px")
            .await
            .context("the stage starts with the shadow")?;
        shadow_switch(page)
            .uncheck(None)
            .await
            .context("switch the card's shadow off")?;
        expect(card)
            .to_have_css("box-shadow", "none")
            .await
            .context("the stage draws no shadow")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn switching_a_group_off_takes_its_fields_from_the_panel() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        shadow_switch(page)
            .uncheck(None)
            .await
            .context("switch the card's shadow off")?;
        expect(by_role(
            &card_group(page, "shadow"),
            AriaRole::Spinbutton,
            "blur",
        ))
        .to_have_count(0)
        .await
        .context("the shadow's blur box is gone")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn switching_a_group_on_again_shows_its_fields_as_they_were_left() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        let blur = || by_role(&card_group(page, "shadow"), AriaRole::Spinbutton, "blur");
        blur()
            .fill("9", None)
            .await
            .context("type a blur into the shadow")?;
        shadow_switch(page)
            .uncheck(None)
            .await
            .context("switch the shadow off")?;
        shadow_switch(page)
            .check(None)
            .await
            .context("switch the shadow on again")?;
        expect(blur())
            .to_have_value("9")
            .await
            .context("the blur box shows what was typed")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_field_typed_into_inside_a_nested_group_reaches_the_story() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        by_role(&card_group(page, "border"), AriaRole::Spinbutton, "width")
            .fill("4", None)
            .await
            .context("type a width into the card's border group")?;
        expect(page.locator(".theoria-stage .k-card"))
            .to_have_css("border-top-width", "4px")
            .await
            .context("the border's width reaches the stage")?;

        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_fields_description_sits_as_far_from_its_control_as_from_its_name() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        let row = page
            .locator(
                r#".theoria-panel [role=group][aria-label="card"] .control-fields .control-row"#,
            )
            .first();
        let head = row
            .locator(".control-head")
            .bounding_box()
            .await?
            .context("the field's name is laid out")?;
        let text = row
            .locator(".control-description p")
            .bounding_box()
            .await?
            .context("the field's description is laid out")?;
        let control = row
            .locator(".control-input")
            .bounding_box()
            .await?
            .context("the field's control is laid out")?;

        let above = text.y - (head.y + head.height);
        let below = control.y - (text.y + text.height);
        ensure!(
            (above - below).abs() < 1.0,
            "{above}px above the description, {below}px below it: \
             name {head:?}, description {text:?}, control {control:?}"
        );

        Ok(())
    })
    .await
}

/// The card arg's group in the panel.
fn card(page: &Page) -> Locator {
    page.locator(r#".theoria-panel [role=group][aria-label="card"]"#)
}

/// The code under the card arg's controls.
fn card_code(page: &Page) -> Locator {
    card(page).locator("> .control-code code")
}

/// The card's Copy button.
fn copy_card(page: &Page) -> Locator {
    by_role(&card(page), AriaRole::Button, "Copy card")
}

/// Types a title over the card's "Patio" and waits for the card's code to be
/// what it was with that one string changed.
async fn retitle_the_card(page: &Page) -> Result<()> {
    let before = card_code(page)
        .text_content()
        .await?
        .context("the card's code has text")?;
    ensure!(
        before.contains(r#""Patio""#),
        "the card's code has no \"Patio\" title to change: {before}"
    );
    by_role(&card(page), AriaRole::Textbox, "title")
        .fill("Porch", None)
        .await
        .context("type a title into the card")?;

    expect(card_code(page))
        .to_have_text(&before.replace(r#""Patio""#, r#""Porch""#))
        .await
        .context("the card's code is the same but for the title typed")
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn an_edit_to_a_field_rewrites_the_struct_args_code() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        retitle_the_card(page).await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_puts_the_struct_args_code_on_the_clipboard() -> Result<()> {
    with_gallery(async |page: &Page| {
        page.context()?
            .grant_permissions(&["clipboard-read", "clipboard-write"], None)
            .await
            .context("let the page use the clipboard")?;
        select(page, "Knobs · struct").await?;
        page.evaluate::<(), ()>("() => navigator.clipboard.writeText('-')", None)
            .await
            .context("put a placeholder on the clipboard")?;
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;

        let shown = card_code(page)
            .text_content()
            .await?
            .context("the card's code has text")?;
        // The click does not wait for the write, so read until the placeholder
        // is gone, for a second at most.
        let copied: String = page
            .evaluate::<(), _>(
                "async () => { for (let i = 0; i < 50; i++) { const text = await navigator.clipboard.readText(); if (text !== '-') return text; await new Promise(r => setTimeout(r, 20)); } return '-'; }",
                None,
            )
            .await
            .context("read the clipboard")?;
        assert_eq!(copied, shown);

        Ok(())
    })
    .await
}

/// Every error the page throws and does not catch, from now on.
async fn listen_to_page_errors(page: &Page) -> Result<UnboundedReceiver<String>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    page.on_pageerror(move |message| {
        tx.send(message).ok();
        async { Ok(()) }
    })
    .await
    .context("listen to page errors")?;
    Ok(rx)
}

/// A `navigator.clipboard` that is not there, as on a page that is not a
/// secure context.
const NO_CLIPBOARD: &str = "undefined";

/// A `navigator.clipboard` that refuses every write.
const REFUSING_CLIPBOARD: &str =
    "({ writeText: () => Promise.reject(new DOMException('refused', 'NotAllowedError')) })";

/// Run `body` on the card's story, on a page whose `navigator.clipboard` is
/// `clipboard`, a JavaScript expression.
async fn with_a_clipboard(
    clipboard: &str,
    body: impl AsyncFnOnce(&Page) -> Result<()>,
) -> Result<()> {
    let script = format!(
        "Object.defineProperty(Navigator.prototype, 'clipboard', {{ get: () => {clipboard} }})"
    );
    with_site(async |page: &Page, site: &Site| {
        page.add_init_script(&script)
            .await
            .context("replace the page's clipboard")?;
        open(page, &site.story("Knobs · struct")).await?;
        expect_marked(page, "Knobs · struct").await?;
        body(page).await
    })
    .await
}

/// What the page throws when the card's Copy button is clicked with
/// `clipboard`. The card is retitled after the click, which shows the page
/// still works and gives an error from the click time to arrive.
async fn errors_from_a_copy_with(clipboard: &str) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    with_a_clipboard(clipboard, async |page: &Page| {
        let mut thrown = listen_to_page_errors(page).await?;
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;
        retitle_the_card(page).await?;

        errors.extend(std::iter::from_fn(|| thrown.try_recv().ok()));
        Ok(())
    })
    .await?;
    Ok(errors)
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_on_a_page_with_no_clipboard_throws_nothing() -> Result<()> {
    let thrown = errors_from_a_copy_with(NO_CLIPBOARD).await?;
    assert_eq!(thrown, Vec::<String>::new());
    Ok(())
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_the_browser_refuses_throws_nothing() -> Result<()> {
    let thrown = errors_from_a_copy_with(REFUSING_CLIPBOARD).await?;
    assert_eq!(thrown, Vec::<String>::new());
    Ok(())
}

/// What the card's Copy button tells a screen reader it did.
fn copy_status(page: &Page) -> Locator {
    card(page).locator("> .control-code [role=status]")
}

/// Clicks the card's Copy button with `clipboard` and waits for its label
/// to read `said`.
async fn copy_with_says(clipboard: &str, said: &str) -> Result<()> {
    with_a_clipboard(clipboard, async |page: &Page| {
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;
        expect(copy_card(page))
            .to_have_text(said)
            .await
            .with_context(|| format!("Copy reads {said:?}"))
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_says_it_copied() -> Result<()> {
    with_gallery(async |page: &Page| {
        page.context()?
            .grant_permissions(&["clipboard-write"], None)
            .await
            .context("let the page write the clipboard")?;
        select(page, "Knobs · struct").await?;
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;

        expect(copy_card(page))
            .to_have_text("Copied")
            .await
            .context("Copy reads Copied")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn what_copy_did_is_told_to_a_screen_reader() -> Result<()> {
    with_a_clipboard(REFUSING_CLIPBOARD, async |page: &Page| {
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;
        expect(copy_status(page))
            .to_have_text("Failed")
            .await
            .context("the status region says Failed")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_on_a_page_with_no_clipboard_says_it_failed() -> Result<()> {
    copy_with_says(NO_CLIPBOARD, "Failed").await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_the_browser_refuses_says_it_failed() -> Result<()> {
    copy_with_says(REFUSING_CLIPBOARD, "Failed").await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_reads_copy_again_after_a_moment() -> Result<()> {
    with_a_clipboard(REFUSING_CLIPBOARD, async |page: &Page| {
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;
        expect(copy_card(page))
            .to_have_text("Failed")
            .await
            .context("Copy reads Failed")?;
        expect(copy_card(page))
            .to_have_text("Copy")
            .await
            .context("Copy reads Copy again")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn copy_is_as_wide_reading_copied_as_reading_copy() -> Result<()> {
    with_gallery(async |page: &Page| {
        page.context()?
            .grant_permissions(&["clipboard-write"], None)
            .await
            .context("let the page write the clipboard")?;
        select(page, "Knobs · struct").await?;
        let width = async || -> Result<f64> {
            Ok(copy_card(page)
                .bounding_box()
                .await?
                .context("the Copy button is laid out")?
                .width)
        };
        let before = width().await?;
        copy_card(page)
            .click(None)
            .await
            .context("click the card's Copy button")?;
        expect(copy_card(page))
            .to_have_text("Copied")
            .await
            .context("Copy reads Copied")?;

        assert_eq!(width().await?, before);
        Ok(())
    })
    .await
}

/// `[clientHeight, scrollHeight]` of what `locator` finds.
async fn heights(locator: &Locator, what: &str) -> Result<(f64, f64)> {
    locator
        .evaluate("el => [el.clientHeight, el.scrollHeight]", None::<()>)
        .await
        .with_context(|| format!("measure {what}"))
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn controls_taller_than_the_window_are_shown_whole_in_a_panel_that_scrolls() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        let (shown, needed) = heights(&page.locator(".theoria-controls"), "the controls").await?;
        ensure!(
            needed <= shown,
            "the controls need {needed}px and show {shown}px"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_long_value_scrolls_inside_its_block() -> Result<()> {
    with_gallery(async |page: &Page| {
        select(page, "Knobs · struct").await?;
        let (shown, needed) = heights(
            &card(page).locator("> .control-code pre"),
            "the card's code",
        )
        .await?;
        ensure!(
            needed > shown,
            "the card's code shows all {needed}px of itself"
        );
        Ok(())
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
        // Alt is left out because Chromium downloads the link on an Alt-click,
        // and Meta because CI's Linux Chromium delivers a Meta-click as a plain
        // one. `Modifiers::is_plain`'s unit tests cover both keys.
        for modifier in [KeyboardModifier::Shift, KeyboardModifier::Control] {
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

/// The divider between the stage and the panel.
fn divider(page: &Page) -> Locator {
    by_role(
        &page.locator(".theoria"),
        AriaRole::Separator,
        "Panel width",
    )
}

fn panel(page: &Page) -> Locator {
    page.locator(".theoria > .theoria-panel")
}

fn stage(page: &Page) -> Locator {
    page.locator(".theoria > .theoria-stage")
}

/// Where `locator` is drawn, once it is shown.
async fn laid_out(locator: &Locator, what: &str) -> Result<BoundingBox> {
    expect(locator.clone())
        .to_be_visible()
        .await
        .with_context(|| format!("{what} is shown"))?;
    locator
        .bounding_box()
        .await
        .with_context(|| format!("measure {what}"))?
        .with_context(|| format!("{what} is laid out"))
}

async fn panel_width(page: &Page) -> Result<f64> {
    Ok(laid_out(&panel(page), "the panel").await?.width)
}

/// Drags the divider, from its middle, to `x` in the window, at its own
/// height. `drag_to` rather than the mouse's own moves, which can hang with a
/// button held under headless Linux.
async fn drag_divider_to(page: &Page, x: f64) -> Result<()> {
    let row = page.locator(".theoria");
    let at = laid_out(&row, "the gallery").await?;
    let from = laid_out(&divider(page), "the divider").await?;
    let to = Position {
        x: x - at.x,
        y: from.y + from.height / 2.0 - at.y,
    };
    divider(page)
        .drag_to(
            &row,
            Some(DragToOptions::builder().target_position(to).build()),
        )
        .await
        .with_context(|| format!("drag the divider to {x}px"))?;
    settle(page).await
}

/// Drags the divider `by` pixels to the right (left when negative).
async fn drag_divider_by(page: &Page, by: f64) -> Result<()> {
    let from = laid_out(&divider(page), "the divider").await?;
    drag_divider_to(page, from.x + from.width / 2.0 + by).await
}

fn ensure_width(what: &str, width: f64, wanted: f64) -> Result<()> {
    ensure!(
        (width - wanted).abs() < 1.0,
        "{what} is {width}px, not {wanted}px"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn dragging_the_divider_into_the_stage_widens_the_panel_as_far() -> Result<()> {
    with_gallery(async |page: &Page| {
        let before = panel_width(page).await?;

        drag_divider_by(page, -100.0).await?;

        ensure_width("the panel", panel_width(page).await?, before + 100.0)
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn dragging_the_divider_across_the_stage_leaves_the_stage_200px() -> Result<()> {
    with_gallery(async |page: &Page| {
        drag_divider_to(page, 1.0).await?;

        let stage = laid_out(&stage(page), "the stage").await?;
        ensure_width("the stage", stage.width, 200.0)
    })
    .await
}

/// `offsetWidth` of what `locator` finds, the whole pixels the gallery
/// measures its columns in.
async fn offset_width(locator: &Locator, what: &str) -> Result<i32> {
    expect(locator.clone())
        .to_be_visible()
        .await
        .with_context(|| format!("{what} is shown"))?;
    locator
        .evaluate("el => el.offsetWidth", None::<()>)
        .await
        .with_context(|| format!("measure {what}"))
}

/// The panel's most as the stage and panel are drawn: all but 200px of the
/// room the two of them share.
async fn most_leaving_the_stage_200px(page: &Page) -> Result<String> {
    let panel = offset_width(&panel(page), "the panel").await?;
    let stage = offset_width(&stage(page), "the stage").await?;
    Ok((panel + stage - 200).to_string())
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn the_divider_reads_the_most_that_leaves_the_stage_200px() -> Result<()> {
    with_gallery(async |page: &Page| {
        let most = most_leaving_the_stage_200px(page).await?;

        expect(divider(page))
            .to_have_attribute("aria-valuemax", &most)
            .await
            .context("the divider's most leaves the stage 200px")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn focusing_the_divider_after_the_window_narrows_reads_its_new_most() -> Result<()> {
    with_gallery(async |page: &Page| {
        let wide = most_leaving_the_stage_200px(page).await?;
        expect(divider(page))
            .to_have_attribute("aria-valuemax", &wide)
            .await
            .context("the divider reads its most in the wide window")?;
        page.set_viewport_size(Viewport {
            width: 1000,
            height: 720,
        })
        .await
        .context("narrow the window")?;
        settle(page).await?;
        let narrow = most_leaving_the_stage_200px(page).await?;

        divider(page).focus().await.context("focus the divider")?;

        expect(divider(page))
            .to_have_attribute("aria-valuemax", &narrow)
            .await
            .with_context(|| format!("the divider's most went from {wide}"))
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn releasing_the_divider_ends_the_drag() -> Result<()> {
    with_gallery(async |page: &Page| {
        drag_divider_by(page, -100.0).await?;
        let dragged = panel_width(page).await?;
        let on = laid_out(&divider(page), "the divider").await?;

        // Still over the divider, the one place its moves are heard once the
        // button is up, and off where the drag ended, so a drag left running
        // would move the panel.
        page.mouse()
            .move_to(on.x + 1.0, on.y + on.height / 2.0, None)
            .await
            .context("move the pointer along the divider")?;
        settle(page).await?;

        ensure_width("the panel", panel_width(page).await?, dragged)
    })
    .await
}

/// Presses `key` on the divider and returns the panel's width before and after.
async fn press_on_the_divider(page: &Page, key: &str) -> Result<(f64, f64)> {
    let before = panel_width(page).await?;
    divider(page)
        .press(key, None)
        .await
        .with_context(|| format!("press {key} on the divider"))?;
    settle(page).await?;
    Ok((before, panel_width(page).await?))
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn arrow_left_on_the_divider_widens_the_panel_by_16px() -> Result<()> {
    with_gallery(async |page: &Page| {
        let (before, after) = press_on_the_divider(page, "ArrowLeft").await?;

        ensure_width("the panel", after, before + 16.0)
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_reload_keeps_the_panels_width() -> Result<()> {
    with_gallery(async |page: &Page| {
        drag_divider_by(page, -100.0).await?;
        let dragged = panel_width(page).await?;

        page.reload(None).await.context("reload the page")?;

        ensure_width("the panel", panel_width(page).await?, dragged)
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_double_click_on_the_divider_puts_the_panels_first_width_back() -> Result<()> {
    with_gallery(async |page: &Page| {
        let first = panel_width(page).await?;
        drag_divider_by(page, -100.0).await?;

        divider(page)
            .dblclick(None)
            .await
            .context("double-click the divider")?;
        settle(page).await?;

        ensure_width("the panel", panel_width(page).await?, first)
    })
    .await
}

/// Runs `body` on the gallery reloaded with a stored panel width wider than
/// any window.
async fn with_a_panel_too_wide_for_the_window(
    body: impl AsyncFnOnce(&Page) -> Result<()>,
) -> Result<()> {
    with_gallery(async |page: &Page| {
        laid_out(&panel(page), "the panel").await?;
        page.evaluate::<(), ()>(
            "() => localStorage.setItem('theoria:panel-width', '5000')",
            None,
        )
        .await
        .context("store a panel width of 5000px")?;
        page.reload(None).await.context("reload the page")?;
        body(page).await
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_panel_too_wide_for_the_window_leaves_the_stage_200px() -> Result<()> {
    with_a_panel_too_wide_for_the_window(async |page: &Page| {
        let stage = laid_out(&stage(page), "the stage").await?;
        ensure_width("the stage", stage.width, 200.0)
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn a_panel_too_wide_for_the_window_reads_as_wide_as_it_is_drawn() -> Result<()> {
    with_a_panel_too_wide_for_the_window(async |page: &Page| {
        let drawn = offset_width(&panel(page), "the panel").await?;

        expect(divider(page))
            .to_have_attribute("aria-valuenow", &drawn.to_string())
            .await
            .context("the divider reads the panel's drawn width")
    })
    .await
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn arrow_right_narrows_a_panel_too_wide_for_the_window_from_its_drawn_width() -> Result<()> {
    with_a_panel_too_wide_for_the_window(async |page: &Page| {
        let (before, after) = press_on_the_divider(page, "ArrowRight").await?;

        ensure_width("the panel", after, before - 16.0)
    })
    .await
}
