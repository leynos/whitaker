//! Behaviour-driven tests for `whitaker check`.
//!
//! The scenarios pin the three externally visible promises: a ready
//! installation is linted with Whitaker's own options, an installation that is
//! missing is refused with the provisioning key rather than a Dylint status,
//! and a library path the user set is never second-guessed.

#![cfg(unix)]

use std::{
    cell::{Ref, RefCell},
    path::PathBuf,
    process::{Command, Output},
};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker::cli::check::argv::DylintOption;

// The shared fixture is written for both root-CLI behaviour suites; this one
// uses the installation and the stub log, not the staging-record helpers.
#[expect(
    dead_code,
    reason = "the shared fixture also serves the root-CLI behaviour suite"
)]
#[path = "behaviour_whitaker/support.rs"]
mod support;

/// The state a `check` scenario accumulates between steps.
#[derive(Default)]
struct CheckWorld {
    output: RefCell<Option<Output>>,
    fixture: RefCell<Option<support::BootstrapFixture>>,
    data_home: RefCell<Option<TempDir>>,
    library_path: RefCell<Option<TempDir>>,
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
            command.env("XDG_DATA_HOME", data_home.path());
        }
        if let Some(library_path) = self.library_path.borrow().as_ref() {
            command.env("DYLINT_LIBRARY_PATH", library_path.path());
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

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> CheckWorld { CheckWorld::default() }

#[given("a ready Whitaker installation")]
fn given_ready_installation(world: &CheckWorld) -> Result<(), String> {
    world
        .fixture
        .replace(Some(support::BootstrapFixture::new()?));
    Ok(())
}

#[given("no Whitaker installation")]
fn given_no_installation(world: &CheckWorld) -> Result<(), String> {
    let data_home = TempDir::new()
        .map_err(|error| format!("an empty data directory should be created: {error}"))?;
    world.data_home.replace(Some(data_home));
    Ok(())
}

#[given("a Dylint library path the user set with no Whitaker suite")]
fn given_user_library_path(world: &CheckWorld) -> Result<(), String> {
    world
        .fixture
        .replace(Some(support::BootstrapFixture::new()?));
    let library_path = TempDir::new()
        .map_err(|error| format!("an empty library path should be created: {error}"))?;
    world.library_path.replace(Some(library_path));
    Ok(())
}

#[when("I run whitaker check")]
fn when_run_check(world: &CheckWorld) -> Result<(), String> { world.run(&["check"]) }

#[when("I run whitaker check with \"{arguments}\"")]
fn when_run_check_with(world: &CheckWorld, arguments: &str) -> Result<(), String> {
    let mut invocation = vec!["check"];
    invocation.extend(arguments.split_ascii_whitespace());
    world.run(&invocation)
}

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

#[then("cargo dylint received the Whitaker suite options and the user arguments")]
fn then_dylint_received_whitaker_options(world: &CheckWorld) -> Result<(), String> {
    let mut expected: Vec<String> = vec!["dylint".to_owned()];
    for option in DylintOption::SET {
        let token = option
            .token()
            .into_string()
            .map_err(|_| format!("{option:?} must render as valid UTF-8"))?;
        expected.push(token);
    }
    expected.push("--".to_owned());
    expected.extend(["-p".to_owned(), "whitaker-common".to_owned()]);

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

#[then("cargo dylint was never started")]
fn then_dylint_never_started(world: &CheckWorld) -> Result<(), String> {
    let recorded = world.fixture()?.recorded();
    if recorded.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "a refused installation must not run Dylint, but the stub recorded {recorded:?}"
        ))
    }
}

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

#[scenario(path = "tests/features/whitaker_check.feature", index = 0)]
fn scenario_ready_installation_is_linted(world: CheckWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check.feature", index = 1)]
fn scenario_uninstalled_machine_is_refused(world: CheckWorld) { let _ = world; }

#[scenario(path = "tests/features/whitaker_check.feature", index = 2)]
fn scenario_user_library_path_is_refused(world: CheckWorld) { let _ = world; }
