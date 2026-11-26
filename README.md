# theoria

A component explorer for [Leptos](https://leptos.dev): Storybook in
miniature. You write stories (named previews of your components, with
editable args), and the `theoria` CLI generates a small Trunk app that
shows them in a gallery with a sidebar, live controls, the story's source,
and its doc comment rendered as Markdown. The name is Greek θεωρία,
"viewing."

theoria is young and its API will change. Nothing is on crates.io yet.

## Writing stories

A story is a `theoria::Story`. Build one by hand:

```rust
use leptos::prelude::*;
use theoria::Story;

let story = Story::new("Buttons/Primary", || view! { <button>"Save"</button> });
```

or with `#[story]`, where each typed parameter becomes a control (`bool`
is a toggle, `f64` a number, `String` a text box) and the attribute sets
the defaults:

```rust
use leptos::prelude::*;
use theoria::story;

/// The yard canvas, drawn to scale.
#[story(name = "Canvas/Yard", yard_w = 70.0, yard_d = 30.0)]
fn yard(yard_w: f64, yard_d: f64) -> impl IntoView {
    view! { <Yard yard_w=yard_w yard_d=yard_d /> }
}
```

The doc comment is the story's description, which the gallery's docs panel
shows as Markdown. A story built by hand gets one from
`.with_description("...")`.

A parameter typed `Signal<T>`, over any type an arg can have, gets the arg's
signal instead of its value, and an edit to it redraws only what reads the
signal. An edit to a plain parameter runs the whole body again, so a story
whose parameters are all signals runs its body once, and state it makes for
itself (an open menu, a count) survives every edit. Read a signal inside a
closure:

```rust
#[story(name = "Theme/Swatch", label = "Ink")]
fn swatch(label: Signal<String>) -> impl IntoView {
    view! { <Swatch label=move || label.get() /> }
}
```

A `theoria::Bounded` parameter is a number with optional bounds and step,
set in the attribute (`radius = Bounded::new(0.25).min(0.0).max(1.0)`);
with both bounds it gets a slider. A `theoria::Color` parameter is any CSS
color as text (`ink = "rgb(36 56 50 / 0.8)"`), with a swatch that shows it
and opens the browser's picker. A `theoria::Choice<T>` parameter is one of
a list of values given in the attribute
(`build = Choice::new(["dev", "0.13.0"])`), picked from a menu.

The `/` in a name nests the story in the sidebar. Your crate collects its
stories in a function returning `theoria::Stories`, usually behind a
`stories` feature so production builds leave them out:

```rust
#[cfg(feature = "stories")]
pub fn all_stories() -> theoria::Stories {
    vec![yard()].into()
}
```

The generated app calls `.mount()` on that value, so the gallery comes from
the same theoria (and the same Leptos) as your stories, whether that is a
path, git, or crates.io.

## Serving the gallery

Point a `theoria.toml` at that function:

```toml
[gallery]
crate = "my-components"            # the package name
path = "crates/my-components"      # relative to this file
aggregator = "my_components::all_stories"
features = ["stories"]
```

Then build the CLI in your own workspace, from the same theoria commit as
your stories. `Cargo.lock` records that commit, so when it moves (by hand or
through Dependabot) the CLI moves with it. Add a package for it, say `tools/gallery`, whose one dependency is `theoria-cli`, written the
way you write `theoria` (the same `git` and the same `branch`, `tag`, or
`rev`; cargo locks a different reference on its own, and the two drift
apart):

```toml
[package]
name = "gallery"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
theoria-cli = { git = "https://github.com/padamson/theoria" }
```

Its `src/main.rs` is the whole CLI:

```rust
fn main() -> std::process::ExitCode {
    theoria_cli::run(std::env::args_os())
}
```

and an alias in `.cargo/config.toml` names it:

```toml
[alias]
theoria = "run -q -p gallery --"
```

```bash
cargo theoria serve    # http://localhost:8081, rebuilt on every source edit
cargo theoria build    # a static gallery under target/theoria/gallery
```

The address bar names the story on screen: `?story=` and the story's full
name, form-encoded but with each `/` left as it is, so `Buttons/Primary
large` is `http://localhost:8081/?story=Buttons/Primary+large`. Each sidebar
entry links to its story, and the bare URL opens the story you had open
last. A browser test opens a story the same way, building the query with
`form_urlencoded::byte_serialize`; the gallery reads a `/` written as `%2F`
too, so that alone opens the story, and `.replace("%2F", "/")` after it
gives the exact `href` and address the gallery writes.

A package of its own, rather than an example in your components' crate,
builds none of your components natively just to start the CLI, and keeps
the CLI out of their test builds.

`cargo install --locked --git https://github.com/padamson/theoria
theoria-cli` puts a `theoria` on your PATH instead, at whatever commit
`main` had that day, which may not match the theoria your stories build on.

The generated app is its own workspace, so each run copies the `Cargo.lock`
of your crate's workspace into it, and the gallery builds on the versions
your crate does. A change to that lockfile needs a re-run of `serve`.

The generated app turns on your crate's `csr` feature, and that feature has
to pass `csr` on to theoria, the way any Leptos library forwards its runtime:

```toml
[dependencies]
theoria = { git = "https://github.com/padamson/theoria", optional = true }

[features]
stories = ["dep:theoria"]
csr = ["leptos/csr", "theoria?/csr"]   # "theoria/csr" if theoria is not optional
```

`mount()` is behind theoria's `csr`, so a crate that leaves this out fails
the gallery build with ``no method named `mount` ``. The app needs the
`wasm32-unknown-unknown` target and
[Trunk](https://trunkrs.dev).

## Agent skill

`skills/theoria-usage/` holds the start of an agent skill for writing
stories. It is hidden from installers until the API settles.

## Development

See [CLAUDE.md](CLAUDE.md) for development commands.

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (MSRV: 1.95)
- [prek](https://github.com/j178/prek) for pre-commit hooks: `cargo install prek && prek install --overwrite`
- For the gallery and its browser tests: `rustup target add wasm32-unknown-unknown`, `cargo install trunk`, and `cargo run --manifest-path crates/theoria-e2e/Cargo.toml --example install-browsers` for Chromium

### Build and test

```bash
cargo build
cargo nextest run
cargo run -p theoria-cli -- build --config theoria-e2e.toml
cargo nextest run --manifest-path crates/theoria-e2e/Cargo.toml --config-file .config/nextest.toml --run-ignored all
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
