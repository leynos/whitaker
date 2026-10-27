//! Query Dylint dependency availability before the install-flow decision.

use std::io::Write;

use tracing::debug;
use whitaker_installer::{
    deps::{
        CommandExecutor,
        DylintToolStatus,
        PathScanFailure,
        check_dylint_tools_with_diagnostics,
    },
    error::Result,
};

/// Query dependency availability and emit bounded PATH diagnostics.
pub(crate) fn check_dylint_tools_for_command(
    executor: &dyn CommandExecutor,
) -> Result<DylintToolStatus> {
    let check = check_dylint_tools_with_diagnostics(executor)?;
    report_path_scan_failures(&check.path_scan_failures);
    Ok(check.status)
}

/// Report skipped PATH entries without exposing their paths.
fn report_path_scan_failures(failures: &[PathScanFailure]) {
    for failure in failures {
        debug!(
            binary_name = ?failure.binary_name,
            failure_category = failure.category.as_str(),
            "skipping PATH scan candidate after lookup failure"
        );
    }
}

/// Install only after a successful query establishes which tools are missing.
pub(super) fn ensure_dylint_tools_with_query(
    quiet: bool,
    stderr: &mut dyn Write,
    query: impl FnOnce() -> Result<DylintToolStatus>,
    install: impl FnOnce(&DylintToolStatus, &mut dyn Write) -> Result<()>,
) -> Result<()> {
    let status = query()?;
    super::ensure_dylint_tools_core(quiet, stderr, status.all_installed(), |stderr| {
        install(&status, stderr)
    })
}
