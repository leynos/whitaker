//! Behaviour-driven tests for the Dylint UI harness helpers.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
#[expect(
    unused_extern_crates,
    reason = "rustc_driver shim retained for Dylint driver harness tests"
)]
extern crate rustc_driver;

use std::{cell::RefCell, convert::Infallible};

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::testing::ui::{HarnessError, run_with_runner};

#[derive(Debug)]
struct StepString(String);

impl StepString {
    fn into_inner(self) -> String {
        self.0
    }
}

impl From<std::string::String> for StepString {
    fn from(value: std::string::String) -> Self {
        Self(value)
    }
}

impl From<StepString> for String {
    fn from(value: StepString) -> Self {
        value.0
    }
}

impl std::str::FromStr for StepString {
    type Err = Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Ok(Self(input.to_owned()))
    }
}

#[derive(Debug)]
struct HarnessWorld {
    crate_name: RefCell<String>,
    directory: RefCell<Utf8PathBuf>,
    runner_failure: RefCell<Option<String>>,
    runner_invocations: RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: RefCell<Option<Result<(), HarnessError>>>,
}

impl Default for HarnessWorld {
    fn default() -> Self {
        Self {
            crate_name: RefCell::new(String::from("demo")),
            directory: RefCell::new(Utf8PathBuf::from("ui")),
            runner_failure: RefCell::new(None),
            runner_invocations: RefCell::new(Vec::new()),
            harness_result: RefCell::new(None),
        }
    }
}

#[fixture]
fn harness_world() -> HarnessWorld {
    HarnessWorld::default()
}

#[given("the harness has no crate name")]
fn clear_crate(harness_world: &HarnessWorld) {
    harness_world.crate_name.borrow_mut().clear();
}

#[given("the harness is prepared for crate {name}")]
fn prepare_crate(harness_world: &HarnessWorld, name: String) {
    *harness_world.crate_name.borrow_mut() = name;
}

#[given("the UI directory is {path}")]
fn prepare_directory(harness_world: &HarnessWorld, path: String) {
    *harness_world.directory.borrow_mut() = Utf8PathBuf::from(path);
}

#[given("the runner will fail with message {message}")]
fn configure_failure(harness_world: &HarnessWorld, message: String) {
    harness_world.runner_failure.borrow_mut().replace(message);
}

#[when("the harness is executed")]
fn execute_harness(harness_world: &HarnessWorld) {
    let crate_name_value = harness_world.crate_name.borrow().clone();
    let directory_value = harness_world.directory.borrow().clone();
    let failure = harness_world.runner_failure.borrow().clone();

    let outcome = run_with_runner(&crate_name_value, directory_value, |name, path| {
        harness_world
            .runner_invocations
            .borrow_mut()
            .push((name.to_owned(), path.to_owned()));
        failure.clone().map_or(Ok(()), Err)
    });

    harness_world.harness_result.borrow_mut().replace(outcome);
}

#[then("the runner is invoked with crate {expected} and directory {path}")]
fn assert_runner_invocation(harness_world: &HarnessWorld, expected: StepString, path: StepString) {
    let borrow = harness_world.runner_invocations.borrow();
    let Some(last) = borrow.last() else {
        panic!("the runner should be invoked");
    };

    let expected_value = expected.into_inner();
    let path_value = path.into_inner();

    assert_eq!(last.0.as_str(), expected_value.as_str());
    assert_eq!(last.1.as_str(), path_value.as_str());
}

#[then("the harness succeeds")]
fn assert_success(harness_world: &HarnessWorld) {
    let borrow = harness_world.harness_result.borrow();
    match borrow.as_ref() {
        Some(Ok(())) => {}
        Some(Err(error)) => panic!("expected success but received {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[then("the harness reports an empty crate name error")]
fn assert_empty_crate_error(harness_world: &HarnessWorld) {
    let borrow = harness_world.harness_result.borrow();
    match borrow.as_ref() {
        Some(Err(HarnessError::EmptyCrateName)) => {}
        Some(Ok(())) => panic!("expected an error but harness succeeded"),
        Some(Err(error)) => panic!("expected empty crate name error, found {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[then("the harness reports an absolute directory error containing {path}")]
fn assert_absolute_error(harness_world: &HarnessWorld, path: String) {
    let borrow = harness_world.harness_result.borrow();
    match borrow.as_ref() {
        Some(Err(HarnessError::AbsoluteDirectory { directory })) => {
            assert_eq!(directory, &Utf8PathBuf::from(path));
        }
        Some(Ok(())) => panic!("expected an error but harness succeeded"),
        Some(Err(error)) => panic!("expected an absolute directory error, found {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[then("the harness reports a runner failure mentioning {snippet}")]
fn assert_runner_failure(harness_world: &HarnessWorld, snippet: StepString) {
    let borrow = harness_world.harness_result.borrow();
    let snippet_value = snippet.into_inner();
    match borrow.as_ref() {
        Some(Err(HarnessError::RunnerFailure { message, .. })) => {
            assert!(message.contains(snippet_value.as_str()));
        }
        Some(Ok(())) => panic!("expected an error but harness succeeded"),
        Some(Err(error)) => panic!("expected a runner failure error, found {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[scenario(path = "tests/features/ui_harness.feature", index = 0)]
fn scenario_runs_successfully(harness_world: HarnessWorld) {
    let _ = harness_world;
}

#[scenario(path = "tests/features/ui_harness.feature", index = 1)]
fn scenario_rejects_empty_crate(harness_world: HarnessWorld) {
    let _ = harness_world;
}

#[scenario(path = "tests/features/ui_harness.feature", index = 2)]
fn scenario_rejects_absolute_directory(harness_world: HarnessWorld) {
    let _ = harness_world;
}

#[scenario(path = "tests/features/ui_harness.feature", index = 3)]
fn scenario_propagates_runner_failure(harness_world: HarnessWorld) {
    let _ = harness_world;
}

#[cfg(windows)]
#[scenario(path = "tests/features/ui_harness.feature", index = 4)]
fn scenario_rejects_unc_directory(harness_world: HarnessWorld) {
    let _ = harness_world;
}

#[cfg(windows)]
#[scenario(path = "tests/features/ui_harness.feature", index = 5)]
fn scenario_rejects_drive_relative_directory(harness_world: HarnessWorld) {
    let _ = harness_world;
}
