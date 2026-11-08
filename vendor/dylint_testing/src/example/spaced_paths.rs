//! End-to-end regression for dependency paths that contain spaces.
//!
//! The test builds a disposable Cargo workspace under a spaced directory and
//! runs it through the real example-test path: `cargo rustc` verbose output,
//! wrapper-tolerant decoding, flag selection, and `compiletest`'s rustc
//! subprocess. A compile failure here means a flag argument lost its
//! boundaries between `run_tests` and the compiler.

use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use cargo_metadata::MetadataCommand;
use tempfile::{Builder, TempDir};

use crate::{ExampleTestContext, VarGuard, example_target, run_example_test, ui};

/// The package whose example the harness compiles.
const HOST_PACKAGE: &str = "spaced_host";

/// The example target the harness compiles.
const EXAMPLE_TARGET: &str = "spaced_example";

/// Build the spaced workspace, run its example through `compiletest`, and
/// assert that both dependency artefacts were produced.
///
/// This is the only unit test that populates the process-global
/// `LINKING_FLAGS` cache in `example.rs`; keep it first if more example
/// regressions are added.
#[test]
fn compiles_example_from_spaced_dependency_paths() -> Result<()> {
    let workspace = build_spaced_workspace()?;
    let target_dir = workspace.path().join("target dir");
    // The nested `cargo rustc` run inherits this, so every dependency path in
    // the recovered compiler arguments contains a space.
    let _target_dir = VarGuard::set("CARGO_TARGET_DIR", &target_dir);
    // `make test` builds this crate with `-Z force-unstable-if-unmarked`, and
    // the nested build inherits it. The fixture's dependencies would then be
    // marked unstable, and the example, compiled by `compiletest` as a
    // separate crate, would fail to load them at `E0658`. That is the same
    // interaction `test-doc` documents, so the fixture build opts out of the
    // inherited flags while the outer test process keeps them.
    let _rustflags = VarGuard::set("RUSTFLAGS", "");

    let metadata = MetadataCommand::new()
        .manifest_path(workspace.path().join("Cargo.toml"))
        .exec()
        .context("Could not load the spaced workspace metadata")?;
    let package = metadata
        .packages
        .iter()
        .find(|package| package.name == HOST_PACKAGE)
        .with_context(|| format!("Could not find `{HOST_PACKAGE}`"))?;
    let target = example_target(package, EXAMPLE_TARGET)?;

    let config = ui::Config::default();
    run_example_test(&ExampleTestContext {
        driver: Path::new("rustc"),
        metadata: &metadata,
        package,
        target: &target,
        config: &config,
    })?;

    // A compiletest pass proves rustc received intact `--extern` and
    // `-L dependency=` values. These assertions prove the paths really were
    // spaced and that no stale artefact satisfied the compile.
    expect_dependency_artefact(&target_dir, "libspaced_dep-")?;
    expect_dependency_artefact(&target_dir, "libspaced_transitive-")?;
    Ok(())
}

/// Create a Cargo workspace whose package and dependency paths contain spaces.
fn build_spaced_workspace() -> Result<TempDir> {
    let workspace = Builder::new()
        .prefix("dylint testing ")
        .tempdir()
        .context("Could not create the spaced workspace")?;
    for (path, contents) in FIXTURE_FILES {
        write_fixture_file(workspace.path(), path, contents)?;
    }
    Ok(workspace)
}

/// Write one fixture file, creating its parent directories.
fn write_fixture_file(root: &Path, relative: &str, contents: &str) -> Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create `{}`", parent.display()))?;
    }
    fs::write(&path, contents).with_context(|| format!("Could not write `{}`", path.display()))
}

/// Assert that a dependency artefact with `prefix` exists in the target dir.
fn expect_dependency_artefact(target_dir: &Path, prefix: &str) -> Result<()> {
    let deps = target_dir.join("debug").join("deps");
    let found = fs::read_dir(&deps)
        .with_context(|| format!("Could not read `{}`", deps.display()))?
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with(prefix));
    ensure!(found, "Expected `{prefix}*` under `{}`", deps.display());
    Ok(())
}

/// The fixture's files, relative to the workspace root.
///
/// The host package depends on `spaced_dep`, which re-exports a type from
/// `spaced_transitive`. Naming that re-exported type in the example forces
/// rustc to load the transitive crate's metadata, so a broken
/// `-L dependency=` value fails the compile as well as a broken `--extern`.
/// The example calls `u32::try_from`, which needs the 2021 edition prelude, so
/// a dropped `--edition=` flag also fails the compile.
const FIXTURE_FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        r#"[workspace]
resolver = "2"

[package]
name = "spaced_host"
version = "0.1.0"
edition = "2021"

[dependencies]
spaced_dep = { path = "spaced dep" }
"#,
    ),
    (
        "examples/spaced_example.rs",
        r"//! Compiled by the UI harness from a workspace under a spaced path.

fn main() {
    let gadget: spaced_dep::Gadget = spaced_dep::gadget();
    let answer = u32::try_from(spaced_dep::answer()).unwrap_or_default();
    let _ = (gadget, answer);
}
",
    ),
    (
        "spaced dep/Cargo.toml",
        r#"[package]
name = "spaced_dep"
version = "0.1.0"
edition = "2021"

[dependencies]
spaced_transitive = { path = "../spaced transitive" }
"#,
    ),
    (
        "spaced dep/src/lib.rs",
        r"//! Direct dependency reached through a spaced directory path.

pub use spaced_transitive::Gadget;

/// Return a gadget from the transitive dependency.
#[must_use]
pub fn gadget() -> Gadget { Gadget::default() }

/// Return the answer from the transitive dependency.
#[must_use]
pub fn answer() -> u32 { u32::from(spaced_transitive::ANSWER) }
",
    ),
    (
        "spaced transitive/Cargo.toml",
        r#"[package]
name = "spaced_transitive"
version = "0.1.0"
edition = "2021"
"#,
    ),
    (
        "spaced transitive/src/lib.rs",
        r"//! Transitive dependency reached through a spaced directory path.

/// A value carried across the dependency boundary.
#[derive(Default)]
pub struct Gadget {
    /// The gadget's value.
    pub value: u8,
}

/// The answer used by the example.
pub const ANSWER: u8 = 42;
",
    ),
];
