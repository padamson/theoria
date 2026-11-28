# T18 — Struct playgrounds
*Pulled by: pyle's `Theme/Playground`, which lists `Theme`'s ten fields as
ten args, rebuilds the struct from them, leaks each edited `&'static str`,
and has no dark palette because eight more rows in one flat panel would not
be readable.*

## Story

As a Leptos developer designing a theme or a component's settings in the
gallery, I want one control per field of a struct, in groups that follow
the struct's shape, with a reset and the value written out as code, so that
a new field reaches the playground without my editing it and the design I
settle on can be pasted into the site.

## Vertical slices

- **T18.0 — draw a story from a test** ✅
  - [x] `Story::view` draws the story's body with its args' current values,
        so a leptotes test can check a story's wiring (which arg feeds what)
        without splitting the body into a function of its own
- **T18.1 — reset** ✅
  - [x] the controls panel has a Reset button that puts every arg of the
        selected story back to its default
- **T18.2 — `#[derive(StoryArg)]` over a struct of leaf fields** ✅
  - [x] a struct that derives `StoryArg` is a `#[story]` arg, taken as the
        struct or as a `Signal` of it, with its default given in the story
        attribute (`theme = DEFAULT`)
  - [x] each field gets a control by its type: `bool`, `f64`, `String`,
        `Bounded`, `Color`, and `Choice<T>` as story params do, and
        `&'static str` a text box
  - [x] `#[story_arg(color)]` on a `&'static str` field makes it a color;
        on the struct, every `&'static str` field
  - [x] an edited `&'static str` is stored once per distinct text, not once
        per edit
  - [x] each field's doc comment is shown with its control
  - [x] a field whose type has no control is a compile error at the field,
        unless it is `#[story_arg(skip)]`, which keeps its default
- **T18.3 — groups** ✅
  - [x] a field whose type derives `StoryArg` is a group of controls under
        the field's name
  - [x] an `Option` of one is a group with a switch: off is `None`, and on
        again brings back the fields as they were left
  - [x] the struct an `Option` holds is `Default`, which the fields start
        at when the arg starts at `None`; without it the arg does not
        compile
- **T18.4 — the value as code**
  - [ ] the panel shows a struct arg's value as `{:#?}` writes it, with a
        button that copies it

## Notes / refs

- Ruled 2026-11-27: an `Option` of a struct needs the struct to be
  `Default`, where its fields start when the arg starts at `None` (a site
  whose theme has no dark palette, turning one on to design it). What
  those values are is the struct's crate's to say: theoria knows no theme,
  and a dark palette worked out from a light one is by role, which only
  the crate that owns the roles knows. A seed attribute on the field was
  the alternative; a story parameter cannot carry one.
- Ruled 2026-11-26: the struct derives `StoryArg`, and its fields' types
  pick their controls, with `#[story_arg(...)]` for what a type cannot say
  (a `&'static str` that is a color). This follows serde and clap, and
  bevy's inspector (`Reflect` with `#[inspector(min, max)]`). It extends
  T17's ruling (the parameter's type picks the control, no per-parameter
  attributes) to structs: story parameters are unchanged. A builder of
  field accessors in the story attribute was the alternative; it names each
  field again in the stories file, and a field added to the struct gets no
  control and no error.
- theoria never learns what a theme is: the derive works on any struct. A
  theme that a site writes out as CSS variables lets the browser redraw
  what reads each one, so the story takes `Signal<Theme>`; a Leptos `Store`
  arg, for components that read fields as signals, waits for a consumer.
- A crate whose theoria dependency is optional writes the derive and its
  attributes inside `cfg_attr(feature = "stories", ...)`.
- `StoryArg`'s supertrait `Sealed`, and `Plain` (what a `Signal<T>` arg
  holds) and `Field` (what a field can be), are public under the
  `#[doc(hidden)]` `theoria::__private`, so the derive can implement them
  in another crate. They are not stable API, so the derive is the only
  supported impl of `StoryArg` outside theoria. A
  struct arg is `Arg::group` over one arg per field, and its value is
  rebuilt from the fields' signals on each read.
- Not pulled yet: a length control (a number and a unit, for pyle's
  `radius`) and a choice that also takes free text (for a site's own font
  stack). Both fields are text boxes until a consumer asks.
