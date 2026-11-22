//! theoria stories for the controls panel — theoria dogfooding its own component
//! as a fixture (one level; not a Gallery-in-Gallery). Compiled only under the
//! `stories` feature.

use leptos::prelude::*;

use super::Controls;
use crate::{Arg, ArgControl, Story, story};

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

pub fn stories() -> Vec<Story> {
    vec![
        knobs(),
        knobs_as_signals(),
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
