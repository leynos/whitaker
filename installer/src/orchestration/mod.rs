//! Installer command orchestration shared by Whitaker binaries.
//!
//! This module coordinates parsed installer requests with the library's build,
//! staging, and wrapper adapters. It deliberately owns the operational flow,
//! while callers own command-line parsing and process termination.

use std::{io::Write, time::Instant};

use camino::{Utf8Path, Utf8PathBuf};

mod dry_run;
mod exit;
mod finish;

use dry_run::run_dry;
pub use exit::exit_code_for_run_result;
use finish::{FinishInstallContext, finish_install_and_record_metrics};

use crate::{
    cli::{Cli, Command, InstallArgs},
    crate_name::CrateName,
    deps::{SourcePolicy, SystemCommandExecutor, install_dylint_tools_with_output},
    dirs::{BaseDirs, SystemBaseDirs},
    error::{InstallerError, Result},
    install_flow::{
        PrebuiltInstallationContext, ensure_dylint_tools_with_executor, try_prebuilt_installation,
    },
    install_metrics::InstallMode,
    list::{determine_target_dir, run_list},
    output::write_stderr_line,
    pipeline::{PipelineContext, perform_build, stage_libraries},
    resolution::{CrateResolutionOptions, resolve_crates, validate_crate_names},
    staged_suite,
    toolchain::Toolchain,
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
        Some(Command::Install(args)) => run_install(args, stdout, stderr, WrapperScripts::Both),
        None => run_install(cli.install_args(), stdout, stderr, WrapperScripts::Both),
    }
}

/// Runs a root-CLI install without replacing its `whitaker` executable.
///
/// The legacy installer continues to use [`run`] and creates both scripts.
/// The root binary creates only `whitaker-ls`, preserving the executable name
/// that the user invoked.
///
/// # Examples
///
/// ```no_run
/// use std::io;
///
/// use whitaker_installer::{cli::InstallArgs, orchestration::run_install_for_whitaker};
///
/// let arguments = InstallArgs::default();
/// let mut diagnostics = io::stderr();
/// run_install_for_whitaker(&arguments, &mut diagnostics)?;
/// # Ok::<(), whitaker_installer::error::InstallerError>(())
/// ```
///
/// # Errors
///
/// Returns an error when the selected installation operation cannot complete.
pub fn run_install_for_whitaker(
    args: &InstallArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<()> {
    run_install(args, stdout, stderr, WrapperScripts::WhitakerLsOnly)
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
fn run_install(
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
    // Step 2: Ensure workspace is available (clone if needed)
    let workspace_root = ensure_whitaker_workspace(args, &dirs, stderr)?;
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
    // Step 3.5: Attempt prebuilt download or staged-suite fast path.
    let fast_path_context = FastPathContext {
        args,
        policy,
        dirs: &dirs,
        requested_crates: &requested_crates,
        toolchain: &toolchain,
        target_dir: &target_dir,
    };
    if let Some((staging_path, install_mode)) =
        try_fast_path_installation(&fast_path_context, stderr)?
    {
        report_suite_source(install_mode, stdout)?;
        let finish_context = FinishInstallContext {
            args,
            dirs: &dirs,
            toolchain: &toolchain,
            staging_path: &staging_path,
            wrapper_scripts,
            install_mode,
            install_started,
        };
        return finish_install_and_record_metrics(&finish_context, stderr);
    }
    let context = PipelineContext {
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
        jobs: args.jobs,
        verbosity: args.verbosity,
        experimental: args.lint_selection.experimental,
        quiet: args.quiet,
    };
    // Step 4: Build and stage
    let build_results = perform_build(&context, &requested_crates, stderr)?;
    let staging_path = stage_libraries(&context, &build_results, stderr)?;
    // Step 5: Generate wrapper scripts if requested
    report_suite_source(InstallMode::Build, stdout)?;
    let finish_context = FinishInstallContext {
        args,
        dirs: &dirs,
        toolchain: &toolchain,
        staging_path: &staging_path,
        wrapper_scripts,
        install_mode: InstallMode::Build,
        install_started,
    };
    finish_install_and_record_metrics(&finish_context, stderr)
}

/// Checks for and installs Dylint tools if missing.
fn ensure_dylint_tools(policy: SourcePolicy, stderr: &mut dyn Write) -> Result<()> {
    let executor = SystemCommandExecutor;
    ensure_dylint_tools_with_executor(&executor, policy, stderr, |status, stderr| {
        install_dylint_tools_with_output(&executor, status, policy, stderr)
    })
}

/// Announce, on stdout, whether the suite came from a published artefact.
///
/// stdout rather than stderr, and a fixed shape rather than prose, because a
/// caller reads it. `install-whitaker` previously had to infer the path from
/// the wording of a fallback notice, which meant a rephrasing could silently
/// turn a source build into a reported success.
fn report_suite_source(install_mode: InstallMode, stdout: &mut dyn Write) -> Result<()> {
    let source = match install_mode {
        InstallMode::Download => "prebuilt",
        InstallMode::Build => "source",
    };
    writeln!(stdout, "whitaker-installer: suite-source={source}")
        .map_err(|source| InstallerError::WriteFailed { source })
}

/// Ensures a Whitaker workspace is available.
fn ensure_whitaker_workspace(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    use crate::workspace::{
        WorkspaceAction, WorkspacePlan, clone_directory, decide_workspace_action, ensure_workspace,
    };

    let plan = WorkspacePlan {
        should_update: !args.skip.no_update,
        suite_ref: args.suite_version.clone(),
    };

    if !args.quiet
        && let Some(clone_dir) = clone_directory(dirs)
    {
        let utf8_cwd = std::env::current_dir()
            .ok()
            .and_then(|p| Utf8PathBuf::try_from(p).ok());

        let Some(cwd) = utf8_cwd else {
            return ensure_workspace(dirs, &plan);
        };

        match decide_workspace_action(&cwd, &clone_dir, !args.skip.no_update) {
            WorkspaceAction::CloneTo(dir) => {
                write_stderr_line(stderr, format!("Cloning Whitaker repository to {dir}..."));
            }
            WorkspaceAction::UpdateAt(dir) => {
                write_stderr_line(stderr, format!("Updating Whitaker repository at {dir}..."));
            }
            WorkspaceAction::UseCurrentDir(_) | WorkspaceAction::UseExisting(_) => {}
        }
        if let Some(reference) = plan.suite_ref.as_ref() {
            write_stderr_line(stderr, format!("Pinning lint suite to {reference}..."));
        }
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
        validate_crate_names(&lint_crates, &options)?;
    }

    Ok(resolve_crates(&lint_crates, &options))
}

#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
