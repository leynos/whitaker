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
    std::ffi::OsString::from_wide(&[0xD800])
}

/// Read the effective locale from one injected source bundle.
pub(super) fn locale(input: Sources) -> ortho_config::OrthoResult<String> {
    Ok(load_with_sources(input)?.settings.diagnostics.locale)
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
