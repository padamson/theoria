# theoria-usage reference

- `Story::new(name: &'static str, view_fn: F) -> Story` where `F: Fn() ->
  V + Send + Sync + 'static, V: IntoView`: a plain story with no args. A
  `/` in `name` nests it in the sidebar.
- `.with_description(md: &'static str)` on a `Story` gives it a Markdown
  description, which the docs panel shows and the sidebar marks. A
  `#[story]` function's doc comment is its description, so this is for a
  story built with `Story::new`. The text is Markdown as written, so a
  line indented four spaces inside the literal is a code block. A second
  description (including one on a `#[story]` story with a doc comment)
  or blank text panics when the story is built.
- `#[story(name = "Group/Name", arg = default, ...)]` on `fn x(arg: T,
  ...) -> impl IntoView`: generates `fn x() -> Story` with one control per
  parameter. `T` is `bool`, `f64`, `String`, `Bounded`, `Color`,
  `Choice<V>`, or a struct that derives `StoryArg`, or `Signal<T>` over
  one of them (the types that implement `theoria::StoryArg`; implement it
  only with the derive);
  anything else is a compile error at the parameter, "`i32` cannot be a
  #[story] arg". A default in the attribute is written as the arg's value
  (`width = 12.0` for an `f64`, not `12`), or as a `&'static str` for a
  `String`. An arg missing from the attribute starts at
  `Default::default()`. Without `name`, the story takes the function's
  name. A key that names no parameter is a compile error at the key,
  and so is a parameter called `name`.
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
- `#[derive(Clone, StoryArg)]` on a struct of named fields (no generic
  parameters or where clause)
  makes it one arg, written `theme: Theme` or `theme: Signal<Theme>`, with
  its default in the attribute as a value of the struct (`theme = THEME`,
  a `const` or any expression). Each field gets the control its type
  picks, as a parameter's does, and a `&'static str` field is a text box;
  the panel draws them under the arg's name, each with the field's doc
  comment as Markdown. `#[story_arg(color)]` on a `&'static str` field
  makes it a color, and on the struct makes every field written
  `&'static str` one (a type alias for it is not seen there). `#[story_arg(skip)]` keeps a field at its default with no control.
  A field whose type also derives `StoryArg` (a `Palette` inside a
  `Theme`) is a group of its own fields under the field's name, with the
  field's doc comment, and takes its options from its own struct: the
  outer struct's `color` does not reach into it.
  A field of any other type is a compile error at the field ("a field of
  type `u32` has no control"), and so is `color` on a field that is not a
  `&'static str`. An edited `&'static str` is leaked once per distinct
  text, for as long as the gallery runs, so the story needs no `Memo` or
  `leak` of its own. In a crate whose theoria is optional, write the
  derive and its options as `#[cfg_attr(feature = "stories",
  derive(theoria::StoryArg))]` and `#[cfg_attr(feature = "stories",
  story_arg(color))]`. A field's control is reached as
  `story.arg("theme")`'s `ArgControl::Group(fields)`, then
  `fields.arg("paper")`, and a nested group's the same way again
  (`fields.arg("palette")`'s `ArgControl::Group`).
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
  The panel's Reset button puts every arg of the story back to its
  default, and `arg.reset()` does it for one; the panel shows either. A
  reset writes only an arg that was edited, so it reruns a story that
  takes values only when one of them changes.
- `story.view()` draws the story's body with its args' current values,
  inside the reactive owner a leptotes `render` closure gives it. To test
  what an arg feeds, build the story in the closure, set the arg through
  its control, and return `story.view()`:

  ```rust
  let html = leptotes::render(|| {
      let story = swatch();
      let ArgControl::Color(ink) = story.arg("ink").unwrap().control() else {
          panic!("`ink` did not get the color control");
      };
      ink.set(Color::from("rebeccapurple"));
      story.view()
  });
  ```

  `story.arg(name)` is `None` for a name the story has no arg for. The
  body needs no split into a function of its own for the test to reach
  it. The test compiles only where the story does: put the stories module
  under `#[cfg(any(test, feature = "stories"))]` and list theoria in
  `[dev-dependencies]` as well as the optional dependency, or run the
  tests with `--features stories` in the hook and in CI.
- `theoria.toml`, `[gallery]` table: `crate` (package name), `path` (to
  the crate, relative to the config file), `aggregator` (path of the fn
  returning `Stories`), and `features` (`csr` is added). An unknown key
  is an error.
- `Stories`: what the aggregator returns, built with `.into()` from a
  `Vec<Story>` or `.collect()` from an iterator of them; it also iterates
  and extends, so per-module aggregators combine. `stories.gallery()` is
  the whole gallery as a view, and `stories.mount()` (behind theoria's
  `csr`) mounts it on `<body>` and logs any panic to the browser console,
  which is all the generated app does.
- The stories crate's `csr` feature enables theoria's: `"theoria?/csr"`
  when theoria is an optional dependency (the usual `stories =
  ["dep:theoria"]`), `"theoria/csr"` when it is not. Cargo rejects the `?`
  form for a dependency that is not optional.
- The gallery build failing with ``no method named `mount` `` means one of
  two things. Found for `Stories`: the crate's `csr` does not enable
  theoria's. Found for a `Vec`: the aggregator returns `Vec<Story>`, not
  `Stories`.
- A story's URL is the gallery's with `?story=` and the story's full name,
  form-encoded but with each `/` left as it is
  (`?story=Version+bar/Playground`). It opens that story
  over the one stored in `localStorage`; with no `story` in the URL the
  stored story opens, else the first. A name no story has is reported on
  the stage, with nothing marked in the sidebar. Selecting a story
  rewrites the URL without a history entry, and each sidebar entry is an
  `<a>` linking to its story's URL, marked `aria-current="page"` when its
  story is on the stage. A browser test opens a story by building that
  URL with `form_urlencoded::byte_serialize`, never by clicking its label,
  which another story in a different group may share. The gallery reads a
  `/` written as `%2F` too; to compare an `href` or the address with what
  the gallery writes, add `.replace("%2F", "/")`.
- `theoria serve [--port 8081]`, `theoria build`, `theoria generate`, each
  with `--config <file>` (default `theoria.toml`). A config change needs a
  re-run of `serve`.
- Build the CLI in the consumer's workspace, so it comes from the same
  theoria commit as the stories (the one in the consumer's `Cargo.lock`):
  a small package (say
  `tools/gallery`) whose one dependency is `theoria-cli`, written with the
  same `git` and the same `branch`, `tag`, or `rev` as `theoria` (a
  different reference is locked on its own and drifts), and whose
  `src/main.rs` is
  `fn main() -> std::process::ExitCode { theoria_cli::run(std::env::args_os()) }`.
  `[alias] theoria = "run -q -p gallery --"` in `.cargo/config.toml` makes
  it `cargo theoria serve`. Not an example in the components' crate: an
  example links that crate, so it builds the components natively, and
  `cargo test` builds the CLI with every run. A `theoria` from `cargo
  install` stays at the commit it was installed from, and one older than
  the library fails or rewrites the gallery in an old shape.
- Each run copies the `Cargo.lock` of the stories crate's workspace into
  the generated app (its own workspace), so the gallery builds on the
  versions that lockfile records and needs no lockfile copied in by hand.
  The app depends on nothing but the stories crate. A lockfile change needs a re-run
  of `serve`. A workspace with no lockfile yet leaves the app's own in
  place. A crate that its workspace neither lists nor excludes is an
  error, since cargo cannot say which lockfile it builds on.
