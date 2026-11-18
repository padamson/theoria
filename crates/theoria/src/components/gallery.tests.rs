//! leptotes component tests for `Gallery`.

use leptos::prelude::*;

use super::Gallery;
use crate::{ArgControl, Stories, Story};

fn alpha_beta() -> Vec<Story> {
    vec![
        Story::new("Alpha", || view! { <p>"alpha-body"</p> }),
        Story::new("Beta", || view! { <p>"beta-body"</p> }),
    ]
}

fn gallery_of(stories: Vec<Story>) -> String {
    leptotes::render(move || view! { <Gallery stories=stories /> })
}

#[test]
fn stories_gallery_is_the_gallery_over_its_stories() {
    let stories: Stories = alpha_beta().into();
    assert_eq!(
        leptotes::render(move || stories.gallery()),
        gallery_of(alpha_beta())
    );
}

fn first() -> impl Iterator<Item = Story> {
    alpha_beta().into_iter().take(1)
}

fn second() -> impl Iterator<Item = Story> {
    alpha_beta().into_iter().skip(1)
}

#[test]
fn stories_collect_in_order() {
    let stories: Stories = alpha_beta().into_iter().collect();
    assert_eq!(
        leptotes::render(move || stories.gallery()),
        gallery_of(alpha_beta())
    );
}

#[test]
fn stories_extend_at_the_end() {
    let mut stories: Stories = first().collect();
    stories.extend(second());
    assert_eq!(
        leptotes::render(move || stories.gallery()),
        gallery_of(alpha_beta())
    );
}

#[test]
fn stories_iterate_in_order() {
    let stories: Stories = alpha_beta().into();
    let names: Vec<_> = stories.into_iter().map(|s| s.name()).collect();
    assert_eq!(names, ["Alpha", "Beta"]);
}

#[test]
fn default_stories_are_empty() {
    assert_eq!(Stories::default().into_iter().count(), 0);
}

#[test]
fn lists_names_and_shows_the_first_story() {
    let stories = vec![
        Story::new("Alpha", || view! { <p>"alpha-body"</p> }),
        Story::new("Beta", || view! { <p>"beta-body"</p> }),
    ];
    let html = leptotes::render(move || view! { <Gallery stories=stories /> });
    assert!(html.contains("Alpha"));
    assert!(html.contains("Beta"));
    assert!(html.contains("alpha-body"), "first story renders on load");
}

#[test]
fn shows_controls_description_and_code_for_a_story_with_args() {
    let html = leptotes::render(move || {
        let stories = vec![Story::__from_macro(
            "Gamma",
            || view! { <p>"gamma-body"</p> }.into_any(),
            vec![("flag", ArgControl::Bool(RwSignal::new(true)))],
            "view! { <Gamma /> }",
            Some("A gamma story."),
        )];
        view! { <Gallery stories=stories /> }
    });
    assert!(html.contains("gamma-body"), "the stage renders the view");
    assert!(html.contains("A gamma story."), "the description renders");
    assert!(
        html.contains("theoria-controls") && html.contains("flag"),
        "controls render"
    );
    assert!(html.contains("theoria-code"), "show-code renders");
}

#[test]
fn only_stories_with_a_description_get_a_nav_docs_badge() {
    let html = leptotes::render(move || {
        let stories = vec![
            Story::new("Alpha", || view! { <p>"alpha-body"</p> }),
            Story::__from_macro(
                "Beta",
                || view! { <p>"beta-body"</p> }.into_any(),
                Vec::new(),
                "view! { <Beta /> }",
                Some("A beta story."),
            ),
        ];
        view! { <Gallery stories=stories /> }
    });
    assert_eq!(
        leptotes::count(&html, "theoria-docs-badge"),
        1,
        "only Beta (with a description) gets the nav badge"
    );
}
