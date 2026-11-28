//! The gallery: a [`StoryNav`](super::StoryNav) sidebar plus the selected
//! story's view on a stage, and its panel beside it, behind a divider that
//! sets the panel's width. The selected story's name is in the URL
//! (`?story=<name>`), and it and the panel's width are in `localStorage`, so
//! a link opens its story and a reload (Trunk's hot reload during
//! development) keeps the one on screen at the width it had.

use leptos::html::{Aside, Main};
use leptos::prelude::*;

use super::{Controls, Markdown, ShowCode, StoryNav};
use crate::Story;

/// `localStorage` key for the selected story name.
const SELECTED_KEY: &str = "theoria:selected-story";
/// `localStorage` key for the panel's width.
const WIDTH_KEY: &str = "theoria:panel-width";

#[component]
pub fn Gallery(stories: Vec<Story>) -> impl IntoView {
    let names: Vec<&'static str> = stories.iter().map(|s| s.name).collect();
    let opened = opening(
        &names,
        story_in_url().as_deref(),
        load(SELECTED_KEY).as_deref(),
    );
    gallery_opened_at(stories, opened)
}

/// The gallery over `stories`, opened at `opened`.
pub(crate) fn gallery_opened_at(stories: Vec<Story>, opened: Opening) -> impl IntoView {
    let names: Vec<&'static str> = stories.iter().map(|s| s.name).collect();
    let has_docs: Vec<bool> = stories.iter().map(|s| s.description().is_some()).collect();

    let (initial, missing) = match opened {
        Opening::Story(index) => (Some(index), None),
        Opening::NoSuchStory(name) => (None, Some(name)),
    };
    let (selected, set_selected) = signal(initial);

    // Record the selection whenever it changes (no-op under ssr / in tests).
    let names_for_save = names.clone();
    Effect::new(move |_| {
        if let Some(name) = selected.get().and_then(|index| names_for_save.get(index)) {
            save(SELECTED_KEY, name);
            show_in_url(name);
        }
    });

    // Keep `stories` reachable from several reactive closures (stage, controls,
    // docs, show-code) — `StoredValue` is `Copy` and read-shared.
    let stories = StoredValue::new(stories);

    let stage = move || match selected.get() {
        Some(index) => stories.with_value(|s| s.get(index).map(Story::view)),
        None => missing.as_ref().map(|name| {
            view! { <p class="theoria-missing">{format!("No story named \"{name}\"")}</p> }
                .into_any()
        }),
    };
    let width = RwSignal::new(stored_width(load(WIDTH_KEY).as_deref()));
    Effect::new(move |_| save(WIDTH_KEY, &width.get().to_string()));
    let columns = Columns {
        stage: NodeRef::new(),
        panel: NodeRef::new(),
        most: RwSignal::new(width.get_untracked()),
    };
    Effect::new(move |_| {
        columns.panel.track();
        columns.measure();
    });

    let panel = move || {
        stories.with_value(|s| {
            selected.get().and_then(|index| s.get(index)).map(|st| {
                let description = st.description();
                let args = st.args().to_vec();
                let source = st.source();
                view! {
                    {divider(width, columns)}
                    <aside
                        class="theoria-panel"
                        node_ref=columns.panel
                        style:flex-basis=move || format!("{}px", width.get())
                        style:min-width=format!("{PANEL_LEAST}px")
                    >
                        {description.map(|d| view! { <Markdown text=d /> })}
                        <Controls args=args />
                        {source.map(|src| view! { <ShowCode source=src /> })}
                    </aside>
                }
            })
        })
    };

    view! {
        <div class="theoria">
            <StoryNav names=names selected=selected set_selected=set_selected has_docs=has_docs />
            <main
                class="theoria-stage"
                node_ref=columns.stage
                style:min-width=format!("{STAGE_LEAST}px")
            >
                {stage}
            </main>
            {panel}
        </div>
    }
}

/// The stage and the panel as drawn, and the panel's most as last measured.
#[derive(Clone, Copy)]
struct Columns {
    stage: NodeRef<Main>,
    panel: NodeRef<Aside>,
    most: RwSignal<i32>,
}

impl Columns {
    /// The panel's drawn width and its most beside the stage, also kept as
    /// the divider's most. `None` until both are drawn.
    fn measure(self) -> Option<(i32, i32)> {
        let panel = self.panel.get_untracked()?.offset_width();
        let most = most_beside(panel, self.stage.get_untracked()?.offset_width());
        self.most.set(most);
        Some((panel, most))
    }
}

/// The divider before the panel, which sets `width` by pointer or key.
fn divider(width: RwSignal<i32>, columns: Columns) -> impl IntoView {
    let drag = StoredValue::new(None::<Drag>);
    let end = move |_| drag.set_value(None);
    view! {
        <div
            class="theoria-divider"
            role="separator"
            tabindex="0"
            aria-label="Panel width"
            aria-orientation="vertical"
            aria-valuenow=move || clamp(width.get(), columns.most.get())
            aria-valuemin=PANEL_LEAST
            aria-valuemax=move || columns.most.get()
            on:focus=move |_| {
                columns.measure();
            }
            on:pointerdown=move |ev| {
                if starts_a_drag(ev.button(), ev.is_primary())
                    && let Some((start, most)) = columns.measure()
                {
                    ev.prevent_default();
                    capture(&ev);
                    drag.set_value(
                        Some(Drag {
                            pointer: ev.pointer_id(),
                            from: ev.client_x(),
                            start,
                            most,
                        }),
                    );
                }
            }
            on:pointermove=move |ev| {
                if let Some(dragged) = drag
                    .get_value()
                    .and_then(|drag| drag.width_at(ev.pointer_id(), ev.client_x()))
                {
                    width.set(dragged);
                }
            }
            on:pointerup=end
            on:pointercancel=end
            on:lostpointercapture=end
            on:keydown=move |ev| {
                if let Some(step) = step_for(&ev.key())
                    && let Some((panel, most)) = columns.measure()
                {
                    ev.prevent_default();
                    width.set(stepped(panel, step, most));
                }
            }
            on:dblclick=move |_| width.set(PANEL_DEFAULT)
        ></div>
    }
}

/// Sends the rest of a pointer's moves to the divider it pressed, wherever
/// the pointer goes.
fn capture(ev: &leptos::ev::PointerEvent) {
    let _ = event_target::<leptos::web_sys::Element>(ev).set_pointer_capture(ev.pointer_id());
}

/// The panel's width, in pixels, until it is dragged.
pub(crate) const PANEL_DEFAULT: i32 = 280;
/// The narrowest the panel goes, which its controls still fit.
pub(crate) const PANEL_LEAST: i32 = 220;
/// The narrowest the divider leaves the stage.
pub(crate) const STAGE_LEAST: i32 = 200;
/// How far an arrow key moves the divider.
pub(crate) const STEP: i32 = 16;

/// The widest the panel goes with it and the stage drawn at these widths.
pub(crate) fn most_beside(panel: i32, stage: i32) -> i32 {
    (panel + stage - STAGE_LEAST).max(PANEL_LEAST)
}

/// `width` kept between the panel's least and `most`.
pub(crate) fn clamp(width: i32, most: i32) -> i32 {
    width.clamp(PANEL_LEAST, most)
}

/// Whether a press (`PointerEvent.button`, `isPrimary`) starts a drag of the
/// divider: the main button of the primary pointer, so a right-click, whose
/// menu can take the release, or a second finger does not.
pub(crate) fn starts_a_drag(button: i16, primary: bool) -> bool {
    button == 0 && primary
}

/// A drag of the divider: the pointer that began it, where it was then, and
/// the panel's width and most then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Drag {
    pub(crate) pointer: i32,
    pub(crate) from: i32,
    pub(crate) start: i32,
    pub(crate) most: i32,
}

impl Drag {
    /// The panel's width with `pointer` at `x`, or `None` for a pointer other
    /// than the one that began the drag.
    pub(crate) fn width_at(self, pointer: i32, x: i32) -> Option<i32> {
        (pointer == self.pointer).then(|| clamp(self.start + self.from - x, self.most))
    }
}

/// Which way a key moves the divider.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Step {
    Wider,
    Narrower,
}

/// The step a key (`KeyboardEvent.key`) takes, if any.
pub(crate) fn step_for(key: &str) -> Option<Step> {
    match key {
        "ArrowLeft" => Some(Step::Wider),
        "ArrowRight" => Some(Step::Narrower),
        _ => None,
    }
}

/// The panel's width a step from `width`.
pub(crate) fn stepped(width: i32, step: Step, most: i32) -> i32 {
    let width = match step {
        Step::Wider => width + STEP,
        Step::Narrower => width - STEP,
    };
    clamp(width, most)
}

/// The panel's width from what `localStorage` holds.
pub(crate) fn stored_width(stored: Option<&str>) -> i32 {
    stored
        .and_then(|width| width.parse::<i32>().ok())
        .map_or(PANEL_DEFAULT, |width| width.max(PANEL_LEAST))
}

/// Which story a gallery opens on load.
#[derive(Debug, PartialEq)]
pub(crate) enum Opening {
    Story(usize),
    NoSuchStory(String),
}

/// The story to open, from the name in the URL, else the stored name, else
/// the first story.
pub(crate) fn opening(names: &[&str], in_url: Option<&str>, stored: Option<&str>) -> Opening {
    let index = |wanted: &str| names.iter().position(|&name| name == wanted);
    match in_url {
        Some(wanted) => {
            index(wanted).map_or_else(|| Opening::NoSuchStory(wanted.into()), Opening::Story)
        }
        None => Opening::Story(stored.and_then(index).unwrap_or(0)),
    }
}

/// The query key that names a story.
const STORY: &str = "story";

/// The URL query that opens the story named `name`.
pub(crate) fn story_href(name: &str) -> String {
    with_story("", name)
}

/// The story named in a URL's query string (`location.search`), if any.
pub(crate) fn story_in(search: &str) -> Option<String> {
    form_urlencoded::parse(without_question_mark(search).as_bytes())
        .find(|(key, _)| key == STORY)
        .map(|(_, name)| name.into_owned())
        .filter(|name| !name.is_empty())
}

/// A URL's query string (`location.search`) with `story` set to `name`, after
/// every other parameter, form-encoded but for `/`, which stays as it is.
pub(crate) fn with_story(search: &str, name: &str) -> String {
    let mut query = form_urlencoded::Serializer::for_suffix(String::from("?"), 1);
    query.extend_pairs(
        form_urlencoded::parse(without_question_mark(search).as_bytes())
            .filter(|(key, _)| key != STORY),
    );
    query.append_pair(STORY, name);
    query.finish().replace("%2F", "/")
}

fn without_question_mark(search: &str) -> &str {
    search.strip_prefix('?').unwrap_or(search)
}

/// The story named in the page's URL.
fn story_in_url() -> Option<String> {
    story_in(&location()?.0)
}

/// The page's `location.search` and `location.hash`. `None` off the browser
/// (ssr/tests).
fn location() -> Option<(String, String)> {
    #[cfg(feature = "csr")]
    {
        let location = web_sys::window()?.location();
        Some((location.search().ok()?, location.hash().ok()?))
    }
    #[cfg(not(feature = "csr"))]
    {
        None
    }
}

/// Point the page's URL at the story named `name`.
fn show_in_url(name: &str) {
    let (search, hash) = location().unwrap_or_default();
    replace_url(&(with_story(&search, name) + &hash));
}

/// Replace the page's URL (no-op off the browser).
fn replace_url(url: &str) {
    #[cfg(feature = "csr")]
    {
        if let Some(history) = web_sys::window().and_then(|w| w.history().ok()) {
            let _ =
                history.replace_state_with_url(&leptos::wasm_bindgen::JsValue::NULL, "", Some(url));
        }
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = url;
    }
}

#[cfg(feature = "csr")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// What `localStorage` holds under `key`. `None` off the browser (ssr/tests).
fn load(key: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        storage()?.get_item(key).ok().flatten()
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = key;
        None
    }
}

/// Keep `value` in `localStorage` under `key` (no-op off the browser).
fn save(key: &str, value: &str) {
    #[cfg(feature = "csr")]
    {
        if let Some(s) = storage() {
            let _ = s.set_item(key, value);
        }
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (key, value);
    }
}
