//! leptotes component tests for `Gallery`.

use leptos::prelude::*;
use leptotes::Dom;

use super::Gallery;
use super::gallery::{
    Drag, Opening, Step, clamp, gallery_opened_at, most_beside, opening, starts_a_drag, step_for,
    stepped, stored_width, story_href, story_in, with_story,
};
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

#[test]
fn the_panel_can_take_all_of_the_stage_but_200px() {
    assert_eq!(most_beside(280, 714), 794);
}

#[test]
fn a_stage_already_under_200px_leaves_the_panel_its_least() {
    assert_eq!(most_beside(220, 150), 220);
}

const MOST: i32 = 794;

#[test]
fn a_width_within_the_bounds_is_kept() {
    assert_eq!(clamp(400, MOST), 400);
}

#[test]
fn a_width_under_the_least_is_the_least() {
    assert_eq!(clamp(219, MOST), 220);
}

#[test]
fn a_width_over_the_most_is_the_most() {
    assert_eq!(clamp(795, MOST), 794);
}

/// A drag by pointer 1 that began at x = 1000 with the panel 300px wide.
const DRAG: Drag = Drag {
    pointer: 1,
    from: 1000,
    start: 300,
    most: MOST,
};

#[test]
fn dragging_left_widens_the_panel_as_far() {
    assert_eq!(DRAG.width_at(1, 900), Some(400));
}

#[test]
fn dragging_right_narrows_the_panel_as_far() {
    assert_eq!(DRAG.width_at(1, 1040), Some(260));
}

#[test]
fn a_drag_stops_at_the_most() {
    assert_eq!(DRAG.width_at(1, 0), Some(794));
}

#[test]
fn a_drag_stops_at_the_least() {
    assert_eq!(DRAG.width_at(1, 2000), Some(220));
}

#[test]
fn another_pointer_moves_nothing_in_a_drag() {
    assert_eq!(DRAG.width_at(2, 900), None);
}

#[test]
fn the_main_button_of_the_primary_pointer_starts_a_drag() {
    assert!(starts_a_drag(0, true));
}

#[test]
fn another_button_starts_no_drag() {
    for button in [1, 2] {
        assert!(!starts_a_drag(button, true), "button {button}");
    }
}

#[test]
fn a_pointer_that_is_not_primary_starts_no_drag() {
    assert!(!starts_a_drag(0, false));
}

#[test]
fn arrow_left_widens_the_panel() {
    assert_eq!(step_for("ArrowLeft"), Some(Step::Wider));
}

#[test]
fn arrow_right_narrows_the_panel() {
    assert_eq!(step_for("ArrowRight"), Some(Step::Narrower));
}

#[test]
fn other_keys_leave_the_divider() {
    for key in ["ArrowUp", "ArrowDown", "Enter", "l"] {
        assert_eq!(step_for(key), None, "{key}");
    }
}

#[test]
fn a_wider_step_adds_16px() {
    assert_eq!(stepped(300, Step::Wider, MOST), 316);
}

#[test]
fn a_narrower_step_takes_16px() {
    assert_eq!(stepped(300, Step::Narrower, MOST), 284);
}

#[test]
fn a_wider_step_stops_at_the_most() {
    assert_eq!(stepped(790, Step::Wider, MOST), 794);
}

#[test]
fn a_narrower_step_stops_at_the_least() {
    assert_eq!(stepped(230, Step::Narrower, MOST), 220);
}

#[test]
fn nothing_stored_is_the_default_width() {
    assert_eq!(stored_width(None), 280);
}

#[test]
fn a_stored_width_is_read_back() {
    assert_eq!(stored_width(Some("350")), 350);
}

#[test]
fn a_stored_width_under_the_least_is_the_least() {
    assert_eq!(stored_width(Some("100")), 220);
}

#[test]
fn a_stored_value_that_is_no_width_is_the_default() {
    for stored in ["", "wide", "350px", "350.5"] {
        assert_eq!(stored_width(Some(stored)), 280, "{stored:?}");
    }
}

/// The value of `property` in an element's `style`, as Leptos writes it
/// (`name: value;` for each declaration).
fn style<'a>(dom: &'a Dom, selector: &str, property: &str) -> Option<&'a str> {
    dom.find(selector)
        .attr("style")?
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .find(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim())
}

#[test]
fn the_divider_sits_between_the_stage_and_the_panel() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.count(".theoria > .theoria-stage + .theoria-divider + .theoria-panel"),
        1,
        "{dom}"
    );
}

#[test]
fn the_divider_is_a_separator() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("role"),
        Some("separator"),
        "{dom}"
    );
}

#[test]
fn the_divider_takes_the_focus_in_page_order() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("tabindex"),
        Some("0"),
        "{dom}"
    );
}

#[test]
fn the_divider_is_named_for_the_panels_width() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("aria-label"),
        Some("Panel width"),
        "{dom}"
    );
}

#[test]
fn the_divider_stands_upright() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("aria-orientation"),
        Some("vertical"),
        "{dom}"
    );
}

#[test]
fn the_divider_reads_the_panels_default_width() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("aria-valuenow"),
        Some("280"),
        "{dom}"
    );
}

#[test]
fn the_divider_reads_the_panels_least() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        dom.find(".theoria-divider").attr("aria-valuemin"),
        Some("220"),
        "{dom}"
    );
}

#[test]
fn the_divider_reads_a_width_within_its_range_before_it_is_measured() {
    let dom = gallery_of(alpha_beta());
    let divider = dom.find(".theoria-divider");
    let value = |attr| -> i32 {
        divider
            .attr(attr)
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("{attr} is no number in {dom}"))
    };
    let (least, now, most) = (
        value("aria-valuemin"),
        value("aria-valuenow"),
        value("aria-valuemax"),
    );
    assert!(least <= now && now <= most, "{least} <= {now} <= {most}");
}

#[test]
fn the_panel_starts_at_its_default_width() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        style(&dom, ".theoria-panel", "flex-basis"),
        Some("280px"),
        "{dom}"
    );
}

#[test]
fn the_panel_shrinks_no_narrower_than_its_least() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        style(&dom, ".theoria-panel", "min-width"),
        Some("220px"),
        "{dom}"
    );
}

#[test]
fn the_stage_shrinks_no_narrower_than_200px() {
    let dom = gallery_of(alpha_beta());
    assert_eq!(
        style(&dom, ".theoria-stage", "min-width"),
        Some("200px"),
        "{dom}"
    );
}

#[test]
fn a_gallery_opened_at_no_such_story_shows_no_divider() {
    let dom = opened_at(Opening::NoSuchStory("Gamma".into()));
    assert_eq!(dom.count(".theoria-divider"), 0, "{dom}");
}
