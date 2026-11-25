---
name: theoria-dev
description: "How theoria itself is built: the crate split and features, the colocated component, story, and test files and how `components/mod.rs` wires them, how components are tested (leptotes under ssr, `story_impl` for the macro, `render` and `generate` for the CLI, theoria-e2e in a browser), what mutation testing covers and why, how the CLI generates the gallery harness, the demand-driven backlog in `docs/PLAN.md` and `docs/stories/`, and the release order. Use when adding or changing a theoria component, the `#[story]` macro, or `theoria-cli`; when adding a story or slice to the backlog; when a theoria test, mutant, or e2e run fails; and before a release. Not for writing stories in a crate that uses theoria, which is the shipped `theoria-usage` skill."
metadata:
  internal: true
---

# Developing theoria

theoria is a component explorer for Leptos. A crate writes stories, and
`theoria-cli` generates a Trunk app that mounts them
(`aggregator().mount()`). This skill is for changing theoria; `CLAUDE.md` has the commands and
gates, and `skills/theoria-usage/` is what a consumer reads.

## The crates

- `crates/theoria`: `Story`, `Stories`, `ArgControl`, and the components (`Gallery`,
  `StoryNav`, `Controls`, `ShowCode`, `Markdown`). It enables no Leptos
  runtime of its own. The `csr`, `ssr`, and `hydrate` features pass through
  to Leptos; `csr` also pulls in `web-sys` for the `localStorage` that keeps
  the selected story across reloads; a consumer's gallery gets it only when
  the stories crate forwards `csr` to theoria. `stories` compiles theoria's own
  stories.
- `crates/theoria-macros`: `#[story]`. The proc-macro entry point is a thin
  adapter; the logic is `story_impl`, which takes and returns
  `proc_macro2::TokenStream` so tests can call it. It emits
  `Story::__from_macro`, which is `#[doc(hidden)]` and not stable API.
  `theoria/src/lib.rs` has `extern crate self as theoria` so the macro's
  `theoria::` paths resolve when theoria uses it on itself. The macro does
  not read a parameter's type: it emits `<T as theoria::StoryArg>` calls
  for every one, so a new control is a `StoryArg` impl in `theoria` (its
  `Value`, its `ArgControl`, and `bind`), not a change to the macro, and an
  unsupported type is a rustc trait error at the parameter.
- `crates/theoria-cli`: the `theoria` binary, whose `main` only calls the
  library's `theoria_cli::run(args)`, so a consumer can build the CLI as a
  three-line package in its own workspace, from the theoria commit in its
  `Cargo.lock`. No Leptos; native only.
- `crates/theoria-e2e`: outside the workspace, with its own `Cargo.lock`,
  because it pulls playwright-rs and tokio and would slow every workspace
  command.

## Components, stories, and tests

Each component is three files in `crates/theoria/src/components/`:
`<name>.rs`, `<name>.stories.rs` (behind `stories`), and `<name>.tests.rs`
(behind `cfg(test)`). The dotted names are not Rust identifiers, so
`components/mod.rs` declares each with `#[path = "..."]`. A new component
needs four edits there: the `mod`, the `pub use`, the stories module and
its line in `stories()`, and the tests module.

Theoria's own stories double as the e2e fixtures (`theoria-e2e.toml` builds
a gallery from `theoria::stories`). Never put a `Gallery` inside a
`Gallery`; `Gallery` is tested with leptotes and through the e2e suite.

## Testing

- **Components:** `leptotes::render(|| view! { ... })` returns the SSR
  HTML, and the test asserts on it. Build signals inside the closure.
  leptotes renders under `ssr`, which the workspace gets through the
  leptotes dev-dependency.
- **Macro:** call `story_impl(attr, item)` and check the tokens, or use
  `#[story]` on a function in `lib.rs`'s `story_macro_tests` and check the
  `Story` it builds (inside an `Owner`, since the args are signals).
- **CLI:** `render` returns the harness's `Cargo.toml`, `src/main.rs`, and
  `Trunk.toml` as strings, and the tests parse the TOML and compare values.
  `run` is tested with the arguments a shell would pass, for its exit code
  and the files it writes.
  `generate` is tested only for the wiring: each rendered string reaches
  its file in a `TempDir`, a bad config is refused, and the right
  `Cargo.lock` is copied in. Its fixtures are workspaces cargo can load
  (`write_workspace`), since `generate` runs `cargo locate-project`, which
  would otherwise look for a workspace above the temp directory.
- **Browser:** theoria-e2e discovers every story from the sidebar at run
  time, so a new story is covered without editing the test. Every browser
  test runs its body through `with_gallery`, which fails when the gallery
  is not built and closes the browser on every path. One test opens no
  browser: it reads the `Cargo.lock` cargo left beside the built gallery
  and fails if it has a package the workspace's lockfile does not record,
  other than the harness itself.
  Every test is `#[ignore]`d, since it needs the gallery. In a Claude Code
  session, run it with `cargo nextest run --manifest-path
  crates/theoria-e2e/Cargo.toml --config-file .config/nextest.toml
  --run-ignored all` as its own Bash call, which the sandbox's `cargo
  nextest run *` exclusion covers; under `cargo test` the sandbox stops
  Chromium at launch.

Mutation testing covers `theoria-cli` and `theoria-macros` only
(`.cargo/mutants.toml` says why `theoria` is left out). The excluded
mutants (`main`, `trunk`, the `story` adapter) are reachable only end to
end.

## The CLI's harness

`generate` writes `Cargo.toml`, `src/main.rs`, `index.html`, and
`Trunk.toml` under `target/theoria/gallery/<crate>/`, one directory per
gallery so two configs do not clobber each other. `csr` is appended to the
target crate's features. The harness is its own workspace, so `generate`
also copies in the `Cargo.lock` the stories crate builds on, on every run:
`cargo locate-project --workspace` names the crate's workspace (its own
manifest, for a crate excluded from the workspace around it), and the
lockfile is the one beside that manifest. Cargo keeps each package the
harness shares with that lockfile at its version, drops the rest, and adds
the harness itself. The stories crate is the harness's only dependency
(`Stories::mount` sets the panic hook), so nothing else is resolved outside
that lockfile. A crate its workspace neither lists nor excludes is
refused, and a workspace with no lockfile yet leaves the harness's own
lockfile as it is. The harness declares no theoria or leptos of its
own, since either could resolve to a second crate whose types would not
match the stories crate's. Its `main` is `aggregator().mount()`: the
aggregator returns `theoria::Stories`, and an inherent method needs no path
to its crate, so the stories crate's theoria and leptos are the ones that
mount. That holds for theoria's own gallery too, with no special case.
`Stories::mount` is behind theoria's `csr`, which a stories crate's own
`csr` must enable (`theoria?/csr` for an optional dependency, `theoria/csr`
otherwise), since cargo does not let a dependency entry enable a transitive
feature. A crate that does not forward it fails the gallery build with
``no method named `mount` ``, where it would otherwise run without the
`localStorage` code. `Trunk.toml` watches the target crate's `src/`, not
the harness, whose `dist/` would loop the rebuild. The variable parts of
the TOML are built as `toml::Value`s, so a Windows path is escaped, and
`main` resolves the config with `std::path::absolute`, not `canonicalize`,
whose Windows result is the verbatim `\\?\` form.

## The backlog

`crates/theoria/docs/PLAN.md` is the overview and the Storybook parity
roadmap; `crates/theoria/docs/stories/` has one doc per user story
(`T<n>`), each split into vertical slices with checkbox acceptance
criteria. A story is written up, and a slice built, only when a consumer
needs it. Record which need pulled it on the story's `Pulled by:` line.

## Releasing

`release.yml` publishes `theoria-macros`, then `theoria`, then
`theoria-cli`; `theoria` resolves `theoria-macros` from crates.io, so the
order matters. The first version of each goes out by hand in that order,
then each crate gets its crates.io trusted publisher (environment
`release`) and `[policy.<crate>] audit-as-crates-io = false` in
`supply-chain/config.toml`, before the tag. `CLAUDE.md`'s "Release
process" has the rest.
