# theoria-usage reference

- `Story::new(name: &'static str, view_fn: F) -> Story` where `F: Fn() ->
  V + Send + Sync + 'static, V: IntoView`: a plain story with no args. A
  `/` in `name` nests it in the sidebar.
- `#[story(name = "Group/Name", arg = default, ...)]` on `fn x(arg: T,
  ...) -> impl IntoView`: generates `fn x() -> Story` with one control per
  parameter. `T` is `bool`, `f64`, or `String`; anything else is a compile
  error. An arg missing from the attribute starts at `Default::default()`.
  Without `name`, the story takes the function's name.
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
