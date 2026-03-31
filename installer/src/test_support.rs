//! Test-only installer hooks shared by behavioural suites.
//!
//! These constants are consumed by behavioural tests and by debug-only helper
//! modules that simulate staged output without invoking a nested workspace
//! build.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Environment variable used by behavioural tests to request synthetic suite
/// staging in debug binaries.
pub const TEST_STAGE_SUITE_ENV: &str = "WHITAKER_INSTALLER_TEST_STAGE_SUITE";

/// Serializes tests that mutate process environment variables.
///
/// Several installer tests temporarily override environment variables through
/// `temp_env`. Those mutations affect the whole process, so parallel tests must
/// coordinate on a single shared lock.
pub fn env_test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("expected environment test lock")
}
