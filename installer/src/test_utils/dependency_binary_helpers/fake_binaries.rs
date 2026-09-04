//! Fake repository installers and executables for dependency-binary tests.

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    dependency_binaries::{
        DependencyBinary, DependencyBinaryInstallError, DependencyBinaryInstaller,
    },
    dirs::BaseDirs,
    installer_packaging::TargetTriple,
    test_support::env_test_guard,
};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs::PermissionsExt, fs_utf8::Dir};

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
pub fn write_fake_binary(path: &Path, is_executable: bool) -> io::Result<()> {
    write_fake_binary_with_status(path, is_executable, 0)
}

/// Writes a fake binary at `path` that exits with the supplied status code.
pub fn write_fake_binary_with_status(
    path: &Path,
    is_executable: bool,
    exit_code: i32,
) -> io::Result<()> {
    let (directory, name) = fixture_parent(path)?;
    directory.write(name, fake_binary_contents(exit_code))?;
    #[cfg(unix)]
    {
        let mode = if is_executable { 0o755 } else { 0o644 };
        let mut permissions = directory.metadata(name)?.permissions();
        permissions.set_mode(mode);
        directory.set_permissions(name, permissions)?;
    }
    #[cfg(not(unix))]
    let _ = is_executable;
    Ok(())
}

fn fixture_parent(path: &Path) -> io::Result<(Dir, &str)> {
    let path = Utf8Path::from_path(path)
        .ok_or_else(|| io::Error::other("fake binary path must be valid UTF-8"))?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("fake binary path must have a parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("fake binary path must have a file name"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((directory, name))
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
pub fn with_fake_path<T>(
    setup: impl FnOnce(&[PathBuf]) -> io::Result<()>,
    run: impl FnOnce() -> T,
) -> io::Result<T> {
    let _guard = env_test_guard();
    let temp_dirs = [tempfile::tempdir()?, tempfile::tempdir()?];
    let path_dirs = temp_dirs
        .iter()
        .map(|dir| dir.path().to_path_buf())
        .collect::<Vec<_>>();
    setup(&path_dirs)?;
    let path = std::env::join_paths(path_dirs.iter().map(PathBuf::as_path))
        .map_err(|error| io::Error::other(format!("join fake PATH directories: {error}")))?;
    Ok(temp_env::with_var("PATH", Some(path), run))
}

/// Runs a closure with `PATH` containing a fake executable in the first entry.
pub fn with_fake_binary_on_path<T>(binary_name: &str, run: impl FnOnce() -> T) -> io::Result<T> {
    with_fake_path(
        |directories| write_fake_binary(&path_binary_location(&directories[0], binary_name), true),
        run,
    )
}

/// Joins `binary_name` onto `directory` with the platform executable suffix,
/// so directly probed fakes are runnable on Windows as well as Unix.
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
