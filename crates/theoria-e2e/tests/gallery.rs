//! theoria's own browser e2e: serve a gallery built from theoria's
//! components-as-fixtures and drive it with playwright-rs.
//!
//! The core check is generic — like Storybook's test-runner, it discovers every
//! story from the gallery's own sidebar at runtime and asserts each one renders
//! and selects. Add a story (a `*.stories.rs`) and it's covered automatically;
//! no compile-time list here. This is theoria dogfooding itself one level — never
//! a `Gallery` inside a `Gallery`.
//!
//! Every test needs the built gallery and Chromium, so each is `#[ignore]`d
//! and fails, naming the build command, when run without the gallery. Install
//! the browser once, build the gallery, then run:
//!   cargo run --manifest-path crates/theoria-e2e/Cargo.toml --example install-browsers
//!   cargo run -p theoria-cli -- build --config theoria-e2e.toml
//!   cargo nextest run --manifest-path crates/theoria-e2e/Cargo.toml --config-file .config/nextest.toml --run-ignored all

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use axum::Router;
use playwright_rs::expect;
use playwright_rs::protocol::{Page, Playwright};
use tower_http::services::ServeDir;

fn dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/theoria/gallery/theoria/dist")
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

/// Run `body` on a page showing the built gallery. Fails when the gallery is
/// not built, and closes the browser whether or not `body` passed.
async fn with_gallery(body: impl AsyncFnOnce(&Page) -> Result<()>) -> Result<()> {
    let dist = dist_dir();
    ensure!(
        dist.join("index.html").exists(),
        "{} not built; run cargo run -p theoria-cli -- build --config theoria-e2e.toml",
        dist.display()
    );

    let (addr, _server) = serve(&dist).await?;
    let pw = Playwright::launch().await.context("launch playwright")?;
    let browser = pw.chromium().launch().await.context("launch chromium")?;
    let result = async {
        let page = browser.new_page().await.context("new page")?;
        page.goto(&format!("http://{addr}"), None)
            .await
            .context("navigate to gallery")?;
        body(&page).await
    }
    .await;
    let closed = browser.close().await.context("close browser");
    result.and(closed)
}

/// Discover every story from the gallery sidebar and assert each one selects and
/// renders non-empty content on the stage. Generic over whatever stories the
/// gallery was built with.
async fn assert_every_story_renders(page: &Page) -> Result<()> {
    let names = page
        .locator(".theoria > .theoria-nav button")
        .all_inner_texts()
        .await
        .context("read story names from the sidebar")?;
    ensure!(!names.is_empty(), "gallery lists at least one story");

    for name in &names {
        page.get_by_text(name, true)
            .click(None)
            .await
            .with_context(|| format!("click story {name:?}"))?;

        let active = page
            .locator(".theoria > .theoria-nav button.active")
            .inner_text()
            .await
            .context("read active story")?;
        ensure!(
            active.trim() == name.trim(),
            "selecting {name:?} should mark it active, but active was {active:?}"
        );

        let stage_children = page
            .locator(".theoria-stage *")
            .count()
            .await
            .context("count stage content")?;
        ensure!(
            stage_children > 0,
            "story {name:?} renders non-empty content"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn every_story_renders_and_selects() -> Result<()> {
    with_gallery(assert_every_story_renders).await
}

/// The controls panel drives the stage live: editing a knob re-renders the
/// story view, and the show-code block carries the captured source. This is what
/// leptotes's SSR snapshots can't prove — the interactive re-render.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn controls_drive_the_stage_live() -> Result<()> {
    with_gallery(async |page: &Page| {
        // Select the knobs demo (a `#[story]` with args → renders a controls panel).
        page.get_by_text("Knobs · demo", true)
            .click(None)
            .await
            .context("select the knobs demo")?;

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

        // Autodocs: the doc-comment renders as Markdown (bold + a list), and the
        // argTypes table documents each arg's type.
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
            .context("read argTypes type column")?;
        ensure!(
            types.iter().any(|t| t.trim() == "bool"),
            "argTypes table documents the bool arg, got {types:?}"
        );

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
        page.get_by_text("Knobs · demo", true)
            .click(None)
            .await
            .context("select the knobs demo")?;
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

/// The selected story survives a full page reload (persisted to localStorage) —
/// this is what keeps you on your story across Trunk's hot reload.
#[tokio::test]
#[ignore = "needs the built gallery and Chromium; run with --run-ignored all after theoria build --config theoria-e2e.toml"]
async fn selection_persists_across_reload() -> Result<()> {
    with_gallery(async |page: &Page| {
        // Select a non-default story...
        page.get_by_text("StoryNav · single", false)
            .click(None)
            .await
            .context("select the second story")?;
        expect(page.locator(".theoria > .theoria-nav button.active"))
            .to_have_text("StoryNav · single")
            .await
            .context("second story is active before reload")?;

        // ...reload the page; the selection is restored from localStorage.
        page.reload(None).await.context("reload the page")?;
        expect(page.locator(".theoria > .theoria-nav button.active"))
            .to_have_text("StoryNav · single")
            .await
            .context("selected story persists across reload")?;

        Ok(())
    })
    .await
}
