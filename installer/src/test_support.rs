//! Test-only installer hooks shared by behavioural suites.
//!
//! These constants are consumed by behavioural tests and by debug-only helper
//! modules that simulate staged output without invoking a nested workspace
//! build.

pub use whitaker_common::test_support::env_test_guard;

/// Guard held while a test serializes process-wide environment mutations.
///
/// This alias matches the concrete guard returned by [`env_test_guard`].
pub type EnvTestGuard = parking_lot::ReentrantMutexGuard<'static, ()>;

/// Environment variable used by behavioural tests to request synthetic suite
/// staging in debug binaries.
pub const TEST_STAGE_SUITE_ENV: &str = "WHITAKER_INSTALLER_TEST_STAGE_SUITE";

/// Environment variable naming a Clippy snapshot manifest that debug
/// installer binaries use instead of the embedded one.
///
/// Integration tests point it at a small fixture snapshot and pass the
/// matching bundle with `--clippy-snapshot-bundle`, so installation and
/// verification run for real without downloading the published history.
/// Release builds ignore it.
pub const TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV: &str =
    "WHITAKER_INSTALLER_TEST_CLIPPY_SNAPSHOT_MANIFEST";
