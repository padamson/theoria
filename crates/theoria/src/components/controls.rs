//! The controls ("knobs") panel: a row per story arg, with its name and type
//! over an editable widget as wide as the panel. Editing a widget writes the
//! arg's signal. A story that takes the arg as a value runs its body again; one
//! that takes it as a `Signal` redraws only what reads it. The args come from
//! the `#[story]` macro. The Reset button puts every arg back to its default.

use leptos::prelude::*;

use super::Markdown;
use crate::{Arg, ArgControl, Bounded, ChoiceControl, Color};

/// A number input's starting `value`: the arg, or none for one that is not
/// finite, since the input rejects `NaN` and `inf` and would log a warning.
fn number_value(value: f64) -> Option<String> {
    value.is_finite().then(|| value.to_string())
}

fn parse<T: std::str::FromStr>(ev: &leptos::ev::Event) -> Option<T> {
    event_target_value(ev).parse().ok()
}

fn show(number: NodeRef<leptos::html::Input>, text: &str) {
    if let Some(input) = number.get_untracked() {
        input.set_value(text);
    }
}

/// The value is set when the box is drawn, not bound to the arg. Chromium
/// reports a typed `1.` as `1`, and writing even that back drops the `.` and
/// moves the caret to the start. A reset draws the rows again over the same
/// boxes, and only `prop:value` reaches a box that has been typed in; the
/// `value` attribute would not.
fn number_box(
    name: &'static str,
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
            aria-label=name
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

fn number_widget(name: &'static str, s: RwSignal<f64>) -> impl IntoView {
    number_box(
        name,
        NodeRef::new(),
        s.get_untracked(),
        (None, None),
        "0.5".to_string(),
        move |value| s.set(value),
        move || s.get_untracked(),
    )
}

fn bounded_widget(name: &'static str, s: RwSignal<Bounded>) -> impl IntoView {
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
                aria-label=name
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
                name,
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
    name: &'static str,
    start: String,
    placeholder: Option<&'static str>,
    get: impl Fn() -> String + Send + Sync + 'static,
    set: impl Fn(String) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <input
            type="text"
            aria-label=name
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

fn color_widget(name: &'static str, s: RwSignal<Color>) -> impl IntoView {
    let swatch = NodeRef::<leptos::html::Span>::new();
    let set = move |text: String| s.set(Color(text));
    view! {
        <span class="theoria-color">
            <span class="theoria-swatch">
                <span node_ref=swatch style=move || s.with(paint)></span>
                <input
                    type="color"
                    aria-label=name
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
                name,
                s.with_untracked(|c| c.as_str().to_string()),
                Some("#rrggbb or any CSS color"),
                move || s.with(|c| c.as_str().to_string()),
                set,
            )}
        </span>
    }
}

fn choice_widget(name: &'static str, choice: ChoiceControl) -> impl IntoView {
    let start = choice.selected.get_untracked();
    let options = choice
        .labels()
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            view! {
                <option
                    value=index.to_string()
                    selected=index == start
                    prop:selected=move || choice.selected() == index
                >
                    {label}
                </option>
            }
        })
        .collect::<Vec<_>>();
    view! {
        <select aria-label=name on:change=move |ev| {
            if let Some(index) = parse(&ev) {
                choice.select(index);
            }
        }>{options}</select>
    }
}

#[component]
pub fn Controls(args: Vec<Arg>) -> impl IntoView {
    (!args.is_empty()).then(move || {
        let args = StoredValue::new(args);
        // A number box is written only when it is drawn, so a reset of any
        // arg draws the rows again, which patches each box in place.
        let rows = move || {
            args.with_value(|args| {
                args.iter().for_each(Arg::track_reset);
                args.iter().copied().map(row).collect::<Vec<_>>()
            })
        };
        let reset = move |_| args.with_value(|args| args.iter().for_each(Arg::reset));
        view! {
            <div class="theoria-controls">
                {rows}
                <button type="button" class="theoria-reset" on:click=reset>"Reset"</button>
            </div>
        }
    })
}

fn row(arg: Arg) -> AnyView {
    let (name, ty) = (arg.name(), arg.ty());
    let control = arg.control();
    let group = matches!(control, ArgControl::Group(_));
    let body = match control {
        ArgControl::Group(group) => {
            let rows = group.args().into_iter().map(row).collect::<Vec<_>>();
            view! { <div class="control-fields">{rows}</div> }.into_any()
        }
        control => view! { <div class="control-input">{widget(name, control)}</div> }.into_any(),
    };
    view! {
        <div class="control-row" role=group.then_some("group") aria-label=group.then_some(name)>
            <div class="control-head">
                <span class="control-name">{name}</span>
                <code class="control-type">{ty}</code>
            </div>
            {arg.description().map(|text| view! {
                <div class="control-description"><Markdown text=text /></div>
            })}
            {body}
        </div>
    }
    .into_any()
}

/// The editable widget for a control that is not a group.
fn widget(name: &'static str, control: ArgControl) -> AnyView {
    match control {
        ArgControl::Bool(s) => view! {
            <input
                type="checkbox"
                aria-label=name
                prop:checked=move || s.get()
                on:change=move |ev| s.set(event_target_checked(&ev))
            />
        }
        .into_any(),
        ArgControl::Num(s) => number_widget(name, s).into_any(),
        ArgControl::Text(s) => text_box(
            name,
            s.get_untracked(),
            None,
            move || s.get(),
            move |text| s.set(text),
        )
        .into_any(),
        ArgControl::Bounded(s) => bounded_widget(name, s).into_any(),
        ArgControl::Color(s) => color_widget(name, s).into_any(),
        ArgControl::Choice(choice) => choice_widget(name, choice).into_any(),
        ArgControl::Group(_) => unreachable!("`row` draws a group's args"),
    }
}
