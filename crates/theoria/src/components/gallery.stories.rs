//! theoria stories for what the gallery does around a story, as fixtures for
//! theoria-e2e. They never put a `Gallery` inside a `Gallery`. Compiled only
//! under the `stories` feature.

use leptos::prelude::*;

use crate::Story;

pub fn stories() -> Vec<Story> {
    vec![
        Story::new("Gallery · panic on click", || {
            view! {
                <button class="k-panic" on:click=|_| panic!("the story panicked on purpose")>
                    "Panic"
                </button>
            }
        })
        .with_description(
            "Clicking the button panics, which logs the panic to the browser console \
             and may leave the gallery broken until the page is reloaded.",
        ),
    ]
}
