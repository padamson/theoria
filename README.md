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

The `/` in a name nests the story in the sidebar. Your crate exposes a
function returning `Vec<theoria::Story>`, usually behind a `stories`
feature so production builds leave the stories out.

## Serving the gallery

Point a `theoria.toml` at that function:

```toml
[gallery]
crate = "my-components"            # the package name
path = "crates/my-components"      # relative to this file
aggregator = "my_components::all_stories"
features = ["stories"]
theoria_path = "../theoria/crates/theoria"
```

Then:

```bash
cargo install --locked --git https://github.com/padamson/theoria theoria-cli
theoria serve          # http://localhost:8081, rebuilt on every source edit
theoria build          # a static gallery under target/theoria/gallery
```

The generated app depends on theoria by path (`theoria_path`, default
`crates/theoria`), so for now the gallery needs a checkout of this repo
beside your crate. It also needs the `wasm32-unknown-unknown` target and
[Trunk](https://trunkrs.dev).

## Agent skill

`skills/theoria-usage/` holds the start of an agent skill for writing
stories. It is hidden from installers until the API settles.

## Development

See [CLAUDE.md](CLAUDE.md) for development commands.

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (MSRV: 1.95)
- [prek](https://github.com/j178/prek) for pre-commit hooks: `cargo install prek && prek install --overwrite`
- For the gallery and its browser tests: `rustup target add wasm32-unknown-unknown`, `cargo install trunk`, and `npx playwright@1.64.1 install chromium`

### Build and test

```bash
cargo build
cargo nextest run
cargo run -p theoria-cli -- build --config theoria-e2e.toml
cargo test --manifest-path crates/theoria-e2e/Cargo.toml
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
