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
  returning `Vec<Story>`), `features` (`csr` is added), `theoria_path`
  (default `crates/theoria`; the generated app depends on theoria by
  path, so it must point at a checkout).
- `theoria serve [--port 8081]`, `theoria build`, `theoria generate`, each
  with `--config <file>` (default `theoria.toml`). A config change needs a
  re-run of `serve`.
