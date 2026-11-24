# theoria-usage reference

- `Story::new(name: &'static str, view_fn: F) -> Story` where `F: Fn() ->
  V + Send + Sync + 'static, V: IntoView`: a plain story with no args. A
  `/` in `name` nests it in the sidebar.
- `#[story(name = "Group/Name", arg = default, ...)]` on `fn x(arg: T,
  ...) -> impl IntoView`: generates `fn x() -> Story` with one control per
  parameter. `T` is `bool`, `f64`, `String`, `Bounded`, `Color`, or
  `Choice<V>`, or `Signal<T>` over one of them (the types that implement
  `theoria::StoryArg`, which is sealed);
  anything else is a compile error at the parameter, "`i32` cannot be a
  #[story] arg". A default in the attribute is written as the arg's value
  (`width = 12.0` for an `f64`, not `12`), or as a `&'static str` for a
  `String`. An arg missing from the attribute starts at
  `Default::default()`. Without `name`, the story takes the function's
  name.
- `Bounded::new(v).min(a).max(b).step(s)` (each bound optional) is a
  number control that gets a slider when it has both bounds and clamps a
  typed value to them, and snaps it to the step (counted from `min`), as
  it is typed; the box shows the result once it loses focus. The value
  always lies within the bounds and on a step. A `min` above the `max`, a
  bound that is not finite, or a step that is not above zero panics when
  the story is built. The body reads it with `.value()`.
- `Color` is any CSS color as text, with a `&'static str` default. Its
  control is a swatch beside a text field. The swatch shows whatever
  color the text holds, alpha included, and is empty for text that is not
  a color. It opens the browser's picker, which writes `#rrggbb` into the
  text and so drops any alpha. The story gets the text as typed. The body
  reads it with `.as_str()` or `to_string()`.
- `Choice<V>` is one of a fixed list of values, picked from a menu (a
  `<select>`). The attribute gives the list: `Choice::new(["dev",
  "0.13.0"])` labels each value by its text, and `Choice::labelled([("Serif",
  "Georgia, serif"), ...])` gives each its own label. The first option
  starts selected unless `.selected("label")` names another. An empty
  list, or a label no option has, panics when the story is built, and a
  `Choice` arg with no list in the attribute does not compile. The body
  reads the selected value with `.value()`.
- A `T` parameter is read inside the generated view closure, so an edit to
  it runs the whole body again (and resets any signal the body creates). A
  `Signal<T>` parameter is the arg's signal, the same one on every run,
  and an edit to it redraws only what reads it. A story whose parameters
  are all signals runs its body once. Read a signal in a closure (`move ||
  x.get()`); a `get()` in the body itself makes the stage rerun the body
  as before.
- `Story::args()` returns `&[Arg]`: each `Arg` has a `name()`, the
  parameter's type as written (`ty()`, shown beside the name in the
  panel, so `Signal<f64>` reads as that), and its `control()`. Each
  control sits under its arg's name and type, as wide as the panel.
- `theoria.toml`, `[gallery]` table: `crate` (package name), `path` (to
  the crate, relative to the config file), `aggregator` (path of the fn
  returning `Stories`), and `features` (`csr` is added). An unknown key
  is an error.
- `Stories`: what the aggregator returns, built with `.into()` from a
  `Vec<Story>` or `.collect()` from an iterator of them; it also iterates
  and extends, so per-module aggregators combine. `stories.gallery()` is
  the whole gallery as a view, and `stories.mount()` (behind theoria's
  `csr`) mounts it on `<body>`, which is all the generated app does.
- The stories crate's `csr` feature enables theoria's: `"theoria?/csr"`
  when theoria is an optional dependency (the usual `stories =
  ["dep:theoria"]`), `"theoria/csr"` when it is not. Cargo rejects the `?`
  form for a dependency that is not optional.
- The gallery build failing with ``no method named `mount` `` means one of
  two things. Found for `Stories`: the crate's `csr` does not enable
  theoria's. Found for a `Vec`: the aggregator returns `Vec<Story>`, not
  `Stories`.
- `theoria serve [--port 8081]`, `theoria build`, `theoria generate`, each
  with `--config <file>` (default `theoria.toml`). A config change needs a
  re-run of `serve`.
