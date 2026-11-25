//! A tiny component explorer (gallery) for Leptos.
//!
//! `theoria` (Greek θεωρία, "viewing / contemplation") renders a list of named
//! component previews ("stories") with a sidebar to switch between them, so a
//! component can be developed and eyeballed in isolation — Storybook for Leptos,
//! in miniature.
//!
//! Built component-driven and TDD'd with leptotes. Components live in
//! `src/components/` Storybook-style: `<name>.rs` (component),
//! `<name>.stories.rs` (theoria stories, behind the `stories` feature), and
//! `<name>.tests.rs` (leptotes tests). theoria's own components double as e2e
//! fixtures; we never nest a `Gallery` inside a `Gallery` (the infinite mirror).

// `must_use_candidate` is a false positive for Leptos `#[component]` fns — the
// framework consumes the returned view.
#![allow(clippy::must_use_candidate)]

// So the `#[story]` macro's `theoria::…` paths (which are written for downstream
// crates) also resolve when theoria uses the macro on itself (e.g. in tests).
extern crate self as theoria;

use leptos::prelude::*;

mod components;

#[cfg(feature = "stories")]
pub use components::stories;
pub use components::{Gallery, StoryNav};
/// The `#[story]` attribute macro (see `theoria-macros`).
pub use theoria_macros::story;

/// A live, editable arg ("knob") backing a control widget. Each variant holds
/// the reactive signal the control reads/writes and the story's view reads.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum ArgControl {
    Bool(RwSignal<bool>),
    Num(RwSignal<f64>),
    Text(RwSignal<String>),
    Bounded(RwSignal<Bounded>),
    Color(RwSignal<Color>),
    Choice(ChoiceControl),
}

/// A choice control's options and selection, whatever the [`Choice`]'s `T`.
#[derive(Clone, Copy)]
pub struct ChoiceControl {
    labels: StoredValue<Vec<String>>,
    selected: Signal<usize>,
    select: Callback<usize>,
}

impl ChoiceControl {
    /// Each option's label, in order.
    #[must_use]
    pub fn labels(&self) -> Vec<String> {
        self.labels.get_value()
    }

    /// The selected option's index, tracked like a signal's `get`.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected.get()
    }

    /// Select the option at `index`. An index past the last option is
    /// ignored.
    pub fn select(&self, index: usize) {
        self.select.run(index);
    }
}

/// One of a fixed list of labelled values, drawn as a `<select>`. The story
/// attribute gives the list, and the first option starts selected.
///
/// ```
/// use leptos::prelude::*;
/// use theoria::{Choice, story};
///
/// #[story(
///     build = Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.14.0"),
///     font = Choice::labelled([("Serif", "Georgia, serif"), ("Sans", "system-ui, sans-serif")]),
/// )]
/// fn bar(build: Signal<Choice<&'static str>>, font: Choice<&'static str>) -> impl IntoView {
///     view! { <p style:font-family=font.value()>{move || build.get().value()}</p> }
/// }
/// ```
///
/// A `Choice` arg with no list in the attribute does not compile:
///
/// ```compile_fail
/// use leptos::prelude::*;
/// use theoria::{Choice, story};
///
/// #[story]
/// fn bar(build: Choice<&'static str>) -> impl IntoView {
///     view! { <p>{build.value()}</p> }
/// }
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Choice<T> {
    options: Vec<(String, T)>,
    selected: usize,
}

impl<T> Choice<T> {
    /// Values that are their own labels.
    ///
    /// # Panics
    ///
    /// If `values` is empty.
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = T>) -> Self
    where
        T: ToString,
    {
        Self::labelled(values.into_iter().map(|value| (value.to_string(), value)))
    }

    /// Values under labels of their own, as `(label, value)`.
    ///
    /// # Panics
    ///
    /// If `options` is empty.
    #[must_use]
    pub fn labelled<L: Into<String>>(options: impl IntoIterator<Item = (L, T)>) -> Self {
        let options: Vec<_> = options
            .into_iter()
            .map(|(label, value)| (label.into(), value))
            .collect();
        assert!(!options.is_empty(), "a Choice needs at least one option");
        Self {
            options,
            selected: 0,
        }
    }

    /// Start at the option labelled `label`, not the first.
    ///
    /// # Panics
    ///
    /// If no option has that label.
    #[must_use]
    pub fn selected(mut self, label: &str) -> Self {
        self.selected = self
            .options
            .iter()
            .position(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("Choice has no option labelled {label:?}"));
        self
    }

    /// The selected option's value.
    #[must_use]
    pub fn value(&self) -> T
    where
        T: Clone,
    {
        self.options[self.selected].1.clone()
    }

    /// Whether selecting `index` changed the selection. An index past the
    /// last option changes nothing.
    fn select(&mut self, index: usize) -> bool {
        let changed = index < self.options.len() && index != self.selected;
        if changed {
            self.selected = index;
        }
        changed
    }
}

/// Any CSS color, as text, drawn as a swatch that opens the browser's
/// picker beside a text field.
///
/// ```
/// use leptos::prelude::*;
/// use theoria::{Color, story};
///
/// #[story(ink = "rgb(36 56 50 / 0.8)")]
/// fn ink(ink: Signal<Color>) -> impl IntoView {
///     view! { <p style:color=move || ink.get().to_string()>"Ink"</p> }
/// }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Color(String);

impl Color {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Color {
    fn from(text: &str) -> Self {
        Self(text.to_string())
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A number with optional bounds and step, drawn as a number input and,
/// with both bounds, a slider. The value stays within the bounds and on a
/// step, counted from the minimum.
///
/// ```
/// use leptos::prelude::*;
/// use theoria::{Bounded, story};
///
/// #[story(radius = Bounded::new(0.25).min(0.0).max(1.0).step(0.0625))]
/// fn corner(radius: Signal<Bounded>) -> impl IntoView {
///     view! { <div style:border-radius=move || format!("{}rem", radius.get().value()) /> }
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounded {
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
}

impl Bounded {
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }

    /// # Panics
    ///
    /// If `min` is not finite, or is above the maximum.
    #[must_use]
    pub fn min(self, min: f64) -> Self {
        Self {
            min: Some(min),
            ..self
        }
        .checked()
    }

    /// # Panics
    ///
    /// If `max` is not finite, or is below the minimum.
    #[must_use]
    pub fn max(self, max: f64) -> Self {
        Self {
            max: Some(max),
            ..self
        }
        .checked()
    }

    fn checked(self) -> Self {
        if let Some(min) = self.min {
            assert!(min.is_finite(), "Bounded min {min} is not finite");
        }
        if let Some(max) = self.max {
            assert!(max.is_finite(), "Bounded max {max} is not finite");
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            assert!(min <= max, "Bounded min {min} is above its max {max}");
        }
        self.with_value(self.value)
    }

    /// # Panics
    ///
    /// If `step` is not a finite number above zero.
    #[must_use]
    pub fn step(self, step: f64) -> Self {
        assert!(
            step.is_finite() && step > 0.0,
            "Bounded step {step} is not above zero"
        );
        Self {
            step: Some(step),
            ..self
        }
        .checked()
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.value
    }

    fn with_value(self, value: f64) -> Self {
        Self {
            value: self.snap(self.clamp(value)),
            ..self
        }
    }

    /// `value` must already be within the bounds.
    fn snap(self, value: f64) -> f64 {
        let Some(step) = self.step else {
            return value;
        };
        let base = self.min.unwrap_or(0.0);
        let mut steps = ((value - base) / step).round();
        if self.max.is_some_and(|max| base + steps * step > max) {
            steps -= 1.0;
        }
        round_to(base + steps * step, decimals(step).max(decimals(base)))
    }

    fn clamp(self, value: f64) -> f64 {
        let value = self.min.map_or(value, |min| value.max(min));
        self.max.map_or(value, |max| value.min(max))
    }
}

/// The digits `value` prints after its decimal point.
fn decimals(value: f64) -> usize {
    let text = value.to_string();
    text.find('.').map_or(0, |point| text.len() - point - 1)
}

/// `value` to `places` decimals, which takes the float noise off a sum of
/// decimal steps. Past what an `f64` holds there is nothing to take off.
fn round_to(value: f64, places: usize) -> f64 {
    match i32::try_from(places) {
        Ok(places) if places <= 15 => {
            let scale = 10f64.powi(places);
            (value * scale).round() / scale
        }
        _ => value,
    }
}

/// One editable arg of a story: its name, its type as the story wrote it,
/// and its control.
#[derive(Clone, Copy)]
pub struct Arg {
    name: &'static str,
    ty: &'static str,
    control: ArgControl,
}

impl Arg {
    #[must_use]
    pub fn new(name: &'static str, ty: &'static str, control: ArgControl) -> Self {
        Self { name, ty, control }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// As the story wrote it (`Signal<f64>`), shown beside the arg's name.
    #[must_use]
    pub fn ty(&self) -> &'static str {
        self.ty
    }

    #[must_use]
    pub fn control(&self) -> ArgControl {
        self.control
    }
}

mod sealed {
    pub trait Sealed {}

    /// A story arg that is its own value. The supertraits repeat `Value`'s
    /// bounds, which rustc does not carry over through `Value = Self`.
    #[diagnostic::on_unimplemented(
        message = "`Signal<{Self}>` cannot be a #[story] arg",
        label = "no control for a signal of this type",
        note = "a Signal arg holds a bool, f64, String, Bounded, Color, or Choice"
    )]
    pub trait Plain: super::StoryArg<Value = Self> + Clone + Send + Sync + 'static {}
}

/// A type a `#[story]` parameter can have: `bool`, `f64`, `String`,
/// [`Bounded`], [`Color`], [`Choice`], or a `Signal` of one, which hands the body the
/// arg's signal, not its value.
///
/// ```
/// use leptos::prelude::*;
/// use theoria::story;
///
/// #[story(label = "Ink", size = 12.0)]
/// fn swatch(label: Signal<String>, bold: bool, size: leptos::prelude::Signal<f64>) -> impl IntoView {
///     view! { <b class:bold=bold data-size=move || size.get()>{move || label.get()}</b> }
/// }
/// ```
///
/// Any other type is a compile error at the parameter:
///
/// ```compile_fail
/// use leptos::prelude::*;
/// use theoria::story;
///
/// #[story(label = "Ink")]
/// fn swatch(label: Signal<String>, bold: i32) -> impl IntoView {
///     view! { <b>{move || label.get()}</b> }
/// }
/// ```
///
/// The trait is sealed, and its methods are the `#[story]` macro's to call.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a #[story] arg",
    label = "no control for this type",
    note = "a #[story] arg is a bool, f64, String, Bounded, Color, or Choice, or a `Signal` of one (not an `RwSignal`)"
)]
pub trait StoryArg: Sized + sealed::Sealed {
    /// What the arg's control edits.
    type Value: Clone + Send + Sync + 'static;

    /// What a default in the story attribute is written as.
    type Init: Into<Self::Value>;

    #[doc(hidden)]
    fn control(signal: RwSignal<Self::Value>) -> ArgControl;

    #[doc(hidden)]
    fn bind(signal: Signal<Self::Value>) -> Self;
}

macro_rules! value_arg {
    ($ty:ty, init: $init:ty, control: $variant:ident) => {
        impl sealed::Sealed for $ty {}
        impl sealed::Plain for $ty {}

        impl StoryArg for $ty {
            type Value = $ty;
            type Init = $init;

            fn control(signal: RwSignal<$ty>) -> ArgControl {
                ArgControl::$variant(signal)
            }

            fn bind(signal: Signal<$ty>) -> Self {
                signal.get()
            }
        }
    };
}

value_arg!(bool, init: bool, control: Bool);
value_arg!(f64, init: f64, control: Num);
value_arg!(String, init: &'static str, control: Text);
value_arg!(Bounded, init: Bounded, control: Bounded);
value_arg!(Color, init: &'static str, control: Color);

impl<T> sealed::Sealed for Choice<T> {}
impl<T: Clone + Send + Sync + 'static> sealed::Plain for Choice<T> {}

impl<T: Clone + Send + Sync + 'static> StoryArg for Choice<T> {
    type Value = Self;
    type Init = Self;

    fn control(signal: RwSignal<Self>) -> ArgControl {
        let labels = signal.with_untracked(|choice| {
            choice
                .options
                .iter()
                .map(|(label, _)| label.clone())
                .collect()
        });
        ArgControl::Choice(ChoiceControl {
            labels: StoredValue::new(labels),
            selected: Signal::derive(move || signal.with(|choice| choice.selected)),
            select: Callback::new(move |index: usize| {
                signal.maybe_update(|choice| choice.select(index));
            }),
        })
    }

    fn bind(signal: Signal<Self>) -> Self {
        signal.get()
    }
}

impl<T: sealed::Plain> sealed::Sealed for Signal<T> {}

impl<T: sealed::Plain> StoryArg for Signal<T> {
    type Value = T;
    type Init = T::Init;

    fn control(signal: RwSignal<T>) -> ArgControl {
        T::control(signal)
    }

    fn bind(signal: Signal<T>) -> Self {
        signal
    }
}

/// One named preview in the gallery.
pub struct Story {
    name: &'static str,
    // `Send + Sync`: Leptos 0.8's reactive closures (the stage re-renders when
    // the selection changes) require their captures to be thread-safe.
    view: Box<dyn Fn() -> AnyView + Send + Sync>,
    /// Editable args ("knobs"), in declaration order. Empty for plain stories.
    args: Vec<Arg>,
    /// The story body source, for "show code".
    source: Option<&'static str>,
    /// A Markdown description, for autodocs.
    description: Option<&'static str>,
}

impl Story {
    /// Build a plain story from a name and a closure that produces the view.
    ///
    /// ```
    /// use leptos::prelude::*;
    /// use theoria::Story;
    ///
    /// let story = Story::new("Buttons/Primary", || view! { <button>"Save"</button> });
    /// assert_eq!(story.name(), "Buttons/Primary");
    /// assert!(story.args().is_empty());
    /// assert_eq!(story.source(), None);
    /// ```
    pub fn new<F, V>(name: &'static str, view_fn: F) -> Self
    where
        F: Fn() -> V + Send + Sync + 'static,
        V: IntoView + 'static,
    {
        Self {
            name,
            view: Box::new(move || view_fn().into_any()),
            args: Vec::new(),
            source: None,
            description: None,
        }
    }

    /// Constructor used by the `#[story]` macro — carries args, source, and the
    /// description. Not part of the stable API.
    #[doc(hidden)]
    pub fn __from_macro<F>(
        name: &'static str,
        view_fn: F,
        args: Vec<Arg>,
        source: &'static str,
        description: Option<&'static str>,
    ) -> Self
    where
        F: Fn() -> AnyView + Send + Sync + 'static,
    {
        Self {
            name,
            view: Box::new(view_fn),
            args,
            source: Some(source),
            description,
        }
    }

    /// Give the story a Markdown description, for the gallery's docs panel.
    ///
    /// ```
    /// use theoria::Story;
    ///
    /// let story = Story::new("Version bar/Without the manifest", || "0.13.0")
    ///     .with_description("What the bar shows when the manifest is **missing**.");
    /// assert_eq!(
    ///     story.description(),
    ///     Some("What the bar shows when the manifest is **missing**.")
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// If the story already has a description (a `#[story]` function's doc
    /// comment is one), or if `description` is blank.
    #[must_use]
    pub fn with_description(mut self, description: &'static str) -> Self {
        assert!(
            self.description.is_none(),
            "story {:?} already has a description",
            self.name
        );
        assert!(
            !description.trim().is_empty(),
            "story {:?} has a blank description",
            self.name
        );
        self.description = Some(description);
        self
    }

    /// The story's display name (a `/`-delimited path; see the sidebar tree).
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The editable args ("knobs"), in declaration order.
    #[must_use]
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// The captured body source, if any (for "show code").
    #[must_use]
    pub fn source(&self) -> Option<&'static str> {
        self.source
    }

    /// The Markdown description, if any (for autodocs).
    #[must_use]
    pub fn description(&self) -> Option<&'static str> {
        self.description
    }
}

/// The stories a crate shows in its gallery, as its aggregator returns them.
///
/// The app `theoria serve` generates calls [`Stories::gallery`] on the
/// aggregator's value, so it mounts the `Gallery` of the same theoria the
/// stories were built with, wherever that theoria comes from.
///
/// ```
/// use theoria::{Stories, Story};
///
/// pub fn all_stories() -> Stories {
///     vec![Story::new("Greeting", || "hello")].into()
/// }
/// ```
#[derive(Default)]
pub struct Stories(Vec<Story>);

impl Stories {
    /// The gallery (sidebar, stage, and docs panel) over these stories.
    pub fn gallery(self) -> impl IntoView {
        view! { <Gallery stories=self.0 /> }
    }

    /// Mount the gallery on the page's `<body>` and log any panic to the
    /// browser console, which is all the app `theoria serve` generates does.
    ///
    /// It is behind `csr` so that a stories crate whose `csr` feature does
    /// not enable theoria's fails to build, where it would otherwise run
    /// without the `localStorage` that keeps the selected story.
    #[cfg(feature = "csr")]
    pub fn mount(self) {
        console_error_panic_hook::set_once();
        leptos::mount::mount_to_body(move || self.gallery());
    }
}

impl From<Vec<Story>> for Stories {
    fn from(stories: Vec<Story>) -> Self {
        Self(stories)
    }
}

impl FromIterator<Story> for Stories {
    fn from_iter<I: IntoIterator<Item = Story>>(stories: I) -> Self {
        Self(stories.into_iter().collect())
    }
}

/// So per-module aggregators combine: `a().into_iter().chain(b()).collect()`.
impl IntoIterator for Stories {
    type Item = Story;
    type IntoIter = std::vec::IntoIter<Story>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Extend<Story> for Stories {
    fn extend<I: IntoIterator<Item = Story>>(&mut self, stories: I) {
        self.0.extend(stories);
    }
}

#[cfg(test)]
mod story_tests {
    use super::*;

    #[test]
    #[should_panic(expected = r#"story "A" already has a description"#)]
    fn a_second_description_panics() {
        let _ = Story::new("A", || "a")
            .with_description("first")
            .with_description("second");
    }

    #[test]
    #[should_panic(expected = r#"story "A" has a blank description"#)]
    fn a_blank_description_panics() {
        let _ = Story::new("A", || "a").with_description(" \n");
    }
}

#[cfg(test)]
mod story_macro_tests {
    use super::*;

    /// A demo widget for the macro test.
    #[story(name = "Demo/Widget", active = true, width = 12.0)]
    fn widget(active: bool, width: f64, label: String) -> impl IntoView {
        // Reference each arg so the body genuinely depends on them.
        view! { <div data-active=active data-width=width>{label}</div> }
    }

    #[test]
    #[should_panic(expected = r#"story "Demo/Widget" already has a description"#)]
    fn a_story_with_a_doc_comment_takes_no_second_description() {
        Owner::new().with(|| {
            let _ = widget().with_description("again");
        });
    }

    #[test]
    fn story_macro_captures_name_args_source_and_description() {
        let owner = Owner::new();
        owner.with(|| {
            let s = widget();
            assert_eq!(s.name(), "Demo/Widget", "name override");
            assert_eq!(s.description(), Some("A demo widget for the macro test."));

            let args = s.args();
            assert_eq!(args.len(), 3, "one control per typed param");
            assert_eq!(args[0].name(), "active");
            assert_eq!(args[1].name(), "width");
            assert_eq!(args[2].name(), "label");
            let [active, width, label] = [0, 1, 2].map(|i| args[i].control());
            assert!(matches!(active, ArgControl::Bool(_)), "bool → toggle");
            assert!(matches!(width, ArgControl::Num(_)), "f64 → number");
            assert!(matches!(label, ArgControl::Text(_)), "String → text");

            // Defaults from the attribute (and Default for the unspecified one).
            if let ArgControl::Bool(b) = active {
                assert!(b.get_untracked(), "active default = true");
            }
            if let ArgControl::Num(n) = width {
                assert!(
                    (n.get_untracked() - 12.0).abs() < 1e-9,
                    "width default = 12.0"
                );
            }
            if let ArgControl::Text(t) = label {
                assert!(t.get_untracked().is_empty(), "label default = \"\"");
            }

            assert!(
                s.source().unwrap().contains("<div"),
                "captured body source: {:?}",
                s.source()
            );
        });
    }

    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Each static belongs to one test, so its contents hold when `cargo test`
    // runs the tests on parallel threads in one process.
    static VALUE_BODY_RUNS: AtomicUsize = AtomicUsize::new(0);
    static SIGNAL_BODY_RUNS: AtomicUsize = AtomicUsize::new(0);
    static HANDED_OVER: Mutex<Option<Signal<f64>>> = Mutex::new(None);
    static HANDED_OVER_PER_RUN: Mutex<Vec<Signal<f64>>> = Mutex::new(Vec::new());

    #[story(width = 1.0)]
    fn width_as_value(width: f64) -> impl IntoView {
        VALUE_BODY_RUNS.fetch_add(1, Ordering::SeqCst);
        view! { <p>{width}</p> }
    }

    #[story(width = 1.0)]
    fn width_as_signal(width: Signal<f64>) -> impl IntoView {
        SIGNAL_BODY_RUNS.fetch_add(1, Ordering::SeqCst);
        view! { <p>{move || width.get()}</p> }
    }

    #[story(width = 1.0)]
    fn width_handed_over(width: Signal<f64>) -> impl IntoView {
        *HANDED_OVER.lock().unwrap() = Some(width);
        view! { <p>{move || width.get()}</p> }
    }

    #[story(width = 1.0)]
    fn width_handed_over_per_run(width: Signal<f64>) -> impl IntoView {
        HANDED_OVER_PER_RUN.lock().unwrap().push(width);
        view! { <p>{move || width.get()}</p> }
    }

    #[story(width = "3".parse().unwrap(), on = Default::default())]
    fn inferred_defaults(width: f64, on: bool) -> impl IntoView {
        view! { <p data-on=on>{width}</p> }
    }

    #[story(label = "hi")]
    fn shouts(mut label: String) -> impl IntoView {
        label.push('!');
        view! { <p>{label}</p> }
    }

    fn arg(story: &Story, name: &str) -> Arg {
        *story
            .args()
            .iter()
            .find(|arg| arg.name() == name)
            .unwrap_or_else(|| {
                let names: Vec<_> = story.args().iter().map(Arg::name).collect();
                panic!("no arg named `{name}`; args: {names:?}")
            })
    }

    fn width_control(story: &Story) -> RwSignal<f64> {
        let width = arg(story, "width");
        let ArgControl::Num(signal) = width.control() else {
            panic!("`width: {}` did not get the number control", width.ty());
        };
        signal
    }

    /// Run the story's view in a memo, the way the gallery's stage runs it in
    /// a reactive closure, edit `width`, and read the memo again, which reruns
    /// the view only if the view read the arg.
    fn edit_the_arg_on_a_live_stage(story: Story) {
        let width = width_control(&story);
        let stage = Memo::new(move |_| drop((story.view)()));
        stage.get();
        width.set(2.0);
        stage.get();
    }

    #[test]
    fn an_edit_to_a_value_arg_reruns_the_story_body() {
        Owner::new().with(|| edit_the_arg_on_a_live_stage(width_as_value()));
        assert_eq!(VALUE_BODY_RUNS.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn an_edit_to_a_signal_arg_leaves_the_story_body_alone() {
        Owner::new().with(|| edit_the_arg_on_a_live_stage(width_as_signal()));
        assert_eq!(SIGNAL_BODY_RUNS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_signal_arg_is_the_signal_its_control_writes() {
        Owner::new().with(|| {
            let story = width_handed_over();
            let width = width_control(&story);
            drop((story.view)());
            width.set(2.0);
            let handed_over = HANDED_OVER.lock().unwrap().expect("the body ran");
            assert_eq!(handed_over.get_untracked(), 2.0);
        });
    }

    // A signal made on each run would be disposed with the run that made it,
    // under a copy the body had handed to a timer or a task.
    #[test]
    fn a_signal_arg_is_the_same_signal_on_every_run() {
        Owner::new().with(|| {
            let story = width_handed_over_per_run();
            drop((story.view)());
            drop((story.view)());
            let runs = HANDED_OVER_PER_RUN.lock().unwrap();
            assert_eq!(runs.len(), 2, "the body ran twice");
            assert!(runs[0] == runs[1], "each run got its own signal");
        });
    }

    #[story(radius = Bounded::new(0.25).min(0.0).max(1.0))]
    fn bounded_radius(radius: Bounded) -> impl IntoView {
        view! { <p>{radius.value()}</p> }
    }

    #[story(ink = "#243832")]
    fn ink(ink: Color) -> impl IntoView {
        view! { <p>{ink.as_str().to_string()}</p> }
    }

    #[story(build = Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.13.0"))]
    fn build(build: Choice<&'static str>) -> impl IntoView {
        view! { <p>{build.value()}</p> }
    }

    fn choice_control(story: &Story, name: &str) -> ChoiceControl {
        let arg = arg(story, name);
        let ArgControl::Choice(control) = arg.control() else {
            panic!("`{name}: {}` did not get the choice control", arg.ty());
        };
        control
    }

    #[test]
    fn a_choice_arg_starts_at_the_option_its_attribute_selects() {
        Owner::new().with(|| {
            let html = (build().view)().to_html();
            assert!(html.contains(">0.13.0<"), "got: {html}");
        });
    }

    #[test]
    fn selecting_an_option_gives_the_story_its_value() {
        Owner::new().with(|| {
            let story = build();
            choice_control(&story, "build").select(2);
            let html = (story.view)().to_html();
            assert!(html.contains(">0.14.0<"), "got: {html}");
        });
    }

    #[test]
    fn selecting_an_option_moves_the_controls_selection() {
        Owner::new().with(|| {
            let control = choice_control(&build(), "build");
            control.select(2);
            assert_eq!(control.selected(), 2);
        });
    }

    #[test]
    fn selecting_past_the_last_option_keeps_the_storys_value() {
        Owner::new().with(|| {
            let story = build();
            choice_control(&story, "build").select(3);
            let html = (story.view)().to_html();
            assert!(html.contains(">0.13.0<"), "got: {html}");
        });
    }

    static CHOICE_BODY_RUNS: AtomicUsize = AtomicUsize::new(0);

    #[story(build = Choice::new(["dev", "0.13.0"]))]
    fn build_counted(build: Choice<&'static str>) -> impl IntoView {
        CHOICE_BODY_RUNS.fetch_add(1, Ordering::SeqCst);
        view! { <p>{build.value()}</p> }
    }

    #[test]
    fn selecting_another_option_reruns_the_story_body() {
        Owner::new().with(|| {
            let story = build_counted();
            let control = choice_control(&story, "build");
            let stage = Memo::new(move |_| drop((story.view)()));
            stage.get();
            control.select(1);
            stage.get();
        });
        assert_eq!(CHOICE_BODY_RUNS.load(Ordering::SeqCst), 2);
    }

    static SAME_CHOICE_BODY_RUNS: AtomicUsize = AtomicUsize::new(0);

    #[story(build = Choice::new(["dev", "0.13.0"]))]
    fn build_counted_again(build: Choice<&'static str>) -> impl IntoView {
        SAME_CHOICE_BODY_RUNS.fetch_add(1, Ordering::SeqCst);
        view! { <p>{build.value()}</p> }
    }

    #[test]
    fn selecting_the_selected_option_leaves_the_story_body_alone() {
        Owner::new().with(|| {
            let story = build_counted_again();
            let control = choice_control(&story, "build");
            let stage = Memo::new(move |_| drop((story.view)()));
            stage.get();
            control.select(0);
            stage.get();
        });
        assert_eq!(SAME_CHOICE_BODY_RUNS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_choice_control_lists_its_options_labels() {
        Owner::new().with(|| {
            let control = choice_control(&build(), "build");
            assert_eq!(control.labels(), ["dev", "0.13.0", "0.14.0"]);
        });
    }

    // What makes a `Choice` arg with no list in the attribute a compile
    // error: the macro falls back on `Default`.
    #[test]
    fn a_choice_has_no_default() {
        trait NoDefault<A> {
            fn check() {}
        }
        impl<T: ?Sized> NoDefault<()> for T {}
        #[allow(dead_code)]
        struct HasDefault;
        impl<T: Default> NoDefault<HasDefault> for T {}
        <Choice<&'static str> as NoDefault<_>>::check();
    }

    #[test]
    fn a_color_arg_gets_the_color_control_at_its_default() {
        Owner::new().with(|| {
            let ink = arg(&ink(), "ink");
            let ArgControl::Color(signal) = ink.control() else {
                panic!("`ink: {}` did not get the color control", ink.ty());
            };
            assert_eq!(signal.get_untracked().as_str(), "#243832");
        });
    }

    #[test]
    fn a_bounded_arg_gets_the_bounded_control_at_its_default() {
        Owner::new().with(|| {
            let radius = arg(&bounded_radius(), "radius");
            let ArgControl::Bounded(signal) = radius.control() else {
                panic!("`radius: {}` did not get the bounded control", radius.ty());
            };
            assert_eq!(signal.get_untracked(), Bounded::new(0.25).min(0.0).max(1.0));
        });
    }

    #[test]
    fn an_arg_keeps_its_type_as_the_story_wrote_it() {
        Owner::new().with(|| {
            assert_eq!(arg(&width_as_value(), "width").ty(), "f64");
            assert_eq!(arg(&width_as_signal(), "width").ty(), "Signal<f64>");
        });
    }

    #[test]
    fn a_default_takes_its_type_from_the_arg() {
        Owner::new().with(|| {
            let story = inferred_defaults();
            assert_eq!(width_control(&story).get_untracked(), 3.0);
        });
    }

    #[test]
    fn a_mut_parameter_is_mutable_in_the_body() {
        let html = leptotes::render(|| (shouts().view)());
        assert!(html.contains("hi!"), "got:\n{html}");
    }

    /// Compiles only if the type has no `StoryArg` impl: with one, both impls
    /// of `NotAnArg` apply to it and the `_` is ambiguous.
    macro_rules! assert_not_a_story_arg {
        ($($ty:ty),+ $(,)?) => {$({
            trait NotAnArg<A> {
                fn check() {}
            }
            impl<T: ?Sized> NotAnArg<()> for T {}
            #[allow(dead_code)]
            struct IsAnArg;
            impl<T: StoryArg> NotAnArg<IsAnArg> for T {}
            <$ty as NotAnArg<_>>::check();
        })+};
    }

    #[test]
    fn only_plain_values_and_signals_of_them_are_story_args() {
        assert_not_a_story_arg!(
            i32,
            &'static str,
            RwSignal<bool>,
            Signal<i32>,
            Signal<Signal<f64>>,
        );
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::Bounded;

    fn unit() -> Bounded {
        Bounded::new(0.5).min(0.0).max(1.0)
    }

    #[test]
    fn a_value_below_the_minimum_clamps_to_it() {
        assert_eq!(unit().clamp(-2.0), 0.0);
    }

    #[test]
    fn a_value_above_the_maximum_clamps_to_it() {
        assert_eq!(unit().clamp(7.0), 1.0);
    }

    #[test]
    fn a_value_in_bounds_is_kept() {
        assert_eq!(unit().clamp(0.25), 0.25);
    }

    #[test]
    fn without_a_maximum_a_large_value_is_kept() {
        assert_eq!(Bounded::new(0.0).min(0.0).clamp(1e9), 1e9);
    }

    #[test]
    fn without_a_minimum_a_small_value_is_kept() {
        assert_eq!(Bounded::new(0.0).max(1.0).clamp(-1e9), -1e9);
    }

    #[test]
    fn a_starting_value_outside_the_bounds_is_clamped() {
        assert_eq!(Bounded::new(5.0).min(0.0).max(1.0).value(), 1.0);
    }

    #[test]
    #[should_panic(expected = "Bounded min 1 is above its max 0")]
    fn a_minimum_above_the_maximum_panics() {
        let _ = Bounded::new(0.5).min(1.0).max(0.0);
    }

    #[test]
    #[should_panic(expected = "Bounded max inf is not finite")]
    fn an_infinite_bound_panics() {
        let _ = Bounded::new(0.5).max(f64::INFINITY);
    }

    #[test]
    #[should_panic(expected = "Bounded min NaN is not finite")]
    fn a_nan_bound_panics() {
        let _ = Bounded::new(0.5).min(f64::NAN);
    }

    #[test]
    #[should_panic(expected = "Bounded step 0 is not above zero")]
    fn a_zero_step_panics() {
        let _ = Bounded::new(0.5).step(0.0);
    }

    #[test]
    #[should_panic(expected = "Bounded step NaN is not above zero")]
    fn a_nan_step_panics() {
        let _ = Bounded::new(0.5).step(f64::NAN);
    }

    fn quarters() -> Bounded {
        Bounded::new(0.0).min(0.0).max(1.0).step(0.25)
    }

    #[test]
    fn a_value_between_steps_snaps_to_the_nearest() {
        assert_eq!(quarters().with_value(0.3).value(), 0.25);
    }

    #[test]
    fn a_starting_value_between_steps_snaps_to_the_nearest() {
        assert_eq!(Bounded::new(0.3).min(0.0).max(1.0).step(0.25).value(), 0.25);
    }

    #[test]
    fn steps_count_from_the_minimum() {
        let from_a_tenth = Bounded::new(0.1).min(0.1).max(2.0).step(0.25);
        assert_eq!(from_a_tenth.with_value(0.3).value(), 0.35);
    }

    // 3 * 0.1 is 0.30000000000000004 in an f64.
    #[test]
    fn a_decimal_step_snaps_to_the_decimal_it_names() {
        let tenths = Bounded::new(0.0).min(0.0).max(1.0).step(0.1);
        assert_eq!(tenths.with_value(0.34).value(), 0.3);
    }

    #[test]
    fn a_maximum_between_steps_snaps_down_to_the_last_step() {
        let two_fifths = Bounded::new(0.0).min(0.0).max(1.0).step(0.4);
        assert_eq!(two_fifths.with_value(1.0).value(), 0.8);
    }

    #[test]
    fn without_a_step_a_value_between_the_bounds_is_kept() {
        assert_eq!(unit().with_value(0.3).value(), 0.3);
    }
}

#[cfg(test)]
mod choice_tests {
    use super::Choice;

    #[test]
    fn the_first_option_starts_selected() {
        assert_eq!(Choice::new(["dev", "0.13.0"]).value(), "dev");
    }

    #[test]
    fn a_labelled_option_gives_its_value_not_its_label() {
        let font = Choice::labelled([("Serif", "Georgia, serif")]);
        assert_eq!(font.value(), "Georgia, serif");
    }

    #[test]
    fn selected_starts_at_the_option_with_that_label() {
        let font = Choice::labelled([("Serif", "Georgia, serif"), ("Sans", "system-ui")]);
        assert_eq!(font.selected("Sans").value(), "system-ui");
    }

    #[test]
    fn selected_starts_at_the_first_option_with_that_label() {
        let twins = Choice::labelled([("A", 1), ("A", 2)]);
        assert_eq!(twins.selected("A").value(), 1);
    }

    #[test]
    #[should_panic(expected = "Choice has no option labelled \"1.0.0\"")]
    fn selected_with_a_label_no_option_has_panics() {
        let _ = Choice::new(["dev", "0.13.0"]).selected("1.0.0");
    }

    #[test]
    #[should_panic(expected = "a Choice needs at least one option")]
    fn a_choice_with_no_options_panics() {
        let _ = Choice::new(Vec::<&str>::new());
    }
}
