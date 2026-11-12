//! Harness behind the experimental macro-aware lint end-to-end smoke test.
//!
//! The registry tests prove the lint is wired into the suite's static tables;
//! the smoke target proves Dylint actually loads the built library and runs it
//! over a consumer crate. The harness builds the suite cdylib for the feature
//! state the test binary was compiled with, stages it under a
//! toolchain-qualified name, and runs `cargo dylint` over a fixed consumer.
//! Every nested Cargo call inherits the compiler wrapper so admission still
//! applies, but `RUSTFLAGS` is scoped so the outer gate's flags cannot change
//! the result.

use std::{
    env,
    fs,
    io,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cargo_metadata::Message;

mod smoke_diagnostics;

/// Package providing the aggregated lint library.
const SUITE_PACKAGE: &str = "whitaker_suite";
/// Lint exercised by this smoke test.
const LINT_NAME: &str = "macro_aware_cognitive_complexity";
/// Stable lint that proves the staged library loaded a real driver.
const STABLE_LINT: &str = "bumpy_road_function";

/// Feature the nested suite build must enable for this test binary's state.
#[cfg(feature = "experimental-macro-aware-cognitive-complexity")]
const SUITE_FEATURE: &str = "experimental-macro-aware-cognitive-complexity";
/// Feature the nested suite build must enable for this test binary's state.
#[cfg(not(feature = "experimental-macro-aware-cognitive-complexity"))]
const SUITE_FEATURE: &str = "dylint-driver";

/// Whether this test binary was compiled with the lint enabled.
#[cfg(feature = "experimental-macro-aware-cognitive-complexity")]
const MACRO_AWARE_ENABLED: bool = true;
/// Whether this test binary was compiled with the lint enabled.
#[cfg(not(feature = "experimental-macro-aware-cognitive-complexity"))]
const MACRO_AWARE_ENABLED: bool = false;

/// A dependency-free consumer with one authored branch.
const CONSUMER_MANIFEST: &str = concat!(
    "[package]\n",
    "name = \"macro_aware_smoke_consumer\"\n",
    "version = \"0.0.0\"\n",
    "edition = \"2021\"\n",
    "\n",
    "[workspace]\n",
);

/// Consumer source whose single `if` scores one, above a zero limit.
const CONSUMER_SOURCE: &str = concat!(
    "//! Smoke consumer for the macro-aware cognitive complexity lint.\n",
    "\n",
    "/// Contains one authored branch, which exceeds a zero limit.\n",
    "pub fn authored_if(flag: bool) {\n",
    "    if flag {}\n",
    "}\n",
);

/// Consumer configuration that makes any authored branch a finding.
const CONSUMER_DYLINT_CONFIG: &str = concat!(
    "[macro_aware_cognitive_complexity]\n",
    "max_complexity = 0\n"
);

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
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the suite crate must have a workspace parent")
        .to_path_buf()
}

/// Return the private directory for this feature state.
fn state_dir() -> PathBuf {
    workspace_root()
        .join("target/tests/macro_aware_smoke")
        .join(if MACRO_AWARE_ENABLED {
            "experimental"
        } else {
            "default"
        })
}

/// Clear stale state and create the directories the test needs.
fn prepare_state_dir() -> PathBuf {
    let directory = state_dir();
    match fs::remove_dir_all(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to clear `{}`: {error}", directory.display()),
    }
    fs::create_dir_all(directory.join("library")).expect("failed to create the staging directory");
    fs::create_dir_all(directory.join("consumer/src"))
        .expect("failed to create the consumer directory");
    directory
}

/// Return the pinned toolchain the suite library is built with.
fn pinned_toolchain() -> String {
    env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .expect(
            "cannot determine the pinned toolchain: run this test through rustup's Cargo proxy so \
             it selects the toolchain from `rust-toolchain.toml`",
        )
}

/// Build the suite library with this test binary's feature state.
fn build_suite_library(workspace: &Path) -> PathBuf {
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
            SUITE_FEATURE,
        ])
        .current_dir(workspace)
        .output()
        .expect("failed to execute the nested suite build");
    let run = Run::from_output(output);
    assert!(run.is_success, "nested suite build failed:\n{}", run.stderr);
    find_cdylib(&run.stdout).expect("cargo did not report a cdylib for the suite package")
}

/// Find the suite cdylib path in a captured cargo build stream.
fn find_cdylib(stdout: &[u8]) -> Option<PathBuf> {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let Ok(Message::CompilerArtifact(artifact)) = message else {
            continue;
        };
        if artifact.target.name != SUITE_PACKAGE || !artifact.target.is_cdylib() {
            continue;
        }
        if let Some(path) = artifact
            .filenames
            .iter()
            .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        {
            return Some(path.clone().into_std_path_buf());
        }
    }
    None
}

/// Copy the cdylib under the toolchain-qualified name Dylint discovers.
fn stage_library(source: &Path, library_dir: &Path) -> PathBuf {
    let file_name = source
        .file_name()
        .expect("the suite cdylib must have a file name")
        .to_string_lossy();
    let suffix = env::consts::DLL_SUFFIX;
    let toolchain = pinned_toolchain();
    let target_name = file_name.strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let destination = library_dir.join(target_name);
    fs::copy(source, &destination).unwrap_or_else(|error| {
        panic!(
            "failed to stage `{}` as `{}`: {error}",
            source.display(),
            destination.display()
        )
    });
    destination
}

/// Write the fixed consumer crate beneath `consumer`.
fn write_consumer(consumer: &Path) {
    fs::write(consumer.join("Cargo.toml"), CONSUMER_MANIFEST)
        .expect("failed to write the consumer manifest");
    fs::write(consumer.join("dylint.toml"), CONSUMER_DYLINT_CONFIG)
        .expect("failed to write the consumer lint configuration");
    fs::write(consumer.join("src/lib.rs"), CONSUMER_SOURCE)
        .expect("failed to write the consumer source");
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
        .expect("failed to execute cargo dylint");
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
        .expect("failed to execute the plain consumer build");
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

/// Build, stage, and exercise the suite against the fixed consumer.
pub(crate) fn run_smoke() {
    let workspace = workspace_root();
    let state = prepare_state_dir();
    let consumer = state.join("consumer");
    let target = state.join("target");
    write_consumer(&consumer);

    let library = build_suite_library(&workspace);
    stage_library(&library, &state.join("library"));

    let listed = run_dylint(
        &consumer,
        &target,
        &state.join("library"),
        &["list", "--all"],
    );
    assert!(
        listed.is_success,
        "`cargo dylint list` failed, so no driver loaded:\n{}",
        listed.stderr
    );
    assert!(
        lists_lint(&listed, STABLE_LINT),
        "the staged suite library must load a real driver and declare `{STABLE_LINT}`:\n{}",
        String::from_utf8_lossy(&listed.stdout)
    );
    assert_eq!(
        lists_lint(&listed, LINT_NAME),
        MACRO_AWARE_ENABLED,
        "the declared lint set does not match this binary's feature state:\n{}",
        String::from_utf8_lossy(&listed.stdout)
    );

    let checked = check_consumer(&consumer, &target);
    assert!(
        checked.is_success,
        "the consumer source must build under `-D warnings`:\n{}",
        checked.stderr
    );

    let run = run_dylint(
        &consumer,
        &target,
        &state.join("library"),
        &["--all", "--", "--message-format", "json"],
    );
    if MACRO_AWARE_ENABLED {
        smoke_diagnostics::assert_enabled_run(&run);
    } else {
        smoke_diagnostics::assert_disabled_run(&run);
    }
}
