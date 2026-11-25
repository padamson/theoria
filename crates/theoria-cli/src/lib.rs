//! `theoria` — generate and serve a Leptos component gallery from a crate's
//! stories (the Storybook-style dev tool for theoria).
//!
//! It reads `theoria.toml`, **generates** a small Trunk app under
//! `target/theoria/gallery` that mounts the `theoria::Stories` the configured
//! crate's aggregator returns, then runs Trunk on it. Nothing to hand-write or
//! commit — the gallery harness is generated on every run.
//!
//! The `theoria` binary is [`run`] and nothing else, so a workspace with
//! stories can build the CLI itself, from the theoria commit in its
//! `Cargo.lock`: a small package that depends on `theoria-cli` (written the way the
//! workspace writes `theoria`), whose `main` is
//!
//! ```no_run
//! fn main() -> std::process::ExitCode {
//!     theoria_cli::run(std::env::args_os())
//! }
//! ```
//!
//! is `theoria`, run as `cargo run -p <that package> -- serve`.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use toml::{Table, Value};

#[derive(Parser)]
#[command(
    name = "theoria",
    version,
    about = "Generate and serve a Leptos component gallery"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
    /// Path to the theoria config file.
    #[arg(long, default_value = "theoria.toml", global = true)]
    config: PathBuf,
}

#[derive(Subcommand)]
enum Cmd {
    /// Generate the gallery harness and serve it with Trunk (hot reload).
    Serve {
        /// Port for the Trunk dev server.
        #[arg(long, default_value_t = 8081)]
        port: u16,
    },
    /// Generate the gallery harness and build it with Trunk.
    Build,
    /// Only generate the gallery harness (no Trunk).
    Generate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    gallery: GalleryConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GalleryConfig {
    /// Package name of the crate that exposes the stories (the dependency name).
    #[serde(rename = "crate")]
    krate: String,
    /// Path to that crate, relative to the config file.
    path: String,
    /// Fully-qualified fn returning `theoria::Stories`, e.g. `my_components::all_stories`.
    aggregator: String,
    /// Extra features to enable on the crate (csr is added automatically).
    #[serde(default)]
    features: Vec<String>,
}

/// Run the `theoria` command line on `args`, whose first item is the
/// program's name, as in [`std::env::args_os`].
///
/// It does what the `theoria` binary does: `--help`, `--version`, or a bad
/// argument prints and exits the process, the way clap does, and an error
/// is printed to stderr as `Error: ` and its chain of causes, and returned
/// as [`ExitCode::FAILURE`].
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match execute(&Cli::parse_from(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", report(&err));
            ExitCode::FAILURE
        }
    }
}

/// What `run` prints for an error: what a `main` returning `Result`
/// prints, which is anyhow's report, with each cause under the error and a
/// backtrace after them when `RUST_BACKTRACE` asks for one.
fn report(err: &anyhow::Error) -> String {
    format!("Error: {err:?}")
}

fn execute(cli: &Cli) -> Result<()> {
    // `absolute`, not `canonicalize`, whose Windows result is the verbatim
    // `\\?\C:\...` form; these paths go into the harness for cargo and Trunk.
    let config_path = std::path::absolute(&cli.config)
        .with_context(|| format!("resolve config path {}", cli.config.display()))?;
    let repo_root = config_path
        .parent()
        .context("config file has no parent directory")?
        .to_path_buf();
    let config = fs::read_to_string(&config_path)
        .with_context(|| format!("read config {}", config_path.display()))?;
    let config: Config = toml::from_str(&config)
        .with_context(|| format!("parse config {}", config_path.display()))?;

    // Per-crate output dir so different galleries (e.g. an app's components vs theoria's own
    // e2e fixtures) don't clobber each other.
    let gallery_dir = repo_root
        .join("target/theoria/gallery")
        .join(&config.gallery.krate);
    generate(&repo_root, &gallery_dir, &config.gallery)?;
    println!("theoria: generated gallery at {}", gallery_dir.display());

    match cli.command {
        Cmd::Generate => Ok(()),
        Cmd::Build => trunk(&gallery_dir, &["build"]),
        Cmd::Serve { port } => {
            let port = port.to_string();
            trunk(&gallery_dir, &["serve", "--port", &port])
        }
    }
}

/// Write the generated Trunk gallery crate (`Cargo.toml`, `index.html`,
/// `src/main.rs`, `Trunk.toml`) into `dir`, with the stories crate's
/// `Cargo.lock` beside them.
fn generate(repo_root: &Path, dir: &Path, g: &GalleryConfig) -> Result<()> {
    // The name becomes a bare key in the harness's `[dependencies]`.
    let is_name_char = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if g.krate.is_empty() || !g.krate.chars().all(is_name_char) {
        bail!(
            "`crate` must be a package name (letters, digits, `-`, `_`), got `{}`",
            g.krate
        );
    }
    let target_abs = repo_root.join(&g.path);
    if !target_abs.join("Cargo.toml").exists() {
        bail!("no crate found at {}", target_abs.display());
    }
    let harness = render(&target_abs, g);
    let lockfile = workspace_lockfile(&target_abs)?;

    fs::create_dir_all(dir.join("src")).context("create gallery dir")?;
    fs::write(dir.join("Cargo.toml"), harness.cargo_toml).context("write Cargo.toml")?;
    fs::write(dir.join("src/main.rs"), harness.main_rs).context("write src/main.rs")?;
    fs::write(dir.join("index.html"), GALLERY_INDEX_HTML).context("write index.html")?;
    fs::write(dir.join("Trunk.toml"), harness.trunk_toml).context("write Trunk.toml")?;
    if let Some(lockfile) = lockfile {
        fs::copy(&lockfile, dir.join("Cargo.lock"))
            .with_context(|| format!("copy {}", lockfile.display()))?;
    }
    Ok(())
}

/// The `Cargo.lock` the crate at `target` builds on: its workspace's, and
/// cargo says which workspace that is.
fn workspace_lockfile(target: &Path) -> Result<Option<PathBuf>> {
    let manifest = target.join("Cargo.toml");
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format", "plain"])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .context("failed to run `cargo locate-project`")?;
    if !output.status.success() {
        bail!(
            "cargo cannot tell which workspace {} is in:\n{}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim_end()
        );
    }
    let workspace_manifest =
        String::from_utf8(output.stdout).context("the workspace's path is not UTF-8")?;
    let lockfile = Path::new(workspace_manifest.trim_end()).with_file_name("Cargo.lock");
    Ok(lockfile.exists().then_some(lockfile))
}

/// The generated files that vary with the config.
struct Harness {
    cargo_toml: String,
    main_rs: String,
    trunk_toml: String,
}

/// `path` as a TOML string value, which escapes a Windows path's backslashes.
fn toml_path(path: &Path) -> Value {
    Value::String(path.display().to_string())
}

/// Render the harness for the stories crate at `target`.
fn render(target: &Path, g: &GalleryConfig) -> Harness {
    let mut features = g.features.clone();
    if !features.iter().any(|f| f == "csr") {
        features.push("csr".to_string());
    }
    let mut stories_dep = Table::new();
    stories_dep.insert("path".into(), toml_path(target));
    stories_dep.insert(
        "features".into(),
        Value::Array(features.into_iter().map(Value::String).collect()),
    );

    let cargo_toml = format!(
        "# GENERATED by `theoria` — do not edit; regenerated on each run.\n\
         [package]\n\
         name = \"theoria-gallery\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\
         publish = false\n\n\
         [[bin]]\n\
         name = \"theoria-gallery\"\n\
         path = \"src/main.rs\"\n\n\
         [dependencies]\n\
         {krate} = {stories_dep}\n\n\
         # Own workspace so it isn't absorbed by the parent repo's workspace.\n\
         [workspace]\n",
        krate = g.krate,
        stories_dep = Value::Table(stories_dep),
    );

    // The harness declares no theoria or leptos of its own: either could
    // resolve to a second crate, whose types would not match the stories
    // crate's. `mount()` is a method on the aggregator's `theoria::Stories`,
    // so the stories crate's theoria and leptos are the ones that mount.
    let main_rs = format!(
        "// GENERATED by `theoria` — do not edit.\n\
         // `mount` is on the aggregator's `theoria::Stories`, behind theoria's\n\
         // `csr` feature, which the stories crate's `csr` feature must enable.\n\
         fn main() {{\n    \
         {aggregator}().mount();\n\
         }}\n",
        aggregator = g.aggregator,
    );

    // Watch the stories crate so editing a component or its stories hot-reloads.
    // Trunk otherwise only watches this generated harness dir, so source edits
    // would go unnoticed. (Don't watch the harness dir itself — its target/dist
    // would cause a rebuild loop.)
    let trunk_toml = format!(
        "# GENERATED by `theoria` — do not edit.\n\
         [watch]\n\
         watch = {watch}\n",
        watch = Value::Array(vec![toml_path(&target.join("src"))]),
    );

    Harness {
        cargo_toml,
        main_rs,
        trunk_toml,
    }
}

fn trunk(dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("trunk")
        .current_dir(dir)
        .args(args)
        .status()
        .context("failed to run `trunk` (install it with `cargo install trunk`)")?;
    if !status.success() {
        bail!("`trunk {}` failed", args.join(" "));
    }
    Ok(())
}

const GALLERY_INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>theoria — component gallery</title>
    <link data-trunk rel="rust" data-wasm-opt="z" />
    <style>
      html, body { height: 100%; }
      body { margin: 0; font-family: -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; background: #faf7f1; color: #2c2a25; }
      /* Each column scrolls independently within the viewport — a long story
         list in the nav (or a tall stage/panel) never scrolls the others out
         of view; only that one column's content scrolls. */
      .theoria { display: flex; gap: 16px; padding: 16px 22px; height: 100vh; box-sizing: border-box; }
      .theoria-nav { flex: 0 0 210px; overflow-y: auto; height: 100%; }
      .theoria-nav ul { list-style: none; margin: 0; padding: 0; }
      .theoria-nav ul.collapsed { display: none; }
      /* Leaf rows: flat, full-width, subtle hover; the selected one is accented. */
      .theoria-nav button { display: flex; align-items: center; width: 100%; text-align: left; padding: 4px 8px; margin: 1px 0; border: none; border-radius: 6px; background: none; color: inherit; cursor: pointer; font: inherit; font-size: 13px; }
      .theoria-nav button:hover { background: #efe9dc; }
      .theoria-nav button.active { background: #5c6b4f; color: #fff; }
      /* Marks a leaf with a docs panel (description/args/show-code) — most
         plain stories have none, so this is the only way to tell from the list. */
      .theoria-docs-badge { margin-left: 8px; flex: 0 0 auto; color: #b3ab9a; }
      .theoria-nav button.active .theoria-docs-badge { color: #cfe0c4; }
      /* Group headers: muted, uppercase, click to collapse. */
      .theoria-group-label { padding: 8px 8px 2px; font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: #8a8275; cursor: pointer; user-select: none; }
      .theoria-group-label:hover { color: #5c6b4f; }
      .theoria-group-label .caret { font-size: 9px; color: #b3ab9a; }
      .theoria-stage { flex: 1 1 auto; background: #fff; border: 1px solid #e3dccb; border-radius: 12px; padding: 16px; min-height: 200px; overflow: auto; height: 100%; box-sizing: border-box; }
      /* Right-hand panel: Markdown description, controls, and show-code. */
      .theoria-panel { flex: 0 0 280px; display: flex; flex-direction: column; gap: 12px; overflow-y: auto; height: 100%; box-sizing: border-box; }
      .theoria-md { font-size: 13px; line-height: 1.55; color: #4a463d; }
      .theoria-md > :first-child { margin-top: 0; }
      .theoria-md h1, .theoria-md h2, .theoria-md h3 { margin: .5em 0 .3em; line-height: 1.2; }
      .theoria-md h1 { font-size: 17px; } .theoria-md h2 { font-size: 15px; } .theoria-md h3 { font-size: 13px; }
      .theoria-md ul, .theoria-md ol { margin: .3em 0; padding-left: 1.2em; }
      .theoria-md code { background: #efe9dc; padding: 1px 4px; border-radius: 4px; font-size: 12px; }
      .theoria-md a { color: #5c6b4f; }
      .theoria-controls { background: #fff; border: 1px solid #e3dccb; border-radius: 10px; overflow: hidden; font-size: 13px; }
      .theoria-controls .control-row { padding: 8px 10px; border-top: 1px solid #f0eadd; }
      .theoria-controls .control-row:first-child { border-top: none; }
      .theoria-controls .control-head { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 0 10px; margin-bottom: 4px; }
      .theoria-controls .control-name { min-width: 0; color: #2c2a25; font-weight: 600; overflow-wrap: anywhere; }
      .theoria-controls .control-type { min-width: 0; color: #5c6b4f; font-size: 12px; text-align: right; overflow-wrap: anywhere; }
      .theoria-controls .control-input input, .theoria-controls .control-input select { width: 100%; box-sizing: border-box; padding: 3px 6px; border: 1px solid #d8d0be; border-radius: 6px; font: inherit; }
      .theoria-controls .control-input input[type=checkbox] { width: auto; }
      .theoria-controls .control-input select { background: #fff; }
      .theoria-controls .control-input input[type=range] { flex: 1 1 6em; min-width: 0; padding: 0; border: none; }
      .theoria-bounded { display: flex; flex-wrap: wrap; gap: 4px 8px; align-items: center; }
      .theoria-bounded input[type=number] { flex: 1 1 4.5em; min-width: 0; }
      .theoria-color { display: flex; gap: 8px; align-items: center; }
      .theoria-swatch { position: relative; flex: none; width: 2.25em; height: 1.75em; box-sizing: border-box; border: 1px solid #d8d0be; border-radius: 6px; overflow: hidden; background: repeating-conic-gradient(#e3dccb 0 25%, #fff 0 50%) 0 0 / 10px 10px; }
      .theoria-swatch:focus-within { outline: 2px solid #5c6b4f; outline-offset: 1px; }
      .theoria-swatch span { position: absolute; inset: 0; }
      .theoria-controls .control-input .theoria-swatch input { position: absolute; inset: 0; height: 100%; padding: 0; border: none; opacity: 0; cursor: pointer; }
      .theoria-color input[type=text] { flex: 1; min-width: 0; }
      .theoria-code { font-size: 12px; }
      .theoria-code summary { cursor: pointer; color: #5c6b4f; font-weight: 600; }
      .theoria-code pre { margin: 8px 0 0; padding: 12px; background: #2c2a25; color: #f3eee3; border-radius: 10px; overflow: auto; }
    </style>
  </head>
  <body></body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn config(krate: &str, aggregator: &str) -> GalleryConfig {
        GalleryConfig {
            krate: krate.into(),
            path: format!("crates/{krate}"),
            aggregator: aggregator.into(),
            features: vec!["stories".into()],
        }
    }

    /// A crate cargo can load: `generate` asks cargo which workspace it is in.
    fn write_crate(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
                 [lib]\npath = \"lib.rs\"\n"
            ),
        )
        .unwrap();
    }

    fn table(src: &str) -> Table {
        toml::from_str(src).unwrap_or_else(|e| panic!("not TOML ({e}):\n{src}"))
    }

    fn rendered_foo() -> Harness {
        render(
            Path::new("/w/crates/foo"),
            &config("foo", "foo::all_stories"),
        )
    }

    fn rendered_theoria() -> Harness {
        render(
            Path::new("/w/crates/theoria"),
            &config("theoria", "theoria::stories"),
        )
    }

    fn watched(harness: &Harness) -> Vec<PathBuf> {
        table(&harness.trunk_toml)["watch"]["watch"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| PathBuf::from(v.as_str().unwrap()))
            .collect()
    }

    // Anything else would resolve outside the stories crate's lockfile, and
    // a theoria or leptos of its own would be a second crate whose types do
    // not match the stories crate's.
    #[test]
    fn the_stories_crate_is_the_harness_only_dependency() {
        let cargo = table(&rendered_foo().cargo_toml);
        let names: Vec<&String> = cargo["dependencies"].as_table().unwrap().keys().collect();
        assert_eq!(names, ["foo"]);
    }

    #[test]
    fn harness_mounts_the_aggregator_value() {
        let main = rendered_foo().main_rs;
        assert!(main.contains("    foo::all_stories().mount();\n"), "{main}");
    }

    #[test]
    fn harness_for_theoria_itself_has_one_theoria_with_its_features() {
        // A second `theoria` key would fail to parse here as a duplicate.
        let cargo = table(&rendered_theoria().cargo_toml);
        assert_eq!(
            cargo["dependencies"]["theoria"]["features"],
            Value::Array(vec!["stories".into(), "csr".into()])
        );
    }

    #[test]
    fn a_configured_csr_is_not_added_twice() {
        let mut g = config("foo", "foo::all_stories");
        g.features = vec!["csr".into()];
        let cargo = table(&render(Path::new("/w/crates/foo"), &g).cargo_toml);
        assert_eq!(
            cargo["dependencies"]["foo"]["features"],
            Value::Array(vec!["csr".into()])
        );
    }

    #[test]
    fn harness_adds_csr_after_the_stories_crate_features() {
        let cargo = table(&rendered_foo().cargo_toml);
        assert_eq!(
            cargo["dependencies"]["foo"]["features"],
            Value::Array(vec!["stories".into(), "csr".into()])
        );
    }

    #[test]
    fn harness_depends_on_the_stories_crate_by_its_path() {
        let cargo = table(&rendered_foo().cargo_toml);
        let path = cargo["dependencies"]["foo"]["path"].as_str().unwrap();
        assert_eq!(Path::new(path), Path::new("/w/crates/foo"));
    }

    #[test]
    fn harness_is_its_own_workspace() {
        let cargo = table(&rendered_foo().cargo_toml);
        assert_eq!(cargo.get("workspace"), Some(&Value::Table(Table::new())));
    }

    #[test]
    fn trunk_watches_the_stories_crate_source() {
        assert_eq!(
            watched(&rendered_foo()),
            [PathBuf::from("/w/crates/foo/src")]
        );
    }

    #[test]
    fn a_windows_path_survives_the_generated_toml() {
        let target = r"C:\Users\me\crates\foo";
        let harness = render(Path::new(target), &config("foo", "foo::all_stories"));
        let cargo = table(&harness.cargo_toml);
        assert_eq!(cargo["dependencies"]["foo"]["path"].as_str(), Some(target));
    }

    #[test]
    fn a_windows_path_survives_the_generated_watch_list() {
        let target = r"C:\Users\me\crates\foo";
        let harness = render(Path::new(target), &config("foo", "foo::all_stories"));
        assert_eq!(watched(&harness), [Path::new(target).join("src")]);
    }

    #[test]
    fn generate_writes_each_rendered_file_under_its_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        let dir = root.join("out");
        let g = config("foo", "foo::all_stories");

        generate(root, &dir, &g).unwrap();

        let harness = render(&root.join("crates/foo"), &g);
        for (file, expected) in [
            ("Cargo.toml", harness.cargo_toml),
            ("src/main.rs", harness.main_rs),
            ("Trunk.toml", harness.trunk_toml),
        ] {
            assert_eq!(
                std::fs::read_to_string(dir.join(file)).unwrap(),
                expected,
                "{file}"
            );
        }
    }

    #[test]
    fn generate_rejects_a_crate_that_is_not_a_package_name() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["my.components", "my components", ""] {
            let err =
                generate(tmp.path(), &tmp.path().join("out"), &config(name, "x::all")).unwrap_err();
            assert!(
                err.to_string().contains(&format!("got `{name}`")),
                "{name:?}: {err:#}"
            );
        }
    }

    #[test]
    fn generate_accepts_hyphens_and_underscores_in_the_crate_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["my-components", "my_components"], &[]);
        for name in ["my-components", "my_components"] {
            let result = generate(root, &root.join("out"), &config(name, "x::all"));
            assert!(result.is_ok(), "{name}: {result:?}");
        }
    }

    #[test]
    fn generate_writes_the_index_page() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        let dir = root.join("out");

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.join("index.html")).unwrap(),
            GALLERY_INDEX_HTML
        );
    }

    /// A workspace at `root` whose members are the crates named, each under
    /// `crates/`, and which leaves out the ones in `exclude`. A fixture with
    /// no workspace above it would take cargo up through the temp directory's
    /// parents, looking for one.
    fn write_workspace(root: &Path, members: &[&str], exclude: &[&str]) {
        let under_crates = |names: &[&str]| -> Vec<String> {
            names.iter().map(|n| format!("crates/{n}")).collect()
        };
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = {:?}\nexclude = {:?}\nresolver = \"3\"\n",
                under_crates(members),
                under_crates(exclude),
            ),
        )
        .unwrap();
        for name in members {
            write_crate(&root.join("crates").join(name), name);
        }
    }

    const WORKSPACE_LOCK: &str = "# the workspace's\n";

    fn harness_lockfile(dir: &Path) -> Option<String> {
        fs::read_to_string(dir.join("Cargo.lock")).ok()
    }

    #[test]
    fn generate_copies_the_workspace_lockfile_into_the_harness() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        fs::write(root.join("Cargo.lock"), WORKSPACE_LOCK).unwrap();
        let dir = root.join("out");

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(harness_lockfile(&dir).as_deref(), Some(WORKSPACE_LOCK));
    }

    #[test]
    fn generate_replaces_the_lockfile_an_earlier_build_left() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        fs::write(root.join("Cargo.lock"), WORKSPACE_LOCK).unwrap();
        let dir = root.join("out");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Cargo.lock"), "# an earlier build's\n").unwrap();

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(harness_lockfile(&dir).as_deref(), Some(WORKSPACE_LOCK));
    }

    #[test]
    fn a_lockfile_beside_a_workspace_member_is_not_the_one_copied() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        fs::write(root.join("Cargo.lock"), WORKSPACE_LOCK).unwrap();
        fs::write(
            root.join("crates/foo/Cargo.lock"),
            "# cargo never reads this\n",
        )
        .unwrap();
        let dir = root.join("out");

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(harness_lockfile(&dir).as_deref(), Some(WORKSPACE_LOCK));
    }

    #[test]
    fn a_crate_excluded_from_the_workspace_gives_the_harness_the_crates_own_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &["apart"]);
        fs::write(root.join("Cargo.lock"), WORKSPACE_LOCK).unwrap();
        write_crate(&root.join("crates/apart"), "apart");
        fs::write(root.join("crates/apart/Cargo.lock"), "# the crate's own\n").unwrap();
        let dir = root.join("out");

        generate(root, &dir, &config("apart", "apart::all_stories")).unwrap();

        assert_eq!(
            harness_lockfile(&dir).as_deref(),
            Some("# the crate's own\n")
        );
    }

    #[test]
    fn generate_writes_no_lockfile_when_the_workspace_has_none() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        let dir = root.join("out");

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(harness_lockfile(&dir), None);
    }

    // Removing it instead would have a workspace that has never been built
    // on its own resolve the whole gallery again on every run.
    #[test]
    fn generate_keeps_the_harness_lockfile_when_the_workspace_has_none() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        let dir = root.join("out");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Cargo.lock"), "# an earlier build's\n").unwrap();

        generate(root, &dir, &config("foo", "foo::all_stories")).unwrap();

        assert_eq!(
            harness_lockfile(&dir).as_deref(),
            Some("# an earlier build's\n")
        );
    }

    #[test]
    fn generate_fails_for_a_crate_its_workspace_does_not_list() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        write_crate(&root.join("crates/stray"), "stray");

        let err = generate(
            root,
            &root.join("out"),
            &config("stray", "stray::all_stories"),
        )
        .unwrap_err();

        // The workspace's manifest is named only in what cargo printed. Each
        // path is joined a component at a time, so on Windows it has the
        // separators cargo prints.
        let message = err.to_string();
        for manifest in [
            root.join("crates").join("stray").join("Cargo.toml"),
            root.join("Cargo.toml"),
        ] {
            assert!(
                message.contains(&manifest.display().to_string()),
                "no {} in: {message}",
                manifest.display()
            );
        }
    }

    #[test]
    fn generate_writes_nothing_for_a_crate_its_workspace_does_not_list() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_workspace(root, &["foo"], &[]);
        write_crate(&root.join("crates/stray"), "stray");
        let dir = root.join("out");

        generate(root, &dir, &config("stray", "stray::all_stories")).unwrap_err();

        assert!(!dir.exists(), "{} was written", dir.display());
    }

    #[test]
    fn generate_fails_when_the_target_crate_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        let err = generate(root, &root.join("out"), &config("ghost", "ghost::all")).unwrap_err();

        assert!(err.to_string().contains("ghost"), "{err:#}");
    }

    const GALLERY: &str =
        "[gallery]\ncrate = \"foo\"\npath = \"crates/foo\"\naggregator = \"foo::all_stories\"\n";

    /// `theoria --config <config> generate`, as a shell passes it.
    fn generate_args(config: &Path) -> [OsString; 4] {
        [
            "theoria".into(),
            "--config".into(),
            config.into(),
            "generate".into(),
        ]
    }

    #[test]
    fn run_generates_the_gallery_the_config_names_under_target() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // The directory is not named for the crate, so the gallery's can
        // only come from `crate`.
        write_workspace(root, &["bar"], &[]);
        fs::write(
            root.join("theoria.toml"),
            GALLERY.replace("crates/foo", "crates/bar"),
        )
        .unwrap();

        let code = run(generate_args(&root.join("theoria.toml")));

        assert_eq!(code, ExitCode::SUCCESS);
        let harness = render(&root.join("crates/bar"), &config("foo", "foo::all_stories"));
        let written = root.join("target/theoria/gallery/foo/src/main.rs");
        assert_eq!(fs::read_to_string(written).unwrap(), harness.main_rs);
    }

    #[test]
    fn run_returns_failure_when_the_command_fails() {
        let tmp = tempfile::tempdir().unwrap();

        let code = run(generate_args(&tmp.path().join("theoria.toml")));

        assert_eq!(code, ExitCode::FAILURE);
    }

    #[test]
    fn the_error_report_names_the_config_and_the_cause_under_it() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("theoria.toml");

        let err = execute(&Cli::parse_from(generate_args(&config))).unwrap_err();

        let not_found = fs::read_to_string(&config).unwrap_err();
        // A prefix, since what follows it is a backtrace when CI sets
        // `RUST_BACKTRACE`.
        let expected = format!(
            "Error: read config {}\n\nCaused by:\n    {not_found}",
            config.display()
        );
        let report = report(&err);
        assert!(report.starts_with(&expected), "{report}");
    }

    #[test]
    fn an_unknown_gallery_key_is_an_error_naming_it() {
        let err = toml::from_str::<Config>(&format!("{GALLERY}theoria_path = \"../theoria\"\n"))
            .err()
            .expect("theoria_path is rejected");

        // `message()`, not `to_string()`, which echoes the offending line.
        assert!(err.message().contains("`theoria_path`"), "{err}");
    }

    #[test]
    fn an_unknown_table_is_an_error_naming_it() {
        let err = toml::from_str::<Config>(&format!("{GALLERY}\n[other]\nx = 1\n"))
            .err()
            .expect("[other] is rejected");

        assert!(err.message().contains("`other`"), "{err}");
    }
}
