//! leptotes component test for `Markdown`.

use leptos::prelude::*;
use leptotes::Dom;

use super::Markdown;

fn rendered() -> Dom {
    leptotes::dom(|| {
        view! {
            <Markdown text="# Title\n\nSome **bold** text and a list:\n\n- one\n- two" />
        }
    })
}

#[test]
fn a_heading_is_drawn_as_a_heading() {
    let dom = rendered();
    assert_eq!(dom.find(".theoria-md > h1").text(), "Title", "{dom}");
}

#[test]
fn emphasis_is_drawn_as_strong() {
    let dom = rendered();
    assert_eq!(dom.find(".theoria-md strong").text(), "bold", "{dom}");
}

#[test]
fn a_list_is_drawn_as_its_items() {
    let dom = rendered();
    assert_eq!(dom.texts(".theoria-md > ul > li"), ["one", "two"], "{dom}");
}
