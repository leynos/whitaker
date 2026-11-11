//! Finalization shared by successful installer paths.
//!
//! Every successful install ends the same way: record where the libraries were
//! staged so the root CLI can forward Dylint invocations from outside a
//! checkout, record the bootstrap the wrappers dispatch through, and then
//! either write the wrapper scripts or print the equivalent shell snippet. The
//! root CLI treats a failed staging record as fatal because forwarding depends
//! on it; the legacy installer only warns, because it can still be driven from
//! inside the repository.

use std::{io::Write, time::Instant};

use camino::Utf8Path;

use crate::{
    cli::InstallArgs,
    dirs::BaseDirs,
    error::{InstallerError, Result},
    install_flow::{
        MetricsWriteContext,
        bootstrap_step::{InstalledSnapshot, record_bootstrap, write_shell_snippet},
        write_install_metrics,
    },
    install_metrics::InstallMode,
    installation_record::record_installation,
    output::write_stderr_line,
    wrapper::{WrapperResult, WrapperScripts, generate_wrapper_scripts_with, path_instructions},
};

/// Counts installation-record operations by operation and outcome.
///
/// The root CLI's forwarding boundary emits the `read` side under the same
/// name, so one metric covers the record's whole lifecycle.
const INSTALLATION_RECORD_TOTAL: &str = "whitaker.installation_record.total";

/// Count one installation-record write and whether it succeeded.
///
/// The outcome is recorded here rather than folded into the install result
/// because the legacy installer treats a record failure as a warning, so the
/// install still succeeds and nothing else would report the failure.
fn record_write_outcome(scripts: WrapperScripts, succeeded: bool) {
    let installer = if scripts.is_root_cli() {
        "root"
    } else {
        "legacy"
    };
    let outcome = if succeeded { "ok" } else { "failed" };
    metrics::describe_counter!(
        INSTALLATION_RECORD_TOTAL,
        "Installation-record operations, by operation and outcome"
    );
    metrics::counter!(
        INSTALLATION_RECORD_TOTAL,
        "operation" => "write",
        "outcome" => outcome,
        "installer" => installer
    )
    .increment(1);
}

/// Aggregates final-step inputs required after a successful install run.
pub(super) struct FinishInstallContext<'a> {
    pub(super) args: &'a InstallArgs,
    pub(super) dirs: &'a dyn BaseDirs,
    /// The Clippy snapshot installed before any library work.
    pub(super) snapshot: &'a InstalledSnapshot,
    /// The toolchain the staged libraries were built for.
    pub(super) toolchain: &'a str,
    pub(super) staging_path: &'a Utf8Path,
    pub(super) wrapper_scripts: WrapperScripts,
    pub(super) install_mode: InstallMode,
    pub(super) install_started: Instant,
}

/// The staging record a successful install writes before anything else.
///
/// Split out from [`FinishInstallContext`] because the record is written
/// before the snapshot is reported on, and because the record-failure policy
/// can then be exercised without building an [`InstalledSnapshot`], whose
/// fields are private to the bootstrap step.
pub(super) struct StagingRecord<'a> {
    pub(super) dirs: &'a dyn BaseDirs,
    pub(super) toolchain: &'a str,
    pub(super) staging_path: &'a Utf8Path,
    pub(super) wrapper_scripts: WrapperScripts,
}

impl<'a> StagingRecord<'a> {
    /// Borrow the record inputs from a completed install.
    const fn from_context(context: &FinishInstallContext<'a>) -> Self {
        Self {
            dirs: context.dirs,
            toolchain: context.toolchain,
            staging_path: context.staging_path,
            wrapper_scripts: context.wrapper_scripts,
        }
    }
}

/// Finalize installation and record aggregate installer metrics.
pub(super) fn finish_install_and_record_metrics(
    context: &FinishInstallContext<'_>,
    stderr: &mut dyn Write,
) -> Result<()> {
    record_installed_path(&StagingRecord::from_context(context), stderr)?;
    finish_install(context, stderr)?;
    let metrics_context = MetricsWriteContext {
        quiet: context.args.quiet,
        dirs: context.dirs,
        install_mode: context.install_mode,
        elapsed: context.install_started.elapsed(),
    };
    write_install_metrics(&metrics_context, stderr);
    Ok(())
}

/// Record the staging path, tolerating failure only for the legacy installer.
///
/// The root CLI forwards into the recorded path from an arbitrary directory, so
/// a silent failure would leave it pointing at a stale or missing library
/// directory. The legacy installer can still be invoked from within the
/// repository, where the default target is discoverable, so it warns instead.
fn record_installed_path(record: &StagingRecord<'_>, stderr: &mut dyn Write) -> Result<()> {
    let recorded = record_installation(record.dirs, record.toolchain, record.staging_path);
    record_write_outcome(record.wrapper_scripts, recorded.is_ok());
    if let Err(record_error) = recorded {
        let error = InstallerError::InstallationRecord {
            message: record_error.to_string(),
        };
        if record.wrapper_scripts.is_root_cli() {
            return Err(error);
        }
        write_stderr_line(stderr, format!("Warning: {error}"));
    }
    Ok(())
}

/// Record the bootstrap, then generate wrappers or print a shell snippet.
///
/// The record is written after the libraries are staged and before any wrapper
/// exists, so a wrapper never points at a half-recorded installation.
fn finish_install(context: &FinishInstallContext<'_>, stderr: &mut dyn Write) -> Result<()> {
    record_bootstrap(context.snapshot, context.toolchain, context.staging_path)?;
    if context.args.skip.skip_wrapper {
        write_shell_snippet(context.snapshot, context.staging_path, stderr);
    } else if context.wrapper_scripts.is_root_cli() {
        write_stderr_line(stderr, "");
        report_installed_binary(stderr);
    } else {
        generate_and_report_wrapper(context.dirs, context.wrapper_scripts, stderr)?;
    }
    Ok(())
}

/// Generates wrapper scripts and reports the result.
fn generate_and_report_wrapper(
    dirs: &dyn BaseDirs,
    scripts: WrapperScripts,
    stderr: &mut dyn Write,
) -> Result<()> {
    let result = generate_wrapper_scripts_with(dirs, scripts)?;
    write_stderr_line(stderr, "");
    write_stderr_line(stderr, "Wrapper scripts created:");
    if scripts == WrapperScripts::Both {
        write_stderr_line(stderr, format!("  - {}", result.whitaker_path.display()));
    }
    write_stderr_line(stderr, format!("  - {}", result.whitaker_ls_path.display()));
    write_stderr_line(stderr, "");

    if result.in_path {
        write_stderr_line(stderr, "You can now run:");
        write_stderr_line(stderr, "  whitaker --all");
        write_stderr_line(stderr, "  whitaker-ls");
    } else {
        let bin_dir = wrapper_bin_dir(&result)?;
        write_stderr_line(stderr, path_instructions(bin_dir));
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Then run:");
        write_stderr_line(stderr, "  whitaker --all");
        write_stderr_line(stderr, "  whitaker-ls");
    }
    Ok(())
}

/// Reports how to use the installed root binary, which needs no script.
///
/// The root CLI supplies `ls` itself, so the legacy `whitaker-ls` hint would
/// name a script that the root install deliberately does not create. The hint
/// is deliberately static: resolving the wrapper directory here would create
/// an empty directory the root install has no use for, and the wrapper paths
/// describe where scripts would go rather than where the root binary lives.
fn report_installed_binary(stderr: &mut dyn Write) {
    write_stderr_line(stderr, "You can now run:");
    write_stderr_line(stderr, "  whitaker ls");
    write_stderr_line(stderr, "  whitaker --all");
}

/// Resolves the directory that holds the conventional wrapper paths.
fn wrapper_bin_dir(result: &WrapperResult) -> Result<&std::path::Path> {
    result
        .whitaker_path
        .parent()
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "wrapper script path has no parent directory".to_owned(),
        })
}

#[cfg(test)]
mod tests;
