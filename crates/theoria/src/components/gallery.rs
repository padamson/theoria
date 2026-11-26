//! The gallery: a [`StoryNav`](super::StoryNav) sidebar plus the selected
//! story's view on a stage. The selected story's name is in the URL
//! (`?story=<name>`) and in `localStorage`, so a link opens its story and a
//! reload (Trunk's hot reload during development) keeps the one on screen.

use leptos::prelude::*;

use super::{Controls, Markdown, ShowCode, StoryNav};
use crate::Story;

/// `localStorage` key for the selected story name (only used in the browser build).
#[cfg(feature = "csr")]
const STORAGE_KEY: &str = "theoria:selected-story";

#[component]
pub fn Gallery(stories: Vec<Story>) -> impl IntoView {
    let names: Vec<&'static str> = stories.iter().map(|s| s.name).collect();
    let opened = opening(
        &names,
        story_in_url().as_deref(),
        load_selected().as_deref(),
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
            save_selected(name);
            show_in_url(name);
        }
    });

    // Keep `stories` reachable from several reactive closures (stage, controls,
    // docs, show-code) — `StoredValue` is `Copy` and read-shared.
    let stories = StoredValue::new(stories);

    let stage = move || match selected.get() {
        Some(index) => stories.with_value(|s| s.get(index).map(|st| (st.view)())),
        None => missing.as_ref().map(|name| {
            view! { <p class="theoria-missing">{format!("No story named \"{name}\"")}</p> }
                .into_any()
        }),
    };
    let panel = move || {
        stories.with_value(|s| {
            selected.get().and_then(|index| s.get(index)).map(|st| {
                let description = st.description();
                let args = st.args().to_vec();
                let source = st.source();
                view! {
                    <aside class="theoria-panel">
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
            <main class="theoria-stage">{stage}</main>
            {panel}
        </div>
    }
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

/// The persisted selected-story name, if any. `None` off the browser (ssr/tests).
fn load_selected() -> Option<String> {
    #[cfg(feature = "csr")]
    {
        storage()?.get_item(STORAGE_KEY).ok().flatten()
    }
    #[cfg(not(feature = "csr"))]
    {
        None
    }
}

/// Persist the selected-story name (no-op off the browser).
fn save_selected(name: &str) {
    #[cfg(feature = "csr")]
    {
        if let Some(s) = storage() {
            let _ = s.set_item(STORAGE_KEY, name);
        }
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = name;
    }
}
