//! Installer command orchestration shared by Whitaker binaries.
//!
//! This module coordinates parsed installer requests with the library's build,
//! staging, and wrapper adapters. It deliberately owns the operational flow,
//! while callers own command-line parsing and process termination.

use std::{io::Write, time::Instant};

use camino::{Utf8Path, Utf8PathBuf};

mod announce;
mod dry_run;
mod entry;
mod exit;
mod finish;
mod lock;

use announce::{report_suite_source, report_workspace_action};
use dry_run::run_dry;
pub use entry::{
    run_bootstrap_command,
    run_bootstrap_command_with_dirs,
    run_bootstrap_command_with_library_path,
    run_install_for_whitaker,
    run_install_for_whitaker_under_lock,
};
pub use exit::exit_code_for_run_result;
use finish::{FinishInstallContext, finish_install_and_record_metrics};
pub use lock::with_install_lock;

use crate::{
    cli::{Cli, Command, InstallArgs},
    crate_name::CrateName,
    deps::{SourcePolicy, SystemCommandExecutor, install_dylint_tools_with_output},
    dirs::{BaseDirs, SystemBaseDirs},
    error::{InstallerError, Result},
    install_flow::{
        PrebuiltInstallationContext,
        bootstrap_step::{InstalledSnapshot, install_clippy_snapshot},
        ensure_dylint_tools_with_executor,
        try_prebuilt_installation,
    },
    install_metrics::InstallMode,
    list::{determine_target_dir, run_list},
    output::write_stderr_line,
    pipeline::{PipelineContext, perform_build, stage_libraries},
    resolution::{CrateResolutionOptions, resolve_crates, validate_crate_names},
    staged_suite,
    toolchain::Toolchain,
    workspace::WorkspacePlan,
    wrapper::WrapperScripts,
};

/// Routes CLI commands to their respective handlers.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use clap::Parser as _;
/// use whitaker_installer::{cli::Cli, orchestration::run};
///
/// let cli = Cli::try_parse_from(["whitaker-installer", "--dry-run"])?;
/// run(&cli, &mut io::sink(), &mut io::sink())?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error when the selected installer operation cannot complete.
pub fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    match &cli.command {
        Some(Command::List(args)) => run_list(args, stdout),
        Some(Command::Install(args)) => {
            run_install_locked(args, stdout, stderr, WrapperScripts::Both)
        }
        // `Run` is normally dispatched by the composition root before this
        // point, because it exits with the child's status rather than a
        // `Result`.
        Some(Command::Run(_)) | None => {
            run_install_locked(cli.install_args(), stdout, stderr, WrapperScripts::Both)
        }
    }
}

/// Run an install while holding the installation lock.
///
/// Every writer of the staging directory takes this lock, so an explicit
/// install and a lazy repair cannot stage the same library at the same time.
/// A dry run is exempt: it only reads, and creating the lock file would give a
/// report a side effect it should not have.
fn run_install_locked(
    args: &InstallArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    wrapper_scripts: WrapperScripts,
) -> Result<()> {
    if args.execution.dry_run {
        return run_install(args, stdout, stderr, wrapper_scripts);
    }
    let dirs = entry::install_dirs()?;
    with_install_lock(&dirs, || run_install(args, stdout, stderr, wrapper_scripts))?
}

/// Returns the set of additional rustup components requested by the CLI flags.
const fn resolve_additional_components(args: &InstallArgs) -> &'static [&'static str] {
    if args.execution.cranelift {
        &["rustc-codegen-cranelift"]
    } else {
        &[]
    }
}

/// Attempts prebuilt download and staged-suite fast paths.
///
/// Returns `Some((staging_path, mode))` if either succeeds, or `None` if
/// the caller should proceed to a full build.
fn try_fast_path_installation(
    context: &FastPathContext<'_>,
    stderr: &mut dyn Write,
) -> Result<Option<(Utf8PathBuf, InstallMode)>> {
    let prebuilt_context = PrebuiltInstallationContext {
        args: context.args,
        policy: context.policy,
        dirs: context.dirs,
        requested_crates: context.requested_crates,
        toolchain_channel: context.toolchain.channel(),
    };
    if let Some(staging_path) = try_prebuilt_installation(&prebuilt_context, stderr)? {
        return Ok(Some((staging_path, InstallMode::Download)));
    }
    if let Some(staging_path) = staged_suite::try_test_staged_suite_installation(
        context.requested_crates,
        context.toolchain,
        context.target_dir,
    )? {
        return Ok(Some((staging_path, InstallMode::Build)));
    }
    Ok(None)
}

/// Runs the install command to build and stage lint libraries.
///
/// Workflow: (1) check/install Dylint dependencies, (2) locate/clone workspace,
/// (3) resolve crates from CLI flags, (4) build in release mode, (5) stage
/// libraries with toolchain-suffixed names, (6) generate wrapper script.
///
/// # Errors
///
/// Returns an error if any step fails.
pub(super) fn run_install(
    args: &InstallArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    wrapper_scripts: WrapperScripts,
) -> Result<()> {
    // The one environment read for the whole run. Everything below consults
    // this answer, so validation and installation cannot disagree.
    let policy = args.source_policy();
    args.validate_source_options_with(policy.no_source_fallback)?;
    let dirs = SystemBaseDirs::new().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine platform directories".to_owned(),
    })?;
    if args.execution.dry_run {
        return run_dry(args, &dirs, stderr);
    }
    let install_started = Instant::now();
    // Step 1: Check and install Dylint dependencies if needed
    if !args.skip.skip_deps {
        ensure_dylint_tools(policy, stderr)?;
    }
    let plan = plan_install(args, &dirs, stderr)?;
    // Step 3.5: Attempt prebuilt download or staged-suite fast path.
    let fast_path_context = FastPathContext {
        args,
        policy,
        dirs: &dirs,
        requested_crates: &plan.requested_crates,
        toolchain: &plan.toolchain,
        target_dir: &plan.target_dir,
    };
    if let Some((staging_path, install_mode)) =
        try_fast_path_installation(&fast_path_context, stderr)?
    {
        report_suite_source(install_mode, stdout)?;
        let finish_context = FinishInstallContext {
            args,
            dirs: &dirs,
            snapshot: &plan.snapshot,
            toolchain: plan.toolchain.channel(),
            staging_path: &staging_path,
            wrapper_scripts,
            install_mode,
            install_started,
        };
        return finish_install_and_record_metrics(&finish_context, stderr);
    }
    let context = PipelineContext {
        workspace_root: &plan.workspace_root,
        toolchain: &plan.toolchain,
        target_dir: &plan.target_dir,
        jobs: args.jobs,
        verbosity: args.verbosity,
        experimental: args.lint_selection.experimental,
        quiet: args.quiet,
    };
    // Step 4: Build and stage
    let build_results = perform_build(&context, &plan.requested_crates, stderr)?;
    let staging_path = stage_libraries(&context, &build_results, stderr)?;
    // Step 5: Generate wrapper scripts if requested
    report_suite_source(InstallMode::Build, stdout)?;
    let finish_context = FinishInstallContext {
        args,
        dirs: &dirs,
        snapshot: &plan.snapshot,
        toolchain: plan.toolchain.channel(),
        staging_path: &staging_path,
        wrapper_scripts,
        install_mode: InstallMode::Build,
        install_started,
    };
    finish_install_and_record_metrics(&finish_context, stderr)
}

/// Build inputs resolved before any compilation or staging work.
struct InstallPlan {
    /// The Clippy snapshot installed ahead of all other installation steps.
    snapshot: InstalledSnapshot,
    /// Root of the Whitaker workspace the libraries are built from.
    workspace_root: Utf8PathBuf,
    /// Crates selected by the command-line request.
    requested_crates: Vec<CrateName>,
    /// Toolchain that will build and run the staged libraries.
    toolchain: Toolchain,
    /// Directory the build artefacts are written to.
    target_dir: Utf8PathBuf,
}

/// Installs the Clippy snapshot and resolves everything the build needs.
///
/// The snapshot is installed first, with no fallback: a release that does not
/// publish its snapshot fails here rather than after minutes of other work.
///
/// # Errors
///
/// Returns an error when the snapshot, workspace, crate selection, toolchain,
/// or target directory cannot be resolved.
fn plan_install(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<InstallPlan> {
    let snapshot = install_clippy_snapshot(args, dirs, stderr)?;
    // Step 2: Ensure workspace is available (clone if needed)
    let workspace_root = ensure_whitaker_workspace(args, dirs, stderr)?;
    // Step 3: Resolve crates and toolchain
    let requested_crates = resolve_requested_crates(args)?;
    let toolchain = resolve_toolchain(&workspace_root, args.toolchain.as_deref())?;
    ensure_toolchain_installed(
        &toolchain,
        resolve_additional_components(args),
        args.quiet,
        stderr,
    )?;
    let target_dir = determine_target_dir(args.target_dir.as_deref())?;
    Ok(InstallPlan {
        snapshot,
        workspace_root,
        requested_crates,
        toolchain,
        target_dir,
    })
}

/// Checks for and installs Dylint tools if missing.
fn ensure_dylint_tools(policy: SourcePolicy, stderr: &mut dyn Write) -> Result<()> {
    let executor = SystemCommandExecutor;
    ensure_dylint_tools_with_executor(&executor, policy, stderr, |status, output| {
        install_dylint_tools_with_output(&executor, status, policy, output)
    })
}

/// Ensures a Whitaker workspace is available.
fn ensure_whitaker_workspace(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    use crate::workspace::ensure_workspace;

    let plan = WorkspacePlan {
        should_update: !args.skip.no_update,
        suite_ref: args.suite_version.clone(),
    };

    if !args.quiet {
        report_workspace_action(dirs, &plan, stderr);
    }
    ensure_workspace(dirs, &plan)
}

/// Detect the workspace toolchain or apply an explicit channel override.
pub(super) fn resolve_toolchain(
    workspace_root: &Utf8Path,
    override_channel: Option<&str>,
) -> Result<Toolchain> {
    override_channel.map_or_else(
        || Toolchain::detect(workspace_root),
        |channel| Ok(Toolchain::with_override(workspace_root, channel)),
    )
}

/// Install the toolchain and extra components when absent.
fn ensure_toolchain_installed(
    toolchain: &Toolchain,
    additional_components: &[&str],
    quiet: bool,
    stderr: &mut dyn Write,
) -> Result<()> {
    let status = toolchain.ensure_installed(additional_components)?;
    if status.installed_toolchain() && !quiet {
        write_stderr_line(
            stderr,
            format!("Toolchain {} installed successfully.", toolchain.channel()),
        );
        write_stderr_line(stderr, "");
    }
    Ok(())
}

/// Aggregates the immutable inputs for fast-path installation attempts.
struct FastPathContext<'a> {
    args: &'a InstallArgs,
    /// The source-fallback rule, resolved once for the whole run.
    ///
    /// `InstallArgs::source_policy` reads the process environment. Resolving it
    /// here and passing it down means the dependency and suite paths cannot
    /// observe different answers, which a change to the environment between two
    /// reads would otherwise allow.
    policy: SourcePolicy,
    dirs: &'a dyn BaseDirs,
    requested_crates: &'a [CrateName],
    toolchain: &'a Toolchain,
    target_dir: &'a Utf8PathBuf,
}

/// Resolves requested crates from the CLI flags.
pub(super) fn resolve_requested_crates(args: &InstallArgs) -> Result<Vec<CrateName>> {
    let lint_crates: Vec<CrateName> = args
        .lint
        .iter()
        .map(|name| CrateName::from(name.as_str()))
        .collect();

    let options = CrateResolutionOptions {
        individual_lints: args.lint_selection.individual_lints,
        experimental: args.lint_selection.experimental,
    };
    if !lint_crates.is_empty() {
        validate_crate_names(&lint_crates)?;
    }

    Ok(resolve_crates(&lint_crates, &options))
}

#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
