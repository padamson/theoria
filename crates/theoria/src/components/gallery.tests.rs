//! leptotes component tests for `Gallery`.

use leptos::prelude::*;
use leptotes::Dom;

use super::Gallery;
use super::gallery::{Opening, gallery_opened_at, opening, story_href, story_in, with_story};
use super::story_nav_tests::badges;
use crate::{Arg, Stories, Story};

fn alpha_beta() -> Vec<Story> {
    vec![
        Story::new("Alpha", || view! { <p>"alpha-body"</p> }),
        Story::new("Beta", || view! { <p>"beta-body"</p> }),
    ]
}

fn gallery_of(stories: Vec<Story>) -> Dom {
    leptotes::dom(move || view! { <Gallery stories=stories /> })
}

#[test]
fn stories_gallery_is_the_gallery_over_its_stories() {
    let stories: Stories = alpha_beta().into();
    assert_eq!(
        leptotes::render(move || stories.gallery()),
        gallery_of(alpha_beta()).to_string()
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
        gallery_of(alpha_beta()).to_string()
    );
}

#[test]
fn stories_extend_at_the_end() {
    let mut stories: Stories = first().collect();
    stories.extend(second());
    assert_eq!(
        leptotes::render(move || stories.gallery()),
        gallery_of(alpha_beta()).to_string()
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
fn the_sidebar_lists_every_story() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(dom.texts(".theoria-nav a"), ["Alpha", "Beta"], "{dom}");
}

#[test]
fn the_first_story_is_on_the_stage_on_load() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(dom.find(".theoria-stage").text(), "alpha-body", "{dom}");
}

fn gamma() -> Dom {
    leptotes::dom(move || {
        let stories = vec![Story::__from_macro(
            "Gamma",
            || view! { <p>"gamma-body"</p> }.into_any(),
            vec![Arg::new("flag", "bool", RwSignal::new(true))],
            "view! { <Gamma /> }",
            Some("A gamma story."),
        )];
        view! { <Gallery stories=stories /> }
    })
}

#[test]
fn a_story_with_args_is_drawn_on_the_stage() {
    let dom = gamma();
    assert_eq!(dom.find(".theoria-stage").text(), "gamma-body", "{dom}");
}

#[test]
fn the_panel_shows_the_storys_description() {
    let dom = gamma();
    assert_eq!(
        dom.find(".theoria-panel > .theoria-md").text(),
        "A gamma story.",
        "{dom}"
    );
}

#[test]
fn the_panel_shows_a_control_per_arg() {
    let dom = gamma();
    assert_eq!(
        dom.texts(".theoria-panel .theoria-controls .control-name"),
        ["flag"],
        "{dom}"
    );
}

#[test]
fn the_panel_shows_the_storys_code() {
    let dom = gamma();
    assert_eq!(
        dom.find(".theoria-panel .theoria-code code").text(),
        "view! { <Gamma /> }",
        "{dom}"
    );
}

#[test]
fn only_stories_with_a_description_get_a_nav_docs_badge() {
    let dom = gallery_of(vec![
        Story::new("Alpha", || view! { <p>"alpha-body"</p> }),
        Story::new("Beta", || view! { <p>"beta-body"</p> }).with_description("A beta story."),
    ]);
    assert_eq!(badges(&dom), [0, 1], "{dom}");
}

fn described() -> Vec<Story> {
    first()
        .map(|story| story.with_description("Why it is **here**."))
        .collect()
}

#[test]
fn a_story_built_by_hand_shows_its_description_as_markdown() {
    let dom = gallery_of(described());
    assert_eq!(
        dom.find(".theoria-panel > .theoria-md strong").text(),
        "here",
        "{dom}"
    );
}

#[test]
fn a_story_built_by_hand_with_a_description_gets_the_nav_docs_badge() {
    let dom = gallery_of(described());
    assert_eq!(badges(&dom), [1], "{dom}");
}

#[test]
fn a_storys_href_reads_back_as_its_name() {
    for name in [
        "Version bar/Playground",
        "Knobs · demo",
        "a&b=c",
        "1+1",
        "100%",
        "#top",
        "a%2Fb",
    ] {
        assert_eq!(
            story_in(&story_href(name)).as_deref(),
            Some(name),
            "through {:?}",
            story_href(name)
        );
    }
}

#[test]
fn a_query_without_story_names_none() {
    assert_eq!((story_in(""), story_in("?other=Alpha")), (None, None));
}

#[test]
fn an_empty_story_names_none() {
    assert_eq!(story_in("?story="), None);
}

#[test]
fn with_story_replaces_the_story_and_keeps_the_rest() {
    assert_eq!(
        with_story("?theme=dark&story=Alpha&zoom=2", "Beta"),
        "?theme=dark&zoom=2&story=Beta"
    );
}

#[test]
fn with_story_adds_the_story_after_the_rest() {
    assert_eq!(with_story("?theme=dark", "Beta"), "?theme=dark&story=Beta");
}

#[test]
fn a_slash_written_as_2f_reads_as_a_slash() {
    assert_eq!(
        story_in("?story=Version+bar%2FPlayground").as_deref(),
        Some("Version bar/Playground")
    );
}

#[test]
fn story_is_read_beside_other_parameters() {
    assert_eq!(story_in("?other=1&story=Alpha").as_deref(), Some("Alpha"));
}

const NAMES: [&str; 2] = ["Alpha", "Beta"];

#[test]
fn a_url_naming_a_story_opens_it_over_the_stored_one() {
    assert_eq!(
        opening(&NAMES, Some("Beta"), Some("Alpha")),
        Opening::Story(1)
    );
}

#[test]
fn a_url_naming_no_story_opens_none_and_names_what_it_asked_for() {
    assert_eq!(
        opening(&NAMES, Some("Gamma"), Some("Alpha")),
        Opening::NoSuchStory("Gamma".into())
    );
}

#[test]
fn a_bare_url_opens_the_stored_story() {
    assert_eq!(opening(&NAMES, None, Some("Beta")), Opening::Story(1));
}

#[test]
fn a_bare_url_opens_the_first_story_when_the_stored_one_is_gone() {
    assert_eq!(opening(&NAMES, None, Some("Gamma")), Opening::Story(0));
}

#[test]
fn a_bare_url_opens_the_first_story_when_nothing_is_stored() {
    assert_eq!(opening(&NAMES, None, None), Opening::Story(0));
}

fn opened_at(opened: Opening) -> Dom {
    leptotes::dom(move || gallery_opened_at(alpha_beta(), opened))
}

#[test]
fn a_gallery_opened_at_a_story_shows_it() {
    let dom = opened_at(Opening::Story(1));
    assert_eq!(dom.find(".theoria-stage").text(), "beta-body", "{dom}");
}

#[test]
fn a_gallery_opened_at_no_such_story_names_it_on_the_stage() {
    let dom = opened_at(Opening::NoSuchStory("Gamma".into()));
    assert_eq!(
        dom.find(".theoria-stage > .theoria-missing").text(),
        r#"No story named "Gamma""#,
        "{dom}"
    );
}

#[test]
fn a_gallery_opened_at_no_such_story_marks_no_story_current() {
    let dom = opened_at(Opening::NoSuchStory("Gamma".into()));
    assert_eq!(dom.count("[aria-current]"), 0, "{dom}");
}

#[test]
fn a_gallery_opened_at_no_such_story_shows_no_panel() {
    let dom = opened_at(Opening::NoSuchStory("Gamma".into()));
    assert_eq!(dom.count(".theoria-panel"), 0, "{dom}");
}
