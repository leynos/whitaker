//! Install-flow helpers for the installer binary.
//!
//! This module keeps prebuilt-download fallback and metrics recording logic
//! separate from CLI orchestration in `main.rs`.

use camino::Utf8Path;
use camino::Utf8PathBuf;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::Write;
use std::time::Duration;
use whitaker_installer::builder::{library_extension, library_prefix};
use whitaker_installer::cli::InstallArgs;
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::deps::{
    CommandExecutor, check_dylint_tools, install_dylint_tools_with_output,
};
#[cfg(test)]
use whitaker_installer::deps::{DependencyInstallOptions, install_dylint_tools_with_options};
use whitaker_installer::dirs::BaseDirs;
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::install_metrics::{InstallMode, RecordOutcome, record_install};
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::prebuilt::{PrebuiltConfig, PrebuiltResult, attempt_prebuilt};
use whitaker_installer::prebuilt_path::prebuilt_library_dir;
use whitaker_installer::resolution::{EXPERIMENTAL_LINT_CRATES, LINT_CRATES, SUITE_CRATE};

pub(crate) fn ensure_dylint_tools_core(
    quiet: bool,
    stderr: &mut dyn Write,
    is_all_installed: bool,
    do_install: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    if is_all_installed {
        return Ok(());
    }

    if !quiet {
        write_stderr_line(stderr, "Installing required Dylint tools...");
    }

    do_install(stderr)?;

    if !quiet {
        write_stderr_line(stderr, "Dylint tools installed successfully.");
        write_stderr_line(stderr, "");
    }

    Ok(())
}

pub(crate) fn ensure_dylint_tools_with_executor(
    executor: &dyn CommandExecutor,
    quiet: bool,
    stderr: &mut dyn Write,
) -> Result<()> {
    let status = check_dylint_tools(executor);
    ensure_dylint_tools_core(quiet, stderr, status.all_installed(), |stderr| {
        install_dylint_tools_with_output(executor, &status, quiet, stderr)
    })
}

#[cfg(test)]
pub(crate) fn ensure_dylint_tools_with_options(
    executor: &dyn CommandExecutor,
    stderr: &mut dyn Write,
    options: DependencyInstallOptions<'_>,
) -> Result<()> {
    let status = check_dylint_tools(executor);
    ensure_dylint_tools_core(options.quiet, stderr, status.all_installed(), |stderr| {
        install_dylint_tools_with_options(executor, &status, stderr, options)
    })
}

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
    try_prebuilt_installation_with(
        context,
        stderr,
        PrebuiltInstallationHooks {
            detect_host_target,
            resolve_destination_dir: prebuilt_library_dir,
            attempt_prebuilt,
            prune_prebuilt_libraries,
        },
    )
}

type DetectHostTargetFn = fn() -> Result<String>;
type ResolveDestinationDirFn = fn(&dyn BaseDirs, &str, &str) -> Result<Utf8PathBuf>;
type AttemptPrebuiltFn = fn(&PrebuiltConfig<'_>, &mut dyn Write) -> PrebuiltResult;
type PruneLibrariesFn = fn(&Utf8Path, &str, &[CrateName]) -> Result<()>;

struct PrebuiltInstallationHooks {
    detect_host_target: DetectHostTargetFn,
    resolve_destination_dir: ResolveDestinationDirFn,
    attempt_prebuilt: AttemptPrebuiltFn,
    prune_prebuilt_libraries: PruneLibrariesFn,
}

fn try_prebuilt_installation_with(
    context: &PrebuiltInstallationContext<'_>,
    stderr: &mut dyn Write,
    hooks: PrebuiltInstallationHooks,
) -> Result<Option<Utf8PathBuf>> {
    let PrebuiltInstallationHooks {
        detect_host_target,
        resolve_destination_dir,
        attempt_prebuilt,
        prune_prebuilt_libraries,
    } = hooks;

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
        match resolve_destination_dir(context.dirs, context.toolchain_channel, &host_target) {
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
    if let Err(error) = prune_prebuilt_libraries(
        &staging_path,
        context.toolchain_channel,
        context.requested_crates,
    ) {
        write_prebuilt_fallback_message(context.args.quiet, &error, stderr);
        return Ok(None);
    }
    Ok(Some(staging_path))
}

fn requested_crate_names(requested_crates: &[CrateName]) -> HashSet<&str> {
    if requested_crates.is_empty() {
        return HashSet::from([SUITE_CRATE]);
    }
    requested_crates.iter().map(CrateName::as_str).collect()
}

fn staged_library_filename(crate_name: &str, toolchain_channel: &str) -> String {
    let normalized = crate_name.replace('-', "_");
    format!(
        "{}{}@{}{}",
        library_prefix(),
        normalized,
        toolchain_channel,
        library_extension()
    )
}

/// Remove staged prebuilt libraries that were not requested for this install.
///
/// Prebuilt archives can contain both suite and constituent crates, while local
/// builds stage only requested crates. Pruning keeps prebuilt installs aligned
/// with local-build behaviour and avoids duplicate lint registration when the
/// wrapper runs `cargo dylint --all`.
fn prune_prebuilt_libraries(
    staging_path: &Utf8Path,
    toolchain_channel: &str,
    requested_crates: &[CrateName],
) -> Result<()> {
    let requested = requested_crate_names(requested_crates);
    let known_crates = LINT_CRATES
        .iter()
        .copied()
        .chain(EXPERIMENTAL_LINT_CRATES.iter().copied())
        .chain(std::iter::once(SUITE_CRATE));

    for crate_name in known_crates {
        if requested.contains(crate_name) {
            continue;
        }

        let stale_path = staging_path.join(staged_library_filename(crate_name, toolchain_channel));
        match fs::remove_file(stale_path.as_std_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(InstallerError::StagingFailed {
                    reason: format!(
                        "failed to remove stale prebuilt library {stale_path}: {error}"
                    ),
                });
            }
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests;
