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
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use dylint_testing::ui::Test;
use harness_lock::ExampleHarnessLock;
use rstest::rstest;
use whitaker_common::test_support::{child_test_path, run_in_child, run_test_runner};

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
    let summary = collect_summary(
        "example_harness_collects_call_site_evidence",
        "collection_zero_diagnostic",
    );

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
    fn run_example(&self, example: &str) {
        let crate_name = env!("CARGO_PKG_NAME");
        let directory = "examples";
        let lock_path = self.lock.path().display();
        whitaker_lint_core::testing::ui::run_with_runner(
            crate_name,
            directory,
            |runner_crate, _| {
                run_test_runner(example, || {
                    let mut test = Test::example(runner_crate, example);
                    test.rustc_flags(["--test"]);
                    test.run();
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
}

/// Runs `example` with the collection-summary variable pointed at a fresh
/// path, returning the appended summary text.
///
/// The lint driver reads the variable from its process environment, so the
/// run happens in a child process that re-runs the test named `test_fn` with
/// the variable set at spawn time. The child takes the harness lock for the
/// run; this process reads the summary after the child exits and removes the
/// file before returning, and no other run shares the fresh path.
fn collect_summary(test_fn: &str, example: &str) -> String {
    let summary_path = unique_summary_path();
    let Some(summary_value) = summary_path.to_str() else {
        panic!("the summary path should be valid UTF-8");
    };
    let test_path = child_test_path(module_path!(), test_fn);
    run_in_child(
        &test_path,
        &[(COLLECTION_SUMMARY_ENV, Some(summary_value))],
        || ExampleHarness::acquire().run_example(example),
    )
    .unwrap_or_else(|error| panic!("the summary child test should pass: {error}"));
    let summary = match std::fs::read_to_string(&summary_path) {
        Ok(summary) => summary,
        Err(error) => panic!("collection summary should be written: {error}"),
    };
    let _cleanup_result = std::fs::remove_file(&summary_path);
    summary
}

fn unique_summary_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("rstest-helper-{suffix}-{}.txt", std::process::id()))
}
