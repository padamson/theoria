//! The controls ("knobs") panel, rendered as an argTypes table: a row per story
//! arg with its name, type, and an editable widget. Editing a widget writes the
//! arg's signal. A story that takes the arg as a value runs its body again; one
//! that takes it as a `Signal` redraws only what reads it. The args come from
//! the `#[story]` macro.

use leptos::prelude::*;

use crate::{Arg, ArgControl};

/// A number input's starting `value`: the arg, or none for one that is not
/// finite, since the input rejects `NaN` and `inf` and would log a warning.
fn number_value(value: f64) -> Option<String> {
    value.is_finite().then(|| value.to_string())
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
                    // The value is set once, not bound to the arg. Chromium
                    // reports a typed `1.` as `1`, and writing even that back
                    // drops the `.` and moves the caret to the start.
                    ArgControl::Num(s) => view! {
                        <input
                            type="number"
                            step="0.5"
                            value=number_value(s.get_untracked())
                            on:input=move |ev| {
                                if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                    s.set(v);
                                }
                            }
                        />
                    }
                    .into_any(),
                    ArgControl::Text(s) => view! {
                        <input
                            type="text"
                            prop:value=move || s.get()
                            on:input=move |ev| s.set(event_target_value(&ev))
                        />
                    }
                    .into_any(),
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
