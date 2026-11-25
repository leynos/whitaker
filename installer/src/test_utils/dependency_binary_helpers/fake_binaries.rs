//! Fake repository installers and executables for dependency-binary tests.

use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::PermissionsExt;
use cap_std::{ambient_authority, fs_utf8::Dir};

use crate::{
    dependency_binaries::{
        DependencyBinary,
        DependencyBinaryInstallError,
        DependencyBinaryInstaller,
    },
    dirs::BaseDirs,
    installer_packaging::TargetTriple,
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
    let utf8_path = Utf8Path::from_path(path)
        .ok_or_else(|| io::Error::other("fake binary path must be valid UTF-8"))?;
    let parent = utf8_path
        .parent()
        .ok_or_else(|| io::Error::other("fake binary path must have a parent directory"))?;
    let name = utf8_path
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

/// Runs a closure with a `PATH` value naming one or more fake directories.
///
/// The value is handed to the closure rather than installed in the process
/// environment: give it to a [`StubExecutor`](crate::test_utils::StubExecutor)
/// with `with_env_var("PATH", path)` so discovery searches only the fakes.
pub fn with_fake_path<T>(
    setup: impl FnOnce(&[PathBuf]) -> io::Result<()>,
    run: impl FnOnce(&OsStr) -> T,
) -> io::Result<T> {
    let fake = FakePath::stage(setup)?;
    Ok(run(fake.as_os_str()))
}

/// A `PATH` value naming two temporary directories, kept alive by this guard.
///
/// Use it where the executor is built before the closure of
/// [`with_fake_path`] would run: stage the fake first, then hand
/// [`Self::as_os_str`] to the executor's `PATH`.
#[derive(Debug)]
pub struct FakePath {
    _directories: [tempfile::TempDir; 2],
    path: std::ffi::OsString,
}

impl FakePath {
    /// Create the two directories and let `setup` populate them.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if a directory cannot be created, `setup` fails,
    /// or the directories cannot be joined into a `PATH` value.
    pub fn stage(setup: impl FnOnce(&[PathBuf]) -> io::Result<()>) -> io::Result<Self> {
        let directories = [tempfile::tempdir()?, tempfile::tempdir()?];
        let path_dirs = directories
            .iter()
            .map(|dir| dir.path().to_path_buf())
            .collect::<Vec<_>>();
        setup(&path_dirs)?;
        let path = std::env::join_paths(path_dirs.iter().map(PathBuf::as_path))
            .map_err(|error| io::Error::other(format!("join fake PATH directories: {error}")))?;
        Ok(Self {
            _directories: directories,
            path,
        })
    }

    /// Stage a fake executable in the leading entry, where it shadows any
    /// real binary of the same name.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if staging fails.
    pub fn with_binary(binary_name: &str) -> io::Result<Self> {
        Self::stage(|directories| write_fake_binary_in_first_directory(directories, binary_name))
    }

    /// The `PATH` value to give an executor.
    #[must_use]
    pub fn as_os_str(&self) -> &OsStr { &self.path }
}

/// Runs a closure with a `PATH` value whose first entry holds a fake executable.
pub fn with_fake_binary_on_path<T>(
    binary_name: &str,
    run: impl FnOnce(&OsStr) -> T,
) -> io::Result<T> {
    with_fake_path(
        |directories| write_fake_binary_in_first_directory(directories, binary_name),
        run,
    )
}

/// Write an executable fake into the leading entry of a fake `PATH`.
///
/// The leading entry is where the process looks first, so a fake placed there
/// shadows any real binary of the same name.
fn write_fake_binary_in_first_directory(
    directories: &[PathBuf],
    binary_name: &str,
) -> io::Result<()> {
    let first = directories
        .first()
        .ok_or_else(|| io::Error::other("fake PATH must contain at least one directory"))?;
    write_fake_binary(&path_binary_location(first, binary_name), true)
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
