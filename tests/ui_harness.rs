//! Behaviour-driven tests for the Dylint UI harness helpers.

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
        Ok(Self(input.to_string()))
    }
}

#[fixture]
fn crate_name() -> RefCell<String> {
    RefCell::new("demo".to_string())
}

#[fixture]
fn directory() -> RefCell<Utf8PathBuf> {
    RefCell::new(Utf8PathBuf::from("ui"))
}

#[fixture]
fn runner_failure() -> RefCell<Option<String>> {
    RefCell::new(None)
}

#[fixture]
fn runner_invocations() -> RefCell<Vec<(String, Utf8PathBuf)>> {
    RefCell::new(Vec::new())
}

#[fixture]
fn harness_result() -> RefCell<Option<Result<(), HarnessError>>> {
    RefCell::new(None)
}

#[given("the harness has no crate name")]
fn clear_crate(crate_name: &RefCell<String>) {
    crate_name.borrow_mut().clear();
}

#[given("the harness is prepared for crate {name}")]
fn prepare_crate(crate_name: &RefCell<String>, name: String) {
    *crate_name.borrow_mut() = name;
}

#[given("the UI directory is {path}")]
fn prepare_directory(directory: &RefCell<Utf8PathBuf>, path: String) {
    *directory.borrow_mut() = Utf8PathBuf::from(path);
}

#[given("the runner will fail with message {message}")]
fn configure_failure(runner_failure: &RefCell<Option<String>>, message: String) {
    runner_failure.borrow_mut().replace(message);
}

#[when("the harness is executed")]
fn execute_harness(
    crate_name: &RefCell<String>,
    directory: &RefCell<Utf8PathBuf>,
    runner_failure: &RefCell<Option<String>>,
    runner_invocations: &RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: &RefCell<Option<Result<(), HarnessError>>>,
) {
    let crate_name_value = crate_name.borrow().clone();
    let directory_value = directory.borrow().clone();
    let failure = runner_failure.borrow().clone();

    let outcome = run_with_runner(&crate_name_value, directory_value, |name, path| {
        runner_invocations
            .borrow_mut()
            .push((name.to_string(), path.to_owned()));
        failure.clone().map_or(Ok(()), Err)
    });

    harness_result.borrow_mut().replace(outcome);
}

#[then("the runner is invoked with crate {expected} and directory {path}")]
fn assert_runner_invocation(
    runner_invocations: &RefCell<Vec<(String, Utf8PathBuf)>>,
    expected: StepString,
    path: StepString,
) {
    let borrow = runner_invocations.borrow();
    let Some(last) = borrow.last() else {
        panic!("the runner should be invoked");
    };

    let expected = expected.into_inner();
    let path = path.into_inner();

    assert_eq!(last.0.as_str(), expected.as_str());
    assert_eq!(last.1.as_str(), path.as_str());
}

#[then("the harness succeeds")]
fn assert_success(harness_result: &RefCell<Option<Result<(), HarnessError>>>) {
    let borrow = harness_result.borrow();
    match borrow.as_ref() {
        Some(Ok(())) => {}
        Some(Err(error)) => panic!("expected success but received {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[then("the harness reports an empty crate name error")]
fn assert_empty_crate_error(harness_result: &RefCell<Option<Result<(), HarnessError>>>) {
    let borrow = harness_result.borrow();
    match borrow.as_ref() {
        Some(Err(HarnessError::EmptyCrateName)) => {}
        Some(Ok(())) => panic!("expected an error but harness succeeded"),
        Some(Err(error)) => panic!("expected empty crate name error, found {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[then("the harness reports an absolute directory error containing {path}")]
fn assert_absolute_error(harness_result: &RefCell<Option<Result<(), HarnessError>>>, path: String) {
    let borrow = harness_result.borrow();
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
fn assert_runner_failure(
    harness_result: &RefCell<Option<Result<(), HarnessError>>>,
    snippet: StepString,
) {
    let borrow = harness_result.borrow();
    let snippet = snippet.into_inner();
    match borrow.as_ref() {
        Some(Err(HarnessError::RunnerFailure { message, .. })) => {
            assert!(message.contains(snippet.as_str()));
        }
        Some(Ok(())) => panic!("expected an error but harness succeeded"),
        Some(Err(error)) => panic!("expected a runner failure error, found {error}"),
        None => panic!("the harness should have been executed"),
    }
}

#[scenario(path = "tests/features/ui_harness.feature", index = 0)]
fn scenario_runs_successfully(
    crate_name: RefCell<String>,
    directory: RefCell<Utf8PathBuf>,
    runner_failure: RefCell<Option<String>>,
    runner_invocations: RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: RefCell<Option<Result<(), HarnessError>>>,
) {
    let _ = (
        crate_name,
        directory,
        runner_failure,
        runner_invocations,
        harness_result,
    );
}

#[scenario(path = "tests/features/ui_harness.feature", index = 1)]
fn scenario_rejects_empty_crate(
    crate_name: RefCell<String>,
    directory: RefCell<Utf8PathBuf>,
    runner_failure: RefCell<Option<String>>,
    runner_invocations: RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: RefCell<Option<Result<(), HarnessError>>>,
) {
    let _ = (
        crate_name,
        directory,
        runner_failure,
        runner_invocations,
        harness_result,
    );
}

#[scenario(path = "tests/features/ui_harness.feature", index = 2)]
fn scenario_rejects_absolute_directory(
    crate_name: RefCell<String>,
    directory: RefCell<Utf8PathBuf>,
    runner_failure: RefCell<Option<String>>,
    runner_invocations: RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: RefCell<Option<Result<(), HarnessError>>>,
) {
    let _ = (
        crate_name,
        directory,
        runner_failure,
        runner_invocations,
        harness_result,
    );
}

#[scenario(path = "tests/features/ui_harness.feature", index = 3)]
fn scenario_propagates_runner_failure(
    crate_name: RefCell<String>,
    directory: RefCell<Utf8PathBuf>,
    runner_failure: RefCell<Option<String>>,
    runner_invocations: RefCell<Vec<(String, Utf8PathBuf)>>,
    harness_result: RefCell<Option<Result<(), HarnessError>>>,
) {
    let _ = (
        crate_name,
        directory,
        runner_failure,
        runner_invocations,
        harness_result,
    );
}
