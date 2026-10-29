//! Whitaker installer CLI entrypoint.
//!
//! This binary builds, links, and stages Dylint lint libraries for local use.
//! After installation, it prints shell configuration snippets for enabling
//! library discovery.

mod install_flow;
mod staged_suite;
mod tracing_config;

use crate::install_flow::bootstrap_step::{
    InstalledSnapshot, install_clippy_snapshot, record_bootstrap, run_bootstrap,
    write_shell_snippet, write_wrappers,
};
use crate::install_flow::{
    MetricsWriteContext, PrebuiltInstallationContext, ensure_dylint_tools_with_executor,
    try_prebuilt_installation, write_install_metrics,
};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::io::Write;
use std::time::Instant;
use whitaker_installer::cli::{Cli, Command, InstallArgs};
use whitaker_installer::crate_name::CrateName;
use whitaker_installer::deps::{
    SourcePolicy, SystemCommandExecutor, install_dylint_tools_with_output,
};
use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::install_metrics::InstallMode;
use whitaker_installer::list::{determine_target_dir, run_list};
use whitaker_installer::output::write_stderr_line;
use whitaker_installer::pipeline::{PipelineContext, perform_build, stage_libraries};
use whitaker_installer::resolution::{
    CrateResolutionOptions, resolve_crates, validate_crate_names,
};
use whitaker_installer::toolchain::Toolchain;

/// Parse options, configure tracing, and run the selected command.
fn main() {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    if let Err(error) = tracing_config::configure_tracing(cli.install_args()) {
        write_stderr_line(&mut stderr, error);
        std::process::exit(1);
    }
    // `run` hands the child's exit status back unchanged, lint failures included.
    if let Some(Command::Run(args)) = &cli.command {
        std::process::exit(run_bootstrap(&args.args, &mut stderr));
    }
    let exit_code = exit_code_for_run_result(run(&cli, &mut stdout, &mut stderr), &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

/// Routes CLI commands to their respective handlers.
fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    match &cli.command {
        Some(Command::List(args)) => run_list(args, stdout),
        Some(Command::Install(args)) => run_install(args, stdout, stderr),
        // Dispatched by `main` before this point, because it exits with the
        // child's status rather than with a `Result`.
        Some(Command::Run(_)) | None => run_install(cli.install_args(), stdout, stderr),
    }
}

/// Returns the set of additional rustup components requested by the CLI flags.
fn resolve_additional_components(args: &InstallArgs) -> &'static [&'static str] {
    if args.cranelift {
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
/// Workflow: (1) check/install Dylint dependencies, (2) install the Clippy
/// snapshot, (3) locate/clone workspace, (4) stage the suite from a prebuilt
/// artefact or a build, (5) record the bootstrap and generate wrapper scripts.
///
/// # Errors
///
/// Returns an error if any step fails.
fn run_install(args: &InstallArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    // The one environment read for the whole run. Everything below consults
    // this answer, so validation and installation cannot disagree.
    let policy = args.source_policy();
    args.validate_source_options_with(policy.no_source_fallback)?;
    let dirs = SystemBaseDirs::new().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine platform directories".to_owned(),
    })?;
    if args.dry_run {
        return run_dry(args, &dirs, stderr);
    }
    let install_started = Instant::now();
    if !args.skip_deps {
        ensure_dylint_tools(policy, stderr)?;
    }
    // Before any workspace or library work, with no fallback.
    let snapshot = install_clippy_snapshot(args, &dirs, stderr)?;
    let workspace_root = ensure_whitaker_workspace(args, &dirs, stderr)?;
    let request = SuiteRequest {
        args,
        policy,
        dirs: &dirs,
        workspace_root: &workspace_root,
    };
    let staged = stage_suite(&request, stderr)?;
    report_suite_source(staged.install_mode, stdout)?;
    let finish_context = FinishInstallContext {
        args,
        dirs: &dirs,
        snapshot: &snapshot,
        toolchain: staged.toolchain.channel(),
        staging_path: &staged.staging_path,
        install_mode: staged.install_mode,
        install_started,
    };
    finish_install_and_record_metrics(&finish_context, stderr)
}

/// The inputs that decide which lint suite to stage, and from where.
struct SuiteRequest<'a> {
    args: &'a InstallArgs,
    policy: SourcePolicy,
    dirs: &'a dyn BaseDirs,
    workspace_root: &'a Utf8Path,
}

/// A staged lint suite and how it was obtained.
struct StagedSuite {
    staging_path: Utf8PathBuf,
    install_mode: InstallMode,
    toolchain: Toolchain,
}

/// Resolve crates and toolchain, then stage the suite from a fast path or a build.
fn stage_suite(request: &SuiteRequest<'_>, stderr: &mut dyn Write) -> Result<StagedSuite> {
    let args = request.args;
    let requested_crates = resolve_requested_crates(args)?;
    let toolchain = resolve_toolchain(request.workspace_root, args.toolchain.as_deref())?;
    ensure_toolchain_installed(
        &toolchain,
        resolve_additional_components(args),
        args.quiet,
        stderr,
    )?;
    let target_dir = determine_target_dir(args.target_dir.as_deref())?;
    let fast_path_context = FastPathContext {
        args,
        policy: request.policy,
        dirs: request.dirs,
        requested_crates: &requested_crates,
        toolchain: &toolchain,
        target_dir: &target_dir,
    };
    if let Some((staging_path, install_mode)) =
        try_fast_path_installation(&fast_path_context, stderr)?
    {
        return Ok(StagedSuite {
            staging_path,
            install_mode,
            toolchain,
        });
    }
    let context = PipelineContext {
        workspace_root: request.workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
        jobs: args.jobs,
        verbosity: args.verbosity,
        experimental: args.experimental,
        quiet: args.quiet,
    };
    let build_results = perform_build(&context, &requested_crates, stderr)?;
    let staging_path = stage_libraries(&context, &build_results, stderr)?;
    Ok(StagedSuite {
        staging_path,
        install_mode: InstallMode::Build,
        toolchain,
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

mod dry_run;

use dry_run::run_dry;

/// Checks for and installs Dylint tools if missing.
fn ensure_dylint_tools(policy: SourcePolicy, stderr: &mut dyn Write) -> Result<()> {
    let executor = SystemCommandExecutor;
    ensure_dylint_tools_with_executor(&executor, policy, stderr, |status, stderr| {
        install_dylint_tools_with_output(&executor, status, policy, stderr)
    })
}

/// Ensures a Whitaker workspace is available.
fn ensure_whitaker_workspace(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    use whitaker_installer::workspace::{
        WorkspaceAction, WorkspacePlan, clone_directory, decide_workspace_action, ensure_workspace,
    };

    let plan = WorkspacePlan {
        should_update: !args.no_update,
        suite_ref: args.suite_version.clone(),
    };

    if !args.quiet {
        if let Some(clone_dir) = clone_directory(dirs) {
            let cwd = std::env::current_dir()
                .ok()
                .and_then(|p| Utf8PathBuf::try_from(p).ok());

            let Some(cwd) = cwd else {
                return ensure_workspace(dirs, &plan);
            };

            match decide_workspace_action(&cwd, &clone_dir, !args.no_update) {
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
    }

    ensure_workspace(dirs, &plan)
}

/// Detects or overrides the toolchain, then verifies it is installed.
fn resolve_toolchain(
    workspace_root: &Utf8Path,
    override_channel: Option<&str>,
) -> Result<Toolchain> {
    match override_channel {
        Some(channel) => Ok(Toolchain::with_override(workspace_root, channel)),
        None => Toolchain::detect(workspace_root),
    }
}

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

/// Common final steps: record the bootstrap, then write wrappers or a snippet.
fn finish_install(context: &FinishInstallContext<'_>, stderr: &mut dyn Write) -> Result<()> {
    record_bootstrap(context.snapshot, context.toolchain, context.staging_path)?;
    if context.args.skip_wrapper {
        write_shell_snippet(context.snapshot, context.staging_path, stderr);
        Ok(())
    } else {
        write_wrappers(context.dirs, stderr)
    }
}

/// Aggregates final-step inputs required after a successful install run.
struct FinishInstallContext<'a> {
    args: &'a InstallArgs,
    dirs: &'a dyn BaseDirs,
    /// The Clippy snapshot installed before any library work.
    snapshot: &'a InstalledSnapshot,
    /// The toolchain the staged libraries were built for.
    toolchain: &'a str,
    staging_path: &'a Utf8Path,
    install_mode: InstallMode,
    install_started: Instant,
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

/// Finalize installation and record aggregate installer metrics.
fn finish_install_and_record_metrics(
    context: &FinishInstallContext<'_>,
    stderr: &mut dyn Write,
) -> Result<()> {
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

/// Resolves requested crates from the CLI flags.
fn resolve_requested_crates(args: &InstallArgs) -> Result<Vec<CrateName>> {
    let lint_crates: Vec<CrateName> = args
        .lint
        .iter()
        .map(|name| CrateName::from(name.as_str()))
        .collect();

    let options = CrateResolutionOptions {
        individual_lints: args.individual_lints,
        experimental: args.experimental,
    };
    if !lint_crates.is_empty() {
        validate_crate_names(&lint_crates, &options)?;
    }

    Ok(resolve_crates(&lint_crates, &options))
}

fn exit_code_for_run_result(result: Result<()>, stderr: &mut dyn Write) -> i32 {
    match result {
        Ok(()) => 0,
        Err(err) => {
            write_stderr_line(stderr, err);
            1
        }
    }
}

#[cfg(test)]
mod tests;
