//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use camino::Utf8Path;
use dylint_testing::ui::Test;
use rstest::rstest;
use std::path::{Path, PathBuf};
use temp_env::with_vars_unset;
use whitaker_common::test_support::{env_test_guard, prepare_fixture, run_test_runner};

/// Describes a single example-based regression to run under the test harness.
struct ExampleHarnessRun<'a> {
    name: &'a str,
    label: &'a str,
    rustc_flags: &'a [&'a str],
}

impl<'a> ExampleHarnessRun<'a> {
    fn new(name: &'a str, label: &'a str) -> Self {
        Self {
            name,
            label,
            rustc_flags: &["--test"],
        }
    }

    fn with_flags(name: &'a str, label: &'a str, rustc_flags: &'a [&'a str]) -> Self {
        Self {
            name,
            label,
            rustc_flags,
        }
    }
}

/// Describes a fixture-based regression to run under the test harness.
struct FixtureHarnessRun<'a> {
    crate_name: &'a str,
    fixture_name: &'a str,
    rustc_flags: &'a [&'a str],
}

fn run_example_under_test_harness(spec: &ExampleHarnessRun<'_>) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(spec.name, || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
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

fn run_fixture_under_test_harness(
    spec: &FixtureHarnessRun<'_>,
    directory: &Utf8Path,
) -> Result<(), String> {
    let source = fixture_source_path(directory, spec.fixture_name);
    let mut env = prepare_fixture(directory, &source)
        .map_err(|error| format!("failed to prepare {}: {error}", spec.fixture_name))?;
    let harness_flags = test_harness_flags(spec.rustc_flags)?;
    let harness_flag_refs: Vec<_> = harness_flags.iter().map(String::as_str).collect();

    run_test_runner(spec.fixture_name, || {
        let _guard = env_test_guard();
        with_vars_unset(
            [
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "CARGO_BUILD_RUSTC_WRAPPER",
            ],
            || {
                let mut test = Test::src_base(spec.crate_name, env.workdir());
                if let Some(config) = env.take_config() {
                    test.dylint_toml(config);
                }
                test.rustc_flags(harness_flag_refs.as_slice());
                test.run();
            },
        );
    })
}

fn fixture_source_path(directory: &Utf8Path, fixture_name: &str) -> PathBuf {
    directory.as_std_path().join(format!("{fixture_name}.rs"))
}

fn test_harness_flags(extra_flags: &[&str]) -> Result<Vec<String>, String> {
    let deps_dir = dependency_directory()?;
    let tokio_rlib = dependency_rlib(&deps_dir, "tokio")?;
    let mut flags: Vec<String> = extra_flags.iter().map(|flag| (*flag).to_owned()).collect();
    flags.extend([
        "--edition=2024".to_owned(),
        "-L".to_owned(),
        format!("dependency={}", deps_dir.display()),
        "--extern".to_owned(),
        format!("tokio={}", tokio_rlib.display()),
    ]);
    Ok(flags)
}

fn dependency_directory() -> Result<PathBuf, String> {
    let test_binary = std::env::current_exe()
        .map_err(|error| format!("failed to locate current test binary: {error}"))?;
    test_binary.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "test binary has no parent directory: {}",
            test_binary.display()
        )
    })
}

fn dependency_rlib(deps_dir: &Path, crate_name: &str) -> Result<PathBuf, String> {
    let prefix = format!("lib{crate_name}-");
    let mut matches: Vec<_> = std::fs::read_dir(deps_dir)
        .map_err(|error| {
            format!(
                "failed to read dependency directory {}: {error}",
                deps_dir.display()
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_str()
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".rlib"))
            })
        })
        .collect();
    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        format!(
            "failed to locate `{crate_name}` rlib in dependency directory {}",
            deps_dir.display()
        )
    })
}

#[rstest]
#[case("pass_expect_in_tokio_test_harness", "Tokio")]
#[case(
    "pass_expect_in_tokio_nonstandard_module_harness",
    "Tokio non-standard module"
)]
#[case("pass_expect_in_rstest_harness", "rstest")]
fn example_compiles_under_test_harness(#[case] example_name: &str, #[case] label: &str) {
    run_example_under_test_harness(&ExampleHarnessRun::new(example_name, label));
}

#[test]
fn tokio_path_loaded_module_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    let spec = FixtureHarnessRun {
        crate_name,
        fixture_name: "pass_expect_in_tokio_path_module_harness",
        rustc_flags: &["--test"],
    };

    whitaker::testing::ui::run_with_runner(crate_name, directory, |_, dir| {
        run_fixture_under_test_harness(&spec, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "Tokio path module regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

#[test]
fn rstest_expect_outside_tests_still_fails_in_non_harness_code() {
    run_example_under_test_harness(&ExampleHarnessRun::with_flags(
        "fail_expect_in_rstest_non_test_module",
        "rstest non-harness",
        &["--test", "-D", "no_expect_outside_tests"],
    ));
}
