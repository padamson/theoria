//! The controls ("knobs") panel, rendered as an argTypes table: a row per story
//! arg with its name, type, and an editable widget. Editing a widget writes the
//! arg's signal. A story that takes the arg as a value runs its body again; one
//! that takes it as a `Signal` redraws only what reads it. The args come from
//! the `#[story]` macro.

use leptos::prelude::*;

use crate::{Arg, ArgControl, Bounded, Color};

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

fn text_box(
    start: String,
    placeholder: Option<&'static str>,
    get: impl Fn() -> String + Send + Sync + 'static,
    set: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <input
            type="text"
            placeholder=placeholder
            value=start
            prop:value=get
            on:input=move |ev| set(event_target_value(&ev))
        />
    }
}

/// The swatch's `style`. The browser drops a color it cannot read, which
/// leaves the swatch empty. Text with a `;` would style more than the color.
fn paint(color: &Color) -> Option<String> {
    let text = color.as_str();
    (!text.contains(';')).then(|| format!("background-color:{text}"))
}

/// Where the picker opens, from the swatch's computed `background-color`.
/// The browser reports a pick only when it differs from the picker's value.
/// So unless the swatch shows an opaque `rgb(r, g, b)`, which the picker
/// holds exactly, it opens one step of blue off: picking the color shown,
/// or black for text that shows none, then still reaches the story.
pub(super) fn picker_start(computed: &str) -> String {
    let opaque = computed.starts_with("rgb(");
    let mut channels = computed
        .strip_prefix("rgba(")
        .or_else(|| computed.strip_prefix("rgb("))
        .unwrap_or_default()
        .split(',')
        .map(|channel| channel.trim().trim_end_matches(')').parse::<u8>());
    let (Some(Ok(r)), Some(Ok(g)), Some(Ok(b))) =
        (channels.next(), channels.next(), channels.next())
    else {
        return "#000001".to_string();
    };
    let b = if opaque { b } else { b ^ 1 };
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn computed_background(el: &leptos::web_sys::Element) -> Option<String> {
    let style = window().get_computed_style(el).ok()??;
    style.get_property_value("background-color").ok()
}

fn color_widget(s: RwSignal<Color>) -> impl IntoView {
    let swatch = NodeRef::<leptos::html::Span>::new();
    let set = move |text: String| s.set(Color(text));
    view! {
        <span class="theoria-color">
            <span class="theoria-swatch">
                <span node_ref=swatch style=move || s.with(paint)></span>
                <input
                    type="color"
                    on:click=move |ev| {
                        let shown = swatch
                            .get_untracked()
                            .and_then(|swatch| computed_background(&swatch))
                            .unwrap_or_default();
                        event_target::<leptos::web_sys::HtmlInputElement>(&ev)
                            .set_value(&picker_start(&shown));
                    }
                    on:input=move |ev| set(event_target_value(&ev))
                />
            </span>
            {text_box(
                s.with_untracked(|c| c.as_str().to_string()),
                Some("#rrggbb or any CSS color"),
                move || s.with(|c| c.as_str().to_string()),
                set,
            )}
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
                    ArgControl::Text(s) => text_box(
                        s.get_untracked(),
                        None,
                        move || s.get(),
                        move |text| s.set(text),
                    )
                    .into_any(),
                    ArgControl::Bounded(s) => bounded_widget(s).into_any(),
                    ArgControl::Color(s) => color_widget(s).into_any(),
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
