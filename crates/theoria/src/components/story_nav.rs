//! The sidebar: a tree of story names. A story name is a `/`-delimited path
//! (e.g. `"Structures/House/Outline"`); the leaf links to the story and the
//! ancestors become groups. Flat names (no `/`) render as top-level leaves, so
//! the sidebar is backward-compatible.

use leptos::ev::MouseEvent;
use leptos::prelude::*;

use super::gallery::story_href;

/// The gallery's sidebar; each leaf links to its story's gallery URL
/// (`?story=<name>`).
#[allow(clippy::needless_pass_by_value)]
#[component]
pub fn StoryNav(
    names: Vec<&'static str>,
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    /// Whether each story (by the same index as `names`) has a Markdown
    /// description — those get a small marker in the sidebar, since most of
    /// a gallery's stories otherwise look identical from the list alone.
    #[prop(default = Vec::new())]
    has_docs: Vec<bool>,
) -> impl IntoView {
    let tree = build_tree(names.clone());
    let leaves = Leaves {
        names: &names,
        has_docs: &has_docs,
        selected,
        set_selected,
    };
    view! {
        <nav class="theoria-nav">
            <ul>{render_nodes(tree, &leaves, 0)}</ul>
        </nav>
    }
}

/// Whether a click is one the gallery takes, rather than one the browser gives
/// another meaning (a new tab or window).
fn is_plain_click(ev: &MouseEvent) -> bool {
    !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key())
}

/// What every leaf of the tree reads.
struct Leaves<'a> {
    names: &'a [&'static str],
    has_docs: &'a [bool],
    selected: ReadSignal<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
}

/// A node in the sidebar tree: a named `Group` of children, or a `Leaf` that
/// selects the story at `index` (its position in the original name list).
#[derive(Debug, PartialEq)]
pub(crate) enum NavNode {
    Group {
        label: String,
        children: Vec<NavNode>,
    },
    Leaf {
        label: String,
        index: usize,
    },
}

/// Build the sidebar tree from story names, splitting each on `/`. Insertion
/// order is preserved; equal group labels at the same level merge.
pub(crate) fn build_tree(names: Vec<&str>) -> Vec<NavNode> {
    let mut roots = Vec::new();
    for (i, name) in names.into_iter().enumerate() {
        let segs: Vec<&str> = name
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if segs.is_empty() {
            // A name that is empty (or all slashes) still gets a leaf.
            roots.push(NavNode::Leaf {
                label: (*name).to_string(),
                index: i,
            });
        } else {
            insert(&mut roots, &segs, i);
        }
    }
    roots
}

fn insert(level: &mut Vec<NavNode>, segs: &[&str], index: usize) {
    match segs {
        [leaf] => level.push(NavNode::Leaf {
            label: (*leaf).to_string(),
            index,
        }),
        [head, rest @ ..] => {
            let pos = level
                .iter()
                .position(|n| matches!(n, NavNode::Group { label, .. } if label == head));
            if let Some(p) = pos {
                if let NavNode::Group { children, .. } = &mut level[p] {
                    insert(children, rest, index);
                }
            } else {
                let mut children = Vec::new();
                insert(&mut children, rest, index);
                level.push(NavNode::Group {
                    label: (*head).to_string(),
                    children,
                });
            }
        }
        [] => {}
    }
}

/// Render a level of the tree (recursively) into list items. Returns a concrete
/// `Vec<AnyView>` so the recursion has a finite type.
fn render_nodes(nodes: Vec<NavNode>, leaves: &Leaves, depth: usize) -> Vec<AnyView> {
    let indent = |d: usize| format!("padding-left: {}px", 8 + d * 12);
    nodes
        .into_iter()
        .map(|node| match node {
            NavNode::Leaf { label, index } => {
                // A small page-with-folded-corner glyph — the conventional
                // "docs" mark — for a leaf whose panel has a description.
                let docs_badge = leaves.has_docs.get(index).copied().unwrap_or(false).then(|| {
                    view! {
                        <svg
                            class="theoria-docs-badge"
                            viewBox="0 0 12 14"
                            width="10"
                            height="11"
                            title="Has a docs panel"
                        >
                            <path
                                d="M1 1h6l3 3v9H1z"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1"
                            />
                            <path d="M7 1v3h3" fill="none" stroke="currentColor" stroke-width="1" />
                        </svg>
                    }
                });
                let (selected, set_selected) = (leaves.selected, leaves.set_selected);
                view! {
                    <li>
                        <a
                            href=story_href(leaves.names[index])
                            aria-current=move || (selected.get() == Some(index)).then_some("page")
                            style=indent(depth)
                            on:click=move |ev: MouseEvent| {
                                if is_plain_click(&ev) {
                                    ev.prevent_default();
                                    set_selected.set(Some(index));
                                }
                            }
                        >
                            {label}
                            {docs_badge}
                        </a>
                    </li>
                }
                .into_any()
            }
            NavNode::Group { label, children } => {
                let kids = render_nodes(children, leaves, depth + 1);
                // Each group is independently collapsible (open by default).
                let open = RwSignal::new(true);
                view! {
                    <li class="theoria-group">
                        <div
                            class="theoria-group-label"
                            style=indent(depth)
                            on:click=move |_| open.update(|o| *o = !*o)
                        >
                            <span class="caret">{move || if open.get() { "▾ " } else { "▸ " }}</span>
                            {label}
                        </div>
                        <ul class:collapsed=move || !open.get()>{kids}</ul>
                    </li>
                }
                .into_any()
            }
        })
        .collect()
}
