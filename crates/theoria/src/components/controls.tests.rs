//! leptotes component tests for `Controls` (the args/knobs panel).

use leptos::prelude::*;

use super::Controls;
use crate::ArgControl;

#[test]
fn renders_a_widget_per_arg() {
    let html = leptotes::render(|| {
        let args = vec![
            ("active", ArgControl::Bool(RwSignal::new(true))),
            ("width", ArgControl::Num(RwSignal::new(12.0))),
            ("label", ArgControl::Text(RwSignal::new("hi".to_string()))),
        ];
        view! { <Controls args=args /> }
    });
    assert!(html.contains("theoria-controls"), "the panel renders");
    assert!(html.contains("active") && html.contains("width") && html.contains("label"));
    assert_eq!(leptotes::count(&html, "<input"), 3, "one widget per arg");
    assert!(html.contains("checkbox"), "bool → checkbox");
    assert!(html.contains("number"), "f64 → number input");
    // The argTypes table documents each arg's type.
    assert!(html.contains(">bool<"), "bool type label");
    assert!(html.contains(">f64<"), "f64 type label");
    assert!(html.contains(">String<"), "String type label");
}

#[test]
fn no_args_renders_nothing() {
    let html = leptotes::render(|| view! { <Controls args=Vec::new() /> });
    assert!(!html.contains("theoria-controls"));
}

fn number_control(value: f64) -> String {
    leptotes::render(move || {
        view! { <Controls args=vec![("width", ArgControl::Num(RwSignal::new(value)))] /> }
    })
}

#[test]
fn a_number_control_starts_at_its_arg() {
    let html = number_control(12.5);
    assert!(html.contains(r#"value="12.5""#), "got:\n{html}");
}

#[test]
fn a_number_control_for_a_non_finite_arg_starts_empty() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let html = number_control(value);
        assert!(!html.contains("value="), "arg {value}; got:\n{html}");
    }
}
