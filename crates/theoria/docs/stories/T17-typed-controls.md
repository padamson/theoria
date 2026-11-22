# T17 — Typed controls
*Pulled by: pyle's theme and version-bar playgrounds, which type colors and a
version into text boxes and rebuild the stage on every keystroke.*

## Story

As a Leptos developer designing a theme in the gallery, I want controls that
match the value (a color picker, a list of choices, a bounded slider) and a
story that redraws only what an edit changes, so that tuning a palette is
quick and the rest of the stage keeps its state.

## Vertical slices

- **T17.0 — args as signals** ✅
  - [x] a `#[story]` parameter typed `Signal<T>`, for `T` in `bool`, `f64`,
        and `String`, gets the same control as `T`
  - [x] the body receives the signal, and an edit to it redraws only what
        reads it; with every parameter a signal the body runs once, and state
        it made survives every edit
  - [x] an unsupported type, or a `Signal` of one, is a compile error at
        the parameter
  - [x] the panel's Type column shows the parameter's type as written, so
        `Signal<f64>` and `f64` read differently
- **T17.1 — bounded numbers**
  - [ ] `theoria::Bounded`, built in the story attribute with a min, max,
        and step, drawn as a slider beside the number input
- **T17.2 — color**
  - [ ] `theoria::Color`, holding any CSS color as text, drawn as a swatch
        that opens the browser's picker beside a text field; the picker
        writes `#rrggbb`
- **T17.3 — choice**
  - [ ] `theoria::Choice<T>`, a list of labelled values built in the story
        attribute, drawn as a `<select>`; the body gets the selected `T`

## Notes / refs

- Ruled 2026-11-21: the parameter's type picks the control (through its
  `theoria::StoryArg` impl, since T17.0), and the story-level default
  carries a control's settings (`radius =
  Bounded::new(0.25).min(0.0).max(1.0)`). No per-parameter attributes and no
  derive.
- `StoryArg` is sealed and its methods are hidden, so the trait can change
  shape for T17.1 to T17.3 without a semver break. A new control is an impl
  in `theoria` (`value_arg!` for one the body takes by value), not a change
  to the macro.
- A default in the attribute is typed as the arg's `Init` (`f64` for an
  `f64`, `&'static str` for a `String`), so an expression such as
  `"3".parse().unwrap()` infers its type from the arg.
- Leptos patches the stage's elements in place when a value-arg story reruns,
  so what a value arg loses on an edit is the body's own state, not the DOM.
  The e2e test checks a count the story keeps.
- Building a playground over a struct (one control per field, groups, the
  value as code to copy, a reset) is a separate story, to be written once
  pyle has used these controls.
