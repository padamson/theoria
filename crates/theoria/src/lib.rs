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
/// `#[derive(StoryArg)]`, which makes a struct a `#[story]` arg (see
/// [`StoryArg`](trait@StoryArg)).
pub use theoria_macros::StoryArg;
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
    Group(GroupControl),
}

/// The args of a group, such as one per field of a struct that derives
/// [`StoryArg`](trait@StoryArg).
#[derive(Clone, Copy)]
pub struct GroupControl {
    args: StoredValue<Vec<Arg>>,
}

impl GroupControl {
    /// The group's args, in order.
    #[must_use]
    pub fn args(&self) -> Vec<Arg> {
        self.args.get_value()
    }

    /// The arg called `name`, if the group has one.
    #[must_use]
    pub fn arg(&self, name: &str) -> Option<Arg> {
        self.args.with_value(|args| find_arg(args, name))
    }
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

    /// Whether moving to `value` changed the arg, so an edit that lands where
    /// it already is does not run the story again.
    fn set(&mut self, value: f64) -> bool {
        let next = self.with_value(value);
        let changed = next != *self;
        *self = next;
        changed
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

/// `description`, for the `kind` called `name`, which must have none yet.
fn described(
    kind: &str,
    name: &str,
    current: Option<&'static str>,
    description: &'static str,
) -> &'static str {
    assert!(
        current.is_none(),
        "{kind} {name:?} already has a description"
    );
    assert!(
        !description.trim().is_empty(),
        "{kind} {name:?} has a blank description"
    );
    description
}

/// The arg called `name` among `args`.
fn find_arg(args: &[Arg], name: &str) -> Option<Arg> {
    args.iter().find(|arg| arg.name == name).copied()
}

/// One editable arg of a story: its name, its type as the story wrote it,
/// and its control.
#[derive(Clone, Copy)]
pub struct Arg {
    name: &'static str,
    ty: &'static str,
    control: ArgControl,
    reset: Callback<()>,
    /// `None` for a group, whose args each have their own.
    resets: Option<Trigger>,
    description: Option<&'static str>,
}

impl Arg {
    /// An arg over `signal`, with the control its type picks. The value the
    /// signal holds now is the arg's default, which [`Arg::reset`] puts back.
    ///
    /// ```
    /// use leptos::prelude::*;
    /// use theoria::{Arg, ArgControl};
    ///
    /// # Owner::new().with(|| {
    /// let width = RwSignal::new(12.0);
    /// let arg = Arg::new("width", "f64", width);
    /// assert!(matches!(arg.control(), ArgControl::Num(_)));
    /// # });
    /// ```
    #[must_use]
    pub fn new<V: __private::Leaf>(
        name: &'static str,
        ty: &'static str,
        signal: RwSignal<V>,
    ) -> Self {
        let start = signal.get_untracked();
        let resets = Trigger::new();
        let reset = Callback::new(move |()| {
            signal.maybe_update(|value| {
                let edited = !value.same(&start);
                if edited {
                    *value = start.clone();
                }
                edited
            });
            resets.notify();
        });
        Self {
            name,
            ty,
            control: V::control(signal),
            reset,
            resets: Some(resets),
            description: None,
        }
    }

    /// An arg over `args`, drawn as their controls under its name, whose
    /// reset puts each of them back.
    ///
    /// ```
    /// use leptos::prelude::*;
    /// use theoria::{Arg, ArgControl};
    ///
    /// # Owner::new().with(|| {
    /// let size = Arg::group("size", "Size", vec![
    ///     Arg::new("width", "f64", RwSignal::new(12.0)),
    ///     Arg::new("depth", "f64", RwSignal::new(30.0)),
    /// ]);
    /// let ArgControl::Group(fields) = size.control() else { unreachable!() };
    /// let names: Vec<_> = fields.args().iter().map(Arg::name).collect();
    /// assert_eq!(names, ["width", "depth"]);
    /// # });
    /// ```
    ///
    /// # Panics
    ///
    /// If `args` is empty, or two of them share a name.
    #[must_use]
    pub fn group(name: &'static str, ty: &'static str, args: Vec<Arg>) -> Self {
        assert!(!args.is_empty(), "arg group {name:?} has no args");
        for (i, arg) in args.iter().enumerate() {
            assert!(
                args[..i].iter().all(|earlier| earlier.name != arg.name),
                "arg group {name:?} has two args named {:?}",
                arg.name
            );
        }
        let args = StoredValue::new(args);
        let reset = Callback::new(move |()| {
            args.with_value(|args| args.iter().for_each(Arg::reset));
        });
        Self {
            name,
            ty,
            control: ArgControl::Group(GroupControl { args }),
            reset,
            resets: None,
            description: None,
        }
    }

    /// Give the arg a Markdown description, shown with its control.
    ///
    /// # Panics
    ///
    /// If the arg already has a description, or if `description` is blank.
    #[must_use]
    pub fn with_description(mut self, description: &'static str) -> Self {
        self.description = Some(described("arg", self.name, self.description, description));
        self
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

    /// The Markdown description, if any: a field's doc comment, for an arg
    /// that `#[derive(StoryArg)]` made.
    #[must_use]
    pub fn description(&self) -> Option<&'static str> {
        self.description
    }

    /// Put the arg back to its default.
    pub fn reset(&self) {
        self.reset.run(());
    }

    /// Track the arg's resets, edited or not, for a control that is not
    /// bound to the arg's signal.
    pub(crate) fn track_reset(&self) {
        if let Some(resets) = self.resets {
            resets.track();
        }
        if let ArgControl::Group(group) = self.control {
            group
                .args
                .with_value(|args| args.iter().for_each(Arg::track_reset));
        }
    }
}

/// What `#[story]` and `#[derive(StoryArg)]` expand to. Not part of the
/// stable API.
#[doc(hidden)]
pub mod __private {
    use std::collections::HashSet;
    use std::sync::{LazyLock, Mutex, PoisonError};

    pub use leptos::prelude::Signal;
    use leptos::prelude::*;

    use super::{Arg, ArgControl, Color};

    pub trait Sealed {}

    /// A story arg that is its own value, which is what a `Signal` arg holds.
    /// The supertraits repeat `Value`'s bounds, which rustc does not carry
    /// over through `Value = Self`.
    #[diagnostic::on_unimplemented(
        message = "`Signal<{Self}>` cannot be a #[story] arg",
        label = "no control for a signal of this type",
        note = "a Signal arg holds a bool, f64, String, Bounded, Color, or Choice, or a struct that derives StoryArg"
    )]
    pub trait Plain: super::StoryArg<Value = Self> + Clone + Send + Sync + 'static {
        /// The arg's value, read from its controls, and the arg.
        fn arg(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg);
    }

    /// A value one control edits.
    pub trait Leaf: Clone + Send + Sync + 'static {
        fn control(signal: RwSignal<Self>) -> ArgControl;

        /// Whether the control would show `self` and `other` the same.
        fn same(&self, other: &Self) -> bool;
    }

    /// The arg over one signal and its control.
    pub fn leaf<T: Leaf>(name: &'static str, ty: &'static str, start: T) -> (Signal<T>, Arg) {
        let signal = RwSignal::new(start);
        (signal.into(), Arg::new(name, ty, signal))
    }

    /// A type a field of a struct that derives `StoryArg` can have.
    #[diagnostic::on_unimplemented(
        message = "a field of type `{Self}` has no control",
        label = "no control for this field's type",
        note = "a StoryArg field is a bool, f64, String, &'static str, Bounded, Color, or Choice; `#[story_arg(skip)]` keeps its value"
    )]
    pub trait Field: Sized + Send + Sync + 'static {
        fn field(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg);
    }

    impl<T: Leaf> Field for T {
        fn field(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg) {
            leaf(name, ty, start)
        }
    }

    impl Field for &'static str {
        fn field(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg) {
            let text = RwSignal::new(start.to_string());
            (interned(text, String::as_str), Arg::new(name, ty, text))
        }
    }

    /// A field `#[story_arg(color)]` draws as a color.
    #[diagnostic::on_unimplemented(
        message = "`#[story_arg(color)]` is for a `&'static str` field, not `{Self}`",
        label = "not a `&'static str`",
        note = "a `Color` field is drawn as a color without the option"
    )]
    pub trait ColorField: Sized + Send + Sync + 'static {
        fn color_field(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg);
    }

    impl ColorField for &'static str {
        fn color_field(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg) {
            let color = RwSignal::new(Color::from(start));
            (interned(color, Color::as_str), Arg::new(name, ty, color))
        }
    }

    /// The text `signal` holds, interned once per edit.
    fn interned<L: Send + Sync + 'static>(
        signal: RwSignal<L>,
        text: fn(&L) -> &str,
    ) -> Signal<&'static str> {
        Memo::new(move |_| signal.with(|value| intern(text(value)))).into()
    }

    /// The struct's value, read from its fields' signals, and the group of
    /// their args.
    pub fn group<T: Send + Sync + 'static>(
        name: &'static str,
        ty: &'static str,
        args: Vec<Arg>,
        value: impl Fn() -> T + Send + Sync + 'static,
    ) -> (Signal<T>, Arg) {
        (Signal::derive(value), Arg::group(name, ty, args))
    }

    pub fn read<T: Clone + Send + Sync + 'static>(signal: Signal<T>) -> T {
        signal.get()
    }

    /// `text`, kept for good the first time it is seen, so a field edited
    /// back and forth leaks each text once.
    pub fn intern(text: &str) -> &'static str {
        static TEXTS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);
        let mut texts = TEXTS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(kept) = texts.get(text) {
            return kept;
        }
        let kept: &'static str = Box::leak(text.into());
        texts.insert(kept);
        kept
    }
}

/// A type a `#[story]` parameter can have: `bool`, `f64`, `String`,
/// [`Bounded`], [`Color`], [`Choice`], a struct that derives `StoryArg`, or
/// a `Signal` of one, which hands the body the arg's signal, not its value.
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
/// A struct that derives `StoryArg` (and `Clone`) is one arg with a control
/// per field, picked by the field's type as a parameter's is, with
/// `&'static str` a text box. Each field's doc comment is shown with its
/// control. `#[story_arg(color)]` makes a `&'static str` field a color, and
/// on the struct makes every field written `&'static str` one. An edited
/// `&'static str` is leaked once per distinct text.
///
/// ```
/// use leptos::prelude::*;
/// use theoria::{StoryArg, story};
///
/// #[derive(Clone, StoryArg)]
/// struct Theme {
///     /// The page's background.
///     #[story_arg(color)]
///     paper: &'static str,
///     font: &'static str,
///     #[story_arg(skip)]
///     version: u32,
/// }
///
/// const THEME: Theme = Theme { paper: "#f6f1e7", font: "Georgia, serif", version: 2 };
///
/// #[story(theme = THEME)]
/// fn page(theme: Signal<Theme>) -> impl IntoView {
///     view! { <main style:background=move || theme.get().paper>"Hello"</main> }
/// }
/// ```
///
/// `#[story_arg(skip)]` keeps a field at its default with no control; a
/// field of any other type without it is a compile error at the field:
///
/// ```compile_fail,E0277
/// use theoria::StoryArg;
///
/// #[derive(Clone, StoryArg)]
/// struct Theme {
///     paper: &'static str,
///     version: u32,
/// }
/// ```
///
/// and so is `#[story_arg(color)]` on a field that is not a `&'static str`:
///
/// ```compile_fail,E0277
/// use theoria::StoryArg;
///
/// #[derive(Clone, StoryArg)]
/// struct Theme {
///     #[story_arg(color)]
///     paper: String,
/// }
/// ```
///
/// Implement it outside theoria with the derive. The traits the derive
/// implements are hidden and not stable API, and the trait's method is the
/// `#[story]` macro's to call.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a #[story] arg",
    label = "no control for this type",
    note = "a #[story] arg is a bool, f64, String, Bounded, Color, or Choice, a struct that derives StoryArg, or a `Signal` of one (not an `RwSignal`)"
)]
pub trait StoryArg: Sized + __private::Sealed {
    /// What the arg's control edits.
    type Value: Clone + Send + Sync + 'static;

    /// What a default in the story attribute is written as.
    type Init: Into<Self::Value>;

    #[doc(hidden)]
    fn bind(signal: Signal<Self::Value>) -> Self;
}

macro_rules! value_arg {
    ($ty:ty, init: $init:ty, control: $variant:ident) => {
        impl __private::Sealed for $ty {}

        impl __private::Leaf for $ty {
            fn control(signal: RwSignal<$ty>) -> ArgControl {
                ArgControl::$variant(signal)
            }

            #[allow(
                clippy::float_cmp,
                reason = "a reset asks whether the arg still holds its exact default"
            )]
            fn same(&self, other: &Self) -> bool {
                self == other
            }
        }

        impl __private::Plain for $ty {
            fn arg(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg) {
                __private::leaf(name, ty, start)
            }
        }

        impl StoryArg for $ty {
            type Value = $ty;
            type Init = $init;

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

impl<T> __private::Sealed for Choice<T> {}
impl<T: Clone + Send + Sync + 'static> __private::Leaf for Choice<T> {
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

    /// The options never change, so the selection is all there is to show.
    fn same(&self, other: &Self) -> bool {
        self.selected == other.selected
    }
}

impl<T: Clone + Send + Sync + 'static> __private::Plain for Choice<T> {
    fn arg(name: &'static str, ty: &'static str, start: Self) -> (Signal<Self>, Arg) {
        __private::leaf(name, ty, start)
    }
}

impl<T: Clone + Send + Sync + 'static> StoryArg for Choice<T> {
    type Value = Self;
    type Init = Self;

    fn bind(signal: Signal<Self>) -> Self {
        signal.get()
    }
}

impl<T: __private::Plain> __private::Sealed for Signal<T> {}

impl<T: __private::Plain> StoryArg for Signal<T> {
    type Value = T;
    type Init = T::Init;

    fn bind(signal: Signal<T>) -> Self {
        signal
    }
}

/// One named preview in the gallery.
pub struct Story {
    name: &'static str,
    // `Send + Sync`: Leptos 0.8's reactive closures (the stage re-renders when
    // the selection changes) require their captures to be thread-safe.
    body: Box<dyn Fn() -> AnyView + Send + Sync>,
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
            body: Box::new(move || view_fn().into_any()),
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
            body: Box::new(view_fn),
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
        self.description = Some(described("story", self.name, self.description, description));
        self
    }

    /// The story's display name (a `/`-delimited path; see the sidebar tree).
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The story's body, drawn with its args' current values. Call it inside
    /// a reactive `Owner`, such as the one a leptotes `dom` closure runs
    /// in. A test can set an arg and draw the story, to check what the arg
    /// feeds:
    ///
    /// ```
    /// use leptos::prelude::*;
    /// use theoria::{ArgControl, Color, story};
    ///
    /// #[story(ink = "#243832")]
    /// fn swatch(ink: Signal<Color>) -> impl IntoView {
    ///     view! { <p style:color=move || ink.get().to_string()>"Ink"</p> }
    /// }
    ///
    /// let dom = leptotes::dom(|| {
    ///     let story = swatch();
    ///     let ArgControl::Color(ink) = story.arg("ink").unwrap().control() else {
    ///         panic!("`ink` did not get the color control");
    ///     };
    ///     ink.set(Color::from("rebeccapurple"));
    ///     story.view()
    /// });
    /// assert_eq!(dom.find("p").attr("style"), Some("color:rebeccapurple;"));
    /// ```
    #[must_use]
    pub fn view(&self) -> AnyView {
        (self.body)()
    }

    /// The editable args ("knobs"), in declaration order.
    #[must_use]
    pub fn args(&self) -> &[Arg] {
        &self.args
    }

    /// The arg called `name`, if the story has one.
    #[must_use]
    pub fn arg(&self, name: &str) -> Option<Arg> {
        find_arg(&self.args, name)
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
    /// without the URL and `localStorage` code that keep the selected story.
    #[cfg(feature = "csr")]
    pub fn mount(self) {
        // Off wasm the JavaScript calls in the hook would panic inside it,
        // aborting with no message; the default hook prints to stderr there.
        #[cfg(target_arch = "wasm32")]
        std::panic::set_hook(Box::new(log_to_console));
        leptos::mount::mount_to_body(move || self.gallery());
    }
}

/// Logs a panic to the browser console with the JavaScript stack that reached
/// it.
#[cfg(all(feature = "csr", target_arch = "wasm32"))]
fn log_to_console(info: &std::panic::PanicHookInfo) {
    let stack = stack_with_frames(100);
    // The trailing blank lines keep Safari's devtools from rewriting the
    // stack's Rust frames (console_error_panic_hook's issue #7).
    web_sys::console::error_1(&format!("{info}\n\nStack:\n\n{stack}\n\n").into());
}

/// The JavaScript stack here, keeping up to `frames` frames where the engine
/// limits them (V8 keeps 10 by default, which the panic machinery fills before
/// the story's own code).
///
/// The limit is read and written through `Reflect`, so an engine without it
/// is left alone and a frozen `Error` is not an exception.
#[cfg(all(feature = "csr", target_arch = "wasm32"))]
fn stack_with_frames(frames: u32) -> String {
    use js_sys::Reflect;
    let error_type = js_sys::Error::new("").constructor();
    let key = js_sys::JsString::from("stackTraceLimit");
    let limit = Reflect::get(&error_type, &key)
        .ok()
        .filter(|limit| limit.as_f64().is_some());
    if limit.is_some() {
        Reflect::set(&error_type, &key, &frames.into()).ok();
    }
    let error = js_sys::Error::new("");
    if let Some(limit) = &limit {
        Reflect::set(&error_type, &key, limit).ok();
    }
    // `js_sys::Error` has no `stack` getter.
    Reflect::get(&error, &"stack".into())
        .ok()
        .and_then(|stack| stack.as_string())
        .unwrap_or_default()
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

    #[test]
    fn view_draws_the_story_body() {
        let dom = leptotes::dom(|| Story::new("A", || view! { <p>"hi"</p> }).view());
        assert_eq!(dom.find("p").text(), "hi", "{dom}");
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
    fn a_name_in_the_attribute_is_the_storys_name() {
        Owner::new().with(|| assert_eq!(widget().name(), "Demo/Widget"));
    }

    #[test]
    fn the_doc_comment_is_the_storys_description() {
        Owner::new().with(|| {
            assert_eq!(
                widget().description(),
                Some("A demo widget for the macro test.")
            );
        });
    }

    #[test]
    fn each_parameter_is_an_arg_named_for_it_in_order() {
        Owner::new().with(|| {
            let story = widget();
            let names: Vec<_> = story.args().iter().map(Arg::name).collect();
            assert_eq!(names, ["active", "width", "label"]);
        });
    }

    #[test]
    fn each_parameter_gets_the_control_its_type_picks() {
        Owner::new().with(|| {
            let story = widget();
            let control = |name| arg(&story, name).control();
            assert!(matches!(control("active"), ArgControl::Bool(_)), "bool");
            assert!(matches!(control("width"), ArgControl::Num(_)), "f64");
            assert!(matches!(control("label"), ArgControl::Text(_)), "String");
        });
    }

    #[test]
    fn a_default_in_the_attribute_is_the_args_starting_value() {
        Owner::new().with(|| {
            let story = widget();
            let ArgControl::Bool(active) = arg(&story, "active").control() else {
                panic!("`active: bool` did not get the toggle");
            };
            assert!(active.get_untracked(), "`active` started off");
            assert_eq!(width_control(&story).get_untracked(), 12.0);
        });
    }

    #[test]
    fn an_arg_without_a_default_starts_at_its_types_default() {
        Owner::new().with(|| {
            let ArgControl::Text(label) = arg(&widget(), "label").control() else {
                panic!("`label: String` did not get the text control");
            };
            assert_eq!(label.get_untracked(), "");
        });
    }

    #[test]
    fn a_storys_source_is_its_body_as_written() {
        Owner::new().with(|| {
            assert_eq!(
                bounded_radius().source(),
                Some("view! { <p>{radius.value()}</p> }")
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
        story.arg(name).unwrap_or_else(|| {
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
        let stage = Memo::new(move |_| drop(story.view()));
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
            drop(story.view());
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
            drop(story.view());
            drop(story.view());
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

    /// What the `build` story draws, after its control selects `pick`.
    fn build_drawn_after(pick: Option<usize>) -> String {
        leptotes::dom(|| {
            let story = build();
            if let Some(pick) = pick {
                choice_control(&story, "build").select(pick);
            }
            story.view()
        })
        .find("p")
        .text()
    }

    #[test]
    fn a_choice_arg_starts_at_the_option_its_attribute_selects() {
        assert_eq!(build_drawn_after(None), "0.13.0");
    }

    #[test]
    fn selecting_an_option_gives_the_story_its_value() {
        assert_eq!(build_drawn_after(Some(2)), "0.14.0");
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
        assert_eq!(build_drawn_after(Some(3)), "0.13.0");
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
            let stage = Memo::new(move |_| drop(story.view()));
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
            let stage = Memo::new(move |_| drop(story.view()));
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
    fn arg_finds_an_arg_by_its_name() {
        Owner::new().with(|| {
            assert_eq!(widget().arg("width").map(|arg| arg.ty()), Some("f64"));
        });
    }

    #[story(r#type = "button")]
    fn raw_named(r#type: String) -> impl IntoView {
        view! { <p>{r#type}</p> }
    }

    #[test]
    fn an_arg_written_as_a_raw_identifier_is_found_by_its_plain_name() {
        Owner::new().with(|| {
            assert_eq!(raw_named().arg("type").map(|arg| arg.ty()), Some("String"));
        });
    }

    #[test]
    fn a_default_keyed_by_a_raw_identifier_is_the_args_starting_value() {
        Owner::new().with(|| {
            let ArgControl::Text(text) = arg(&raw_named(), "type").control() else {
                panic!("`r#type: String` did not get the text control");
            };
            assert_eq!(text.get_untracked(), "button");
        });
    }

    #[test]
    fn without_a_name_in_the_attribute_a_story_is_named_for_its_fn() {
        Owner::new().with(|| assert_eq!(inferred_defaults().name(), "inferred_defaults"));
    }

    #[test]
    fn a_story_without_a_doc_comment_has_no_description() {
        Owner::new().with(|| assert_eq!(inferred_defaults().description(), None));
    }

    #[test]
    fn a_story_arg_resets_to_the_default_in_its_attribute() {
        Owner::new().with(|| {
            let story = widget();
            let width = story.arg("width").expect("`widget` has a `width` arg");
            let ArgControl::Num(s) = width.control() else {
                panic!("`width` did not get the number control");
            };
            s.set(40.0);
            width.reset();
            assert_eq!(s.get_untracked(), 12.0);
        });
    }

    #[test]
    fn arg_is_none_for_a_name_the_story_lacks() {
        Owner::new().with(|| {
            assert_eq!(widget().arg("height").map(|arg| arg.ty()), None);
        });
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
        let dom = leptotes::dom(|| shouts().view());
        assert_eq!(dom.find("p").text(), "hi!", "{dom}");
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
mod arg_tests {
    use super::*;

    #[test]
    fn reset_puts_a_number_back() {
        Owner::new().with(|| {
            let width = RwSignal::new(12.0);
            let arg = Arg::new("width", "f64", width);
            width.set(3.5);
            arg.reset();
            assert_eq!(width.get_untracked(), 12.0);
        });
    }

    /// How many times a panel that tracks `tracked`'s resets draws, once
    /// before `act` and once after.
    fn draws(tracked: Arg, act: impl FnOnce()) -> usize {
        let runs = StoredValue::new(0);
        let panel = Memo::new(move |_| {
            tracked.track_reset();
            runs.update_value(|n| *n += 1);
        });
        panel.get();
        act();
        panel.get();
        runs.get_value()
    }

    fn width() -> Arg {
        Arg::new("width", "f64", RwSignal::new(12.0))
    }

    #[test]
    fn a_reset_redraws_what_tracks_the_arg() {
        Owner::new().with(|| {
            let arg = width();
            assert_eq!(draws(arg, || arg.reset()), 2);
        });
    }

    #[test]
    fn a_group_reset_redraws_what_tracks_the_group() {
        Owner::new().with(|| {
            let group = Arg::group("size", "Size", vec![width()]);
            assert_eq!(draws(group, || group.reset()), 2);
        });
    }

    #[test]
    fn a_reset_of_one_arg_in_a_group_redraws_what_tracks_the_group() {
        Owner::new().with(|| {
            let width = width();
            let group = Arg::group("size", "Size", vec![width]);
            assert_eq!(draws(group, || width.reset()), 2);
        });
    }

    #[test]
    #[should_panic(expected = r#"arg group "size" has no args"#)]
    fn a_group_with_no_args_panics() {
        Owner::new().with(|| {
            let _ = Arg::group("size", "Size", Vec::new());
        });
    }

    #[test]
    #[should_panic(expected = r#"arg group "size" has two args named "width""#)]
    fn a_group_with_two_args_of_one_name_panics() {
        Owner::new().with(|| {
            let _ = Arg::group("size", "Size", vec![width(), width()]);
        });
    }

    #[test]
    fn group_arg_is_none_for_a_name_the_group_lacks() {
        Owner::new().with(|| {
            let ArgControl::Group(fields) = Arg::group("size", "Size", vec![width()]).control()
            else {
                panic!("a group did not get the group control");
            };
            assert_eq!(fields.arg("depth").map(|arg| arg.name()), None);
        });
    }

    #[test]
    #[should_panic(expected = r#"arg "width" already has a description"#)]
    fn a_second_arg_description_panics() {
        Owner::new().with(|| {
            let _ = width().with_description("first").with_description("second");
        });
    }

    #[test]
    #[should_panic(expected = r#"arg "width" has a blank description"#)]
    fn a_blank_arg_description_panics() {
        Owner::new().with(|| {
            let _ = width().with_description("  ");
        });
    }

    #[test]
    fn a_reset_of_an_unedited_number_writes_nothing() {
        Owner::new().with(|| {
            let width = RwSignal::new(12.0);
            let arg = Arg::new("width", "f64", width);
            let runs = StoredValue::new(0);
            let reader = Memo::new(move |_| {
                width.track();
                runs.update_value(|n| *n += 1);
            });
            reader.get();
            arg.reset();
            reader.get();
            assert_eq!(runs.get_value(), 1);
        });
    }

    #[test]
    fn a_reset_of_an_unedited_choice_writes_nothing() {
        Owner::new().with(|| {
            let build = RwSignal::new(Choice::new(["dev", "0.13.0"]));
            let arg = Arg::new("build", "Choice<&str>", build);
            let runs = StoredValue::new(0);
            let reader = Memo::new(move |_| {
                build.track();
                runs.update_value(|n| *n += 1);
            });
            reader.get();
            arg.reset();
            reader.get();
            assert_eq!(runs.get_value(), 1);
        });
    }

    #[test]
    fn reset_puts_a_choice_back() {
        Owner::new().with(|| {
            let build = RwSignal::new(Choice::new(["dev", "0.13.0", "0.14.0"]).selected("0.13.0"));
            let arg = Arg::new("build", "Choice<&str>", build);
            let ArgControl::Choice(choice) = arg.control() else {
                panic!("`build` did not get the choice control");
            };
            choice.select(2);
            arg.reset();
            assert_eq!(build.get_untracked().value(), "0.13.0");
        });
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::{Bounded, decimals, round_to};

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

    #[test]
    fn moving_to_a_new_value_is_a_change() {
        let mut radius = Bounded::new(0.25).min(0.0).max(1.0);
        assert!(radius.set(0.5), "{radius:?}");
    }

    #[test]
    fn moving_stores_the_new_value() {
        let mut radius = Bounded::new(0.25).min(0.0).max(1.0);
        radius.set(0.5);
        assert_eq!(radius.value(), 0.5);
    }

    #[test]
    fn moving_past_a_bound_the_value_sits_on_is_no_change() {
        let mut radius = Bounded::new(1.0).min(0.0).max(1.0);
        assert!(!radius.set(5.0), "{radius:?}");
    }

    #[test]
    fn a_maximum_on_a_step_is_a_value_the_arg_can_take() {
        assert_eq!(quarters().with_value(1.0).value(), 1.0);
    }

    #[test]
    fn a_numbers_decimals_are_the_digits_after_its_point() {
        assert_eq!(decimals(0.0625), 4);
    }

    #[test]
    fn a_whole_number_has_no_decimals() {
        assert_eq!(decimals(3.0), 0);
    }

    // `1e-320` prints with 320 decimals, and `10f64.powi(320)` is infinite,
    // so scaling by it would make the value NaN.
    #[test]
    fn rounding_to_more_places_than_an_f64_holds_leaves_the_value() {
        assert_eq!(round_to(0.5, decimals(1e-320)), 0.5);
    }
}

#[cfg(test)]
mod color_tests {
    use super::Color;

    #[test]
    fn a_color_displays_as_written() {
        assert_eq!(
            Color::from("rgb(36 56 50 / 0.8)").to_string(),
            "rgb(36 56 50 / 0.8)"
        );
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

#[cfg(test)]
mod derive_tests {
    use super::*;

    /// Every kind of field the derive draws.
    #[derive(Clone, StoryArg)]
    struct Fields {
        /// Whether the card is raised.
        raised: bool,
        width: f64,
        label: String,
        heading: &'static str,
        #[story_arg(color)]
        ink: &'static str,
        paper: Color,
        radius: Bounded,
        font: Choice<&'static str>,
        #[story_arg(skip)]
        version: u32,
    }

    fn fields() -> Fields {
        Fields {
            raised: true,
            width: 12.0,
            label: "Patio".to_string(),
            heading: "Yard",
            ink: "#243832",
            paper: Color::from("#f6f1e7"),
            radius: Bounded::new(0.5).min(0.0).max(1.0),
            font: Choice::labelled([("Serif", "Georgia, serif"), ("Sans", "system-ui")]),
            version: 7,
        }
    }

    #[story(card = fields())]
    fn card(card: Signal<Fields>) -> impl IntoView {
        view! {
            <p
                class:raised=move || card.get().raised
                style:color=move || card.get().ink
                data-version=move || card.get().version
            >
                {move || card.get().heading}
            </p>
        }
    }

    #[story(card = fields())]
    fn card_as_value(card: Fields) -> impl IntoView {
        view! { <p>{card.heading}</p> }
    }

    fn group_of(story: &Story, arg: &str) -> GroupControl {
        let ArgControl::Group(group) = story.arg(arg).expect("the arg").control() else {
            panic!("`{arg}` did not get a group of controls");
        };
        group
    }

    /// The `card` arg's field called `name`.
    fn field(story: &Story, name: &str) -> Arg {
        let group = group_of(story, "card");
        group.arg(name).unwrap_or_else(|| {
            let names: Vec<_> = group.args().iter().map(Arg::name).collect();
            panic!("no field `{name}`; fields: {names:?}")
        })
    }

    fn color(story: &Story, name: &str) -> RwSignal<Color> {
        let ArgControl::Color(color) = field(story, name).control() else {
            panic!("`{name}` did not get the color control");
        };
        color
    }

    fn text(story: &Story, name: &str) -> RwSignal<String> {
        let ArgControl::Text(text) = field(story, name).control() else {
            panic!("`{name}` did not get the text control");
        };
        text
    }

    #[test]
    fn a_struct_arg_has_a_control_per_field_but_a_skipped_one() {
        Owner::new().with(|| {
            let names: Vec<_> = group_of(&card(), "card")
                .args()
                .iter()
                .map(Arg::name)
                .collect();
            assert_eq!(
                names,
                [
                    "raised", "width", "label", "heading", "ink", "paper", "radius", "font"
                ]
            );
        });
    }

    #[test]
    fn each_field_gets_the_control_its_type_picks() {
        Owner::new().with(|| {
            let story = card();
            let control = |name| field(&story, name).control();
            assert!(matches!(control("raised"), ArgControl::Bool(_)), "bool");
            assert!(matches!(control("width"), ArgControl::Num(_)), "f64");
            assert!(matches!(control("label"), ArgControl::Text(_)), "String");
            assert!(matches!(control("heading"), ArgControl::Text(_)), "&str");
            assert!(matches!(control("paper"), ArgControl::Color(_)), "Color");
            assert!(
                matches!(control("radius"), ArgControl::Bounded(_)),
                "Bounded"
            );
            assert!(matches!(control("font"), ArgControl::Choice(_)), "Choice");
        });
    }

    #[test]
    fn a_str_field_marked_color_gets_the_color_control() {
        Owner::new().with(|| {
            let ink = color(&card(), "ink");
            assert_eq!(ink.get_untracked().as_str(), "#243832");
        });
    }

    #[derive(Clone, StoryArg)]
    #[story_arg(color)]
    struct Palette {
        ink: &'static str,
        paper: &'static str,
        label: String,
    }

    #[story(palette = Palette { ink: "#243832", paper: "#f6f1e7", label: "Day".to_string() })]
    fn palette(palette: Palette) -> impl IntoView {
        view! { <p style:color=palette.ink>{palette.label}</p> }
    }

    #[test]
    fn color_on_the_struct_makes_every_str_field_a_color() {
        Owner::new().with(|| {
            let colors: Vec<_> = group_of(&palette(), "palette")
                .args()
                .iter()
                .filter(|arg| matches!(arg.control(), ArgControl::Color(_)))
                .map(Arg::name)
                .collect();
            assert_eq!(colors, ["ink", "paper"]);
        });
    }

    #[test]
    fn a_struct_arg_starts_at_the_default_in_its_attribute() {
        let dom = leptotes::dom(|| card().view());
        assert_eq!(dom.find("p").text(), "Yard", "{dom}");
    }

    #[test]
    fn an_edit_to_a_field_reaches_a_story_that_takes_the_signal() {
        let dom = leptotes::dom(|| {
            let story = card();
            text(&story, "heading").set("Porch".to_string());
            story.view()
        });
        assert_eq!(dom.find("p").text(), "Porch", "{dom}");
    }

    #[test]
    fn an_edit_to_a_color_field_reaches_the_story() {
        let dom = leptotes::dom(|| {
            let story = card();
            let ink = color(&story, "ink");
            ink.set(Color::from("rebeccapurple"));
            story.view()
        });
        assert_eq!(
            dom.find("p")
                .attr("style")
                .map(|style| style.trim_end_matches(';')),
            Some("color:rebeccapurple"),
            "{dom}"
        );
    }

    #[test]
    fn an_edit_to_a_field_reaches_a_story_that_takes_the_value() {
        let dom = leptotes::dom(|| {
            let story = card_as_value();
            text(&story, "heading").set("Porch".to_string());
            story.view()
        });
        assert_eq!(dom.find("p").text(), "Porch", "{dom}");
    }

    #[test]
    fn a_skipped_field_keeps_its_default() {
        let dom = leptotes::dom(|| card().view());
        assert_eq!(dom.find("p").attr("data-version"), Some("7"), "{dom}");
    }

    /// A struct that implements `Drop` refuses a move out of its fields.
    #[derive(Clone, StoryArg)]
    struct Dropped {
        label: String,
    }

    impl Drop for Dropped {
        fn drop(&mut self) {}
    }

    #[story(dropped = Dropped { label: "Patio".to_string() })]
    fn dropped(dropped: Dropped) -> impl IntoView {
        view! { <p>{dropped.label.clone()}</p> }
    }

    #[test]
    fn a_struct_that_implements_drop_starts_at_its_default() {
        let dom = leptotes::dom(|| dropped().view());
        assert_eq!(dom.find("p").text(), "Patio", "{dom}");
    }

    // Denied here so that a local the derive names after a field fails the
    // build rather than warning in a consumer's.
    #[deny(non_snake_case)]
    mod camel_case {
        use super::*;

        #[derive(Clone, StoryArg)]
        pub(super) struct Camel {
            #[allow(non_snake_case)]
            pub(super) inkColor: &'static str,
        }

        #[story(camel = Camel { inkColor: "Yard" })]
        pub(super) fn camel(camel: Signal<Camel>) -> impl IntoView {
            move || camel.get().inkColor
        }
    }

    #[test]
    fn a_camel_case_field_is_named_as_written() {
        Owner::new().with(|| {
            let names: Vec<_> = group_of(&camel_case::camel(), "camel")
                .args()
                .iter()
                .map(Arg::name)
                .collect();
            assert_eq!(names, ["inkColor"]);
        });
    }

    use std::sync::Mutex;

    // Each static belongs to one test, so its contents hold when `cargo test`
    // runs the tests on parallel threads in one process.
    static HEADINGS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static INKS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

    #[story(card = fields())]
    fn card_heading_kept(card: Signal<Fields>) -> impl IntoView {
        move || HEADINGS.lock().unwrap().push(card.get().heading)
    }

    #[story(card = fields())]
    fn card_ink_kept(card: Signal<Fields>) -> impl IntoView {
        move || INKS.lock().unwrap().push(card.get().ink)
    }

    /// Draw `story` on a live stage, and run each edit in turn.
    fn edit_on_a_live_stage(story: Story, edits: &[&dyn Fn()]) {
        let stage = Memo::new(move |_| drop(story.view().to_html()));
        for edit in edits {
            edit();
            stage.get();
        }
    }

    /// Every `text` in `kept` is the one `&'static str`.
    fn assert_kept_once(kept: &[&'static str], text: &str) {
        let copies: Vec<_> = kept.iter().filter(|kept| **kept == text).collect();
        assert!(copies.len() >= 2, "{text:?} read once; kept: {kept:?}");
        let pointers: Vec<_> = copies.iter().map(|copy| copy.as_ptr()).collect();
        assert!(
            pointers.iter().all(|p| *p == pointers[0]),
            "{text:?} kept at {pointers:?}"
        );
    }

    #[test]
    fn a_str_field_edited_back_to_a_text_gets_the_same_str() {
        Owner::new().with(|| {
            let story = card_heading_kept();
            let heading = text(&story, "heading");
            let edits = ["Porch", "Deck", "Porch"].map(|t| move || heading.set(t.to_string()));
            edit_on_a_live_stage(story, &[&edits[0], &edits[1], &edits[2]]);
        });
        assert_kept_once(&HEADINGS.lock().unwrap(), "Porch");
    }

    #[test]
    fn a_color_field_edited_back_to_a_color_gets_the_same_str() {
        Owner::new().with(|| {
            let story = card_ink_kept();
            let ink = color(&story, "ink");
            let edits = ["red", "blue", "red"].map(|c| move || ink.set(Color::from(c)));
            edit_on_a_live_stage(story, &[&edits[0], &edits[1], &edits[2]]);
        });
        assert_kept_once(&INKS.lock().unwrap(), "red");
    }

    #[test]
    fn a_fields_doc_comment_is_its_description() {
        Owner::new().with(|| {
            assert_eq!(
                field(&card(), "raised").description(),
                Some("Whether the card is raised.")
            );
        });
    }

    #[test]
    fn a_field_with_no_doc_comment_has_no_description() {
        Owner::new().with(|| {
            assert_eq!(field(&card(), "width").description(), None);
        });
    }

    #[test]
    fn a_reset_puts_every_field_back() {
        Owner::new().with(|| {
            let story = card();
            text(&story, "heading").set("Porch".to_string());
            text(&story, "label").set("Deck".to_string());
            story.arg("card").unwrap().reset();
            assert_eq!(
                [
                    text(&story, "heading").get_untracked(),
                    text(&story, "label").get_untracked()
                ],
                ["Yard", "Patio"]
            );
        });
    }

    #[test]
    fn a_struct_arg_keeps_its_type_as_the_story_wrote_it() {
        Owner::new().with(|| {
            assert_eq!(card().arg("card").unwrap().ty(), "Signal<Fields>");
        });
    }

    #[test]
    fn a_fields_type_is_as_the_struct_wrote_it() {
        Owner::new().with(|| {
            assert_eq!(field(&card(), "heading").ty(), "&'static str");
        });
    }
}

#[cfg(test)]
mod intern_tests {
    use super::__private::intern;

    #[test]
    fn a_text_is_kept_once() {
        let (first, second) = (intern("Porch"), intern("Porch"));
        assert!(std::ptr::eq(first, second), "{first:p} and {second:p}");
    }

    #[test]
    fn interned_text_reads_as_the_text_given() {
        assert_eq!(intern("Deck"), "Deck");
    }
}
