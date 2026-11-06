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
    if let Err(record_error) =
        record_installation(record.dirs, record.toolchain, record.staging_path)
    {
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

#[cfg(test)]
mod tests {
    //! Tests for the staging-record failure policy.

    use std::path::PathBuf;

    use camino::Utf8PathBuf;
    use tempfile::TempDir;

    use super::{StagingRecord, record_installed_path};
    use crate::{dirs::BaseDirs, error::InstallerError, wrapper::WrapperScripts};

    /// A directory provider with no Whitaker data directory.
    ///
    /// Recording needs a data directory, so this is the smallest way to make
    /// `record_installation` fail without touching the host's filesystem.
    struct NoDataDirectory;

    impl BaseDirs for NoDataDirectory {
        fn home(&self) -> Option<PathBuf> { None }

        fn executables(&self) -> Option<PathBuf> { None }

        fn whitaker_data(&self) -> Option<PathBuf> { None }
    }

    /// A staging directory that exists, so only the record path can fail.
    fn staging_directory() -> (TempDir, Utf8PathBuf) {
        let directory = TempDir::new().expect("a temporary staging directory should be created");
        let path = Utf8PathBuf::try_from(directory.path().to_path_buf())
            .expect("a temporary path should be UTF-8");
        (directory, path)
    }

    #[test]
    fn a_root_install_treats_a_record_failure_as_fatal() {
        let (_staging, staging_path) = staging_directory();
        let record = StagingRecord {
            dirs: &NoDataDirectory,
            toolchain: "nightly-2026-05-28",
            staging_path: &staging_path,
            wrapper_scripts: WrapperScripts::None,
        };
        let mut stderr = Vec::new();

        let error = record_installed_path(&record, &mut stderr)
            .expect_err("the root CLI cannot forward without a recorded path");

        assert!(
            matches!(error, InstallerError::InstallationRecord { .. }),
            "expected an installation-record error, got {error:?}"
        );
        assert!(
            stderr.is_empty(),
            "a fatal record failure should not also warn: {}",
            String::from_utf8_lossy(&stderr)
        );
    }

    #[test]
    fn a_legacy_install_warns_about_a_record_failure_and_continues() {
        let (_staging, staging_path) = staging_directory();
        let record = StagingRecord {
            dirs: &NoDataDirectory,
            toolchain: "nightly-2026-05-28",
            staging_path: &staging_path,
            wrapper_scripts: WrapperScripts::Both,
        };
        let mut stderr = Vec::new();

        record_installed_path(&record, &mut stderr)
            .expect("the legacy installer can still run from inside the repository");

        let warning = String::from_utf8(stderr).expect("the warning should be UTF-8");
        assert!(
            warning.starts_with("Warning: "),
            "the legacy installer should warn, got {warning:?}"
        );
        assert!(
            warning.contains("failed to record installed lint path"),
            "the warning should name the failure, got {warning:?}"
        );
    }
}
