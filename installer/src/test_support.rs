//! Test-only installer hooks shared by behavioural suites.
//!
//! These constants are consumed by behavioural tests and by debug-only helper
//! modules that simulate staged output without invoking a nested workspace
//! build.

pub use whitaker_common::test_support::env_test_guard;

/// Environment variable used by behavioural tests to request synthetic suite
/// staging in debug binaries.
pub const TEST_STAGE_SUITE_ENV: &str = "WHITAKER_INSTALLER_TEST_STAGE_SUITE";
