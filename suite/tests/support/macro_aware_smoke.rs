//! Harness behind the experimental macro-aware lint end-to-end smoke test.
//!
//! The registry tests prove the lint is wired into the suite's static tables;
//! the smoke target proves Dylint actually loads the built library and runs it
//! over a consumer crate. The harness builds the suite cdylib once, stages it
//! under a toolchain-qualified name, and runs `cargo dylint` over a fixed
//! consumer configured three ways: listing the lint, omitting it, and listing a
//! misspelt name.
//! Every nested Cargo call inherits the compiler wrapper so admission still
//! applies, but `RUSTFLAGS` is scoped so the outer gate's flags cannot change
//! the result.

use std::{
    env,
    io,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cargo_metadata::Message;

mod smoke_consumers;
mod smoke_diagnostics;
mod smoke_state;

use smoke_consumers::{CONSUMER_MANIFEST, CONSUMER_SOURCE, OptIn};
use smoke_state::{State, lock_nested_builds, prepare_state_dir, stage_library, write_consumer};

/// Fail the harness with `action` and the I/O error, without `expect`.
trait OrPanic<T> {
    /// Return the value, or panic naming the step that failed.
    fn or_panic(self, action: &str) -> T;
}

impl<T> OrPanic<T> for io::Result<T> {
    fn or_panic(self, action: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{action}: {error}"),
        }
    }
}

/// Package providing the aggregated lint library.
const SUITE_PACKAGE: &str = "whitaker_suite";
/// Lint exercised by this smoke test.
const LINT_NAME: &str = "macro_aware_cognitive_complexity";
/// Stable lint that proves the staged library loaded a real driver.
const STABLE_LINT: &str = "bumpy_road_function";

/// Feature the nested suite build enables. It is the same build the installer
/// publishes: the experimental lint is compiled in, and each consumer's
/// `dylint.toml` decides whether it runs.
const SUITE_FEATURE: &str = "dylint-driver";

/// Features of a local `--experimental` build, which starts every experimental
/// lint enabled whatever a consumer's `dylint.toml` lists.
const ENABLE_ALL_FEATURES: &str = "dylint-driver,experimental-enable-all";

/// Captured output of one nested command.
struct Run {
    /// Whether the command exited successfully.
    is_success: bool,
    /// Raw standard output.
    stdout: Vec<u8>,
    /// Rendered standard error.
    stderr: String,
}

impl Run {
    /// Capture one finished command's status and streams.
    fn from_output(output: Output) -> Self {
        Self {
            is_success: output.status.success(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

/// Return the workspace root above this crate's manifest directory.
fn workspace_root() -> PathBuf {
    let Some(root) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() else {
        panic!("the suite crate must have a workspace parent");
    };
    root.to_path_buf()
}

/// Return the pinned toolchain the suite library is built with.
fn pinned_toolchain() -> String {
    let Some(toolchain) = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
    else {
        panic!(
            "cannot determine the pinned toolchain: run this test through rustup's Cargo proxy so \
             it selects the toolchain from `rust-toolchain.toml`"
        );
    };
    toolchain
}

/// Build the suite library with this test binary's feature state.
fn build_suite_library(workspace: &Path, features: &str) -> PathBuf {
    let output = Command::new("cargo")
        .args([
            "build",
            "--frozen",
            "--lib",
            "--quiet",
            "--message-format=json",
            "--package",
            SUITE_PACKAGE,
            "--features",
            features,
        ])
        .current_dir(workspace)
        // `--frozen` already forbids network access; the variable makes that
        // visible to readers of the command.
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .or_panic("failed to execute the nested suite build");
    let run = Run::from_output(output);
    assert!(run.is_success, "nested suite build failed:\n{}", run.stderr);
    let Some(library) = find_cdylib(&run.stdout) else {
        panic!("cargo did not report a cdylib for the suite package");
    };
    library
}

/// Find the suite cdylib path in a captured cargo build stream.
fn find_cdylib(stdout: &[u8]) -> Option<PathBuf> {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let Ok(Message::CompilerArtifact(artefact)) = message else {
            continue;
        };
        if artefact.target.name != SUITE_PACKAGE || !artefact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artefact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Some(path.clone().into_std_path_buf());
        }
    }
    None
}

/// Run `cargo dylint` in the consumer with the staged library.
fn run_dylint(consumer: &Path, target: &Path, library: &Path, arguments: &[&str]) -> Run {
    let output = Command::new("cargo")
        .arg("dylint")
        .args(arguments)
        .current_dir(consumer)
        .env("CARGO_TARGET_DIR", target)
        .env("DYLINT_LIBRARY_PATH", library)
        .env("DYLINT_RUSTFLAGS", "-D warnings")
        .env("DYLINT_LOCALE", "en-GB")
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env("CARGO_NET_OFFLINE", "true")
        .env_remove("RUSTFLAGS")
        .output()
        .or_panic("failed to execute cargo dylint");
    Run::from_output(output)
}

/// Run a plain consumer build under `-D warnings`.
fn check_consumer(consumer: &Path, target: &Path) -> Run {
    let output = Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(consumer)
        .env("CARGO_TARGET_DIR", target)
        .env("RUSTUP_TOOLCHAIN", pinned_toolchain())
        .env("CARGO_NET_OFFLINE", "true")
        .env("RUSTFLAGS", "-D warnings")
        .env_remove("DYLINT_LIBRARY_PATH")
        .env_remove("DYLINT_RUSTFLAGS")
        .output()
        .or_panic("failed to execute the plain consumer build");
    Run::from_output(output)
}

/// Return whether `cargo dylint list` declared `name`.
fn lists_lint(run: &Run, name: &str) -> bool {
    String::from_utf8_lossy(&run.stdout).lines().any(|line| {
        line.trim_start()
            .strip_prefix(name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// Assert the staged library loads a real driver and declares the lints.
///
/// The experimental lint is declared whether or not the consumer opts in, so a
/// leftover `#[allow]` or `#[expect]` naming it never trips `unknown_lints`.
fn assert_declares_lints(consumer: &Path, target: &Path, library: &Path) {
    let listed = run_dylint(consumer, target, library, &["list", "--all"]);
    assert!(
        listed.is_success,
        "`cargo dylint list` failed, so no driver loaded:\n{}",
        listed.stderr
    );
    for name in [STABLE_LINT, LINT_NAME] {
        assert!(
            lists_lint(&listed, name),
            "the staged suite library must declare `{name}`:\n{}",
            String::from_utf8_lossy(&listed.stdout)
        );
    }
}

/// Run the consumer under Dylint with the JSON message stream.
fn lint_consumer(consumer: &Path, target: &Path, library: &Path) -> Run {
    run_dylint(
        consumer,
        target,
        library,
        &["--all", "--", "--message-format", "json"],
    )
}

/// Prepare one consumer and return its crate and target directories.
fn prepare_consumer(state: &State, opt_in: OptIn) -> (PathBuf, PathBuf) {
    let relative = Path::new(opt_in.directory()).join("consumer");
    write_consumer(state, &relative, opt_in);
    let consumer = state.path.join(relative);
    let target = state.path.join(opt_in.directory()).join("target");
    (consumer, target)
}

/// A consumer that lists the lint runs it and is denied under `-D warnings`.
fn smoke_listed(state: &State, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::Listed);
    assert_declares_lints(&consumer, &target, library);

    let checked = check_consumer(&consumer, &target);
    assert!(
        checked.is_success,
        "the consumer source must build under `-D warnings`:\n{}",
        checked.stderr
    );
    smoke_diagnostics::assert_enabled_run(&lint_consumer(&consumer, &target, library));
}

/// A consumer that configures but does not list the lint never sees it run.
fn smoke_unlisted(state: &State, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::Unlisted);
    assert_declares_lints(&consumer, &target, library);
    smoke_diagnostics::assert_disabled_run(&lint_consumer(&consumer, &target, library));
}

/// A leftover `#[expect]` on an unlisted lint is reported as unfulfilled.
fn smoke_unlisted_with_expect(state: &State, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::UnlistedWithExpect);
    let run = lint_consumer(&consumer, &target, library);
    smoke_diagnostics::assert_unfulfilled_expectation(&run);
}

/// An `experimental-enable-all` suite runs the lint for a consumer that lists nothing.
fn smoke_enable_all(state: &State, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::EnableAll);
    smoke_diagnostics::assert_enabled_run(&lint_consumer(&consumer, &target, library));
}

/// A consumer that lists an unknown name fails with a message naming it.
fn smoke_misspelt(state: &State, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::Misspelt);
    let run = lint_consumer(&consumer, &target, library);
    smoke_diagnostics::assert_unknown_name_rejected(&run, &format!("{LINT_NAME}_typo"));
}

/// Build and stage the suite once, then exercise each opt-in state.
pub(crate) fn run_smoke() {
    let workspace = workspace_root();
    let state = prepare_state_dir();
    let library_dir = state.path.join("library");
    let library_root = state
        .root
        .open_dir("library")
        .or_panic("failed to open the staging library directory");

    // The two builds share Cargo's target directory, so each is staged before
    // another process can overwrite it. The lock spans both builds and their
    // staging, and is released when the harness returns.
    let _build_lock = lock_nested_builds(&state);
    let library = build_suite_library(&workspace, SUITE_FEATURE);
    stage_library(&library, &library_dir, &library_root);

    smoke_listed(&state, &library_dir);
    smoke_unlisted(&state, &library_dir);
    smoke_unlisted_with_expect(&state, &library_dir);
    smoke_misspelt(&state, &library_dir);

    // The second build overwrites the first's output, which is already staged.
    let all_dir = state.path.join("library-enable-all");
    state
        .root
        .create_dir_all("library-enable-all")
        .or_panic("failed to create the enable-all staging directory");
    let all_root = state
        .root
        .open_dir("library-enable-all")
        .or_panic("failed to open the enable-all staging directory");
    let all_library = build_suite_library(&workspace, ENABLE_ALL_FEATURES);
    stage_library(&all_library, &all_dir, &all_root);
    smoke_enable_all(&state, &all_dir);

    // A failed run returns before this point and keeps its directory for
    // diagnosis; a passing one leaves nothing behind.
    drop(state.parent.remove_dir_all(&state.name));
}
