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

/// Finalize installation and record aggregate installer metrics.
pub(super) fn finish_install_and_record_metrics(
    context: &FinishInstallContext<'_>,
    stderr: &mut dyn Write,
) -> Result<()> {
    record_installed_path(context, stderr)?;
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
fn record_installed_path(context: &FinishInstallContext<'_>, stderr: &mut dyn Write) -> Result<()> {
    if let Err(record_error) =
        record_installation(context.dirs, context.toolchain, context.staging_path)
    {
        let error = InstallerError::InstallationRecord {
            message: record_error.to_string(),
        };
        if context.wrapper_scripts.is_root_cli() {
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
    if scripts.is_root_cli() {
        report_installed_binary(&result, stderr)?;
        return Ok(());
    }
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
/// name a script that the root install deliberately does not create.
fn report_installed_binary(result: &WrapperResult, stderr: &mut dyn Write) -> Result<()> {
    if result.in_path {
        write_stderr_line(stderr, "You can now run:");
    } else {
        write_stderr_line(stderr, path_instructions(wrapper_bin_dir(result)?));
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Then run:");
    }
    write_stderr_line(stderr, "  whitaker ls");
    write_stderr_line(stderr, "  whitaker --all");
    Ok(())
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
