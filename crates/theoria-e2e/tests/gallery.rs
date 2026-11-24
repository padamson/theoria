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
use std::sync::{Arc, Mutex};

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

async fn select(page: &Page, story: &str) -> Result<()> {
    page.get_by_text(story, true)
        .click(None)
        .await
        .with_context(|| format!("select the story {story:?}"))
}

/// Discover every story from the gallery sidebar and assert each one selects and
/// renders non-empty content on the stage. Generic over whatever stories the
/// gallery was built with.
async fn assert_every_story_renders(page: &Page) -> Result<()> {
    let buttons = page.locator(".theoria > .theoria-nav button");
    // `all_inner_texts` does not wait, and the page loads before the app
    // mounts, so wait for the sidebar or the list reads empty.
    expect(buttons.first())
        .to_be_visible()
        .await
        .context("the sidebar lists a story")?;
    let names = buttons
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

        // Autodocs: the doc-comment renders as Markdown (bold + a list), and the
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

        let (shown, needed): (f64, f64) = text
            .evaluate("el => [el.clientWidth, el.scrollWidth]", None::<()>)
            .await
            .context("measure the text field")?;
        ensure!(
            needed <= shown,
            "the text needs {needed}px and the field shows {shown}px"
        );

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
