//! UI harness for `no_unwrap_or_else_panic` fixtures.

use camino::Utf8Path;
use dylint_testing::ui::Test;
use rstest::rstest;
use std::path::Path;
use std::{fs, io};
use temp_env::with_vars_unset;
use whitaker_common::test_support::{
    env_test_guard, prepare_fixture, run_fixtures_with, run_test_runner,
};

/// Describes a single example-based regression to run under the test harness.
///
/// `name` is the example target name, `label` is the human-readable case name
/// used in panic messages, and `rustc_flags` supplies the extra harness flags
/// passed to `dylint_testing`.
struct ExampleHarnessRun<'a> {
    /// Example target name passed to `Test::example`.
    name: &'a str,
    /// Human-readable label used when a regression runner fails.
    label: &'a str,
    /// Extra `rustc` flags used for the harness invocation.
    rustc_flags: &'a [&'a str],
}

impl<'a> ExampleHarnessRun<'a> {
    /// Creates a run spec using the default `--test` harness flag.
    fn new(name: &'a str, label: &'a str) -> Self {
        Self {
            name,
            label,
            rustc_flags: &["--test"],
        }
    }

    /// Creates a run spec with caller-supplied rustc flags (no defaults
    /// applied).
    fn with_flags(name: &'a str, label: &'a str, rustc_flags: &'a [&'a str]) -> Self {
        Self {
            name,
            label,
            rustc_flags,
        }
    }
}

#[test]
fn ui() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "ui";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, dir| {
        run_fixtures(crate_name, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
        )
    });
}

/// Runs an example-based regression under the dylint UI test harness.
///
/// Applies `spec.rustc_flags` to the compilation and formats any failure
/// message using `spec.label`.
fn run_example_under_test_harness(spec: &ExampleHarnessRun<'_>) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(spec.name, || {
            let _guard = env_test_guard();
            with_vars_unset(
                [
                    "RUSTC_WRAPPER",
                    "RUSTC_WORKSPACE_WRAPPER",
                    "CARGO_BUILD_RUSTC_WRAPPER",
                ],
                || {
                    let mut test = Test::example(crate_name, spec.name);
                    test.rustc_flags(spec.rustc_flags);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "{} example regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}",
            spec.label
        )
    });
}

fn run_fixtures(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    run_fixtures_with(crate_name, directory, run_fixture)
}

fn run_fixture(crate_name: &str, directory: &Utf8Path, source: &Path) -> Result<(), String> {
    let fixture_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");
    let mut env = prepare_fixture(directory, source)
        .map_err(|error| format!("failed to prepare {fixture_name}: {error}"))?;

    let mut test = Test::src_base(crate_name, env.workdir());
    if let Some(config) = env.take_config() {
        test.dylint_toml(config);
    }
    if let Some(flags) = read_rustc_flags(source)
        .map_err(|error| format!("failed to load rustc flags for {fixture_name}: {error}"))?
    {
        test.rustc_flags(flags);
    }

    run_test_runner(fixture_name, || test.run())
}

/// Load optional rustc flags from a `.rustc-flags` sidecar file.
///
/// Each non-empty line is treated as whitespace-delimited flags. Lines may
/// include comments after `#`, which are stripped before parsing.
///
/// # Example
///
/// ```ignore
/// # use std::path::Path;
/// # use crate::read_rustc_flags;
/// // fixtures/case.rs has a fixtures/case.rustc-flags sidecar file containing:
/// // --test
/// // -C opt-level=1
/// let flags = read_rustc_flags(Path::new("fixtures/case.rs"))?;
/// assert_eq!(
///     flags,
///     Some(vec!["--test".into(), "-C".into(), "opt-level=1".into()])
/// );
/// # Ok::<(), std::io::Error>(())
/// ```
fn read_rustc_flags(source: &Path) -> io::Result<Option<Vec<String>>> {
    let path = source.with_extension("rustc-flags");
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)?;
    let flags: Vec<String> = contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .flat_map(|line| line.split_whitespace().map(str::to_owned))
        .collect();

    if flags.is_empty() {
        return Ok(None);
    }

    Ok(Some(flags))
}

#[rstest]
#[case("pass_unwrap_in_rstest_harness", "rstest")]
fn example_compiles_under_test_harness(#[case] name: &str, #[case] label: &str) {
    run_example_under_test_harness(&ExampleHarnessRun::new(name, label));
}

#[test]
fn rstest_unwrap_outside_tests_still_fails_in_non_harness_code() {
    run_example_under_test_harness(&ExampleHarnessRun::with_flags(
        "fail_unwrap_in_rstest_non_test_module",
        "rstest non-harness",
        &["--test", "-D", "no_unwrap_or_else_panic"],
    ));
}
