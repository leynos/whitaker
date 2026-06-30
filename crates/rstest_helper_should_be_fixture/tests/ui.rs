//! UI harness for the `rstest_helper_should_be_fixture` lint.
//!
//! These fixtures execute the lint driver and keep the current
//! diagnostic-silent contract. The example harness asserts the driver-owned
//! collector records real call-site evidence, while the trybuild cases retain
//! compile-time coverage for the same source shapes without depending on
//! diagnostics that later roadmap tasks will introduce.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use dylint_testing::ui::Test;
use rstest::rstest;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Duration;
use whitaker_common::test_support::{env_test_guard, run_test_runner};

// Internal test-only hook mirrored in the lint driver. It asks
// `check_crate_post` to append passive collection summaries for harness
// assertions without making the lint user-visible.
const COLLECTION_SUMMARY_ENV: &str = "WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY";

#[rstest]
#[case("bootstrap_zero_diagnostic")]
#[case("collection_zero_diagnostic")]
fn example_compiles_without_diagnostics(#[case] example: &str) {
    run_example(example);
}

#[test]
fn example_harness_collects_call_site_evidence() {
    let summary_path = unique_summary_path();
    let _guard = EnvVarGuard::set(COLLECTION_SUMMARY_ENV, summary_path.as_os_str());

    run_example("collection_zero_diagnostic");

    let summary =
        std::fs::read_to_string(&summary_path).expect("collection summary should be written");
    let _ = std::fs::remove_file(&summary_path);

    assert!(summary.contains("callee_count=2"), "{summary}");
    assert!(summary.contains("record_count=2"), "{summary}");
    assert!(summary.contains("callee=Builder::<'_>::build;records=1"));
    assert!(summary.contains("callee=helper;records=1"));
}

#[test]
fn trybuild_fixtures_compile_without_diagnostics() {
    let cases = trybuild::TestCases::new();

    cases.pass("tests/ui/bootstrap_zero_diagnostic.rs");
    cases.pass("tests/ui/collection_zero_diagnostic.rs");
}

fn run_example(example: &str) {
    let _lock = ExampleHarnessLock::acquire();
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(example, || {
            let mut test = Test::example(crate_name, example);
            test.rustc_flags(["--test"]);
            test.run();
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
        )
    });
}

struct ExampleHarnessLock {
    path: PathBuf,
}

impl ExampleHarnessLock {
    fn acquire() -> Self {
        let path = std::env::temp_dir().join("rstest-helper-example-harness.lock");
        for _ in 0..300 {
            match std::fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => panic!("create example harness lock: {error}"),
            }
        }

        panic!("timed out waiting for example harness lock");
    }
}

impl Drop for ExampleHarnessLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.path);
    }
}

fn unique_summary_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "rstest-helper-collection-{}-{}.txt",
        std::process::id(),
        unique_summary_path as usize,
    ))
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &OsStr) -> Self {
        let _env_guard = env_test_guard();
        let previous = std::env::var_os(key);
        // SAFETY: `env_test_guard` serializes this environment mutation.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        let _env_guard = env_test_guard();
        match &self.previous {
            Some(previous) => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
                unsafe {
                    std::env::set_var(self.key, previous);
                }
            }
            None => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }
}
