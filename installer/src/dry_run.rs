//! The dry run: report what an install would do, and change nothing.
//!
//! Separated from `orchestration.rs` because it shares none of the install
//! pipeline's state and answers a different question, and because the
//! entry point had reached the repository's file-size limit.

use camino::Utf8PathBuf;
use std::io::Write;

use crate::artefact::suite_ref::SuiteRef;
use crate::cli::InstallArgs;
use crate::crate_name::CrateName;
use crate::dirs::BaseDirs;
use crate::error::Result;
use crate::install_flow::detect_host_target;
use crate::list::determine_target_dir;
use crate::orchestration::{resolve_requested_crates, resolve_toolchain};
use crate::output::{DryRunInfo, write_stderr_line};
use crate::prebuilt_path::prebuilt_library_dir;
use crate::toolchain::Toolchain;

/// Runs in dry-run mode, showing configuration without side effects.
pub(crate) fn run_dry(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<()> {
    use crate::workspace::resolve_workspace_path;

    let workspace_root = resolve_workspace_path(dirs)?;
    let requested_crates = resolve_requested_crates(args)?;
    let toolchain = resolve_toolchain(&workspace_root, args.toolchain.as_deref())?;
    toolchain.verify_installed()?;
    let target_dir = determine_dry_run_target_dir(args, dirs, &toolchain, &requested_crates)?;
    let info = DryRunInfo {
        workspace_root: &workspace_root,
        toolchain: toolchain.channel(),
        target_dir: &target_dir,
        verbosity: args.verbosity,
        quiet: args.quiet,
        skip_deps: args.skip_deps,
        skip_wrapper: args.skip_wrapper,
        no_update: args.no_update,
        suite_ref: args.suite_version.as_ref().map(SuiteRef::as_str),
        jobs: args.jobs,
        crates: &requested_crates,
    };
    write_stderr_line(stderr, info.display_text());
    Ok(())
}

fn determine_dry_run_target_dir(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    toolchain: &Toolchain,
    requested_crates: &[CrateName],
) -> Result<Utf8PathBuf> {
    let build_target_dir = determine_target_dir(args.target_dir.as_deref())?;
    if !args.should_attempt_prebuilt(requested_crates) {
        return Ok(build_target_dir);
    }
    let Ok(host_target) = detect_host_target() else {
        return Ok(build_target_dir);
    };
    Ok(prebuilt_library_dir(dirs, toolchain.channel(), &host_target).unwrap_or(build_target_dir))
}
