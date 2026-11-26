//! theoria stories for `StoryNav` — theoria dogfooding its own component as a
//! fixture (one level; not a Gallery-in-Gallery). Compiled only under the
//! `stories` feature. The `Flat` and `Grouped` stories share their last
//! segments, so the e2e suite can tell a story's full name from its label.

use leptos::prelude::*;

use super::StoryNav;
use crate::Story;

fn fixture(testid: &'static str, names: Vec<&'static str>) -> impl IntoView {
    let (selected, set_selected) = signal(Some(0usize));
    view! {
        <div data-testid=testid>
            <StoryNav names=names selected=selected set_selected=set_selected />
        </div>
    }
}

pub fn stories() -> Vec<Story> {
    vec![
        Story::new("StoryNav/Flat/Three", || {
            fixture("fixture-flat-three", vec!["One", "Two", "Three"])
        }),
        Story::new("StoryNav/Flat/Single", || {
            fixture("fixture-flat-single", vec!["Only"])
        }),
        Story::new("StoryNav/Grouped/Three", || {
            fixture(
                "fixture-grouped-three",
                vec!["Theme/Playground", "Version bar/Playground", "Hero"],
            )
        }),
        Story::new("StoryNav/Grouped/Single", || {
            fixture("fixture-grouped-single", vec!["Group/Only"])
        }),
    ]
}
