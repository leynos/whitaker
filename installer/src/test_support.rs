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

/// Environment variable that relocates Whitaker's data directory in debug
/// builds.
///
/// The root install resolves its data directory from a platform API that
/// ignores `XDG_DATA_HOME` on Windows, so an integration test that drives the
/// binary cannot keep the installation record inside its own temporary
/// directory by environment alone. Debug builds honour this variable for that
/// case; release builds ignore it, so it cannot move a real installation.
pub const TEST_DATA_HOME_ENV: &str = "WHITAKER_INSTALLER_TEST_DATA_HOME";

/// Environment variable that relocates Whitaker's executable directory in
/// debug builds.
///
/// It mirrors [`TEST_DATA_HOME_ENV`] for the bin root. On Windows the bin root
/// derives from the home directory in the profile known folder, which ignores
/// both `HOME` and `XDG_BIN_HOME`, so an integration test could otherwise
/// inspect a temporary directory the binary never used. Debug builds honour
/// this variable for that case; release builds ignore it, so it cannot move a
/// real installation.
pub const TEST_EXECUTABLE_HOME_ENV: &str = "WHITAKER_INSTALLER_TEST_EXECUTABLE_HOME";

/// Environment variable naming a Clippy snapshot manifest that debug
/// installer binaries use instead of the embedded one.
///
/// Integration tests point it at a small fixture snapshot and pass the
/// matching bundle with `--clippy-snapshot-bundle`, so installation and
/// verification run for real without downloading the published history.
/// Release builds ignore it.
pub const TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV: &str =
    "WHITAKER_INSTALLER_TEST_CLIPPY_SNAPSHOT_MANIFEST";
