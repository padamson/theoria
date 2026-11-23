//! theoria stories for the controls panel — theoria dogfooding its own component
//! as a fixture (one level; not a Gallery-in-Gallery). Compiled only under the
//! `stories` feature.

use leptos::prelude::*;

use super::Controls;
use crate::{Arg, ArgControl, Bounded, Color, Story, story};

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

pub fn stories() -> Vec<Story> {
    vec![
        knobs(),
        knobs_as_signals(),
        knobs_bounded(),
        knobs_color(),
        Story::new("Controls · panel", || {
            view! {
                <Controls args=vec![
                    Arg::new("active", "bool", ArgControl::Bool(RwSignal::new(true))),
                    Arg::new("width_ft", "f64", ArgControl::Num(RwSignal::new(12.0))),
                    Arg::new(
                        "label",
                        "String",
                        ArgControl::Text(RwSignal::new("Patio".to_string())),
                    ),
                ] />
            }
        }),
    ]
}
