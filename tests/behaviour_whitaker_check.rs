//! Behaviour-driven tests for `whitaker check`.
//!
//! The scenarios pin the externally visible promises: a ready installation is
//! linted with Whitaker's own options, a missing installation is refused with
//! the provisioning key rather than a Dylint status when `--no-install` is
//! given, and a library path the user set replaces the libraries Dylint loads.
//!
//! A user-managed path is inspected in both cases. When a bootstrap has been
//! recorded, the user path replaces only the libraries: the bootstrap still
//! supplies the Clippy snapshot and the driver. When no bootstrap has been
//! recorded, the user path is the whole installation, so `check` requires it
//! to name exactly one Whitaker suite library and refuses the path otherwise.
//! `check` always selects `--lib=whitaker_suite`, so Dylint's own failure to
//! load it would look like a lint result.

#![cfg(unix)]

use std::{
    cell::{Ref, RefCell},
    path::PathBuf,
    process::{Command, Output},
};

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker_installer::{
    crate_name::CrateName,
    resolution::SUITE_CRATE,
    stager::Stager,
    test_support::TEST_DATA_HOME_ENV,
    test_utils::FIXTURE_TOOLCHAIN,
};

// The shared fixture is written for both root-CLI behaviour suites. The
// helpers only the root-CLI suite needs live in `behaviour_whitaker/root_cli.rs`,
// which this suite does not include, so nothing here is declared unused.
#[path = "behaviour_whitaker/support.rs"]
mod support;

/// The state a `check` scenario accumulates between steps.
#[derive(Default)]
struct CheckWorld {
    output: RefCell<Option<Output>>,
    fixture: RefCell<Option<support::BootstrapFixture>>,
    data_home: RefCell<Option<TempDir>>,
    library_path: RefCell<Option<TempDir>>,
    recording_cargo: RefCell<Option<RecordingCargo>>,
}

/// A `cargo` that records its invocation, so a scenario without an
/// installation can still prove the command was never started.
struct RecordingCargo {
    /// The directory holding the stub, for `PATH`.
    dir: TempDir,
    /// The file the stub appends to, which must stay absent when unused.
    log: PathBuf,
}

impl RecordingCargo {
    /// Create the stub.
    ///
    /// # Errors
    ///
    /// Returns a description of the first filesystem step that failed.
    fn new() -> Result<Self, String> {
        let dir = TempDir::new()
            .map_err(|error| format!("a stub directory should be created: {error}"))?;
        let script = "#!/bin/sh\nprintf '%s\\n' \"$@\" >>\"$STUB_LOG\"\nexit 0\n";
        write_executable(&dir.path().join("cargo"), script)?;
        Ok(Self {
            log: dir.path().join("cargo.log"),
            dir,
        })
    }

    /// Whether the stub was executed at all.
    fn ran(&self) -> bool { self.log.exists() }
}

impl CheckWorld {
    /// The output of the last invocation.
    fn recorded_output(&self) -> Result<Ref<'_, Output>, String> {
        Ref::filter_map(self.output.borrow(), Option::as_ref)
            .map_err(|_| "the check process output should be recorded".to_owned())
    }

    /// The installation fixture, when a scenario installed one.
    fn fixture(&self) -> Result<Ref<'_, support::BootstrapFixture>, String> {
        Ref::filter_map(self.fixture.borrow(), Option::as_ref)
            .map_err(|_| "an installation fixture should be present".to_owned())
    }

    /// Run `whitaker check` with `arguments` under the scenario's environment.
    fn run(&self, arguments: &[&str]) -> Result<(), String> {
        let mut command = if let Some(fixture) = self.fixture.borrow().as_ref() {
            fixture.command()?
        } else {
            let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
            // An inherited library path would select the user-managed
            // branch, which is a different scenario.
            command.env_remove("DYLINT_LIBRARY_PATH");
            command
        };
        if let Some(data_home) = self.data_home.borrow().as_ref() {
            command
                .env("XDG_DATA_HOME", data_home.path())
                // The platform data directory ignores `XDG_DATA_HOME` on
                // macOS, and a debug binary honours this override on every
                // platform. Set both, with an isolated home, so a ready
                // installation on the host cannot answer for a scenario that
                // means "nothing is installed".
                .env("HOME", data_home.path())
                .env(TEST_DATA_HOME_ENV, data_home.path().join("whitaker"));
        }
        if let Some(library_path) = self.library_path.borrow().as_ref() {
            command.env("DYLINT_LIBRARY_PATH", library_path.path());
        }
        if let Some(cargo) = self.recording_cargo.borrow().as_ref() {
            command
                .env("PATH", path_with(&cargo.dir)?)
                .env("STUB_LOG", &cargo.log);
        }
        command.args(arguments);
        let output = command
            .current_dir(workspace_root())
            .output()
            .map_err(|error| format!("the whitaker check process should start: {error}"))?;
        self.output.replace(Some(output));
        Ok(())
    }
}

/// Provide the scenario world fixture.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> CheckWorld { CheckWorld::default() }

/// Script a ready Whitaker installation.
#[given("a ready Whitaker installation")]
fn given_ready_installation(world: &CheckWorld) -> Result<(), String> {
    world
        .fixture
        .replace(Some(support::BootstrapFixture::new()?));
    Ok(())
}

/// Script a machine whose data directory holds no installation.
#[given("no Whitaker installation")]
fn given_no_installation(world: &CheckWorld) -> Result<(), String> { without_installation(world) }

/// Give the scenario an empty data directory, so nothing is installed.
fn without_installation(world: &CheckWorld) -> Result<(), String> {
    let data_home = TempDir::new()
        .map_err(|error| format!("an empty data directory should be created: {error}"))?;
    world.data_home.replace(Some(data_home));
    Ok(())
}

/// Script a user path with no suite and no recorded installation.
#[given("a Dylint library path the user set with no Whitaker suite")]
fn given_user_library_path_without_suite(world: &CheckWorld) -> Result<(), String> {
    // No installation at all, so this empty path is the whole installation and
    // cannot name the suite `check` asks Dylint for.
    without_installation(world)?;
    let library_path = TempDir::new()
        .map_err(|error| format!("an empty library path should be created: {error}"))?;
    world.library_path.replace(Some(library_path));
    Ok(())
}

/// Script a user path holding two suite libraries for different toolchains.
#[given("a Dylint library path the user set with two Whitaker suites")]
fn given_user_library_path_with_two_suites(world: &CheckWorld) -> Result<(), String> {
    // Two libraries matching one suite make Dylint refuse to choose with
    // status `1`, which looks like a lint result, so the path must be refused
    // before Dylint starts.
    let library_path = TempDir::new()
        .map_err(|error| format!("an empty library path should be created: {error}"))?;
    stage_suite_for(&library_path, FIXTURE_TOOLCHAIN)?;
    stage_suite_for(&library_path, "nightly-2026-05-29")?;
    world.library_path.replace(Some(library_path));
    Ok(())
}

/// Script a user path holding the suite for the fixture toolchain.
#[given("a Dylint library path the user set")]
fn given_user_library_path(world: &CheckWorld) -> Result<(), String> {
    let library_path = TempDir::new()
        .map_err(|error| format!("an empty library path should be created: {error}"))?;
    stage_suite(&library_path)?;
    world.library_path.replace(Some(library_path));
    Ok(())
}

/// Stage a Whitaker suite library in `directory`, as an installation does.
///
/// `check` always selects `--lib=whitaker_suite`, so a library path that does
/// not hold the suite is refused before Dylint can exit `1` for it.
fn stage_suite(directory: &TempDir) -> Result<(), String> {
    stage_suite_for(directory, FIXTURE_TOOLCHAIN)
}

/// Stage one Whitaker suite library built for `toolchain`.
fn stage_suite_for(directory: &TempDir, toolchain: &str) -> Result<(), String> {
    let filename = Stager::new(Utf8PathBuf::from("unused"), toolchain)
        .staged_filename(&CrateName::from(SUITE_CRATE));
    std::fs::write(directory.path().join(filename), b"")
        .map_err(|error| format!("the suite file should be created: {error}"))
}

/// Provide a `cargo` stub that records its invocation for the no-launch check.
#[given("a cargo command that records its invocation")]
fn given_recording_cargo(world: &CheckWorld) -> Result<(), String> {
    world.recording_cargo.replace(Some(RecordingCargo::new()?));
    Ok(())
}

/// Run `whitaker check` with no extra arguments.
#[when("I run whitaker check")]
fn when_run_check(world: &CheckWorld) -> Result<(), String> { world.run(&["check"]) }

/// Run `whitaker check` with `arguments` from the feature file.
#[when("I run whitaker check with \"{arguments}\"")]
fn when_run_check_with(world: &CheckWorld, arguments: &str) -> Result<(), String> {
    let mut invocation = vec!["check"];
    invocation.extend(arguments.split_ascii_whitespace());
    world.run(&invocation)
}

/// Require the process to have exited with `expected`.
#[then("the exit code is {expected:i32}")]
fn then_exit_code(world: &CheckWorld, expected: i32) -> Result<(), String> {
    let actual = world.recorded_output()?.status.code();
    if actual == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "expected process exit code {expected}, found {actual:?}, stderr: {}",
            String::from_utf8_lossy(&world.recorded_output()?.stderr)
        ))
    }
}

/// Require `cargo dylint` to have received the fixed options and user arguments.
#[then("cargo dylint received the Whitaker suite options and the user arguments")]
fn then_dylint_received_whitaker_options(world: &CheckWorld) -> Result<(), String> {
    // The tokens are literal on purpose: deriving them from the production
    // option set would let that set drop an option without this test noticing.
    let expected: Vec<String> = [
        "dylint",
        "--lib=whitaker_suite",
        "--no-metadata",
        "--",
        "-p",
        "whitaker-common",
    ]
    .map(str::to_owned)
    .to_vec();

    let forwarded = world.fixture()?.forwarded_arguments();
    if forwarded == expected {
        Ok(())
    } else {
        Err(format!(
            "cargo dylint should receive Whitaker's options, its separator, then the user's \
             arguments: {forwarded:?}"
        ))
    }
}

/// Require that `cargo dylint` was never started.
///
/// The scenario observes cargo either through the installed fixture's stub or
/// through the recording cargo a no-installation scenario installs.
#[then("cargo dylint was never started")]
fn then_dylint_never_started(world: &CheckWorld) -> Result<(), String> {
    if let Some(fixture) = world.fixture.borrow().as_ref() {
        let recorded = fixture.recorded();
        return if recorded.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "a refused installation must not run Dylint, but the stub recorded {recorded:?}"
            ))
        };
    }
    let ran = world
        .recording_cargo
        .borrow()
        .as_ref()
        .ok_or_else(|| "the scenario should observe cargo one way or another".to_owned())?
        .ran();
    if ran {
        Err("a refused check must not start cargo at all".to_owned())
    } else {
        Ok(())
    }
}

/// Require `cargo dylint` to have loaded the library path the user set.
#[then("cargo dylint was given the library path the user set")]
fn then_dylint_used_the_user_library_path(world: &CheckWorld) -> Result<(), String> {
    let expected = world
        .library_path
        .borrow()
        .as_ref()
        .map(|path| path.path().display().to_string())
        .ok_or_else(|| "the scenario should set a library path".to_owned())?;
    match world.fixture()?.library_path() {
        Some(actual) if actual == expected => Ok(()),
        other => Err(format!(
            "cargo dylint should receive the user's library path {expected}, got {other:?}"
        )),
    }
}

/// Require standard error to report the reason key `key`.
#[then("stderr reports the reason key \"{key}\"")]
fn then_stderr_reports_key(world: &CheckWorld, key: &str) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!("whitaker: error[{key}]:");
    if stderr.contains(&expected) {
        Ok(())
    } else {
        Err(format!("stderr should report {expected}, got {stderr:?}"))
    }
}

/// The workspace root, so a run resolves the same package the tests do.
fn workspace_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")) }

/// Put `dir` first on `PATH`, keeping the inherited entries after it.
///
/// # Errors
///
/// Returns an error when the inherited `PATH` cannot be joined.
fn path_with(dir: &TempDir) -> Result<std::ffi::OsString, String> {
    let mut entries = vec![dir.path().to_path_buf()];
    entries.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(entries).map_err(|error| format!("the stub PATH should be valid: {error}"))
}

/// Write an executable script, creating its directory.
///
/// # Errors
///
/// Returns a description of the first filesystem step that failed.
fn write_executable(path: &std::path::Path, contents: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, contents)
        .map_err(|error| format!("the stub should be written: {error}"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("the stub should be executable: {error}"))
}

/// Bind the ready installation linted scenario.
#[scenario(path = "tests/features/whitaker_check.feature", index = 0)]
fn scenario_ready_installation_is_linted(world: CheckWorld) { let _ = world; }

/// Bind the uninstalled machine refused scenario.
#[scenario(path = "tests/features/whitaker_check.feature", index = 1)]
fn scenario_uninstalled_machine_is_refused(world: CheckWorld) { let _ = world; }

/// Bind the user path without the suite refused scenario.
#[scenario(path = "tests/features/whitaker_check.feature", index = 2)]
fn scenario_user_library_path_is_refused(world: CheckWorld) { let _ = world; }

/// Bind the user path overriding the recorded libraries scenario.
#[scenario(path = "tests/features/whitaker_check.feature", index = 3)]
fn scenario_user_library_path_overrides(world: CheckWorld) { let _ = world; }

/// Bind the user path with two suites refused scenario.
#[scenario(path = "tests/features/whitaker_check.feature", index = 4)]
fn scenario_user_library_path_is_ambiguous(world: CheckWorld) { let _ = world; }
