//! Dependency installation for Dylint tools.
//!
//! This module checks whether `cargo-dylint` and `dylint-link` are already
//! available, then installs any missing tools by preferring repository-hosted
//! release archives before falling back to `cargo binstall` or `cargo install`.

use crate::dependency_binaries::{
    DependencyBinaryInstaller, RepositoryDependencyBinaryInstaller, find_dependency_binary,
    host_target,
};
use crate::dirs::{BaseDirs, SystemBaseDirs};
use crate::error::{InstallerError, Result};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

mod install;
use install::*;

/// Abstraction for running external commands.
pub trait CommandExecutor {
    /// Runs a command with arguments and returns the captured output.
    ///
    /// # Errors
    ///
    /// Returns any I/O errors encountered while spawning or running the command.
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output>;
}

/// Executes commands on the host system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommandExecutor;

impl CommandExecutor for SystemCommandExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        Command::new(cmd)
            .args(args)
            .output()
            .map_err(InstallerError::from)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DependencyTool {
    package: &'static str,
    command: &'static str,
    args: &'static [&'static str],
}

const CARGO_DYLINT_TOOL: DependencyTool = DependencyTool {
    package: "cargo-dylint",
    command: "cargo",
    args: &["dylint", "--version"],
};

const DYLINT_LINK_TOOL: DependencyTool = DependencyTool {
    package: "dylint-link",
    command: "dylint-link",
    args: &["--version"],
};

const DEPENDENCY_TOOLS: [DependencyTool; 2] = [CARGO_DYLINT_TOOL, DYLINT_LINK_TOOL];

/// Stable, bounded failure labels returned while checking a PATH entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum PathScanFailureCategory {
    /// A PATH entry cannot be represented as UTF-8 for capability-based access.
    NonUtf8Path,
    /// A PATH entry cannot be opened as a directory capability.
    DirectoryOpen,
    /// Candidate metadata cannot be read through the directory capability.
    Metadata,
}

impl PathScanFailureCategory {
    /// Return the low-cardinality field value used by trace events.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonUtf8Path => "non_utf8_path",
            Self::DirectoryOpen => "directory_open",
            Self::Metadata => "metadata",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct PathScanFailure {
    /// Executable being searched for; no filesystem path is retained.
    pub binary_name: String,
    /// Bounded reason a PATH entry was skipped.
    pub category: PathScanFailureCategory,
}

/// Collect the first match and classified failures for one PATH scan.
#[derive(Debug, Default)]
pub(crate) struct PathScanResult {
    binary: Option<std::path::PathBuf>,
    failures: Vec<PathScanFailure>,
}

impl PathScanResult {
    /// Build a result for a found binary and any skipped-entry failures.
    #[cfg(test)]
    fn new(binary: Option<std::path::PathBuf>, failures: Vec<PathScanFailure>) -> Self {
        Self { binary, failures }
    }
}

/// Query PATH and its filesystem entries for one executable.
pub(crate) trait BinaryLocator {
    /// Return the first matching executable and bounded skipped-entry details.
    fn find_binary(&self, binary_name: &str) -> PathScanResult;
}

/// Use the process PATH and host filesystem to locate dependency binaries.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemBinaryLocator;

impl BinaryLocator for SystemBinaryLocator {
    fn find_binary(&self, binary_name: &str) -> PathScanResult {
        find_binary_on_path(binary_name)
    }
}

/// Status and diagnostics from a successful availability query.
#[derive(Debug)]
#[doc(hidden)]
pub struct DylintToolCheck {
    /// Availability for each required tool.
    pub status: DylintToolStatus,
    /// PATH entries skipped before a match was found.
    pub path_scan_failures: Vec<PathScanFailure>,
}

/// Classify metadata errors, omitting the ordinary miss for an absent candidate.
fn metadata_failure_category(kind: io::ErrorKind) -> Option<PathScanFailureCategory> {
    (kind != io::ErrorKind::NotFound).then_some(PathScanFailureCategory::Metadata)
}

/// Classify directory-open errors, omitting PATH entries that no longer exist.
fn directory_open_failure_category(kind: io::ErrorKind) -> Option<PathScanFailureCategory> {
    (kind != io::ErrorKind::NotFound).then_some(PathScanFailureCategory::DirectoryOpen)
}

/// Status of Dylint tool availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DylintToolStatus {
    /// Whether `cargo-dylint` is installed.
    pub cargo_dylint: bool,
    /// Whether `dylint-link` is installed.
    pub dylint_link: bool,
}

impl DylintToolStatus {
    /// Returns `true` when both tools are installed.
    #[must_use]
    pub fn all_installed(&self) -> bool {
        self.cargo_dylint && self.dylint_link
    }
}

/// How a run should react when a published artefact is missing.
///
/// Carried together because they travel together: every layer that decides
/// whether to print also decides whether a source build is acceptable, and
/// threading two booleans through the same five signatures invites them to
/// drift apart.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourcePolicy {
    /// Whether stderr output should be suppressed.
    pub quiet: bool,
    /// Whether a missing artefact is an error rather than a source build.
    pub no_source_fallback: bool,
}

/// Additional install options used by test-support hooks.
#[cfg(any(test, feature = "test-support"))]
pub struct DependencyInstallOptions<'a> {
    /// Base directories used by the repository installer.
    pub dirs: &'a dyn BaseDirs,
    /// The repository-first installer implementation.
    pub repository_installer: &'a dyn DependencyBinaryInstaller,
    /// Host target override used for repository asset naming.
    pub target: Option<crate::installer_packaging::TargetTriple>,
    /// How the run reacts to a missing published artefact.
    pub policy: SourcePolicy,
}

/// Check whether the Dylint tools are installed.
///
/// A failed command or PATH query is returned as an error so an uncertain
/// result cannot be mistaken for permission to install a missing tool.
///
/// # Errors
///
/// Returns an error when a command, registry, or PATH query cannot establish
/// the availability of a dependency.
pub fn check_dylint_tools(executor: &dyn CommandExecutor) -> Result<DylintToolStatus> {
    check_dylint_tools_with_locator(executor, &SystemBinaryLocator).map(|check| check.status)
}

/// Check tool availability and return bounded PATH diagnostics for the caller.
///
/// The command boundary owns any tracing of returned diagnostics. The query
/// itself has no logging side effects.
///
/// # Errors
///
/// Returns an error when a command, registry, or PATH query cannot establish
/// the availability of a dependency.
#[doc(hidden)]
pub fn check_dylint_tools_with_diagnostics(
    executor: &dyn CommandExecutor,
) -> Result<DylintToolCheck> {
    check_dylint_tools_with_locator(executor, &SystemBinaryLocator)
}

/// Check tool availability while retaining PATH diagnostics for the caller.
pub(crate) fn check_dylint_tools_with_locator(
    executor: &dyn CommandExecutor,
    locator: &dyn BinaryLocator,
) -> Result<DylintToolCheck> {
    let mut path_scan_failures = Vec::new();
    let cargo_dylint = is_tool_installed_with_locator(
        executor,
        &CARGO_DYLINT_TOOL,
        locator,
        &mut path_scan_failures,
    )?;
    let dylint_link = is_tool_installed_with_locator(
        executor,
        &DYLINT_LINK_TOOL,
        locator,
        &mut path_scan_failures,
    )?;

    Ok(DylintToolCheck {
        status: DylintToolStatus {
            cargo_dylint,
            dylint_link,
        },
        path_scan_failures,
    })
}

/// Install missing tools without emitting progress output.
pub fn install_dylint_tools(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
) -> Result<()> {
    let mut sink = io::sink();
    install_dylint_tools_with_output(
        executor,
        status,
        SourcePolicy {
            quiet: true,
            no_source_fallback: false,
        },
        &mut sink,
    )
}

/// Install missing tools while writing progress output to `stderr`.
pub fn install_dylint_tools_with_output(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    policy: SourcePolicy,
    stderr: &mut dyn Write,
) -> Result<()> {
    let repository_installer = RepositoryDependencyBinaryInstaller;
    let system_dirs = SystemBaseDirs::new();
    let target = host_target();
    let dirs = system_dirs.as_ref().map(|dirs| dirs as &dyn BaseDirs);
    let cargo_fallback_mode = cargo_fallback_mode(executor)?;
    install_missing_tools(
        executor,
        status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                dirs,
                Some(&repository_installer as &dyn DependencyBinaryInstaller),
                target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet: policy.quiet,
            no_source_fallback: policy.no_source_fallback,
        },
    )
}

/// Install missing tools with injected repository-install hooks.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn install_dylint_tools_with_options(
    executor: &dyn CommandExecutor,
    status: &DylintToolStatus,
    stderr: &mut dyn Write,
    options: DependencyInstallOptions<'_>,
) -> Result<()> {
    let cargo_fallback_mode = cargo_fallback_mode(executor)?;
    install_missing_tools(
        executor,
        status,
        stderr,
        &InstallContext {
            repo: repository_install_context(
                Some(options.dirs),
                Some(options.repository_installer),
                options.target.as_ref(),
            ),
            cargo_fallback_mode,
            quiet: options.policy.quiet,
            no_source_fallback: options.policy.no_source_fallback,
        },
    )
}

/// Arguments used to query Cargo's registry of installed binaries.
const CARGO_INSTALL_LIST_ARGS: [&str; 2] = ["install", "--list"];

/// Check whether a dependency tool is installed at its manifest version.
///
/// Dylint-link is checked through PATH and Cargo's install list because the
/// wrapper cannot reliably report its own version. The embedded dependency
/// manifest must provide the expected version before availability is decided.
fn is_tool_installed(executor: &dyn CommandExecutor, tool: &DependencyTool) -> Result<bool> {
    is_tool_installed_with_locator(executor, tool, &SystemBinaryLocator, &mut Vec::new())
}

/// Check one tool while returning any PATH failures to the application boundary.
fn is_tool_installed_with_locator(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    locator: &dyn BinaryLocator,
    path_scan_failures: &mut Vec<PathScanFailure>,
) -> Result<bool> {
    let dependency = find_dependency_binary(tool.package)
        .map_err(|error| InstallerError::DependencyCheck {
            tool: tool.package,
            reason: format!("dependency manifest could not be read: {error}"),
        })?
        .ok_or_else(|| InstallerError::DependencyCheck {
            tool: tool.package,
            reason: String::from("dependency manifest has no pinned version"),
        })?;
    let expected_version = dependency.version();

    if tool == &DYLINT_LINK_TOOL {
        return is_dylint_link_installed(executor, expected_version, locator, path_scan_failures);
    }
    is_versioned_tool_installed(executor, tool, expected_version)
}

/// Verify `dylint-link` presence and, when known, its Cargo-recorded version.
///
/// Presence is established by finding an executable on PATH without running
/// the linker wrapper. An expected version is compared with Cargo's install
/// list; skipped-entry classifications are traced here, without logging paths.
fn is_dylint_link_installed(
    executor: &dyn CommandExecutor,
    expected_version: &str,
    locator: &dyn BinaryLocator,
    path_scan_failures: &mut Vec<PathScanFailure>,
) -> Result<bool> {
    // Presence is established by resolving an executable file on PATH.
    // `dylint-link` is a linker wrapper with no reliable self-reporting
    // subcommand, so it is never executed to prove it works.
    let path_scan = locator.find_binary(DYLINT_LINK_TOOL.command);
    path_scan_failures.extend(path_scan.failures.iter().cloned());
    if path_scan.binary.is_none() {
        if path_scan.failures.is_empty() {
            return Ok(false);
        }
        let categories = path_scan
            .failures
            .iter()
            .map(|failure| failure.category.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(InstallerError::DependencyCheck {
            tool: DYLINT_LINK_TOOL.package,
            reason: format!(
                "PATH scanning encountered {categories} failures and found no executable"
            ),
        });
    }
    // `dylint-link` is a pure linker wrapper: it forwards its entire argument
    // list to the underlying linker and can never report its own version, so
    // `dylint-link --version` is not a usable version source. Query Cargo's
    // registry of installed binaries instead, which records the version each
    // binary was installed at.
    Ok(cargo_installed_version(executor, DYLINT_LINK_TOOL.package)?
        .is_some_and(|version| version == expected_version))
}

/// Probe a tool and, when available, compare its reported version.
///
/// A valid version mismatch means the pinned tool is absent. Unexpected command
/// failures and successful output without a version are query errors.
fn is_versioned_tool_installed(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
    expected_version: &str,
) -> Result<bool> {
    let output = executor.run(tool.command, tool.args)?;
    if !output.status.success() {
        if tool == &CARGO_DYLINT_TOOL && cargo_reports_missing_subcommand(&output) {
            return Ok(false);
        }
        return Err(InstallerError::DependencyCheck {
            tool: tool.package,
            reason: command_error_message(&output),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(actual_version) = first_semver_token(&stdout) else {
        return Err(InstallerError::DependencyCheck {
            tool: tool.package,
            reason: String::from("version command succeeded without reporting a semantic version"),
        });
    };
    Ok(actual_version == expected_version)
}

/// Recognize Cargo's explicit diagnostic for an absent `cargo-dylint` command.
fn cargo_reports_missing_subcommand(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    stderr.contains("no such command") && stderr.contains("dylint")
}

/// Extracts the installed version of `package` from `cargo install --list`.
///
/// Matches lines of the form `dylint-link v6.0.1:` (or
/// `dylint-link v6.0.1 (/path):` for path installs), returning the version
/// with the leading `v` and trailing `:` stripped.
fn cargo_installed_version(
    executor: &dyn CommandExecutor,
    package: &'static str,
) -> Result<Option<String>> {
    let output = executor.run("cargo", &CARGO_INSTALL_LIST_ARGS)?;
    if !output.status.success() {
        return Err(InstallerError::DependencyCheck {
            tool: package,
            reason: command_error_message(&output),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().find_map(|line| {
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some(package) {
            return None;
        }
        tokens
            .next()
            .and_then(|token| token.strip_prefix('v'))
            .map(|version| version.trim_end_matches(':').to_owned())
    }))
}

/// Returns the first whitespace-separated token shaped like `MAJOR.MINOR.PATCH`.
fn first_semver_token(text: &str) -> Option<&str> {
    text.split_whitespace().find(|token| {
        let components: Vec<&str> = token.split('.').collect();
        components.len() == 3
            && components.iter().all(|component| {
                !component.is_empty()
                    && component
                        .chars()
                        .all(|character| character.is_ascii_digit())
            })
    })
}

/// Return whether `cargo binstall --version` exits successfully.
///
/// This checks command availability only; it does not compare a version.
fn is_binstall_available(executor: &dyn CommandExecutor) -> Result<bool> {
    command_succeeds(executor, "cargo", &["binstall", "--version"])
}

/// Check whether a named executable resolves to a file on the current PATH.
///
/// This test helper reports presence only; it does not verify an installed
/// package version.
#[cfg(test)]
fn is_binary_on_path(binary_name: &str) -> bool {
    SystemBinaryLocator
        .find_binary(binary_name)
        .binary
        .is_some()
}

/// Find the first executable match while retaining skipped-entry classifications.
///
/// Return an empty report when PATH is unset. Otherwise, scan entries in their
/// configured order and return the classifications without emitting logs.
fn find_binary_on_path(binary_name: &str) -> PathScanResult {
    let Some(path_var) = std::env::var_os("PATH") else {
        return PathScanResult::default();
    };

    scan_path_directories(std::env::split_paths(&path_var), binary_name)
}

/// Scan ordered PATH entries and retain classified failures for the caller.
///
/// The report preserves every skipped-entry category before the first match so
/// the installer boundary can emit bounded diagnostics without coupling the
/// filesystem query helpers to tracing.
fn scan_path_directories(
    directories: impl IntoIterator<Item = std::path::PathBuf>,
    binary_name: &str,
) -> PathScanResult {
    let mut result = PathScanResult::default();
    for directory in directories {
        result.binary = find_binary_in_directory(&directory, binary_name, &mut result.failures);
        if result.binary.is_some() {
            break;
        }
    }
    result
}

/// Find the first executable candidate relative to one UTF-8 PATH directory.
///
/// Non-UTF-8 entries and directories that cannot be opened are skipped so the
/// caller can continue searching later PATH entries. Candidate metadata is
/// inspected through the opened directory capability rather than ambient
/// filesystem access. Failures are returned as categories without retaining
/// the directory path.
fn find_binary_in_directory(
    directory: &Path,
    binary_name: &str,
    failures: &mut Vec<PathScanFailure>,
) -> Option<std::path::PathBuf> {
    let Some(utf8_directory) = Utf8Path::from_path(directory) else {
        failures.push(PathScanFailure {
            binary_name: binary_name.to_owned(),
            category: PathScanFailureCategory::NonUtf8Path,
        });
        return None;
    };
    let directory_capability = match Dir::open_ambient_dir(utf8_directory, ambient_authority()) {
        Ok(directory_capability) => directory_capability,
        Err(error) => {
            if let Some(category) = directory_open_failure_category(error.kind()) {
                failures.push(PathScanFailure {
                    binary_name: binary_name.to_owned(),
                    category,
                });
            }
            return None;
        }
    };

    let mut failure_categories = Vec::new();
    let candidate = binary_candidates(binary_name)
        .into_iter()
        .find(|candidate| {
            is_executable_file(
                &directory_capability,
                Utf8Path::new(candidate),
                &mut failure_categories,
            )
        });
    failures.extend(
        failure_categories
            .into_iter()
            .map(|category| PathScanFailure {
                binary_name: binary_name.to_owned(),
                category,
            }),
    );
    candidate.map(|candidate| directory.join(candidate))
}

/// Return executable candidate names in their PATH-search order.
///
/// Unix returns the requested name unchanged. Windows expands extensionless
/// names using `PATHEXT`; an existing extension is preserved as the sole
/// candidate.
fn binary_candidates(binary_name: &str) -> Vec<String> {
    #[cfg(windows)]
    let mut candidates = Vec::new();
    #[cfg(not(windows))]
    let candidates = vec![binary_name.to_owned()];
    #[cfg(windows)]
    {
        if Path::new(binary_name).extension().is_some() {
            candidates.push(binary_name.to_owned());
        } else {
            let lowercase_name = binary_name.to_ascii_lowercase();
            candidates.extend(
                windows_path_extensions()
                    .into_iter()
                    .filter(|extension| !lowercase_name.ends_with(&extension.to_ascii_lowercase()))
                    .map(|extension| format!("{binary_name}{extension}")),
            );
        }
    }
    candidates
}

/// Parse `PATHEXT` into suffixes used to probe Windows PATH entries.
#[cfg(windows)]
fn windows_path_extensions() -> Vec<String> {
    let path_ext = std::env::var_os("PATHEXT")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| std::ffi::OsString::from(".COM;.EXE;.BAT;.CMD"));

    path_ext
        .to_string_lossy()
        .split(';')
        .filter_map(|extension| {
            let trimmed = extension.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.starts_with('.') {
                Some(trimmed.to_owned())
            } else {
                Some(format!(".{trimmed}"))
            }
        })
        .collect()
}

/// Classify a candidate through its containing directory capability.
///
/// Return whether the candidate is a regular file with an executable bit.
///
/// Missing candidates return `false` without a failure category; other
/// metadata errors add the bounded `Metadata` category and also return `false`.
#[cfg(unix)]
fn is_executable_file(
    directory: &Dir,
    candidate: &Utf8Path,
    failures: &mut Vec<PathScanFailureCategory>,
) -> bool {
    use cap_std::fs::PermissionsExt;

    match directory.metadata(candidate) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(error) => {
            if let Some(category) = metadata_failure_category(error.kind()) {
                failures.push(category);
            }
            false
        }
    }
}

/// Classify a candidate as a regular file through its directory capability.
///
/// Missing candidates return `false` without a failure category; other
/// metadata errors add the bounded `Metadata` category. Otherwise, return
/// whether the candidate is a regular file.
#[cfg(not(unix))]
fn is_executable_file(
    directory: &Dir,
    candidate: &Utf8Path,
    failures: &mut Vec<PathScanFailureCategory>,
) -> bool {
    match directory.metadata(candidate) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => {
            if let Some(category) = metadata_failure_category(error.kind()) {
                failures.push(category);
            }
            false
        }
    }
}

#[cfg(test)]
mod availability_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod path_scan_tests {
    //! Verify that PATH scan failure categories remain stable and bounded.

    use super::{
        PathScanFailureCategory, directory_open_failure_category, metadata_failure_category,
    };

    /// Treat an absent PATH candidate as routine while preserving other failures.
    #[test]
    fn metadata_failures_exclude_missing_candidates() {
        assert_eq!(
            metadata_failure_category(std::io::ErrorKind::NotFound),
            None
        );
        assert_eq!(
            metadata_failure_category(std::io::ErrorKind::PermissionDenied),
            Some(PathScanFailureCategory::Metadata)
        );
    }

    /// Treat disappeared PATH directories as misses while preserving other errors.
    #[test]
    fn directory_open_failures_exclude_missing_directories() {
        assert_eq!(
            directory_open_failure_category(std::io::ErrorKind::NotFound),
            None
        );
        assert_eq!(
            directory_open_failure_category(std::io::ErrorKind::PermissionDenied),
            Some(PathScanFailureCategory::DirectoryOpen)
        );
    }

    /// Keep path-scan failure category labels stable and bounded.
    #[test]
    fn path_scan_failure_categories_are_stable_and_bounded() {
        assert_eq!(
            PathScanFailureCategory::NonUtf8Path.as_str(),
            "non_utf8_path"
        );
        assert_eq!(
            PathScanFailureCategory::DirectoryOpen.as_str(),
            "directory_open"
        );
        assert_eq!(PathScanFailureCategory::Metadata.as_str(), "metadata");
    }
}
