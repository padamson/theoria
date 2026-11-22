//! leptotes component tests for `Controls` (the args/knobs panel).

use leptos::prelude::*;

use super::Controls;
use crate::{Arg, ArgControl, Bounded};

#[test]
fn renders_a_widget_per_arg() {
    let html = leptotes::render(|| {
        let args = vec![
            Arg::new("active", "bool", ArgControl::Bool(RwSignal::new(true))),
            Arg::new("width", "Signal<f64>", ArgControl::Num(RwSignal::new(12.0))),
            Arg::new(
                "label",
                "String",
                ArgControl::Text(RwSignal::new("hi".to_string())),
            ),
        ];
        view! { <Controls args=args /> }
    });
    assert!(html.contains("theoria-controls"), "the panel renders");
    assert!(html.contains("active") && html.contains("width") && html.contains("label"));
    assert_eq!(leptotes::count(&html, "<input"), 3, "one widget per arg");
    assert!(html.contains("checkbox"), "bool → checkbox");
    assert!(html.contains("number"), "f64 → number input");
    // The argTypes table shows each arg's type as the story wrote it.
    assert!(html.contains(">bool<"), "bool type label");
    assert!(
        html.contains(">Signal&lt;f64&gt;<"),
        "Signal<f64> type label"
    );
    assert!(html.contains(">String<"), "String type label");
}

#[test]
fn no_args_renders_nothing() {
    let html = leptotes::render(|| view! { <Controls args=Vec::new() /> });
    assert!(!html.contains("theoria-controls"));
}

fn number_control(value: f64) -> String {
    leptotes::render(move || {
        view! {
            <Controls args=vec![
                Arg::new("width", "f64", ArgControl::Num(RwSignal::new(value))),
            ] />
        }
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

fn bounded_control(bounded: Bounded) -> String {
    leptotes::render(move || {
        view! {
            <Controls args=vec![
                Arg::new("radius", "Bounded", ArgControl::Bounded(RwSignal::new(bounded))),
            ] />
        }
    })
}

fn inputs_of_type(html: &str, ty: &str) -> usize {
    leptotes::count(html, &format!(r#"type="{ty}""#))
}

#[test]
fn a_bounded_control_with_both_bounds_draws_a_slider() {
    let html = bounded_control(Bounded::new(0.25).min(0.0).max(1.0).step(0.0625));
    assert_eq!(inputs_of_type(&html, "range"), 1, "got:\n{html}");
}

#[test]
fn a_bounded_control_with_both_bounds_draws_a_number_box() {
    let html = bounded_control(Bounded::new(0.25).min(0.0).max(1.0).step(0.0625));
    assert_eq!(inputs_of_type(&html, "number"), 1, "got:\n{html}");
}

#[test]
fn a_bounded_controls_inputs_both_carry_its_bounds_and_step() {
    let html = bounded_control(Bounded::new(0.25).min(0.0).max(1.0).step(0.0625));
    for attr in [r#"min="0""#, r#"max="1""#, r#"step="0.0625""#] {
        assert_eq!(leptotes::count(&html, attr), 2, "{attr}; got:\n{html}");
    }
}

#[test]
fn a_bounded_control_without_both_bounds_draws_no_slider() {
    let html = bounded_control(Bounded::new(3.0).min(0.0));
    assert_eq!(inputs_of_type(&html, "range"), 0, "got:\n{html}");
}

#[test]
fn a_bounded_control_without_both_bounds_draws_a_number_box() {
    let html = bounded_control(Bounded::new(3.0).min(0.0));
    assert_eq!(inputs_of_type(&html, "number"), 1, "got:\n{html}");
}

#[test]
fn a_bounded_control_without_a_step_takes_any_value() {
    let html = bounded_control(Bounded::new(3.0));
    assert_eq!(leptotes::count(&html, r#"step="any""#), 1, "got:\n{html}");
}

#[test]
fn a_bounded_number_box_starts_at_its_value() {
    let html = bounded_control(Bounded::new(0.25).min(0.0).max(1.0));
    assert_eq!(leptotes::count(&html, r#"value="0.25""#), 1, "got:\n{html}");
}
