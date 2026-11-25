# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- `theoria`: the `Gallery` component (sidebar tree of stories, stage, live controls, show-code panel, Markdown autodocs), `Story` with `Story::new` and `with_description` (a Markdown description for a story built by hand, shown in the docs panel), `Stories` (what a crate's aggregator returns, with `gallery()` and, under `csr`, `mount()`, which also logs a panic to the browser console), `Arg` and `ArgControl` (a story's args: each one's name, its type as written, and its control, with `ChoiceControl` for a choice's options and selection), `StoryArg` (the sealed trait of types a `#[story]` parameter can have, each picking its control), `Bounded` (a number with optional bounds and step, drawn with a slider when it has both bounds), `Color` (any CSS color as text, drawn as a swatch and a text field), `Choice<T>` (one of a fixed list of labelled values, drawn as a menu), and the selected story persisted in `localStorage` under the `csr` feature
- `theoria-macros`: `#[story]`, which turns a function's `bool`, `f64`, `String`, `Bounded`, `Color`, and `Choice<T>` parameters into live controls and captures its body source and doc comment; a parameter typed `Signal<T>` over one of them gets the arg's signal, so an edit to it redraws only what reads it instead of running the whole body again; a key in the attribute that names no parameter is a compile error
- `theoria-cli`: the `theoria` binary, whose `serve`, `build`, and `generate` write a Trunk app under `target/theoria/gallery` from a `theoria.toml` and run Trunk on it, rebuilding on source edits; the app only calls `mount()` on the aggregator's `Stories`, so it shares one theoria and one Leptos with the stories wherever they come from, and each run copies the `Cargo.lock` of the stories crate's workspace into it, so the gallery builds on the versions that lockfile records; `theoria_cli::run`, which is all the binary's `main` calls, so a workspace with stories can build the CLI as a three-line package of its own, from the theoria commit in its `Cargo.lock`

[Unreleased]: https://github.com/padamson/theoria/commits/main
