# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- `theoria`: the `Gallery` component (sidebar tree of stories, stage, live controls, show-code panel, Markdown autodocs), `Story` with `Story::new`, `ArgControl`, and the selected story persisted in `localStorage` under the `csr` feature
- `theoria-macros`: `#[story]`, which turns a function's `bool`, `f64`, and `String` parameters into live controls and captures its body source and doc comment
- `theoria-cli`: the `theoria` binary, whose `serve`, `build`, and `generate` write a Trunk app under `target/theoria/gallery` from a `theoria.toml` and run Trunk on it, rebuilding on source edits

[Unreleased]: https://github.com/padamson/theoria/commits/main
