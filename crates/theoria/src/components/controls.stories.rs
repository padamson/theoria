//! theoria stories for the controls panel — theoria dogfooding its own component
//! as a fixture (one level; not a Gallery-in-Gallery). Compiled only under the
//! `stories` feature.

use leptos::prelude::*;

use super::Controls;
use crate::{Arg, Bounded, Choice, Color, Story, StoryArg, story};

/// A **knobs** demo: edit the controls and watch the stage update live.
///
/// It exercises the autodocs panel against theoria's own components:
///
/// - `on` toggles the flag shown on the stage.
/// - `count` and `label` echo straight through.
///
/// The `#[story]` macro reads each arg's signal in the view (so a control
/// edit runs this body again) and captures the source shown under "Show code".
#[story(name = "Knobs · demo", on = true, count = 2.0, label = "hi")]
fn knobs(on: bool, count: f64, label: String) -> impl IntoView {
    view! {
        <div data-testid="knobs-out">
            <p class="k-label">{label}</p>
            <p class="k-count">{count}</p>
            <p class="k-flag">{if on { "ON" } else { "OFF" }}</p>
        </div>
    }
}

/// The same knobs taken as **signals**: the body runs once, and each edit
/// redraws only the text that reads it.
///
/// So state the story makes for itself survives an edit: click the button,
/// then change a knob, and the count stays. A story that takes values runs
/// its body again on every edit, and the count would start over.
///
/// A `Signal<T>` arg gets the same control as `T`. Read it inside a closure
/// (`move || label.get()`); a read in the body itself reruns the whole story.
#[story(name = "Knobs · signals", on = true, count = 2.0, label = "hi")]
fn knobs_as_signals(on: Signal<bool>, count: Signal<f64>, label: Signal<String>) -> impl IntoView {
    let clicks = RwSignal::new(0);
    view! {
        <div data-testid="signals-out">
            <p class="k-label">{move || label.get()}</p>
            <p class="k-count">{move || count.get()}</p>
            <p class="k-flag">{move || if on.get() { "ON" } else { "OFF" }}</p>
            <button class="k-clicks" on:click=move |_| clicks.update(|n| *n += 1)>
                {move || format!("clicked {}", clicks.get())}
            </button>
        </div>
    }
}

/// A **bounded** number: a number box with a slider between `min` and
/// `max`. Type a value out of range and the story gets the nearest bound;
/// type one between steps and it gets the nearest step.
#[story(name = "Knobs · bounded", radius = Bounded::new(0.25).min(0.0).max(1.0).step(0.0625))]
fn knobs_bounded(radius: Signal<Bounded>) -> impl IntoView {
    view! {
        <div
            class="k-box"
            style="width: 6rem; height: 3rem; border: 2px solid currentColor"
            style:border-radius=move || format!("{}rem", radius.get().value())
        >
            <p class="k-radius">{move || radius.get().value()}</p>
        </div>
    }
}

/// A **color**: pick one in the swatch, or type any CSS color into the text,
/// such as `rgb(36 56 50 / 0.8)`. The swatch shows whichever it holds.
#[story(name = "Knobs · color", ink = "#243832")]
fn knobs_color(ink: Signal<Color>) -> impl IntoView {
    view! {
        <div style="width: 6rem; height: 3rem" style:background=move || ink.get().to_string()></div>
        <p class="k-ink">{move || ink.get().to_string()}</p>
    }
}

/// A **choice**: one of a fixed list, picked from a menu. Here each value
/// is its own label.
#[story(name = "Knobs · choice", build = Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.13.0"))]
fn knobs_choice(build: Signal<Choice<&'static str>>) -> impl IntoView {
    view! { <p class="k-build">{move || build.get().value()}</p> }
}

/// A choice of **labelled** values: the menu shows each label, and the story
/// gets the value under it.
#[story(
    name = "Knobs · labelled choice",
    font = Choice::labelled([("Serif", "Georgia, serif"), ("Sans", "system-ui, sans-serif")])
        .selected("Sans"),
)]
fn knobs_labelled_choice(font: Choice<&'static str>) -> impl IntoView {
    let font = font.value();
    view! { <p class="k-font" style:font-family=font>{font}</p> }
}

/// A card's look, as a struct whose fields are the controls.
#[derive(Clone, StoryArg)]
struct Card {
    /// The card's heading.
    title: &'static str,
    /// The heading's color, any CSS color.
    #[story_arg(color)]
    ink: &'static str,
    /// The card's background.
    #[story_arg(color)]
    paper: &'static str,
    /// Bold heading.
    bold: bool,
    /// Corner radius, in `rem`.
    radius: Bounded,
    /// The line around the card.
    border: Border,
    /// The card's shadow, or none.
    shadow: Option<Shadow>,
}

/// A line's look, as a struct that is a field of `Card`.
#[derive(Clone, StoryArg)]
struct Border {
    /// The line's color, any CSS color.
    #[story_arg(color)]
    color: &'static str,
    /// Its width, in `px`.
    width: Bounded,
}

/// A shadow under the card, as a struct an `Option` holds.
#[derive(Clone, StoryArg)]
struct Shadow {
    /// The shadow's color, any CSS color.
    #[story_arg(color)]
    color: &'static str,
    /// How far it blurs, in `px`.
    blur: Bounded,
}

/// Where the shadow's fields start when it is switched on from none.
impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "rgb(0 0 0 / 0.25)",
            blur: Bounded::new(6.0).min(0.0).max(24.0).step(1.0),
        }
    }
}

/// A **struct** arg: `Card` derives `StoryArg`, so each of its fields gets
/// the control its type picks, under the arg's name, with the field's doc
/// comment beside it. A `&'static str` field is a text box, or a color with
/// `#[story_arg(color)]`. A field whose type derives `StoryArg` too, such as
/// `border`, is a group of its own fields under its name. An `Option` of one,
/// such as `shadow`, has a switch: off is `None`, and on again brings back the
/// fields as they were left.
#[story(
    name = "Knobs · struct",
    card = Card {
        title: "Patio",
        ink: "#243832",
        paper: "#f6f1e7",
        bold: true,
        radius: Bounded::new(0.5).min(0.0).max(2.0).step(0.125),
        border: Border { color: "#c9bfa8", width: Bounded::new(1.0).min(0.0).max(8.0).step(1.0) },
        shadow: Some(Shadow::default()),
    },
)]
fn knobs_struct(card: Signal<Card>) -> impl IntoView {
    view! {
        <div
            class="k-card"
            style="width: 10rem; padding: 0.75rem"
            style:background=move || card.get().paper
            style:border-radius=move || format!("{}rem", card.get().radius.value())
            style:border=move || {
                let border = card.get().border;
                format!("{}px solid {}", border.width.value(), border.color)
            }
            style:box-shadow=move || {
                card.get().shadow.map_or_else(
                    || "none".to_string(),
                    |shadow| format!("0 2px {}px {}", shadow.blur.value(), shadow.color),
                )
            }
        >
            <p
                class="k-title"
                style:color=move || card.get().ink
                style:font-weight=move || if card.get().bold { "700" } else { "400" }
            >
                {move || card.get().title}
            </p>
        </div>
    }
}

pub fn stories() -> Vec<Story> {
    vec![
        knobs(),
        knobs_as_signals(),
        knobs_bounded(),
        knobs_color(),
        knobs_choice(),
        knobs_labelled_choice(),
        knobs_struct(),
        // At the panel's width, with a name and a type too long for one line.
        Story::new("Controls · panel", || {
            view! {
                <div style="width: 280px">
                    <Controls args=vec![
                        Arg::new("active", "bool", RwSignal::new(true)),
                        Arg::new("width_ft", "f64", RwSignal::new(12.0)),
                        Arg::new(
                            "label",
                            "String",
                            RwSignal::new("Patio".to_string()),
                        ),
                        Arg::new(
                            "selected_row_background_highlight_color_when_hovered",
                            "leptos::prelude::Signal<theoria::Color>",
                            RwSignal::new(Color::from("#243832")),
                        ),
                    ] />
                </div>
            }
        }),
    ]
}
