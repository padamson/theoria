# T9 — `theoria serve`: generate and serve the gallery

*Pulled by: viewing components without hand-writing or committing a gallery crate.*

## Story

As a Leptos developer, I want a CLI that discovers a crate's stories and
**generates and serves** a gallery, so that I never hand-write or commit a
gallery app — the Storybook `dev` experience for theoria.

## Vertical slices

- **T9.0 — generate harness**
  - [x] from `theoria.toml`, write `Cargo.toml` + `index.html` + `src/main.rs`
  - [x] the generated crate is gitignored (under `target/`); nothing to commit
- **T9.1 — build / serve via Trunk**
  - [x] `theoria serve` serves with hot reload; `theoria build` builds
- **T9.2 — watch the source crate (true hot reload)**
  - [x] the generated `Trunk.toml` watches the stories crate's `src/`, so editing
    a component or its `*.stories.rs` rebuilds and refreshes the browser
- **T9.3 — build on the stories crate's lockfile** (pulled by pyle and slp,
  whose galleries were built on versions no lockfile of theirs recorded)
  - [x] every run copies the `Cargo.lock` of the stories crate's workspace
    into the harness, replacing the one the last build left, so cargo keeps
    each package the two share at that lockfile's version
  - [x] cargo says which workspace that is, so a crate excluded from the
    workspace around it gets its own lockfile
  - [x] a crate its workspace neither lists nor excludes is refused
- **T9.4 — build the CLI from the theoria commit the stories use** (pulled by pyle,
  whose CI read theoria's commit out of `Cargo.lock` to install the CLI,
  and by slp and pyle, whose installed `theoria` was older than the
  library their stories built on)
  - [x] `theoria-cli` is a library too: `theoria_cli::run(args)` is the
    whole CLI, and the binary's `main` only calls it
  - [x] a workspace with stories builds the CLI as a three-line package of
    its own, on its own lockfile, with no native build of its components

## Notes / refs

- Lives in `crates/theoria-cli` (lib `theoria_cli`, bin `theoria`); native
  orchestration, no Leptos.
- The harness declares no theoria of its own: it calls `mount()` on the
  aggregator's `Stories`, so it builds with the theoria the stories crate
  has.
