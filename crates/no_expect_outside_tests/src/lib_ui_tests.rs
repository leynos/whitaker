//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use camino::Utf8Path;
use dylint_testing::ui::Test;
use rstest::rstest;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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
    directory: &'a str,
    fixture_name: &'a str,
    label: &'a str,
    rustc_flags: &'a [&'a str],
    extern_crates: &'a [&'a str],
}

#[derive(Debug)]
struct DependencyRlib {
    path: PathBuf,
    modified: SystemTime,
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
    let harness_flags = test_harness_flags(spec.rustc_flags, spec.extern_crates)?;
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

fn run_fixture_harness_test(spec: &FixtureHarnessRun<'_>) {
    let crate_name = spec.crate_name;
    let directory = spec.directory;
    whitaker::testing::ui::run_with_runner(crate_name, directory, |_, dir| {
        run_fixture_under_test_harness(spec, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "{} regression should execute without diffs: \
             RunnerFailure {{ crate_name: \"{crate_name}\", \
             directory: \"{directory}\", message: {error:?} }}",
            spec.label
        )
    });
}

fn fixture_source_path(directory: &Utf8Path, fixture_name: &str) -> PathBuf {
    directory.as_std_path().join(format!("{fixture_name}.rs"))
}

fn test_harness_flags(extra_flags: &[&str], extern_crates: &[&str]) -> Result<Vec<String>, String> {
    let mut flags: Vec<String> = extra_flags.iter().map(|flag| (*flag).to_owned()).collect();
    flags.push("--edition=2024".to_owned());
    if extern_crates.is_empty() {
        return Ok(flags);
    }

    let deps_dir = dependency_directory()?;
    flags.extend([
        "-L".to_owned(),
        format!("dependency={}", deps_dir.display()),
    ]);
    for crate_name in extern_crates {
        let dependency_rlib = dependency_rlib(&deps_dir, crate_name)?;
        flags.extend([
            "--extern".to_owned(),
            format!("{crate_name}={}", dependency_rlib.display()),
        ]);
    }
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
    let mut matches = dependency_rlib_matches(deps_dir, &prefix)?;
    matches.sort_by(|left, right| {
        // Prefer the artefact produced most recently by the current build, then
        // fall back to a stable path ordering when timestamps tie.
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    matches
        .into_iter()
        .next()
        .map(|artifact| artifact.path)
        .ok_or_else(|| {
            format!(
                "failed to locate `{crate_name}` rlib in dependency directory {}",
                deps_dir.display()
            )
        })
}

fn dependency_rlib_matches(deps_dir: &Path, prefix: &str) -> Result<Vec<DependencyRlib>, String> {
    std::fs::read_dir(deps_dir)
        .map_err(|error| {
            format!(
                "failed to read dependency directory {}: {error}",
                deps_dir.display()
            )
        })?
        .map(|entry_result| {
            let entry = entry_result.map_err(|error| {
                format!(
                    "failed to read dependency entry in {}: {error}",
                    deps_dir.display()
                )
            })?;
            dependency_rlib_candidate(entry.path(), prefix)
        })
        .filter_map(|candidate| candidate.transpose())
        .collect()
}

fn dependency_rlib_candidate(
    path: PathBuf,
    prefix: &str,
) -> Result<Option<DependencyRlib>, String> {
    if !is_dependency_rlib(&path, prefix) {
        return Ok(None);
    }

    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("failed to read metadata for {}: {error}", path.display()))?;
    let modified = metadata.modified().map_err(|error| {
        format!(
            "failed to read modified time for {}: {error}",
            path.display()
        )
    })?;

    Ok(Some(DependencyRlib { path, modified }))
}

fn is_dependency_rlib(path: &Path, prefix: &str) -> bool {
    path.file_name().is_some_and(|name| {
        name.to_str()
            .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".rlib"))
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

#[rstest]
#[case(
    "examples",
    "pass_expect_in_tokio_path_module_harness",
    "Tokio path module with config",
    &["tokio"],
)]
#[case(
    "examples",
    "pass_expect_in_tokio_path_module_harness_no_config",
    "Tokio path module without config",
    &["tokio"],
)]
#[case(
    "ui",
    "pass_expect_in_file_backed_test_module",
    "File-backed cfg(test)",
    &[],
)]
fn fixture_compiles_under_test_harness(
    #[case] directory: &str,
    #[case] fixture_name: &str,
    #[case] label: &str,
    #[case] extern_crates: &[&str],
) {
    run_fixture_harness_test(&FixtureHarnessRun {
        crate_name: env!("CARGO_PKG_NAME"),
        directory,
        fixture_name,
        label,
        rustc_flags: &["--test"],
        extern_crates,
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

#[test]
fn tokio_expect_outside_tests_still_fails_in_non_test_code() {
    run_fixture_harness_test(&FixtureHarnessRun {
        crate_name: env!("CARGO_PKG_NAME"),
        directory: "examples",
        fixture_name: "fail_expect_in_tokio_crate_non_test_fn",
        label: "Tokio non-test function",
        rustc_flags: &["--test", "-D", "no_expect_outside_tests"],
        extern_crates: &["tokio"],
    });
}

#[cfg(test)]
#[path = "dependency_rlib_tests.rs"]
mod dependency_rlib_tests;
