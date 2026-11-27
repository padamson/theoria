//! leptotes component tests for `StoryNav`, plus unit tests for the pure
//! path → tree builder behind the grouped sidebar.

use leptos::prelude::*;
use leptotes::Dom;

use super::StoryNav;
use super::controls_tests::each;
use super::gallery::story_href;
use super::story_nav::{Modifiers, NavNode, build_tree};

fn nav(names: Vec<&'static str>, selected: Option<usize>) -> Dom {
    leptotes::dom(move || {
        let (selected, set_selected) = signal(selected);
        view! { <StoryNav names=names selected=selected set_selected=set_selected /> }
    })
}

/// How many docs badges each sidebar link carries, in order.
pub(super) fn badges(dom: &Dom) -> Vec<usize> {
    dom.find_all(".theoria-nav a")
        .iter()
        .map(|link| link.count(".theoria-docs-badge"))
        .collect()
}

#[test]
fn lists_every_name() {
    let dom = nav(vec!["Alpha", "Beta"], Some(0));
    assert_eq!(dom.texts("a"), ["Alpha", "Beta"], "{dom}");
}

#[test]
fn a_storys_href_is_its_full_name_form_encoded_but_for_slashes() {
    assert_eq!(
        story_href("Version bar/Playground"),
        "?story=Version+bar/Playground"
    );
    assert_eq!(story_href("Knobs · demo"), "?story=Knobs+%C2%B7+demo");
}

#[test]
fn each_leaf_links_to_its_storys_url() {
    let dom = nav(vec!["Theme/Playground", "Version bar/Playground"], Some(0));
    assert_eq!(
        dom.attrs("a", "href"),
        ["?story=Theme/Playground", "?story=Version+bar/Playground"],
        "{dom}"
    );
}

#[test]
fn no_has_docs_shows_no_badges() {
    let dom = nav(vec!["Alpha", "Beta"], Some(0));
    assert_eq!(dom.count(".theoria-docs-badge"), 0, "{dom}");
}

#[test]
fn marks_only_the_stories_with_docs() {
    let dom = leptotes::dom(|| {
        let (selected, set_selected) = signal(Some(0usize));
        view! {
            <StoryNav
                names=vec!["Alpha", "Beta"]
                selected=selected
                set_selected=set_selected
                has_docs=vec![true, false]
            />
        }
    });
    assert_eq!(badges(&dom), [1, 0], "{dom}");
}

#[test]
fn marks_the_selected_item_as_the_current_page() {
    let dom = nav(vec!["Alpha", "Beta"], Some(1));
    assert_eq!(
        each(dom.find_all("a"), "aria-current"),
        [None, Some("page")],
        "{dom}"
    );
}

#[test]
fn marks_nothing_when_no_story_is_selected() {
    let dom = nav(vec!["Alpha", "Beta"], None);
    assert_eq!(dom.count("[aria-current]"), 0, "{dom}");
}

fn structures() -> Dom {
    nav(vec!["Structures/House", "Structures/Wall", "Grid"], Some(0))
}

#[test]
fn names_sharing_a_path_segment_share_one_group() {
    let dom = structures();
    assert_eq!(dom.count(".theoria-group"), 1, "{dom}");
}

#[test]
fn a_group_is_labelled_by_its_shared_segment() {
    let dom = structures();
    let label = dom.find(".theoria-group-label");
    let caret = label.find(".caret").text();
    assert_eq!(
        label.text().strip_prefix(caret.as_str()).map(str::trim),
        Some("Structures"),
        "{dom}"
    );
}

#[test]
fn a_group_starts_open() {
    let dom = structures();
    assert_eq!(dom.count(".theoria-group > ul.collapsed"), 0, "{dom}");
}

#[test]
fn a_groups_stories_link_from_inside_it_by_their_last_segment() {
    let dom = structures();
    assert_eq!(
        dom.find(".theoria-group").texts("a"),
        ["House", "Wall"],
        "{dom}"
    );
}

#[test]
fn a_name_without_a_path_links_from_the_top_level() {
    let dom = structures();
    assert_eq!(dom.texts("nav > ul > li > a"), ["Grid"], "{dom}");
}

#[test]
fn build_tree_merges_groups_and_keeps_indices() {
    let tree = build_tree(vec!["A/x", "A/y", "B", "A/z"]);
    assert_eq!(
        tree,
        vec![
            NavNode::Group {
                label: "A".into(),
                children: vec![
                    NavNode::Leaf {
                        label: "x".into(),
                        index: 0
                    },
                    NavNode::Leaf {
                        label: "y".into(),
                        index: 1
                    },
                    NavNode::Leaf {
                        label: "z".into(),
                        index: 3
                    },
                ],
            },
            NavNode::Leaf {
                label: "B".into(),
                index: 2
            },
        ]
    );
}

#[test]
fn build_tree_nests_deeply() {
    let tree = build_tree(vec!["X/Y/leaf"]);
    assert_eq!(
        tree,
        vec![NavNode::Group {
            label: "X".into(),
            children: vec![NavNode::Group {
                label: "Y".into(),
                children: vec![NavNode::Leaf {
                    label: "leaf".into(),
                    index: 0
                }],
            }],
        }]
    );
}

#[test]
fn a_click_with_no_modifier_is_plain() {
    assert!(Modifiers::default().is_plain());
}

#[test]
fn a_click_with_any_one_modifier_is_not_plain() {
    let none = Modifiers::default();
    for (key, held) in [
        ("ctrl", Modifiers { ctrl: true, ..none }),
        ("meta", Modifiers { meta: true, ..none }),
        (
            "shift",
            Modifiers {
                shift: true,
                ..none
            },
        ),
        ("alt", Modifiers { alt: true, ..none }),
    ] {
        assert!(!held.is_plain(), "a {key}-click counted as plain");
    }
}

#[test]
fn each_level_of_groups_indents_its_links_one_step_further() {
    let dom = nav(vec!["Top", "A/In one", "A/B/In two"], Some(0));
    let indents: Vec<&str> = dom
        .attrs("a", "style")
        .into_iter()
        .map(|style| style.trim_end_matches(';'))
        .collect();
    assert_eq!(
        indents,
        [
            "padding-left: 8px",
            "padding-left: 20px",
            "padding-left: 32px"
        ],
        "{dom}"
    );
}
