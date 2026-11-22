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
pub enum ArgControl {
    Bool(RwSignal<bool>),
    Num(RwSignal<f64>),
    Text(RwSignal<String>),
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

    /// As the story wrote it (`Signal<f64>`), for the panel's Type column.
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
        note = "a Signal arg holds a bool, f64, or String"
    )]
    pub trait Plain:
        super::StoryArg<Value = Self> + Clone + Default + Send + Sync + 'static
    {
    }
}

/// A type a `#[story]` parameter can have: `bool`, `f64`, `String`, or a
/// `Signal` of one, which hands the body the arg's signal, not its value.
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
    note = "a #[story] arg is a bool, f64, or String, or a `Signal` of one (not an `RwSignal`)"
)]
pub trait StoryArg: Sized + sealed::Sealed {
    /// What the arg's control edits.
    type Value: Clone + Default + Send + Sync + 'static;

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

    /// Mount the gallery on the page's `<body>`, which is all the app
    /// `theoria serve` generates does.
    ///
    /// It is behind `csr` so that a stories crate whose `csr` feature does
    /// not enable theoria's fails to build, where it would otherwise run
    /// without the `localStorage` that keeps the selected story.
    #[cfg(feature = "csr")]
    pub fn mount(self) {
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
mod story_macro_tests {
    use super::*;

    /// A demo widget for the macro test.
    #[story(name = "Demo/Widget", active = true, width = 12.0)]
    fn widget(active: bool, width: f64, label: String) -> impl IntoView {
        // Reference each arg so the body genuinely depends on them.
        view! { <div data-active=active data-width=width>{label}</div> }
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
