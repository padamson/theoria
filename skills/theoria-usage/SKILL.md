---
name: theoria-usage
description: Use when designing or building a Leptos component in a crate that has theoria stories, or when writing or reviewing theoria stories for Leptos components (`theoria::Story`, the `#[story]` attribute, a crate's `stories` feature and aggregator function), when setting up or debugging a `theoria.toml` and the `theoria serve` or `theoria build` gallery, or when deciding whether a check belongs in a story, a leptotes test, or a playwright-rust end-to-end test.
license: MIT OR Apache-2.0
metadata:
  version: "0.1.14"
  internal: true
---

# theoria-usage

<!--
Hidden from installers (`metadata.internal: true`) until the API settles;
nothing loads it yet. Bump `metadata.version` on every content change.
-->

## When to use

A Leptos component needs to be looked at in isolation, with its args
changed live, before or instead of wiring it into the app.

## How it works

A crate writes stories: `Story::new(name, || view! { ... })`, or a
function under `#[story(...)]` whose typed parameters (`bool`, `f64`,
`String`, `Bounded`, `Color`, `Choice<T>`, or a `Signal` of one) become
controls and whose doc comment becomes the description. It collects them in a
function returning `theoria::Stories`, behind a `stories` feature. `theoria.toml`
names that crate and function, and the `theoria` CLI generates a Trunk
app under `target/theoria/gallery` that calls `.mount()` on the
function's value, so the app and the stories share one theoria (and one
Leptos) wherever it comes from.

A story is for looking. A claim about the rendered HTML belongs in a
leptotes test, and a claim about behavior in the browser (clicks,
persistence, layout) in a playwright-rust test against a built gallery,
which opens each story by its URL (`?story=<full name>`).

- [`references/usage.md`](references/usage.md): the API and CLI surface.
- [`references/components.md`](references/components.md): the steps for
  building a component with its tests and stories, and the mistakes they
  catch.
