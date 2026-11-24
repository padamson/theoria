//! leptotes component tests for `Controls` (the args/knobs panel).

use leptos::prelude::*;

use super::Controls;
use super::controls::picker_start;
use crate::{Arg, ArgControl, Bounded, Choice, Color, StoryArg};

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
    // Each row shows its arg's type as the story wrote it.
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

fn color_control(color: &str) -> String {
    let color = Color::from(color);
    leptotes::render(move || {
        view! {
            <Controls args=vec![
                Arg::new("ink", "Color", ArgControl::Color(RwSignal::new(color.clone()))),
            ] />
        }
    })
}

#[test]
fn a_color_control_draws_a_swatch() {
    let html = color_control("#243832");
    assert_eq!(inputs_of_type(&html, "color"), 1, "got:\n{html}");
}

#[test]
fn a_color_control_draws_a_text_field() {
    let html = color_control("#243832");
    assert_eq!(inputs_of_type(&html, "text"), 1, "got:\n{html}");
}

/// The one `<input>` tag of type `ty` in `html`.
fn input_tag<'a>(html: &'a str, ty: &str) -> &'a str {
    let marker = format!(r#"type="{ty}""#);
    let tags: Vec<&str> = html
        .split("<input")
        .skip(1)
        .map(|rest| &rest[..rest.find('>').unwrap_or(rest.len())])
        .filter(|tag| tag.contains(&marker))
        .collect();
    assert_eq!(tags.len(), 1, "one {ty} input; got:\n{html}");
    tags[0]
}

#[test]
fn a_color_controls_swatch_is_painted_with_the_color_as_written() {
    let html = color_control("rgb(36 56 50 / 0.8)");
    assert_eq!(
        leptotes::count(&html, "background-color:rgb(36 56 50 / 0.8)"),
        1,
        "got:\n{html}"
    );
}

#[test]
fn text_that_would_style_more_than_a_color_paints_no_swatch() {
    let html = color_control("teal; width: 9em");
    assert_eq!(leptotes::count(&html, "style="), 0, "got:\n{html}");
}

#[test]
fn a_color_controls_text_field_says_what_it_takes() {
    let html = color_control("#243832");
    let text = input_tag(&html, "text");
    assert!(
        text.contains(r##"placeholder="#rrggbb or any CSS color""##),
        "got: {text}"
    );
}

#[test]
fn the_picker_opens_at_an_opaque_color() {
    assert_eq!(picker_start("rgb(0, 128, 128)"), "#008080");
}

#[test]
fn the_picker_opens_one_step_off_a_color_with_alpha() {
    assert_eq!(picker_start("rgba(36, 56, 50, 0.8)"), "#243833");
}

#[test]
fn the_picker_opens_one_step_off_white_with_alpha_inside_the_range() {
    assert_eq!(picker_start("rgba(255, 255, 255, 0.5)"), "#fffffe");
}

#[test]
fn the_picker_opens_one_step_off_black_for_text_that_shows_no_color() {
    assert_eq!(picker_start("rgba(0, 0, 0, 0)"), "#000001");
}

#[test]
fn the_picker_opens_one_step_off_black_for_a_color_outside_rgb() {
    assert_eq!(picker_start("oklch(0.5 0.1 200)"), "#000001");
}

#[test]
fn a_color_controls_text_starts_at_the_color_as_written() {
    let html = color_control("rgb(36 56 50 / 0.8)");
    assert_eq!(
        leptotes::count(&html, r#"value="rgb(36 56 50 / 0.8)""#),
        1,
        "got:\n{html}"
    );
}

#[test]
fn a_text_control_starts_at_its_arg() {
    let html = leptotes::render(|| {
        view! {
            <Controls args=vec![
                Arg::new("label", "String", ArgControl::Text(RwSignal::new("Patio".to_string()))),
            ] />
        }
    });
    let text = input_tag(&html, "text");
    assert!(text.contains(r#"value="Patio""#), "got: {text}");
}

fn choice_control(choice: Choice<&'static str>) -> String {
    leptotes::render(move || {
        let control = Choice::control(RwSignal::new(choice.clone()));
        view! { <Controls args=vec![Arg::new("build", "Choice<&'static str>", control)] /> }
    })
}

#[test]
fn a_choice_control_draws_a_select() {
    let html = choice_control(Choice::new(["dev", "0.13.0"]));
    assert_eq!(leptotes::count(&html, "<select"), 1, "got:\n{html}");
}

#[test]
fn a_choice_control_lists_each_option_under_its_label() {
    let html = choice_control(Choice::labelled([
        ("Serif", "Georgia, serif"),
        ("Sans", "system-ui"),
    ]));
    for label in [">Serif</option>", ">Sans</option>"] {
        assert_eq!(leptotes::count(&html, label), 1, "{label}; got:\n{html}");
    }
}

/// The labels of the options marked `selected` in `html`.
fn selected_labels(html: &str) -> Vec<&str> {
    html.split("<option")
        .skip(1)
        .filter_map(|rest| {
            let (tag, after) = rest.split_once('>')?;
            let (label, _) = after.split_once("</option>")?;
            tag.contains(" selected").then_some(label)
        })
        .collect()
}

#[test]
fn a_choice_control_marks_the_selected_option_alone() {
    let html = choice_control(Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.13.0"));
    assert_eq!(selected_labels(&html), ["0.13.0"], "got:\n{html}");
}

fn named(html: &str, name: &str) -> usize {
    leptotes::count(html, &format!(r#"aria-label="{name}""#))
}

#[test]
fn a_checkbox_is_named_for_its_arg() {
    let html = leptotes::render(|| {
        view! {
            <Controls args=vec![
                Arg::new("active", "bool", ArgControl::Bool(RwSignal::new(true))),
            ] />
        }
    });
    assert_eq!(named(&html, "active"), 1, "got:\n{html}");
}

#[test]
fn a_number_box_is_named_for_its_arg() {
    let html = number_control(12.5);
    assert_eq!(named(&html, "width"), 1, "got:\n{html}");
}

#[test]
fn a_text_field_is_named_for_its_arg() {
    let html = leptotes::render(|| {
        view! {
            <Controls args=vec![
                Arg::new("label", "String", ArgControl::Text(RwSignal::new("Patio".to_string()))),
            ] />
        }
    });
    assert_eq!(named(&html, "label"), 1, "got:\n{html}");
}

#[test]
fn a_bounded_controls_number_box_and_slider_are_both_named_for_its_arg() {
    let html = bounded_control(Bounded::new(0.25).min(0.0).max(1.0));
    assert_eq!(named(&html, "radius"), 2, "got:\n{html}");
}

#[test]
fn a_color_controls_swatch_and_text_field_are_both_named_for_its_arg() {
    let html = color_control("#243832");
    assert_eq!(named(&html, "ink"), 2, "got:\n{html}");
}

#[test]
fn a_menu_is_named_for_its_arg() {
    let html = choice_control(Choice::new(["dev", "0.13.0"]));
    assert_eq!(named(&html, "build"), 1, "got:\n{html}");
}
