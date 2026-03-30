//! Test-only installer hooks shared by behavioural suites.
//!
//! These helpers are intentionally inert in release binaries so production
//! installer invocations cannot stage synthetic artefacts by inheriting test
//! environment variables.

/// Environment variable used by behavioural tests to request synthetic suite
/// staging in debug binaries.
pub const TEST_STAGE_SUITE_ENV: &str = "WHITAKER_INSTALLER_TEST_STAGE_SUITE";

/// Returns `true` when a debug build should stage a synthetic suite artefact
/// for behavioural tests.
pub fn synthetic_suite_staging_requested() -> bool {
    cfg!(debug_assertions) && std::env::var_os(TEST_STAGE_SUITE_ENV).is_some()
}
