//! leptotes component test for `ShowCode`.

use leptos::prelude::*;
use leptotes::Dom;

use super::ShowCode;

fn shown() -> Dom {
    leptotes::dom(|| view! { <ShowCode source="view! { <Yard /> }" /> })
}

#[test]
fn the_toggle_is_labelled_show_code() {
    let dom = shown();
    assert_eq!(
        dom.find("details.theoria-code > summary").text(),
        "Show code",
        "{dom}"
    );
}

#[test]
fn the_source_is_shown_as_written_in_a_code_block() {
    let dom = shown();
    assert_eq!(
        dom.find("details.theoria-code pre > code").text(),
        "view! { <Yard /> }",
        "{dom}"
    );
}
