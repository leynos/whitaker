//! The child `cargo dylint` process: its release check and its exit status.

use std::{
    ffi::OsStr,
    process::{Command, ExitStatus, Output},
};

use super::error::BootstrapError;

/// Require the `cargo-dylint` on `PATH` to be the bootstrapped release.
#[tracing::instrument(name = "bootstrap.cargo_dylint_version", skip_all, fields(expected = %expected))]
pub(super) fn check_cargo_dylint(cargo: &OsStr, expected: &str) -> Result<(), BootstrapError> {
    let output = dylint_version_output(cargo)?;
    let found = reported_version(&output);
    if found == expected {
        return Ok(());
    }
    Err(BootstrapError::DylintVersion {
        expected: expected.to_owned(),
        found,
    })
}

/// Run `cargo dylint --version`, requiring a successful exit.
fn dylint_version_output(cargo: &OsStr) -> Result<Output, BootstrapError> {
    let output = Command::new(cargo)
        .args(["dylint", "--version"])
        .output()
        .map_err(|error| BootstrapError::CargoDylint {
            reason: error.to_string(),
        })?;
    if output.status.success() {
        return Ok(output);
    }
    Err(BootstrapError::CargoDylint {
        reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// Return the version token printed by `cargo dylint --version`.
fn reported_version(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_owned()
}

/// Map a child's status to the code this process should exit with.
///
/// A normal exit keeps its code exactly. A Unix process killed by a signal
/// has no code, so it maps to `128 + signal`, the value a shell reports.
#[must_use]
pub fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}
