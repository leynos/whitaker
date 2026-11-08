//! Shared fixtures and helpers for the configuration integration scenarios.

use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};
use ortho_config::MapEnv;
use tempfile::TempDir;
use whitaker_cli::{load_with_sources, Sources};

/// Write a fixture relative to its capability-owned directory.
pub(super) fn write(root: &TempDir, name: &str, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    Dir::open_ambient_dir(root.path(), ambient_authority())?.write(name, contents)
}

/// Allocate a private project root for each scenario.
#[rstest::fixture]
pub(super) fn root() -> std::io::Result<TempDir> {
    // Keep the fixture body multiline for the rstest expansion.
    TempDir::new()
}

/// Produce an operating-system string that cannot be decoded as Unicode.
#[cfg(unix)]
pub(super) fn non_unicode_value() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(b"\xff").to_os_string()
}

/// Produce an operating-system string that cannot be decoded as Unicode.
#[cfg(windows)]
pub(super) fn non_unicode_value() -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt;
    std::ffi::OsString::from_wide(&[0xd800])
}

/// Read the effective locale from one injected source bundle.
pub(super) fn locale(input: &Sources) -> ortho_config::OrthoResult<String> {
    Ok(load_with_sources(input)?.settings.diagnostics.locale)
}

/// Fail the scenario with `message` when `condition` is false.
///
/// These scenarios return `Result` so a fallible fixture can be propagated
/// with `?`, and `clippy::panic_in_result_fn` forbids `assert!` inside such a
/// function. Use this only where the scenario must stay fallible; general
/// assertions belong in a `()`-returning helper, where a panic reads better.
///
/// # Errors
///
/// Returns an error carrying `message` when `condition` is false.
pub(super) fn ensure(
    condition: bool,
    message: impl Into<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        return Ok(());
    }
    let failure: String = message.into();
    Err(failure.into())
}

/// Fail the scenario unless `actual` equals `expected`, naming both values.
///
/// Both sides are borrowed so a caller can compare a field without moving it
/// out of the loaded settings, and so a heterogeneous pair such as
/// `Vec<String>` against `[&str; 2]` still satisfies [`PartialEq`].
///
/// # Errors
///
/// Returns an error quoting `context` and showing both values when they differ.
pub(super) fn ensure_eq<T, U>(
    actual: &T,
    expected: &U,
    context: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: PartialEq<U> + std::fmt::Debug + ?Sized,
    U: std::fmt::Debug + ?Sized,
{
    if actual == expected {
        return Ok(());
    }
    Err(format!("{context}: expected {expected:?}, got {actual:?}").into())
}

/// Construct sources rooted in a private test directory.
pub(super) fn sources(root: &TempDir, env: Arc<MapEnv>, cli: serde_json::Value) -> Sources {
    Sources {
        project_root: root.path().to_path_buf(),
        config_path: None,
        discovery: Arc::new(
            env.as_ref()
                .clone()
                .with_var("XDG_CONFIG_DIRS", root.path().join("system"))
                .with_var("XDG_CONFIG_HOME", root.path().join("user")),
        ),
        environment: env,
        command: None,
        cli,
    }
}
