//! Production adapters for installer, inventory, and Dylint subprocess ports.

use std::{
    ffi::{OsStr, OsString},
    io::Write,
    process::{Command, ExitStatus},
};

use camino::Utf8PathBuf;
use whitaker_installer::{
    bootstrap::is_installed,
    dependency_binaries::host_target,
    dirs::SystemBaseDirs,
    installation_record::installed_paths,
    list::{collect_installed, detect_active_toolchain},
    orchestration::{run_bootstrap_command_with_library_path, run_install_for_whitaker},
    prebuilt_path::prebuilt_library_dir,
    stager::default_target_dir,
};

use crate::ports::{
    CliError,
    InstallRequest,
    InstallService,
    LintInventory,
    LintRunner,
    ListRequest,
    Services,
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
    pub const fn new() -> Self { Self }

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
    /// Run the forwarded invocation through the installed bootstrap.
    ///
    /// Running it here rather than as a bare `cargo dylint` is what makes a
    /// cold run work: the bootstrap exports the verified Clippy snapshot and
    /// the managed driver slot, and serializes the first driver build. The
    /// library path is the one this crate resolved, so Dylint can still pick
    /// the toolchain a project declares, and an explicit
    /// `DYLINT_LIBRARY_PATH` stays authoritative.
    ///
    /// A machine with no recorded bootstrap falls back to starting
    /// `cargo dylint` directly. That keeps the legacy forwarding contract for
    /// a source checkout or a manually configured `DYLINT_LIBRARY_PATH`, and
    /// there is no installed bootstrap for that path to bypass.
    fn forward(&self, args: &[OsString], stderr: &mut dyn Write) -> Result<i32, CliError> {
        let library_path = active_library_path()?;
        if has_installed_bootstrap() {
            return Ok(run_bootstrap_command_with_library_path(
                args,
                library_path.as_deref(),
                stderr,
            ));
        }
        forward_without_bootstrap(args, library_path.as_deref())
    }
}

/// Report whether an installation recorded a bootstrap to run from.
fn has_installed_bootstrap() -> bool {
    SystemBaseDirs::new().is_some_and(|dirs| is_installed(&dirs))
}

/// Run `cargo dylint` directly, for a machine with no installed bootstrap.
fn forward_without_bootstrap(
    args: &[OsString],
    library_path: Option<&OsStr>,
) -> Result<i32, CliError> {
    let mut command = Command::new("cargo");
    command.arg("dylint").args(args);
    if let Some(path) = library_path {
        command.env("DYLINT_LIBRARY_PATH", path);
    }
    let status = command
        .status()
        .map_err(|source| CliError::Forward { source })?;
    Ok(child_exit_code(status))
}

/// Map a child's status to the code this process should exit with.
fn child_exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .or_else(|| signal_exit_code(status))
        .unwrap_or(1)
}

#[cfg(unix)]
fn signal_exit_code(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal().map(|signal| 128 + signal)
}

#[cfg(not(unix))]
const fn signal_exit_code(_status: ExitStatus) -> Option<i32> { None }

/// Resolves the library path used when forwarding a command to cargo-dylint.
///
/// # Errors
///
/// Returns [`CliError::InstallationRecord`] when an installation record exists
/// but cannot be locked, read, or parsed. A missing record is not an error: it
/// simply means no toolchain has been installed yet.
fn active_library_path() -> Result<Option<OsString>, CliError> {
    let explicit_path = std::env::var_os("DYLINT_LIBRARY_PATH");
    if explicit_path.as_ref().is_some_and(|path| !path.is_empty()) {
        return Ok(explicit_path);
    }
    let recorded_paths = recorded_library_paths()?;
    let active_toolchain = detect_active_toolchain();
    let build_path = active_toolchain.as_ref().and_then(|toolchain| {
        default_target_dir().map(|target_dir| target_dir.join(toolchain).join("release"))
    });
    let prebuilt_path = active_toolchain.as_deref().and_then(prebuilt_library_path);
    Ok(active_library_path_for(
        explicit_path,
        recorded_paths,
        build_path,
        prebuilt_path,
    ))
}

/// Reads the per-toolchain staging directories recorded by earlier installs.
///
/// An unavailable platform data directory yields no recorded paths, matching
/// the "nothing installed" case. A record that exists but fails to load is
/// propagated so the caller can report it instead of silently forwarding with
/// an empty library path.
fn recorded_library_paths() -> Result<Vec<Utf8PathBuf>, CliError> {
    SystemBaseDirs::new().map_or_else(
        || Ok(Vec::new()),
        |dirs| installed_paths(&dirs).map_err(|source| CliError::InstallationRecord { source }),
    )
}

/// Includes recorded and default directories so Dylint can select by toolchain.
fn active_library_path_for(
    explicit_path: Option<OsString>,
    recorded_paths: Vec<Utf8PathBuf>,
    build_path: Option<Utf8PathBuf>,
    prebuilt_path: Option<Utf8PathBuf>,
) -> Option<OsString> {
    if let Some(path) = explicit_path.filter(|path| !path.is_empty()) {
        return Some(path);
    }
    let mut candidates = recorded_paths;
    for candidate in [build_path, prebuilt_path].into_iter().flatten() {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    existing_recorded_directories(candidates)
}

/// Joins every existing recorded path so Dylint can select by toolchain.
fn existing_recorded_directories(paths: Vec<Utf8PathBuf>) -> Option<OsString> {
    let existing: Vec<_> = paths
        .into_iter()
        .filter(|path| path.is_absolute() && path.is_dir())
        .map(Utf8PathBuf::into_std_path_buf)
        .filter(|path| std::env::join_paths([path]).is_ok())
        .collect();
    (!existing.is_empty())
        .then(|| std::env::join_paths(existing).ok())
        .flatten()
}

/// Resolves the prebuilt library directory for the active host and toolchain.
fn prebuilt_library_path(toolchain: &str) -> Option<Utf8PathBuf> {
    let dirs = SystemBaseDirs::new()?;
    let target = host_target()?;
    prebuilt_library_dir(&dirs, toolchain, target.as_str()).ok()
}

#[cfg(test)]
mod tests {
    //! Tests for adapter path selection and child-process behaviour.

    use std::ffi::OsString;

    use camino::{Utf8Path, Utf8PathBuf};
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use tempfile::TempDir;
    use whitaker_common::test_support::{with_env_var, with_env_var_removed};
    use whitaker_installer::installation_record::InstallationRecordError;

    use super::{
        active_library_path,
        active_library_path_for,
        has_installed_bootstrap,
        recorded_library_paths,
    };
    use crate::ports::CliError;

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
        let recorded_path = create_test_directory(&temporary, "recorded");
        let expected = OsString::from("/custom/dylint/lib");
        let actual = active_library_path_for(
            Some(expected.clone()),
            vec![recorded_path],
            Some(build_path),
            Some(prebuilt_path),
        );

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn explicit_environment_path_precedes_recorded_and_default_paths() {
        let expected = OsString::from("/custom/dylint/lib");

        with_env_var("DYLINT_LIBRARY_PATH", &expected, || {
            let actual = active_library_path().expect("an explicit path never reads the record");
            assert_eq!(actual, Some(expected.clone()));
        });
    }

    /// Point the platform data directory at `data_home`.
    fn with_data_home<R>(data_home: &Utf8Path, callback: impl FnOnce() -> R) -> R {
        with_env_var_removed("DYLINT_LIBRARY_PATH", || {
            with_env_var("XDG_DATA_HOME", data_home, callback)
        })
    }

    /// Write `contents` as the installation record under `data_home`.
    fn write_record(data_home: &Utf8Path, contents: &[u8]) -> Utf8PathBuf {
        let record_dir = data_home.join("whitaker");
        std::fs::create_dir_all(&record_dir)
            .expect("record directory should be created inside the temporary directory");
        let record_path = record_dir.join("installed_lint_paths.json");
        std::fs::write(&record_path, contents).expect("record should be written");
        record_path
    }

    #[test]
    fn malformed_installation_record_is_reported() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let data_home = utf8_path(temporary.path());
        write_record(&data_home, b"not JSON");

        with_data_home(&data_home, || {
            let error = active_library_path().expect_err("a malformed record must not be ignored");
            assert!(
                matches!(
                    error,
                    CliError::InstallationRecord {
                        source: InstallationRecordError::Parse { .. }
                    }
                ),
                "unexpected error: {error}"
            );
        });
    }

    #[test]
    fn unreadable_installation_record_is_reported() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let data_home = utf8_path(temporary.path());
        // A directory in place of the record opens but cannot be read, which
        // fails the same way an unreadable file does without depending on the
        // test runner's privileges.
        std::fs::create_dir_all(data_home.join("whitaker/installed_lint_paths.json"))
            .expect("a directory should stand in for the unreadable record");

        with_data_home(&data_home, || {
            let error =
                active_library_path().expect_err("an unreadable record must not be ignored");
            assert!(
                matches!(
                    error,
                    CliError::InstallationRecord {
                        source: InstallationRecordError::Io { .. }
                    }
                ),
                "unexpected error: {error}"
            );
        });
    }

    #[test]
    fn absent_installation_record_yields_no_recorded_paths() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let data_home = utf8_path(temporary.path());

        with_data_home(&data_home, || {
            let recorded = recorded_library_paths().expect("a missing record is not a failure");
            assert!(recorded.is_empty());
        });
    }

    #[test]
    fn absent_bootstrap_metadata_reports_no_installation() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let data_home = utf8_path(temporary.path());

        with_data_home(&data_home, || {
            assert!(
                !has_installed_bootstrap(),
                "no bootstrap.json means there is nothing to run from"
            );
        });
    }

    #[test]
    fn recorded_bootstrap_metadata_reports_an_installation() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let data_home = utf8_path(temporary.path());
        // The metadata's contents are irrelevant here: the selection asks only
        // whether a record exists, and a broken record must still be reported
        // by the bootstrap rather than silently bypassed.
        let bootstrap_dir = data_home.join("whitaker-bootstrap");
        std::fs::create_dir_all(&bootstrap_dir)
            .expect("bootstrap directory should be created inside the temporary directory");
        std::fs::write(bootstrap_dir.join("bootstrap.json"), b"{}")
            .expect("bootstrap metadata should be written");

        with_data_home(&data_home, || {
            assert!(
                has_installed_bootstrap(),
                "a recorded bootstrap must be used"
            );
        });
    }

    #[test]
    fn build_directory_precedes_prebuilt_directory_in_candidate_list() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let build_path = create_test_directory(&temporary, "build");
        let prebuilt_path = create_test_directory(&temporary, "prebuilt");
        let expected = std::env::join_paths([
            build_path.as_std_path().to_path_buf(),
            prebuilt_path.as_std_path().to_path_buf(),
        ])
        .expect("temporary paths should form an environment path list");

        let actual =
            active_library_path_for(None, Vec::new(), Some(build_path), Some(prebuilt_path));

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn prebuilt_directory_is_used_when_build_directory_is_absent() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let missing_build_path = utf8_path(&temporary.path().join("build"));
        let prebuilt_path = create_test_directory(&temporary, "prebuilt");
        let expected = prebuilt_path.clone().into_os_string();

        let actual = active_library_path_for(
            None,
            Vec::new(),
            Some(missing_build_path),
            Some(prebuilt_path),
        );

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn recorded_and_default_directories_cover_each_toolchain() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let stable = create_test_directory(&temporary, "stable-custom-target");
        let nightly = create_test_directory(&temporary, "nightly-custom-target");
        let active_build = create_test_directory(&temporary, "active-default-target");
        let expected = std::env::join_paths([
            stable.as_std_path().to_path_buf(),
            nightly.as_std_path().to_path_buf(),
            active_build.as_std_path().to_path_buf(),
        ])
        .expect("temporary paths should form an environment path list");

        let actual = active_library_path_for(None, vec![stable, nightly], Some(active_build), None);

        assert_eq!(actual, Some(expected));
    }

    #[cfg(unix)]
    #[test]
    fn an_unrepresentable_recorded_directory_does_not_hide_valid_paths() {
        let temporary = TempDir::new().expect("temporary directories should be created");
        let valid = create_test_directory(&temporary, "valid");
        let invalid = create_test_directory(&temporary, "invalid:directory");
        let expected = valid.clone().into_os_string();

        let actual = active_library_path_for(None, vec![invalid, valid], None, None);

        assert_eq!(actual, Some(expected));
    }
}
