//! List command implementation.
//!
//! This module provides the `run_list` command handler and supporting functions
//! for querying and displaying installed lint libraries.

use camino::Utf8PathBuf;
use log::trace;
use std::io::Write;

use crate::cli::ListArgs;
use crate::error::{InstallerError, Result};
use crate::list_output::{format_human, format_json};
use crate::scanner::scan_installed;
use crate::stager::default_target_dir;
use crate::toolchain::Toolchain;

/// Lists installed lint libraries and their associated lints.
///
/// Scans the staging directory for installed libraries, detects the active
/// toolchain from `rust-toolchain.toml` in the current directory (if present),
/// and formats the output for display.
///
/// Output is written to stdout (human-readable by default, JSON with `--json`).
///
/// # Errors
///
/// Returns an error if:
/// - The staging directory cannot be scanned
/// - Writing to stdout fails
pub fn run_list(args: &ListArgs, stdout: &mut dyn Write) -> Result<()> {
    let target_dir = determine_target_dir(args.target_dir.clone())?;

    let installed = scan_installed(&target_dir).map_err(|e| InstallerError::ScanFailed {
        reason: e.to_string(),
    })?;

    let active_toolchain = detect_active_toolchain();

    let output = if args.json {
        format_json(&installed, active_toolchain.as_deref())
    } else {
        format_human(&installed, active_toolchain.as_deref())
    };

    writeln!(stdout, "{output}").map_err(|e| InstallerError::WriteFailed {
        reason: e.to_string(),
    })?;

    Ok(())
}

/// Detect the active toolchain from `rust-toolchain.toml` in the current directory.
///
/// Returns `None` if:
/// - The current directory cannot be determined
/// - The path is not valid UTF-8
/// - No `rust-toolchain.toml` file exists
/// - The toolchain file cannot be parsed
pub fn detect_active_toolchain() -> Option<String> {
    let cwd = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            trace!("detect_active_toolchain: failed to get current dir: {e}");
            return None;
        }
    };

    let utf8_cwd = match Utf8PathBuf::try_from(cwd) {
        Ok(path) => path,
        Err(e) => {
            trace!("detect_active_toolchain: current dir is not valid UTF-8: {e}");
            return None;
        }
    };

    detect_active_toolchain_in(&utf8_cwd)
}

/// Detect the active toolchain from `rust-toolchain.toml` in the given directory.
///
/// This is the internal implementation that accepts a path for testability.
/// Use [`detect_active_toolchain`] for production code.
///
/// Returns `None` if:
/// - No `rust-toolchain.toml` file exists in the directory
/// - The toolchain file cannot be parsed
pub(crate) fn detect_active_toolchain_in(dir: &Utf8PathBuf) -> Option<String> {
    match Toolchain::detect(dir) {
        Ok(tc) => Some(tc.channel().to_owned()),
        Err(e) => {
            trace!("detect_active_toolchain_in: toolchain detection failed: {e}");
            None
        }
    }
}

/// Determines the target directory from CLI or falls back to the default.
///
/// If a target directory is provided via CLI, it is used directly. Otherwise,
/// the default staging directory from [`crate::stager::default_target_dir`] is
/// used.
///
/// # Errors
///
/// Returns [`InstallerError::StagingFailed`] if no target directory can be
/// determined (neither provided nor default available).
pub fn determine_target_dir(cli_target: Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    cli_target
        .or_else(default_target_dir)
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "could not determine default target directory".to_owned(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::fs;
    use tempfile::TempDir;

    // -------------------------------------------------------------------------
    // Fixtures
    // -------------------------------------------------------------------------

    /// A temporary directory converted to a UTF-8 path for test isolation.
    struct TempTarget {
        _temp: TempDir,
        path: Utf8PathBuf,
    }

    #[fixture]
    fn temp_target() -> TempTarget {
        let temp = TempDir::new().expect("failed to create temp dir");
        let path = Utf8PathBuf::try_from(temp.path().to_owned()).expect("non-UTF8 temp path");
        TempTarget { _temp: temp, path }
    }

    /// A Write implementation that always fails, for testing error paths.
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("simulated write failure"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("simulated flush failure"))
        }
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Helper to create a mock installed library in the target directory for tests
    fn create_mock_library(target_dir: &Utf8PathBuf, toolchain: &str) {
        let release_dir = target_dir.join(toolchain).join("release");
        fs::create_dir_all(&release_dir).expect("failed to create release dir");
        fs::write(
            release_dir.join(format!("libsuite@{toolchain}.so")),
            b"mock library",
        )
        .expect("failed to create mock library");
    }

    // -------------------------------------------------------------------------
    // run_list tests
    // -------------------------------------------------------------------------

    #[rstest]
    fn run_list_outputs_human_readable_format(temp_target: TempTarget) {
        let args = ListArgs {
            json: false,
            target_dir: Some(temp_target.path.clone()),
        };
        let mut stdout = Vec::new();

        let result = run_list(&args, &mut stdout);

        assert!(result.is_ok(), "expected success, got: {result:?}");
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("No lints installed"), "got: {output}");
    }

    #[rstest]
    fn run_list_outputs_json_format(temp_target: TempTarget) {
        create_mock_library(&temp_target.path, "nightly-2025-09-18");
        let args = ListArgs {
            json: true,
            target_dir: Some(temp_target.path.clone()),
        };
        let mut stdout = Vec::new();

        let result = run_list(&args, &mut stdout);

        assert!(result.is_ok(), "expected success, got: {result:?}");
        let output = String::from_utf8_lossy(&stdout);
        assert!(output.contains("toolchains"), "expected 'toolchains' field");
        assert!(
            output.contains("\"active\""),
            "expected 'active' field in toolchain entries"
        );
    }

    #[rstest]
    fn run_list_returns_write_failed_on_stdout_error(temp_target: TempTarget) {
        let args = ListArgs {
            json: false,
            target_dir: Some(temp_target.path.clone()),
        };
        let mut failing_stdout = FailingWriter;

        let result = run_list(&args, &mut failing_stdout);

        assert!(result.is_err(), "expected error on write failure");
        assert!(matches!(
            result.unwrap_err(),
            InstallerError::WriteFailed { .. }
        ));
    }

    #[rstest]
    fn run_list_scans_installed_libraries(temp_target: TempTarget) {
        create_mock_library(&temp_target.path, "nightly-2025-09-18");
        let args = ListArgs {
            json: false,
            target_dir: Some(temp_target.path.clone()),
        };
        let mut stdout = Vec::new();

        let result = run_list(&args, &mut stdout);

        assert!(result.is_ok(), "expected success, got: {result:?}");
        let output = String::from_utf8_lossy(&stdout);
        assert!(
            output.contains("nightly-2025-09-18"),
            "expected toolchain in output, got: {output}"
        );
        assert!(
            output.contains("suite"),
            "expected suite crate in output, got: {output}"
        );
    }

    // -------------------------------------------------------------------------
    // detect_active_toolchain_in tests
    // -------------------------------------------------------------------------

    #[rstest]
    fn detect_active_toolchain_in_returns_none_when_no_toolchain_file(temp_target: TempTarget) {
        let result = detect_active_toolchain_in(&temp_target.path);
        assert!(
            result.is_none(),
            "expected None for directory without rust-toolchain.toml"
        );
    }

    #[rstest]
    fn detect_active_toolchain_in_returns_channel_when_toolchain_file_exists(
        temp_target: TempTarget,
    ) {
        // Create a rust-toolchain.toml file
        let toolchain_content = r#"[toolchain]
channel = "nightly-2025-09-18"
"#;
        fs::write(
            temp_target.path.join("rust-toolchain.toml"),
            toolchain_content,
        )
        .expect("failed to write rust-toolchain.toml");

        let result = detect_active_toolchain_in(&temp_target.path);

        assert_eq!(result, Some("nightly-2025-09-18".to_owned()));
    }
}
