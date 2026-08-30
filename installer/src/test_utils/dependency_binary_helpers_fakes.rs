//! Fake binaries, PATH staging, and repository-installer doubles.
//!
//! These helpers stage executables in temporary directories and point `PATH`
//! at them so dependency-install probes can be exercised without touching a
//! real toolchain.

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    dependency_binaries::{
        DependencyBinary,
        DependencyBinaryInstallError,
        DependencyBinaryInstaller,
    },
    dirs::BaseDirs,
    installer_packaging::TargetTriple,
    test_support::env_test_guard,
};

/// Repository installer test double that always reports a missing archive.
pub struct AlwaysNotFoundRepositoryInstaller;

impl DependencyBinaryInstaller for AlwaysNotFoundRepositoryInstaller {
    fn install(
        &self,
        dependency: &DependencyBinary,
        target: &TargetTriple,
        _dirs: &dyn BaseDirs,
    ) -> std::result::Result<PathBuf, DependencyBinaryInstallError> {
        Err(DependencyBinaryInstallError::NotFound {
            url: format!(
                "https://example.test/{}-{}-v{}.tgz",
                dependency.package(),
                target,
                dependency.version()
            ),
        })
    }
}

/// Writes a fake binary at `path` that exits successfully.
///
/// # Errors
///
/// Returns any I/O error raised while writing the binary or setting its
/// permissions.
pub fn write_fake_binary(path: &Path, is_executable: bool) -> std::io::Result<()> {
    write_fake_binary_with_status(path, is_executable, 0)
}

/// Writes a fake binary at `path` that exits with the supplied status code.
///
/// # Errors
///
/// Returns any I/O error raised while writing the binary or setting its
/// permissions.
pub fn write_fake_binary_with_status(
    path: &Path,
    is_executable: bool,
    exit_code: i32,
) -> std::io::Result<()> {
    fs::write(path, fake_binary_contents(exit_code))?;
    #[cfg(unix)]
    {
        let mode = if is_executable { 0o755 } else { 0o644 };
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(mode);
        fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    let _ = is_executable;
    Ok(())
}

fn fake_binary_contents(exit_code: i32) -> Vec<u8> {
    #[cfg(windows)]
    {
        format!("@echo off\r\nexit /b {exit_code}\r\n").into_bytes()
    }
    #[cfg(not(windows))]
    {
        format!("#!/bin/sh\nexit {exit_code}\n").into_bytes()
    }
}

/// Runs a closure with `PATH` pointing at one or more fake directories.
///
/// # Errors
///
/// Returns an I/O error when the fake directories cannot be created, the
/// `setup` closure fails, or the fake `PATH` cannot be joined.
pub fn with_fake_path<T>(
    setup: impl FnOnce(&[PathBuf]) -> std::io::Result<()>,
    run: impl FnOnce() -> T,
) -> std::io::Result<T> {
    let _guard = env_test_guard();
    let temp_dirs = [tempfile::tempdir()?, tempfile::tempdir()?];
    let path_dirs = temp_dirs
        .iter()
        .map(|dir| dir.path().to_path_buf())
        .collect::<Vec<_>>();
    setup(&path_dirs)?;
    let path = std::env::join_paths(path_dirs.iter().map(PathBuf::as_path))
        .map_err(std::io::Error::other)?;
    Ok(temp_env::with_var("PATH", Some(path), run))
}

/// Runs a closure with `PATH` containing a fake executable in the first entry.
///
/// # Errors
///
/// Returns an I/O error when the fake `PATH` cannot be prepared or the fake
/// binary cannot be written.
pub fn with_fake_binary_on_path<T>(
    binary_name: &str,
    run: impl FnOnce() -> T,
) -> std::io::Result<T> {
    with_fake_path(
        |directories| {
            let first_dir = directories.first().ok_or_else(|| {
                std::io::Error::other("fake PATH should contain at least one directory")
            })?;
            write_fake_binary(&path_binary_location(first_dir, binary_name), true)
        },
        run,
    )
}

/// Joins `binary_name` onto `directory` with the platform executable suffix,
/// so directly probed fakes are runnable on Windows as well as Unix.
#[must_use]
pub fn path_binary_location(directory: &Path, binary_name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        directory.join(format!("{binary_name}.cmd"))
    }
    #[cfg(not(windows))]
    {
        directory.join(binary_name)
    }
}
