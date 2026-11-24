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

Then:

```bash
cargo install --locked --git https://github.com/padamson/theoria theoria-cli
theoria serve          # http://localhost:8081, rebuilt on every source edit
theoria build          # a static gallery under target/theoria/gallery
```

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
