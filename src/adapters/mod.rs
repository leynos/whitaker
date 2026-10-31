//! Production adapters for installer, inventory, and Dylint subprocess ports.

use std::{ffi::OsString, process::Command};

use camino::Utf8PathBuf;
use whitaker_installer::{
    dependency_binaries::host_target,
    dirs::SystemBaseDirs,
    list::{collect_installed, detect_active_toolchain},
    orchestration::run_install_for_whitaker,
    prebuilt_path::prebuilt_library_dir,
    stager::default_target_dir,
};

use crate::ports::{
    CliError, InstallRequest, InstallService, LintInventory, LintRunner, ListRequest, Services,
    StagedInventory,
};

/// The system-backed adapters used by the `whitaker` executable.
#[derive(Debug, Default)]
pub struct SystemServices;

impl SystemServices {
    /// Creates the production adapter set.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = ports;
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Borrows the implementation through the command ports.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::adapters::SystemServices;
    ///
    /// let services = SystemServices::new();
    /// let ports = services.ports();
    /// let _ = (ports.install, ports.inventory, ports.lint_runner);
    /// ```
    #[must_use]
    pub fn ports(&self) -> Services<'_> {
        Services {
            install: self,
            inventory: self,
            lint_runner: self,
        }
    }
}

impl InstallService for SystemServices {
    fn install(
        &self,
        request: &InstallRequest,
        stdout: &mut dyn std::io::Write,
        stderr: &mut dyn std::io::Write,
    ) -> Result<(), CliError> {
        run_install_for_whitaker(&request.arguments, stdout, stderr).map_err(|error| {
            CliError::Install {
                message: error.to_string(),
            }
        })
    }
}

impl LintInventory for SystemServices {
    fn list(&self, request: &ListRequest) -> Result<StagedInventory, CliError> {
        let installed =
            collect_installed(&request.arguments).map_err(|error| CliError::Inventory {
                message: error.to_string(),
            })?;
        Ok(StagedInventory {
            installed,
            active_toolchain: detect_active_toolchain(),
        })
    }
}

impl LintRunner for SystemServices {
    fn forward(&self, args: &[OsString]) -> Result<u8, CliError> {
        let mut command = Command::new("cargo");
        command.arg("dylint").args(args);
        if let Some(library_path) = active_library_path() {
            command.env("DYLINT_LIBRARY_PATH", library_path);
        }
        let status = command
            .status()
            .map_err(|source| CliError::Forward { source })?;
        Ok(status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1))
    }
}

/// Resolves the library path used when forwarding a command to cargo-dylint.
fn active_library_path() -> Option<OsString> {
    let active_toolchain = detect_active_toolchain();
    let build_path = active_toolchain.as_ref().and_then(|toolchain| {
        default_target_dir().map(|target_dir| target_dir.join(toolchain).join("release"))
    });
    let prebuilt_path = active_toolchain.as_deref().and_then(prebuilt_library_path);
    active_library_path_for(
        std::env::var_os("DYLINT_LIBRARY_PATH"),
        build_path,
        prebuilt_path,
    )
}

/// Chooses an explicit path, then a build path, then a prebuilt path.
fn active_library_path_for(
    explicit_path: Option<OsString>,
    build_path: Option<Utf8PathBuf>,
    prebuilt_path: Option<Utf8PathBuf>,
) -> Option<OsString> {
    explicit_path
        .filter(|path| !path.is_empty())
        .or_else(|| existing_directory(build_path))
        .or_else(|| existing_directory(prebuilt_path))
}

/// Returns a directory path only when it currently names an existing directory.
fn existing_directory(directory: Option<Utf8PathBuf>) -> Option<OsString> {
    directory
        .filter(|candidate| candidate.is_dir())
        .map(|candidate| candidate.into_std_path_buf().into_os_string())
}

/// Resolves the prebuilt library directory for the active host and toolchain.
fn prebuilt_library_path(toolchain: &str) -> Option<Utf8PathBuf> {
    let dirs = SystemBaseDirs::new()?;
    let target = host_target()?;
    prebuilt_library_dir(&dirs, toolchain, target.as_str()).ok()
}

#[cfg(test)]
mod tests {
    //! Tests for adapter path selection and child-process behavior.

    use std::ffi::OsString;

    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use tempfile::TempDir;

    use super::active_library_path_for;

    fn utf8_path(path: &std::path::Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(path.to_path_buf())
            .expect("temporary test paths should be valid UTF-8")
    }

    fn create_test_directory(temporary: &TempDir, name: &str) -> Utf8PathBuf {
        let root = Dir::open_ambient_dir(
            Utf8Path::from_path(temporary.path()).expect("temporary path should be valid UTF-8"),
            ambient_authority(),
        )
        .expect("temporary directory should be opened as a filesystem capability");
        root.create_dir_all(name)
            .expect("test directory should be created inside the temporary directory");

        utf8_path(&temporary.path().join(name))
    }

    #[test]
    fn explicit_library_path_wins_when_build_and_prebuilt_directories_exist() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let build_path = create_test_directory(&temporary, "build");
        let prebuilt_path = create_test_directory(&temporary, "prebuilt");
        let expected = OsString::from("/custom/dylint/lib");
        let actual = active_library_path_for(
            Some(expected.clone()),
            Some(build_path),
            Some(prebuilt_path),
        );

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn existing_build_directory_precedes_existing_prebuilt_directory() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let build_path = create_test_directory(&temporary, "build");
        let prebuilt_path = create_test_directory(&temporary, "prebuilt");
        let expected = build_path.clone().into_os_string();

        let actual = active_library_path_for(None, Some(build_path), Some(prebuilt_path));

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn prebuilt_directory_is_used_when_build_directory_is_absent() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let missing_build_path = utf8_path(&temporary.path().join("build"));
        let prebuilt_path = create_test_directory(&temporary, "prebuilt");
        let expected = prebuilt_path.clone().into_os_string();

        let actual = active_library_path_for(None, Some(missing_build_path), Some(prebuilt_path));

        assert_eq!(actual, Some(expected));
    }
}
