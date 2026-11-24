//! UI harness for the `rstest_helper_should_be_fixture` lint.
//!
//! These fixtures execute the lint driver and keep the current
//! diagnostic-silent contract. The example harness asserts the driver-owned
//! collector records real call-site evidence, while the trybuild cases retain
//! compile-time coverage for the same source shapes without depending on
//! diagnostics that later roadmap tasks will introduce.
#![cfg(feature = "dylint-driver")]
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use dylint_testing::ui::Test;
use harness_lock::ExampleHarnessLock;
use rstest::rstest;
use whitaker_common::test_support::run_test_runner;

// Internal test-only hook mirrored in the lint driver. It asks
// `check_crate_post` to append redacted, shape-only passive collection
// summaries for harness assertions without making the lint user-visible.
const COLLECTION_SUMMARY_ENV: &str = "WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY";

#[path = "ui/harness_lock.rs"]
mod harness_lock;
#[path = "ui/lock_model.rs"]
mod lock_model;
#[rstest]
#[case("bootstrap_zero_diagnostic")]
#[case("collection_zero_diagnostic")]
fn example_compiles_without_diagnostics(#[case] example: &str) {
    ExampleHarness::acquire().run_example(example);
}
#[test]
fn example_harness_collects_call_site_evidence() {
    let summary = ExampleHarness::acquire().collect_summary("collection_zero_diagnostic");

    for expected in [
        "callee_count=3",
        "record_count=9",
        concat!(
            "callee=Builder::<'_>::build;records=2\n",
            "fingerprint=unsupported,fixture-local\n",
            "fingerprint=unsupported,fixture-local",
        ),
        "callee=helper;records=2",
        "callee=nested_helper;records=5",
        "fingerprint=unsupported,fixture-local",
        "fingerprint=fixture-local,fixture-local,const-path,const-path",
        "fingerprint=fixture-local,const-lit,const-path,const-path",
    ] {
        assert!(summary.contains(expected), "{summary}");
    }
    assert!(!summary.contains("literal"), "{summary}");
}
#[test]
fn collection_summary_paths_are_fresh_per_call() {
    assert_ne!(unique_summary_path(), unique_summary_path());
}
#[test]
fn trybuild_fixtures_compile_without_diagnostics() {
    let _lock = ExampleHarnessLock::acquire()
        .expect("example harness lock should serialize trybuild fixture compilation");
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/bootstrap_zero_diagnostic.rs");
    cases.pass("tests/ui/collection_zero_diagnostic.rs");
}
/// RAII fixture that holds the example-harness lock for its whole lifetime.
///
/// Constructing it acquires [`ExampleHarnessLock`], and it is only released when
/// the fixture drops. Because every example run — and any summary read — happens
/// through a borrow of the fixture, the lock spans the entire critical section
/// (env-var setup, compilation, summary read, and cleanup). This stops callers
/// from re-implementing the acquire/set/run/read/remove dance and accidentally
/// releasing the lock mid-assertion, which would let a concurrent run append to
/// the same summary path.
struct ExampleHarness {
    lock: ExampleHarnessLock,
}

impl ExampleHarness {
    fn acquire() -> Self {
        match ExampleHarnessLock::acquire() {
            Ok(lock) => Self { lock },
            Err(error) => panic!("example harness lock should be acquired: {error}"),
        }
    }

    /// Compiles and runs one example while the lock is held.
    fn run_example(&self, example: &str) { self.run_example_with(example, None); }

    /// Compiles and runs one example with the collection-summary variable
    /// pointing at `summary_path`, when given.
    ///
    /// The path reaches the lint driver through the harness, so this process's
    /// environment is unchanged.
    fn run_example_with(&self, example: &str, summary_path: Option<&Path>) {
        let crate_name = env!("CARGO_PKG_NAME");
        let directory = "examples";
        let lock_path = self.lock.path().display();
        whitaker_lint_core::testing::ui::run_with_runner(
            crate_name,
            directory,
            |runner_crate, _| {
                run_test_runner(example, || {
                    example_test(runner_crate, example, summary_path).run();
                })
            },
        )
        .unwrap_or_else(|error| {
            panic!(
                "UI tests should execute without diffs: RunnerFailure {{ crate_name: \
                 \"{crate_name}\", directory: \"{directory}\", lock: \"{lock_path}\", message: \
                 {error} }}"
            )
        });
    }

    /// Runs `example` with the collection-summary env var pointed at a fresh
    /// path, returning the appended summary text.
    ///
    /// The scoped env override and the harness lock are both held across the
    /// run and the read, and the summary file is removed before returning, so
    /// no concurrently scheduled run can append to the same path mid-read.
    fn collect_summary(&self, example: &str) -> String {
        let summary_path = unique_summary_path();
        self.run_example_with(example, Some(&summary_path));
        let summary = match std::fs::read_to_string(&summary_path) {
            Ok(summary) => summary,
            Err(error) => panic!("collection summary should be written: {error}"),
        };
        let _cleanup_result = std::fs::remove_file(&summary_path);
        summary
    }
}

/// Build the `--test` example run, pointing the collection-summary variable at
/// `summary_path` when given.
fn example_test(crate_name: &str, example: &str, summary_path: Option<&Path>) -> Test {
    let mut test = Test::example(crate_name, example);
    test.rustc_flags(["--test"]);
    if let Some(path) = summary_path {
        test.env(COLLECTION_SUMMARY_ENV, path.as_os_str());
    }
    test
}

fn unique_summary_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("rstest-helper-{suffix}-{}.txt", std::process::id()))
}
