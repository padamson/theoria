//! leptotes component tests for `Controls` (the args/knobs panel).

use leptos::prelude::*;

use leptotes::{Dom, Element};

use super::Controls;
use super::controls::{copy_said, picker_start};
use crate::{Arg, Bounded, Choice, Color, Story, StoryArg, story};

/// `attr` on every element in `elements`, in order, `None` where it is missing.
pub(super) fn each<'a>(elements: Vec<Element<'a>>, attr: &str) -> Vec<Option<&'a str>> {
    elements.into_iter().map(|el| el.attr(attr)).collect()
}

/// A one-declaration `style` value without the `;` Leptos writes after it.
fn declaration(style: Option<&str>) -> Option<&str> {
    style.map(|style| style.trim_end_matches(';'))
}

fn three_args() -> Dom {
    leptotes::dom(|| {
        let args = vec![
            Arg::new("active", "bool", RwSignal::new(true)),
            Arg::new("width", "Signal<f64>", RwSignal::new(12.0)),
            Arg::new("label", "String", RwSignal::new("hi".to_string())),
        ];
        view! { <Controls args=args /> }
    })
}

#[test]
fn each_row_is_named_for_its_arg() {
    let dom = three_args();
    assert_eq!(
        dom.texts(".control-name"),
        ["active", "width", "label"],
        "{dom}"
    );
}

#[test]
fn each_row_shows_its_args_type_as_the_story_wrote_it() {
    let dom = three_args();
    assert_eq!(
        dom.texts(".control-type"),
        ["bool", "Signal<f64>", "String"],
        "{dom}"
    );
}

#[test]
fn each_arg_gets_the_input_its_type_picks() {
    let dom = three_args();
    assert_eq!(
        each(dom.find_all("input"), "type"),
        [Some("checkbox"), Some("number"), Some("text")],
        "{dom}"
    );
}

#[test]
fn a_panel_with_args_has_a_reset_button() {
    let dom = number_control(12.5);
    assert_eq!(dom.count("button.theoria-reset"), 1, "{dom}");
}

#[test]
fn no_args_draws_no_panel() {
    let dom = leptotes::dom(|| view! { <Controls args=Vec::new() /> });
    assert_eq!(dom.count(".theoria-controls"), 0, "{dom}");
}

fn number_control(value: f64) -> Dom {
    leptotes::dom(move || {
        view! {
            <Controls args=vec![
                Arg::new("width", "f64", RwSignal::new(value)),
            ] />
        }
    })
}

#[test]
fn a_number_control_starts_at_its_arg() {
    let dom = number_control(12.5);
    assert_eq!(dom.find("input").attr("value"), Some("12.5"), "{dom}");
}

#[test]
fn a_number_control_for_a_non_finite_arg_starts_empty() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let dom = number_control(value);
        assert_eq!(dom.find("input").attr("value"), None, "arg {value}; {dom}");
    }
}

fn bounded_control(bounded: Bounded) -> Dom {
    leptotes::dom(move || {
        view! {
            <Controls args=vec![
                Arg::new("radius", "Bounded", RwSignal::new(bounded)),
            ] />
        }
    })
}

fn quarter_steps() -> Dom {
    bounded_control(Bounded::new(0.25).min(0.0).max(1.0).step(0.0625))
}

#[test]
fn a_bounded_control_with_both_bounds_draws_a_number_box_then_a_slider() {
    let dom = quarter_steps();
    assert_eq!(
        each(dom.find_all("input"), "type"),
        [Some("number"), Some("range")],
        "{dom}"
    );
}

#[test]
fn a_bounded_controls_inputs_both_carry_its_min() {
    let dom = quarter_steps();
    assert_eq!(
        each(dom.find_all("input"), "min"),
        [Some("0"), Some("0")],
        "{dom}"
    );
}

#[test]
fn a_bounded_controls_inputs_both_carry_its_max() {
    let dom = quarter_steps();
    assert_eq!(
        each(dom.find_all("input"), "max"),
        [Some("1"), Some("1")],
        "{dom}"
    );
}

#[test]
fn a_bounded_controls_inputs_both_carry_its_step() {
    let dom = quarter_steps();
    assert_eq!(
        each(dom.find_all("input"), "step"),
        [Some("0.0625"), Some("0.0625")],
        "{dom}"
    );
}

#[test]
fn a_bounded_control_without_both_bounds_draws_only_a_number_box() {
    let dom = bounded_control(Bounded::new(3.0).min(0.0));
    assert_eq!(
        each(dom.find_all("input"), "type"),
        [Some("number")],
        "{dom}"
    );
}

#[test]
fn a_bounded_control_without_a_step_takes_any_value() {
    let dom = bounded_control(Bounded::new(3.0));
    assert_eq!(dom.find("input").attr("step"), Some("any"), "{dom}");
}

#[test]
fn a_bounded_number_box_starts_at_its_value() {
    let dom = bounded_control(Bounded::new(0.25).min(0.0).max(1.0));
    assert_eq!(
        dom.find("input[type=number]").attr("value"),
        Some("0.25"),
        "{dom}"
    );
}

fn color_control(color: &str) -> Dom {
    let color = Color::from(color);
    leptotes::dom(move || {
        view! {
            <Controls args=vec![
                Arg::new("ink", "Color", RwSignal::new(color.clone())),
            ] />
        }
    })
}

#[test]
fn a_color_control_draws_a_picker_then_a_text_field() {
    let dom = color_control("#243832");
    assert_eq!(
        each(dom.find_all("input"), "type"),
        [Some("color"), Some("text")],
        "{dom}"
    );
}

#[test]
fn a_color_controls_swatch_is_painted_with_the_color_as_written() {
    let dom = color_control("rgb(36 56 50 / 0.8)");
    assert_eq!(
        declaration(dom.find(".theoria-swatch > span").attr("style")),
        Some("background-color:rgb(36 56 50 / 0.8)"),
        "{dom}"
    );
}

#[test]
fn text_that_would_style_more_than_a_color_styles_nothing() {
    let dom = color_control("teal; width: 9em");
    assert_eq!(dom.count("[style]"), 0, "{dom}");
}

#[test]
fn a_color_controls_text_field_says_what_it_takes() {
    let dom = color_control("#243832");
    assert_eq!(
        dom.find("input[type=text]").attr("placeholder"),
        Some("#rrggbb or any CSS color"),
        "{dom}"
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
    let dom = color_control("rgb(36 56 50 / 0.8)");
    assert_eq!(
        dom.find("input[type=text]").attr("value"),
        Some("rgb(36 56 50 / 0.8)"),
        "{dom}"
    );
}

fn text_control() -> Dom {
    leptotes::dom(|| {
        view! {
            <Controls args=vec![
                Arg::new("label", "String", RwSignal::new("Patio".to_string())),
            ] />
        }
    })
}

#[test]
fn a_text_control_starts_at_its_arg() {
    let dom = text_control();
    assert_eq!(dom.find("input").attr("value"), Some("Patio"), "{dom}");
}

fn choice_control(choice: Choice<&'static str>) -> Dom {
    leptotes::dom(move || {
        let build = RwSignal::new(choice.clone());
        view! { <Controls args=vec![Arg::new("build", "Choice<&'static str>", build)] /> }
    })
}

#[test]
fn a_choice_control_draws_a_select() {
    let dom = choice_control(Choice::new(["dev", "0.13.0"]));
    assert_eq!(dom.count("select"), 1, "{dom}");
}

#[test]
fn a_choice_control_lists_each_option_under_its_label() {
    let dom = choice_control(Choice::labelled([
        ("Serif", "Georgia, serif"),
        ("Sans", "system-ui"),
    ]));
    assert_eq!(dom.texts("select option"), ["Serif", "Sans"], "{dom}");
}

#[test]
fn a_choice_control_marks_the_selected_option_alone() {
    let dom = choice_control(Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.13.0"));
    assert_eq!(dom.texts("option[selected]"), ["0.13.0"], "{dom}");
}

#[test]
fn a_checkbox_is_named_for_its_arg() {
    let dom = leptotes::dom(|| {
        view! {
            <Controls args=vec![
                Arg::new("active", "bool", RwSignal::new(true)),
            ] />
        }
    });
    assert_eq!(
        each(dom.find_all("input"), "aria-label"),
        [Some("active")],
        "{dom}"
    );
}

#[test]
fn a_number_box_is_named_for_its_arg() {
    let dom = number_control(12.5);
    assert_eq!(
        each(dom.find_all("input"), "aria-label"),
        [Some("width")],
        "{dom}"
    );
}

#[test]
fn a_text_field_is_named_for_its_arg() {
    let dom = text_control();
    assert_eq!(
        each(dom.find_all("input"), "aria-label"),
        [Some("label")],
        "{dom}"
    );
}

#[test]
fn a_bounded_controls_number_box_and_slider_are_both_named_for_its_arg() {
    let dom = bounded_control(Bounded::new(0.25).min(0.0).max(1.0));
    assert_eq!(
        each(dom.find_all("input"), "aria-label"),
        [Some("radius"), Some("radius")],
        "{dom}"
    );
}

#[test]
fn a_color_controls_picker_and_text_field_are_both_named_for_its_arg() {
    let dom = color_control("#243832");
    assert_eq!(
        each(dom.find_all("input"), "aria-label"),
        [Some("ink"), Some("ink")],
        "{dom}"
    );
}

#[test]
fn a_menu_is_named_for_its_arg() {
    let dom = choice_control(Choice::new(["dev", "0.13.0"]));
    assert_eq!(
        dom.find("select").attr("aria-label"),
        Some("build"),
        "{dom}"
    );
}

fn group_control() -> Dom {
    leptotes::dom(|| {
        let size = Arg::group(
            "size",
            "Size",
            vec![
                Arg::new("width", "f64", RwSignal::new(12.0)),
                Arg::new("raised", "bool", RwSignal::new(true)),
            ],
        );
        view! { <Controls args=vec![size] /> }
    })
}

#[test]
fn a_group_is_labelled_by_its_name() {
    let dom = group_control();
    assert_eq!(
        dom.find("[role=group]").attr("aria-label"),
        Some("size"),
        "{dom}"
    );
}

#[test]
fn a_group_draws_a_control_per_arg_inside_it() {
    let dom = group_control();
    assert_eq!(
        each(dom.find("[role=group]").find_all("input"), "type"),
        [Some("number"), Some("checkbox")],
        "{dom}"
    );
}

#[test]
fn a_group_inside_a_group_is_drawn_inside_it_under_its_name() {
    let dom = leptotes::dom(|| {
        let border = Arg::group(
            "border",
            "Border",
            vec![Arg::new("width", "f64", RwSignal::new(1.0))],
        );
        let card = Arg::group(
            "card",
            "Card",
            vec![Arg::new("raised", "bool", RwSignal::new(true)), border],
        );
        view! { <Controls args=vec![card] /> }
    });
    assert_eq!(
        dom.find("[role=group][aria-label=card] [role=group]")
            .attr("aria-label"),
        Some("border"),
        "{dom}"
    );
}

/// A `glow` group of one number, switched `on` or off.
fn switched_group(on: bool) -> Dom {
    leptotes::dom(move || {
        let glow = Arg::group(
            "glow",
            "Option<Glow>",
            vec![Arg::new("depth", "f64", RwSignal::new(2.0))],
        )
        .switched(RwSignal::new(on));
        view! { <Controls args=vec![glow] /> }
    })
}

/// The checkbox in a group's head.
const SWITCH: &str = "[role=group] > .control-head input[type=checkbox]";

#[test]
fn a_switched_groups_head_has_a_checkbox_named_for_it() {
    let dom = switched_group(true);
    assert_eq!(dom.find(SWITCH).attr("aria-label"), Some("glow"), "{dom}");
}

#[test]
fn a_switched_on_groups_checkbox_is_checked() {
    let dom = switched_group(true);
    assert!(dom.find(SWITCH).attr("checked").is_some(), "{dom}");
}

#[test]
fn a_switched_off_groups_checkbox_is_not_checked() {
    let dom = switched_group(false);
    assert_eq!(dom.find(SWITCH).attr("checked"), None, "{dom}");
}

#[test]
fn a_switched_on_group_draws_its_fields() {
    let dom = switched_group(true);
    assert_eq!(dom.count("[role=group] .control-fields input"), 1, "{dom}");
}

#[test]
fn a_switched_off_group_draws_none_of_its_fields() {
    let dom = switched_group(false);
    assert_eq!(dom.count("[role=group] .control-fields"), 0, "{dom}");
}

#[test]
fn a_group_without_a_switch_has_no_checkbox_in_its_head() {
    let dom = group_control();
    assert_eq!(dom.count(".control-head input"), 0, "{dom}");
}

#[test]
fn a_group_shows_its_type() {
    let dom = group_control();
    assert_eq!(
        dom.find("[role=group] > .control-head .control-type")
            .text(),
        "Size",
        "{dom}"
    );
}

#[test]
fn an_args_description_is_drawn_as_markdown_with_its_control() {
    let dom = leptotes::dom(|| {
        let width =
            Arg::new("width", "f64", RwSignal::new(12.0)).with_description("The width, in `ft`.");
        view! { <Controls args=vec![width] /> }
    });
    assert_eq!(
        dom.find(".control-row > .control-description code").text(),
        "ft",
        "{dom}"
    );
}

#[derive(Clone, Debug, StoryArg)]
struct Size {
    width: f64,
    edge: Edge,
}

#[derive(Clone, Debug, Default, StoryArg)]
struct Edge {
    depth: f64,
}

#[story(size = Size { width: 12.0, edge: Edge { depth: 2.0 } })]
fn sized(size: Size, label: String) -> impl IntoView {
    view! { <p>{label}{size.width}</p> }
}

#[story]
fn edged(edge: Option<Edge>) -> impl IntoView {
    view! { <p>{edge.map(|edge| edge.depth)}</p> }
}

fn panel_of(story: Story) -> Dom {
    leptotes::dom(move || view! { <Controls args=story.args().to_vec() /> })
}

#[test]
fn a_struct_arg_shows_its_value_as_pretty_debug_writes_it() {
    let dom = panel_of(sized());
    assert_eq!(
        dom.find("[role=group][aria-label=size] .control-code code")
            .text_content(),
        "Size {\n    width: 12.0,\n    edge: Edge {\n        depth: 2.0,\n    },\n}",
        "{dom}"
    );
}

#[test]
fn a_struct_inside_a_struct_arg_shows_no_value_of_its_own() {
    let dom = panel_of(sized());
    assert_eq!(dom.count(".control-code"), 1, "{dom}");
}

#[test]
fn an_arg_that_is_not_a_struct_shows_no_value() {
    let dom = three_args();
    assert_eq!(dom.count(".control-code"), 0, "{dom}");
}

#[test]
fn a_struct_arg_switched_off_shows_its_value_as_none() {
    let dom = panel_of(edged());
    assert_eq!(
        dom.find(".control-code code").text_content(),
        "None",
        "{dom}"
    );
}

#[test]
fn a_struct_args_copy_button_is_named_for_it() {
    let dom = panel_of(sized());
    assert_eq!(
        dom.find("[role=group][aria-label=size] button.control-copy")
            .attr("aria-label"),
        Some("Copy size"),
        "{dom}"
    );
}

#[test]
fn what_copy_says_starts_empty_in_a_status_region() {
    let dom = panel_of(sized());
    assert_eq!(
        dom.find("[role=group][aria-label=size] .control-code [role=status]")
            .text(),
        "",
        "{dom}"
    );
}

#[test]
fn a_copy_that_lands_says_copied() {
    assert_eq!(copy_said(true), "Copied");
}

#[test]
fn a_copy_that_fails_says_so() {
    assert_eq!(copy_said(false), "Failed");
}

#[test]
fn a_struct_args_copy_button_reads_copy() {
    let dom = panel_of(sized());
    assert_eq!(dom.find("button.control-copy").text(), "Copy", "{dom}");
}
