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

/// Feature the nested suite build enables. It is the same build the installer
/// publishes: the experimental lint is compiled in, and each consumer's
/// `dylint.toml` decides whether it runs.
const SUITE_FEATURE: &str = "dylint-driver";

/// What a consumer's `dylint.toml` says about the experimental lint.
#[derive(Clone, Copy)]
enum OptIn {
    /// Lists the lint under `[whitaker] experimental`.
    Listed,
    /// Configures the lint's threshold but never lists it.
    Unlisted,
    /// Lists a name that is not a lint.
    Misspelt,
}

impl OptIn {
    /// Directory name under the state directory for this consumer.
    const fn directory(self) -> &'static str {
        match self {
            Self::Listed => "listed",
            Self::Unlisted => "unlisted",
            Self::Misspelt => "misspelt",
        }
    }

    /// The consumer's `dylint.toml`.
    fn dylint_config(self) -> String {
        let threshold = CONSUMER_DYLINT_CONFIG;
        match self {
            Self::Listed => format!("[whitaker]\nexperimental = [\"{LINT_NAME}\"]\n\n{threshold}"),
            Self::Unlisted => threshold.to_owned(),
            Self::Misspelt => {
                format!("[whitaker]\nexperimental = [\"{LINT_NAME}_typo\"]\n\n{threshold}")
            }
        }
    }
}

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
    let Some(root) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() else {
        panic!("the suite crate must have a workspace parent");
    };
    root.to_path_buf()
}

/// Return the private directory this test owns.
fn state_dir() -> PathBuf { workspace_root().join("target/tests/macro_aware_smoke") }

/// Clear stale state and create the directories the test needs.
fn prepare_state_dir() -> PathBuf {
    let directory = state_dir();
    match fs::remove_dir_all(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => panic!("failed to clear `{}`: {error}", directory.display()),
    }
    fs::create_dir_all(directory.join("library")).unwrap_or_else(|error| {
        panic!("failed to create the staging directory: {error}");
    });
    directory
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
        .unwrap_or_else(|error| panic!("failed to execute the nested suite build: {error}"));
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

/// Copy the cdylib under the toolchain-qualified name Dylint discovers.
fn stage_library(source: &Path, library_dir: &Path) -> PathBuf {
    let Some(path_name) = source.file_name() else {
        panic!("the suite cdylib must have a file name");
    };
    let file_name = path_name.to_string_lossy();
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

/// Write the fixed consumer crate beneath `consumer`, configured as `opt_in`.
fn write_consumer(consumer: &Path, opt_in: OptIn) {
    fs::create_dir_all(consumer.join("src"))
        .unwrap_or_else(|error| panic!("failed to create the consumer directory: {error}"));
    fs::write(consumer.join("Cargo.toml"), CONSUMER_MANIFEST)
        .unwrap_or_else(|error| panic!("failed to write the consumer manifest: {error}"));
    fs::write(consumer.join("dylint.toml"), opt_in.dylint_config())
        .unwrap_or_else(|error| panic!("failed to write the consumer lint configuration: {error}"));
    fs::write(consumer.join("src/lib.rs"), CONSUMER_SOURCE)
        .unwrap_or_else(|error| panic!("failed to write the consumer source: {error}"));
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
        .unwrap_or_else(|error| panic!("failed to execute cargo dylint: {error}"));
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
        .unwrap_or_else(|error| panic!("failed to execute the plain consumer build: {error}"));
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
fn prepare_consumer(state: &Path, opt_in: OptIn) -> (PathBuf, PathBuf) {
    let consumer = state.join(opt_in.directory()).join("consumer");
    let target = state.join(opt_in.directory()).join("target");
    write_consumer(&consumer, opt_in);
    (consumer, target)
}

/// A consumer that lists the lint runs it and is denied under `-D warnings`.
fn smoke_listed(state: &Path, library: &Path) {
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
fn smoke_unlisted(state: &Path, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::Unlisted);
    assert_declares_lints(&consumer, &target, library);
    smoke_diagnostics::assert_disabled_run(&lint_consumer(&consumer, &target, library));
}

/// A consumer that lists an unknown name fails with a message naming it.
fn smoke_misspelt(state: &Path, library: &Path) {
    let (consumer, target) = prepare_consumer(state, OptIn::Misspelt);
    let run = lint_consumer(&consumer, &target, library);
    smoke_diagnostics::assert_unknown_name_rejected(&run, &format!("{LINT_NAME}_typo"));
}

/// Build and stage the suite once, then exercise each opt-in state.
pub(crate) fn run_smoke() {
    let workspace = workspace_root();
    let state = prepare_state_dir();
    let library_dir = state.join("library");

    let library = build_suite_library(&workspace);
    stage_library(&library, &library_dir);

    smoke_listed(&state, &library_dir);
    smoke_unlisted(&state, &library_dir);
    smoke_misspelt(&state, &library_dir);
}
