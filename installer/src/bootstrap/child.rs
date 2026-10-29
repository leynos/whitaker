//! The child `cargo dylint` process: its release check and its exit status.

use super::error::BootstrapError;
use std::ffi::OsStr;
use std::process::{Command, ExitStatus};

/// Require the `cargo-dylint` on `PATH` to be the bootstrapped release.
pub(super) fn check_cargo_dylint(cargo: &OsStr, expected: &str) -> Result<(), BootstrapError> {
    let output = Command::new(cargo)
        .args(["dylint", "--version"])
        .output()
        .map_err(|error| BootstrapError::CargoDylint {
            reason: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(BootstrapError::CargoDylint {
            reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let found = stdout.split_whitespace().last().unwrap_or_default();
    if found == expected {
        Ok(())
    } else {
        Err(BootstrapError::DylintVersion {
            expected: expected.to_owned(),
            found: found.to_owned(),
        })
    }
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
