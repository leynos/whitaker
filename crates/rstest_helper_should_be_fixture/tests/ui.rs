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

use dylint_testing::ui::Test;
use rstest::rstest;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use whitaker_common::test_support::{EnvVarGuard, run_test_runner};

use harness_lock::ExampleHarnessLock;

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

    for expected in [
        "callee_count=3",
        "record_count=9",
        "callee=Builder::<'_>::build;records=2\nfingerprint=unsupported,fixture-local\nfingerprint=unsupported,fixture-local",
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
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/bootstrap_zero_diagnostic.rs");
    cases.pass("tests/ui/collection_zero_diagnostic.rs");
}
fn run_example(example: &str) {
    let _lock = ExampleHarnessLock::acquire().expect("example harness lock should be acquired");
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

fn unique_summary_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("rstest-helper-{suffix}-{}.txt", std::process::id()))
}
