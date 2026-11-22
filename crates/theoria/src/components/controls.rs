//! The controls ("knobs") panel, rendered as an argTypes table: a row per story
//! arg with its name, type, and an editable widget. Editing a widget writes the
//! arg's signal. A story that takes the arg as a value runs its body again; one
//! that takes it as a `Signal` redraws only what reads it. The args come from
//! the `#[story]` macro.

use leptos::prelude::*;

use crate::{Arg, ArgControl, Bounded};

/// A number input's starting `value`: the arg, or none for one that is not
/// finite, since the input rejects `NaN` and `inf` and would log a warning.
fn number_value(value: f64) -> Option<String> {
    value.is_finite().then(|| value.to_string())
}

fn parse(ev: &leptos::ev::Event) -> Option<f64> {
    event_target_value(ev).parse().ok()
}

fn show(number: NodeRef<leptos::html::Input>, text: &str) {
    if let Some(input) = number.get_untracked() {
        input.set_value(text);
    }
}

/// The value is set when the box is built, not bound to the arg. Chromium
/// reports a typed `1.` as `1`, and writing even that back drops the `.` and
/// moves the caret to the start. The `value` attribute alone would not reach
/// a box that has been typed in, which the panel reuses for the next story.
fn number_box(
    number: NodeRef<leptos::html::Input>,
    start: f64,
    (min, max): (Option<f64>, Option<f64>),
    step: String,
    set: impl Fn(f64) + Copy + Send + Sync + 'static,
    current: impl Fn() -> f64 + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <input
            type="number"
            node_ref=number
            min=min
            max=max
            step=step
            value=number_value(start)
            prop:value=number_value(start).unwrap_or_default()
            on:input=move |ev| {
                if let Some(value) = parse(&ev) {
                    set(value);
                }
            }
            on:blur=move |_| show(number, &number_value(current()).unwrap_or_default())
        />
    }
}

fn number_widget(s: RwSignal<f64>) -> impl IntoView {
    number_box(
        NodeRef::new(),
        s.get_untracked(),
        (None, None),
        "0.5".to_string(),
        move |value| s.set(value),
        move || s.get_untracked(),
    )
}

fn bounded_widget(s: RwSignal<Bounded>) -> impl IntoView {
    let start = s.get_untracked();
    let step = start
        .step
        .map_or_else(|| "any".to_string(), |step| step.to_string());
    let number = NodeRef::<leptos::html::Input>::new();
    let set = move |value: f64| {
        s.maybe_update(|b| {
            let next = b.with_value(value);
            let changed = next != *b;
            *b = next;
            changed
        });
    };
    let slider = start.min.zip(start.max).map(|(min, max)| {
        view! {
            <input
                type="range"
                min=min
                max=max
                step=step.clone()
                prop:value=move || s.get().value.to_string()
                on:input=move |ev| {
                    if let Some(value) = parse(&ev) {
                        set(value);
                        show(number, &event_target_value(&ev));
                    }
                }
            />
        }
    });
    view! {
        <span class="theoria-bounded">
            {number_box(
                number,
                start.value,
                (start.min, start.max),
                step,
                set,
                move || s.get_untracked().value,
            )}
            {slider}
        </span>
    }
}

#[component]
pub fn Controls(args: Vec<Arg>) -> impl IntoView {
    (!args.is_empty()).then(move || {
        let rows = args
            .into_iter()
            .map(|arg| {
                let (name, ty) = (arg.name(), arg.ty());
                let widget = match arg.control() {
                    ArgControl::Bool(s) => view! {
                        <input
                            type="checkbox"
                            prop:checked=move || s.get()
                            on:change=move |ev| s.set(event_target_checked(&ev))
                        />
                    }
                    .into_any(),
                    ArgControl::Num(s) => number_widget(s).into_any(),
                    ArgControl::Text(s) => view! {
                        <input
                            type="text"
                            prop:value=move || s.get()
                            on:input=move |ev| s.set(event_target_value(&ev))
                        />
                    }
                    .into_any(),
                    ArgControl::Bounded(s) => bounded_widget(s).into_any(),
                };
                view! {
                    <tr class="control-row">
                        <td class="control-name">{name}</td>
                        <td class="control-type"><code>{ty}</code></td>
                        <td class="control-input">{widget}</td>
                    </tr>
                }
            })
            .collect::<Vec<_>>();
        view! {
            <table class="theoria-controls">
                <thead>
                    <tr>
                        <th>"Arg"</th>
                        <th>"Type"</th>
                        <th>"Control"</th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
        }
    })
}
