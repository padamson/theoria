//! leptotes component tests for `StoryNav`, plus unit tests for the pure
//! path → tree builder behind the grouped sidebar.

use leptos::prelude::*;

use super::StoryNav;
use super::gallery::story_href;
use super::story_nav::{NavNode, build_tree};

fn nav(names: Vec<&'static str>, selected: Option<usize>) -> String {
    leptotes::render(move || {
        let (selected, set_selected) = signal(selected);
        view! { <StoryNav names=names selected=selected set_selected=set_selected /> }
    })
}

/// Each sidebar link's opening tag and the text it starts with, in order.
fn links(html: &str) -> Vec<(&str, &str)> {
    html.split("<a ")
        .skip(1)
        .map(|rest| {
            let (tag, after) = rest.split_once('>').expect("a link's tag closes");
            (tag, after.split('<').next().unwrap_or(after))
        })
        .collect()
}

fn labels(html: &str) -> Vec<&str> {
    links(html).into_iter().map(|(_, text)| text).collect()
}

fn hrefs(html: &str) -> Vec<&str> {
    links(html)
        .into_iter()
        .filter_map(|(tag, _)| tag.split_once(r#"href=""#)?.1.split('"').next())
        .collect()
}

#[test]
fn lists_every_name() {
    let html = nav(vec!["Alpha", "Beta"], Some(0));
    assert_eq!(labels(&html), ["Alpha", "Beta"], "got:\n{html}");
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
    let html = nav(vec!["Theme/Playground", "Version bar/Playground"], Some(0));
    assert_eq!(
        hrefs(&html),
        ["?story=Theme/Playground", "?story=Version+bar/Playground"],
        "got:\n{html}"
    );
}

#[test]
fn no_has_docs_shows_no_badges() {
    let html = nav(vec!["Alpha", "Beta"], Some(0));
    assert!(
        !html.contains("theoria-docs-badge"),
        "no badge without has_docs; got:\n{html}"
    );
}

#[test]
fn marks_only_the_stories_with_docs() {
    let html = leptotes::render(|| {
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
    assert_eq!(
        leptotes::count(&html, "theoria-docs-badge"),
        1,
        "only the story with a description gets the badge"
    );
}

#[test]
fn marks_the_selected_item_as_the_current_page() {
    let html = nav(vec!["Alpha", "Beta"], Some(1));
    let current: Vec<bool> = links(&html)
        .iter()
        .map(|(tag, _)| tag.contains(r#"aria-current="page""#))
        .collect();
    assert_eq!(current, [false, true], "got:\n{html}");
}

#[test]
fn marks_nothing_when_no_story_is_selected() {
    let html = nav(vec!["Alpha", "Beta"], None);
    assert_eq!(leptotes::count(&html, "aria-current"), 0, "got:\n{html}");
}

#[test]
fn groups_names_by_path() {
    let html = nav(vec!["Structures/House", "Structures/Wall", "Grid"], Some(0));
    // The shared "Structures" group renders once as a (non-link) label; each
    // leaf renders as a link (by its last path segment).
    assert_eq!(
        leptotes::count(&html, r#"class="theoria-group-label""#),
        1,
        "got:\n{html}"
    );
    assert!(html.contains("Structures") && html.contains("House") && html.contains("Wall"));
    assert_eq!(
        links(&html).len(),
        3,
        "one link per leaf story; got:\n{html}"
    );
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
