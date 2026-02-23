//! Install-flow helpers for the installer binary.
//!
//! This module keeps prebuilt-download fallback and metrics recording logic
//! separate from CLI orchestration in `main.rs`.

use camino::Utf8PathBuf;
use std::io::Write;
use std::time::Duration;
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::install_metrics::{InstallMode, RecordOutcome, record_install};
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt};
use whitaker_installer::prebuilt_path::prebuilt_library_dir;

/// Context needed to attempt prebuilt installation.
pub(crate) struct PrebuiltInstallationContext<'a> {
    /// CLI install arguments.
    pub(crate) args: &'a InstallArgs,
    /// Base directory provider.
    pub(crate) dirs: &'a dyn BaseDirs,
    /// Crates requested for this installation.
    pub(crate) requested_crates: &'a [CrateName],
    /// Toolchain channel resolved for this install.
    pub(crate) toolchain_channel: &'a str,
}

/// Context for recording one successful install in aggregate metrics.
pub(crate) struct MetricsWriteContext<'a> {
    /// Whether installer output is suppressed.
    pub(crate) quiet: bool,
    /// Base directory provider used to locate Whitaker data directory.
    pub(crate) dirs: &'a dyn BaseDirs,
    /// Terminal install mode (download or build).
    pub(crate) install_mode: InstallMode,
    /// Elapsed duration for this successful install run.
    pub(crate) elapsed: Duration,
}

/// Write fallback message when prebuilt installation fails.
pub(crate) fn write_prebuilt_fallback_message(
    quiet: bool,
    error: &dyn std::fmt::Display,
    stderr: &mut dyn Write,
) {
    if quiet {
        return;
    }
    write_stderr_line(stderr, format!("Prebuilt download unavailable: {error}"));
    write_stderr_line(stderr, "Falling back to local compilation.");
    write_stderr_line(stderr, "");
}

/// Attempt prebuilt installation and return staged path when successful.
pub(crate) fn try_prebuilt_installation(
    context: &PrebuiltInstallationContext<'_>,
    stderr: &mut dyn Write,
) -> Result<Option<Utf8PathBuf>> {
    if !context
        .args
        .should_attempt_prebuilt(context.requested_crates)
    {
        return Ok(None);
    }

    let host_target = match detect_host_target() {
        Ok(target) => target,
        Err(error) => {
            write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
            return Ok(None);
        }
    };

    let destination_dir =
        match prebuilt_library_dir(context.dirs, context.toolchain_channel, &host_target) {
            Ok(destination) => destination,
            Err(error) => {
                write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
                return Ok(None);
            }
        };

    let prebuilt_config = PrebuiltConfig {
        target: &host_target,
        toolchain: context.toolchain_channel,
        destination_dir: &destination_dir,
        quiet: context.args.quiet,
    };

    let PrebuiltResult::Success { staging_path } = attempt_prebuilt(&prebuilt_config, stderr)
    else {
        return Ok(None);
    };
    Ok(Some(staging_path))
}

/// Detect the host target triple by parsing `rustc -vV` output.
pub(crate) fn detect_host_target() -> Result<String> {
    let output = std::process::Command::new("rustc")
        .args(["-vV"])
        .output()
        .map_err(|error| InstallerError::ToolchainDetection {
            reason: format!("failed to run `rustc -vV`: {error}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InstallerError::ToolchainDetection {
            reason: format!(
                "`rustc -vV` exited with {}: {}",
                output.status,
                stderr.trim()
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Ok(host.trim().to_owned());
        }
    }
    Err(InstallerError::ToolchainDetection {
        reason: "could not determine host target from `rustc -vV`".to_owned(),
    })
}

/// Best-effort metrics recording for successful installation runs.
pub(crate) fn write_install_metrics(context: &MetricsWriteContext<'_>, stderr: &mut dyn Write) {
    match record_install(context.dirs, context.install_mode, context.elapsed) {
        Ok(record_outcome) => write_metrics_summary(context.quiet, &record_outcome, stderr),
        Err(error) => {
            if !context.quiet {
                write_stderr_line(
                    stderr,
                    format!("Warning: could not record install metrics: {error}"),
                );
            }
        }
    }
}

fn write_metrics_summary(quiet: bool, record_outcome: &RecordOutcome, stderr: &mut dyn Write) {
    if quiet {
        return;
    }

    if record_outcome.recovered_from_corrupt_file() {
        write_stderr_line(
            stderr,
            "Install metrics file was invalid and has been reset.",
        );
    }
    write_stderr_line(stderr, record_outcome.metrics().summary_line());
}
