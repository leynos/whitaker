//! End-to-end regression for dependency paths that contain spaces.
//!
//! The test builds a disposable Cargo workspace under a spaced directory and
//! runs it through the real example-test path: `cargo rustc` verbose output,
//! wrapper-tolerant decoding, flag selection, and `compiletest`'s rustc
//! subprocess. Because `compiletest` only exposes a string, a successful
//! compile alone would not prove which argument boundaries reached the
//! compiler. The test therefore installs a tiny recorder as the compiler: it
//! logs the argv it will execute with, expanding response files exactly as
//! rustc does, and then runs the real rustc. The assertion checks the exact
//! `--extern` and `-L` values at the compiler boundary while the compilation
//! itself still happens.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};
use cargo_metadata::MetadataCommand;
use tempfile::{Builder, TempDir};

use crate::{ExampleTestContext, VarGuard, example_target, run_example_test, ui};

mod fixtures;

use fixtures::{FIXTURE_FILES, RECORDER_MANIFEST, RECORDER_SOURCE};

/// The package whose example the harness compiles.
const HOST_PACKAGE: &str = "spaced_host";

/// The example target the harness compiles.
const EXAMPLE_TARGET: &str = "spaced_example";

/// Names the file the argv recorder appends to.
const ARGV_LOG_ENV: &str = "DYLINT_TESTING_ARGV_LOG";

/// Ends one recorded compiler invocation.
const INVOCATION_END: &str = "-- invocation end --";

/// Build the spaced workspace, compile its example through a recording
/// compiler, and assert the exact dependency flags rustc received.
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

    // Recover the production flags first so the boundary assertion checks the
    // exact values the harness selected. `run_example_test` then reuses the
    // same cached flags.
    let expected =
        DependencyFlags::from_recovered(super::linking_flags(&metadata, package, &target)?)?;
    let recorder = build_argv_recorder(workspace.path())?;
    let log = workspace.path().join("rustc-argv.log");
    let _log = VarGuard::set(ARGV_LOG_ENV, &log);

    let config = ui::Config::default();
    run_example_test(&ExampleTestContext {
        driver: recorder.as_path(),
        metadata: &metadata,
        package,
        target: &target,
        config: &config,
    })?;

    // The recorder forwarded every argument to rustc, so the assertions below
    // are about the real compiler boundary, not about an encoded string.
    assert_reaches_compiler(&log, &expected)?;
    expect_dependency_artefact(&target_dir, "libspaced_dep-")?;
    expect_dependency_artefact(&target_dir, "libspaced_transitive-")?;
    Ok(())
}

/// The dependency arguments whose boundaries the transport must preserve.
#[derive(Debug)]
struct DependencyFlags {
    /// The value following the first `--extern` flag.
    extern_value: String,

    /// The value following the first non-native `-L` flag.
    search_value: String,
}

impl DependencyFlags {
    /// Select the first `--extern` and `-L` values from the recovered flags.
    ///
    /// # Errors
    ///
    /// Returns an error if the recovered flags contain no such flag, a flag has
    /// no value, or either value is not spaced. A successful compile could
    /// otherwise pass without exercising the defect.
    fn from_recovered(flags: &[String]) -> Result<Self> {
        let extern_value = value_after(flags, "--extern")?;
        let search_value = value_after(flags, "-L")?;
        ensure!(
            extern_value.contains(' '),
            "`--extern` value is not spaced: {extern_value:?}"
        );
        ensure!(
            search_value.contains(' '),
            "`-L` value is not spaced: {search_value:?}"
        );
        Ok(Self {
            extern_value,
            search_value,
        })
    }
}

/// Return the argument that follows the first occurrence of `flag`.
///
/// # Errors
///
/// Returns an error if `flag` is absent or is the last argument.
fn value_after(flags: &[String], flag: &str) -> Result<String> {
    let index = flags
        .iter()
        .position(|candidate| candidate == flag)
        .with_context(|| format!("Recovered flags contain no `{flag}`"))?;
    flags
        .get(index + 1)
        .cloned()
        .with_context(|| format!("`{flag}` has no value"))
}

/// Compile the recorder and return its executable path.
///
/// The recorder lives in its own spaced workspace so its path and output
/// directory are also exercised with spaces. Cargo builds it, so compiler
/// admission still applies.
fn build_argv_recorder(workspace: &Path) -> Result<PathBuf> {
    write_fixture_file(workspace, "argv recorder/Cargo.toml", RECORDER_MANIFEST)?;
    write_fixture_file(workspace, "argv recorder/src/main.rs", RECORDER_SOURCE)?;
    let manifest = workspace.join("argv recorder").join("Cargo.toml");
    let target_dir = workspace.join("recorder target");
    let status = Command::new("cargo")
        .args(["build", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .context("Could not run Cargo to build the argv recorder")?;
    ensure!(status.success(), "Cargo could not build the argv recorder");
    let suffix = std::env::consts::EXE_SUFFIX;
    Ok(target_dir
        .join("debug")
        .join(format!("argv_recorder{suffix}")))
}

/// Assert that one recorded invocation carries the exact spaced values.
///
/// # Errors
///
/// Returns an error if the log cannot be read, records no invocation, or no
/// single invocation carries both the `--extern` and `-L` pairs.
fn assert_reaches_compiler(log: &Path, expected: &DependencyFlags) -> Result<()> {
    let contents =
        fs::read_to_string(log).with_context(|| format!("Could not read `{}`", log.display()))?;
    let received = split_invocations(&contents).iter().any(|args| {
        receives_pair(args, "--extern", &expected.extern_value)
            && receives_pair(args, "-L", &expected.search_value)
    });
    ensure!(
        received,
        "rustc did not receive `--extern {}` and `-L {}`; recorded: {contents}",
        expected.extern_value,
        expected.search_value
    );
    Ok(())
}

/// Split a recorder log into one argument vector per invocation.
///
/// Each argument occupies one line. Fixture paths contain no line feeds, so a
/// line boundary is unambiguous.
fn split_invocations(contents: &str) -> Vec<Vec<&str>> {
    let mut invocations = Vec::new();
    let mut current = Vec::new();
    for line in contents.lines() {
        if line == INVOCATION_END {
            invocations.push(std::mem::take(&mut current));
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        invocations.push(current);
    }
    invocations
}

/// Report whether `args` contains `flag` immediately followed by `value`.
fn receives_pair(args: &[&str], flag: &str, value: &str) -> bool {
    args.windows(2)
        .any(|window| window[0] == flag && window[1] == value)
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
