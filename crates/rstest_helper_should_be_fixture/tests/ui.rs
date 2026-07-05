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
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};
use whitaker_common::test_support::{EnvVarGuard, run_test_runner};

// Internal test-only hook mirrored in the lint driver. It asks
// `check_crate_post` to append redacted, shape-only passive collection
// summaries for harness assertions without making the lint user-visible.
const COLLECTION_SUMMARY_ENV: &str = "WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY";
// The example harness lock coordinates separate nextest processes. Windows CI
// can legitimately hold it for several minutes, so only remove directories
// old enough to be abandoned by a crashed process.
const EXAMPLE_HARNESS_LOCK_STALE_AFTER: Duration = Duration::from_secs(30 * 60);
const EXAMPLE_HARNESS_LOCK_WAIT_LIMIT: Duration = Duration::from_secs(35 * 60);
const EXAMPLE_HARNESS_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
    assert!(
        summary.contains("callee=Builder::<'_>::build;records=1"),
        "{summary}"
    );
    assert!(summary.contains("callee=helper;records=1"), "{summary}");
    assert!(
        summary.contains("fingerprint=unsupported,fixture-local"),
        "{summary}"
    );
    assert!(
        summary.contains("fingerprint=fixture-local,fixture-local,const-path,const-path"),
        "{summary}"
    );
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

struct ExampleHarnessLock {
    path: PathBuf,
}

impl ExampleHarnessLock {
    fn acquire() -> io::Result<Self> {
        let path = std::env::temp_dir().join("rstest-helper-example-harness.lock");
        Self::acquire_at(path, EXAMPLE_HARNESS_LOCK_WAIT_LIMIT)
    }

    fn acquire_at(path: PathBuf, wait_limit: Duration) -> io::Result<Self> {
        let started_at = Instant::now();

        loop {
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    wait_for_example_harness_lock_release(&path, started_at, wait_limit)?;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for ExampleHarnessLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.path);
    }
}

fn wait_for_example_harness_lock_release(
    path: &std::path::Path,
    started_at: Instant,
    wait_limit: Duration,
) -> io::Result<()> {
    recover_stale_example_harness_lock(path)?;
    if started_at.elapsed() >= wait_limit {
        return Err(example_harness_lock_timeout(path));
    }
    std::thread::sleep(EXAMPLE_HARNESS_LOCK_POLL_INTERVAL);
    Ok(())
}

fn example_harness_lock_timeout(path: &std::path::Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        format!(
            "timed out waiting for example harness lock at {}",
            path.display(),
        ),
    )
}

fn recover_stale_example_harness_lock(path: &std::path::Path) -> io::Result<()> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let modified = metadata.modified()?;

    if example_harness_lock_is_stale(modified, SystemTime::now()) {
        remove_stale_example_harness_lock(path)?;
    }

    Ok(())
}

fn remove_stale_example_harness_lock(path: &std::path::Path) -> io::Result<()> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn example_harness_lock_is_stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age > EXAMPLE_HARNESS_LOCK_STALE_AFTER)
}

fn unique_summary_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);

    std::env::temp_dir().join(format!(
        "rstest-helper-collection-{}-{}.txt",
        std::process::id(),
        suffix,
    ))
}

#[test]
fn example_harness_lock_stale_policy_keeps_recent_locks() {
    let recent = SystemTime::now() - Duration::from_secs(60);

    assert!(!example_harness_lock_is_stale(recent, SystemTime::now()));
}

#[test]
fn example_harness_lock_stale_policy_rejects_abandoned_locks() {
    let old = SystemTime::now() - (EXAMPLE_HARNESS_LOCK_STALE_AFTER + Duration::from_secs(1));

    assert!(example_harness_lock_is_stale(old, SystemTime::now()));
}

#[test]
fn stale_lock_recovery_treats_missing_directory_as_released() {
    let path = unique_summary_path();

    recover_stale_example_harness_lock(&path)
        .expect("missing lock directory should be treated as already released");
}

#[test]
fn example_harness_lock_reports_active_contention_timeout() {
    let path = unique_summary_path();
    std::fs::create_dir(&path).expect("test lock directory should be created");

    let Err(error) = ExampleHarnessLock::acquire_at(path.clone(), Duration::ZERO) else {
        panic!("active lock contention should time out");
    };
    let _ = std::fs::remove_dir(&path);

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn stale_lock_removal_treats_missing_directory_as_released() {
    let path = unique_summary_path();

    remove_stale_example_harness_lock(&path)
        .expect("missing stale lock directory should be treated as already released");
}
