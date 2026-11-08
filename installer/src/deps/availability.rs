//! Availability queries for the Dylint dependency tools.
//!
//! Answers whether each tool is present at its pinned version without
//! installing anything. Filesystem access happens through the [`BinaryLocator`]
//! supplied by the caller, so these queries stay testable without touching the
//! host PATH.

use std::process::Output;

use super::{
    CARGO_DYLINT_TOOL,
    CommandExecutor,
    DYLINT_LINK_TOOL,
    DependencyTool,
    DylintToolCheck,
    DylintToolStatus,
    PathScanFailure,
    SystemBinaryLocator,
    install_cargo::{command_error_message, command_succeeds},
    path_scan::BinaryLocator,
};
use crate::{
    dependency_binaries::find_dependency_binary,
    error::{InstallerError, Result},
};

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

/// Arguments used to query Cargo's registry of installed binaries.
const CARGO_INSTALL_LIST_ARGS: [&str; 2] = ["install", "--list"];

/// Check whether a dependency tool is installed at its manifest version.
///
/// Dylint-link is checked through PATH and Cargo's install list because the
/// wrapper cannot reliably report its own version. The embedded dependency
/// manifest must provide the expected version before availability is decided.
pub(super) fn is_tool_installed(
    executor: &dyn CommandExecutor,
    tool: &DependencyTool,
) -> Result<bool> {
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
/// list. If PATH scanning finds no executable after entry failures, or the
/// Cargo query fails, return an error; skipped-entry classifications are
/// collected for the caller.
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
/// Matches lines of the form `dylint-link v6.3.0:` (or
/// `dylint-link v6.3.0 (/path):` for path installs), returning the version
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
pub(super) fn is_binstall_available(executor: &dyn CommandExecutor) -> Result<bool> {
    command_succeeds(executor, "cargo", &["binstall", "--version"])
}
