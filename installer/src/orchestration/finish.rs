//! Finalization shared by successful installer paths.

use std::{io::Write, time::Instant};

use camino::Utf8Path;

use crate::{
    cli::InstallArgs,
    dirs::BaseDirs,
    error::{InstallerError, Result},
    install_flow::{MetricsWriteContext, write_install_metrics},
    install_metrics::InstallMode,
    installation_record::record_installation,
    output::{ShellSnippet, write_stderr_line},
    toolchain::Toolchain,
    wrapper::{WrapperScripts, generate_wrapper_scripts_with, path_instructions},
};

/// Aggregates final-step inputs required after a successful install run.
pub(super) struct FinishInstallContext<'a> {
    pub(super) args: &'a InstallArgs,
    pub(super) dirs: &'a dyn BaseDirs,
    pub(super) toolchain: &'a Toolchain,
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
    if let Err(record_error) = record_installation(
        context.dirs,
        context.toolchain.channel(),
        context.staging_path,
    ) {
        let error = InstallerError::InstallationRecord {
            message: record_error.to_string(),
        };
        if context.wrapper_scripts == WrapperScripts::WhitakerLsOnly {
            return Err(error);
        }
        write_stderr_line(stderr, format!("Warning: {error}"));
    }
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

/// Common final steps: generate wrapper scripts or print shell snippet.
fn finish_install(context: &FinishInstallContext<'_>, stderr: &mut dyn Write) -> Result<()> {
    if context.args.skip.skip_wrapper {
        write_stderr_line(stderr, "");
        write_stderr_line(
            stderr,
            ShellSnippet::new(context.staging_path).display_text(),
        );
    } else {
        generate_and_report_wrapper(
            context.dirs,
            context.staging_path,
            context.wrapper_scripts,
            stderr,
        )?;
    }
    Ok(())
}

/// Generates wrapper scripts and reports the result.
fn generate_and_report_wrapper(
    dirs: &dyn BaseDirs,
    staging_path: &Utf8Path,
    scripts: WrapperScripts,
    stderr: &mut dyn Write,
) -> Result<()> {
    let result = generate_wrapper_scripts_with(dirs, staging_path, scripts)?;
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
        let bin_dir =
            result
                .whitaker_path
                .parent()
                .ok_or_else(|| InstallerError::StagingFailed {
                    reason: "wrapper script path has no parent directory".to_owned(),
                })?;
        write_stderr_line(stderr, path_instructions(bin_dir));
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Then run:");
        write_stderr_line(stderr, "  whitaker --all");
        write_stderr_line(stderr, "  whitaker-ls");
    }
    Ok(())
}
