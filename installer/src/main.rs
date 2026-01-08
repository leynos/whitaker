//! Whitaker installer CLI entrypoint.
//!
//! This binary builds, links, and stages Dylint lint libraries for local use.
//! After installation, it prints shell configuration snippets for enabling
//! library discovery.

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::io::Write;
use whitaker_installer::builder::{
    BuildConfig, Builder, CrateName, CrateResolutionOptions, resolve_crates, validate_crate_names,
};
use whitaker_installer::cli::{Cli, Command, InstallArgs, ListArgs};
use whitaker_installer::deps::{check_dylint_tools, install_dylint_tools};
use whitaker_installer::dirs::{BaseDirs, SystemBaseDirs};
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::list_output::{format_human, format_json};
use whitaker_installer::output::{DryRunInfo, ShellSnippet, success_message};
use whitaker_installer::scanner::{lints_for_library, scan_installed};
use whitaker_installer::stager::{Stager, default_target_dir};
use whitaker_installer::toolchain::Toolchain;
use whitaker_installer::wrapper::{generate_wrapper_scripts, path_instructions};

struct RunContext<'a> {
    args: &'a InstallArgs,
    workspace_root: &'a Utf8Path,
    toolchain: &'a Toolchain,
    target_dir: &'a Utf8Path,
}

fn main() {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let run_result = run(&cli, &mut stdout, &mut stderr);
    let exit_code = exit_code_for_run_result(run_result, &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<()> {
    // Dispatch based on command
    match &cli.command {
        Some(Command::List(args)) => run_list(args, stdout),
        Some(Command::Install(args)) => run_install(args, stderr),
        None => run_install(cli.install_args(), stderr),
    }
}

/// Runs the list command.
fn run_list(args: &ListArgs, stdout: &mut dyn Write) -> Result<()> {
    let target_dir = determine_target_dir(args.target_dir.clone())?;

    let installed = scan_installed(&target_dir).map_err(|e| InstallerError::ScanFailed {
        reason: e.to_string(),
    })?;

    // Try to detect active toolchain from rust-toolchain.toml in current directory
    let active_toolchain = std::env::current_dir()
        .ok()
        .and_then(|p| Utf8PathBuf::try_from(p).ok())
        .and_then(|p| Toolchain::detect(&p).ok())
        .map(|t| t.channel().to_owned());

    let output = if args.json {
        format_json(&installed, active_toolchain.as_deref())
    } else {
        format_human(&installed, active_toolchain.as_deref())
    };

    writeln!(stdout, "{output}").map_err(|e| InstallerError::ScanFailed {
        reason: e.to_string(),
    })?;

    Ok(())
}

/// Runs the install command.
fn run_install(args: &InstallArgs, stderr: &mut dyn Write) -> Result<()> {
    let dirs = SystemBaseDirs::new().ok_or_else(|| InstallerError::WorkspaceNotFound {
        reason: "could not determine platform directories".to_owned(),
    })?;

    // Dry-run mode: show what would be done without side effects
    if args.dry_run {
        return run_dry(args, &dirs, stderr);
    }

    // Step 1: Check and install Dylint dependencies if needed
    if !args.skip_deps {
        ensure_dylint_tools(args.quiet, stderr)?;
    }

    // Step 2: Ensure workspace is available (clone if needed)
    let workspace_root = ensure_whitaker_workspace(args, &dirs, stderr)?;

    // Step 3: Resolve crates and toolchain
    let crates = resolve_requested_crates(args)?;
    let toolchain = resolve_toolchain(&workspace_root, args.toolchain.as_deref())?;
    let target_dir = determine_target_dir(args.target_dir.clone())?;

    let context = RunContext {
        args,
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
    };

    // Step 4: Build and stage
    let build_results = perform_build(&context, &crates, stderr)?;
    let staging_path = stage_libraries(&context, &build_results, stderr)?;

    // Step 5: Generate wrapper scripts if requested
    if args.skip_wrapper {
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, ShellSnippet::new(&staging_path).display_text());
    } else {
        generate_and_report_wrapper(&dirs, &staging_path, stderr)?;
    }

    Ok(())
}

/// Runs in dry-run mode, showing configuration without side effects.
fn run_dry(args: &InstallArgs, dirs: &dyn BaseDirs, stderr: &mut dyn Write) -> Result<()> {
    use whitaker_installer::workspace::resolve_workspace_path;

    let workspace_root = resolve_workspace_path(dirs)?;
    let crates = resolve_requested_crates(args)?;
    let toolchain = resolve_toolchain(&workspace_root, args.toolchain.as_deref())?;
    let target_dir = determine_target_dir(args.target_dir.clone())?;

    let info = DryRunInfo {
        workspace_root: &workspace_root,
        toolchain: toolchain.channel(),
        target_dir: &target_dir,
        verbosity: args.verbosity,
        quiet: args.quiet,
        skip_deps: args.skip_deps,
        skip_wrapper: args.skip_wrapper,
        no_update: args.no_update,
        jobs: args.jobs,
        crates: &crates,
    };
    write_stderr_line(stderr, info.display_text());
    Ok(())
}

/// Checks for and installs Dylint tools if missing.
fn ensure_dylint_tools(quiet: bool, stderr: &mut dyn Write) -> Result<()> {
    let status = check_dylint_tools();

    if status.all_installed() {
        return Ok(());
    }

    if !quiet {
        write_stderr_line(stderr, "Installing required Dylint tools...");
    }

    install_dylint_tools(&status)?;

    if !quiet {
        write_stderr_line(stderr, "Dylint tools installed successfully.");
        write_stderr_line(stderr, "");
    }

    Ok(())
}

/// Ensures a Whitaker workspace is available.
fn ensure_whitaker_workspace(
    args: &InstallArgs,
    dirs: &dyn BaseDirs,
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    use whitaker_installer::workspace::{
        WorkspaceAction, clone_directory, decide_workspace_action, ensure_workspace,
    };

    // Emit progress messages for clone/update operations before delegating
    if !args.quiet
        && let Some(clone_dir) = clone_directory(dirs)
    {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| Utf8PathBuf::try_from(p).ok());

        let Some(cwd) = cwd else {
            // Cannot determine cwd; skip messaging but proceed with workspace resolution
            return ensure_workspace(dirs, !args.no_update);
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
    }

    ensure_workspace(dirs, !args.no_update)
}

/// Detects or overrides the toolchain, then verifies it is installed.
fn resolve_toolchain(
    workspace_root: &Utf8Path,
    override_channel: Option<&str>,
) -> Result<Toolchain> {
    let toolchain = match override_channel {
        Some(channel) => Toolchain::with_override(workspace_root, channel),
        None => Toolchain::detect(workspace_root)?,
    };
    toolchain.verify_installed()?;
    Ok(toolchain)
}

/// Resolves requested crates from the CLI flags.
fn resolve_requested_crates(args: &InstallArgs) -> Result<Vec<CrateName>> {
    let lint_crates: Vec<CrateName> = args
        .lint
        .iter()
        .map(|name| CrateName::from(name.as_str()))
        .collect();

    if !lint_crates.is_empty() {
        validate_crate_names(&lint_crates)?;
    }

    let options = CrateResolutionOptions {
        individual_lints: args.individual_lints,
        experimental: args.experimental,
    };
    Ok(resolve_crates(&lint_crates, &options))
}

/// Determines the target directory from CLI or falls back to the default.
fn determine_target_dir(cli_target: Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    cli_target
        .or_else(default_target_dir)
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "could not determine default target directory".to_owned(),
        })
}

/// Builds all requested crates.
fn perform_build(
    context: &RunContext<'_>,
    crates: &[CrateName],
    stderr: &mut dyn Write,
) -> Result<Vec<whitaker_installer::builder::BuildResult>> {
    if !context.args.quiet {
        write_stderr_line(
            stderr,
            format!(
                "Building {} lint crate(s) with toolchain {}...",
                crates.len(),
                context.toolchain.channel()
            ),
        );
        write_stderr_line(stderr, "  Crates to build:");
        for crate_name in crates {
            write_stderr_line(stderr, format!("    - {crate_name}"));
        }
        write_stderr_line(stderr, "");
    }

    let config = build_config_for_context(context);
    Builder::new(config).build_all(crates)
}

/// Stages built libraries and returns the staging path.
fn stage_libraries(
    context: &RunContext<'_>,
    build_results: &[whitaker_installer::builder::BuildResult],
    stderr: &mut dyn Write,
) -> Result<Utf8PathBuf> {
    let stager = Stager::new(context.target_dir.to_owned(), context.toolchain.channel());
    let staging_path = stager.staging_path();

    if !context.args.quiet {
        write_stderr_line(stderr, format!("Staging libraries to {staging_path}..."));
    }

    stager.prepare()?;
    stager.stage_all(build_results)?;

    if !context.args.quiet {
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, success_message(build_results.len(), &staging_path));

        // Show installed lints
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Installed lints:");
        for result in build_results {
            let lint_names = lints_for_library(&result.crate_name);
            for lint in lint_names {
                write_stderr_line(stderr, format!("  - {lint}"));
            }
        }
    }

    Ok(staging_path)
}

/// Generates wrapper scripts and reports the result.
fn generate_and_report_wrapper(
    dirs: &dyn BaseDirs,
    staging_path: &Utf8Path,
    stderr: &mut dyn Write,
) -> Result<()> {
    let result = generate_wrapper_scripts(dirs, staging_path)?;

    write_stderr_line(stderr, "");
    write_stderr_line(
        stderr,
        format!("Wrapper script created: {}", result.script_path.display()),
    );

    if result.in_path {
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "You can now run: whitaker --all");
    } else {
        write_stderr_line(stderr, "");
        // The script path is constructed via bin_dir.join("whitaker"), so parent()
        // always returns the bin directory.
        let bin_dir = result
            .script_path
            .parent()
            .expect("wrapper script path always has a parent directory");
        let instructions = path_instructions(bin_dir);
        write_stderr_line(stderr, instructions);
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Then run: whitaker --all");
    }

    Ok(())
}

fn build_config_for_context(context: &RunContext<'_>) -> BuildConfig {
    BuildConfig {
        toolchain: context.toolchain.clone(),
        target_dir: context.workspace_root.join("target"),
        jobs: context.args.jobs,
        verbosity: context.args.verbosity,
        experimental: context.args.experimental,
    }
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

fn write_stderr_line(stderr: &mut dyn Write, message: impl std::fmt::Display) {
    if writeln!(stderr, "{message}").is_err() {
        // Best-effort logging; ignore write failures.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn exit_code_for_run_result_returns_zero_on_success() {
        let mut stderr = Vec::new();
        let exit_code = exit_code_for_run_result(Ok(()), &mut stderr);
        assert_eq!(exit_code, 0);
        assert!(stderr.is_empty());
    }

    #[test]
    fn exit_code_for_run_result_prints_error_and_returns_one() {
        let err = InstallerError::LintCrateNotFound {
            name: CrateName::from("nonexistent_lint"),
        };

        let mut stderr = Vec::new();
        let exit_code = exit_code_for_run_result(Err(err), &mut stderr);
        assert_eq!(exit_code, 1);

        let stderr_text = String::from_utf8(stderr).expect("stderr was not UTF-8");
        assert!(stderr_text.contains("lint crate nonexistent_lint not found"));
    }

    #[rstest]
    #[case::default_suite_only(InstallArgs::default(), false, true)]
    #[case::individual_lints(
        InstallArgs { individual_lints: true, ..InstallArgs::default() },
        true,
        false
    )]
    fn resolve_requested_crates_respects_individual_lints_flag(
        #[case] args: InstallArgs,
        #[case] expect_lint: bool,
        #[case] expect_suite: bool,
    ) {
        let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
        assert_eq!(
            crates.contains(&CrateName::from("module_max_lines")),
            expect_lint
        );
        assert_eq!(crates.contains(&CrateName::from("suite")), expect_suite);
    }

    #[test]
    fn resolve_requested_crates_returns_specific_lints_when_provided() {
        let args = InstallArgs {
            lint: vec!["module_max_lines".to_owned()],
            ..InstallArgs::default()
        };

        let crates = resolve_requested_crates(&args).expect("expected crate resolution to succeed");
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[test]
    fn resolve_requested_crates_rejects_unknown_lints() {
        let args = InstallArgs {
            lint: vec!["nonexistent_lint".to_owned()],
            ..InstallArgs::default()
        };

        let err = resolve_requested_crates(&args).expect_err("expected crate resolution to fail");
        assert!(matches!(
            err,
            InstallerError::LintCrateNotFound { name } if name == CrateName::from("nonexistent_lint")
        ));
    }

    #[test]
    fn build_config_propagates_verbosity_level() {
        let cli = Cli::parse_from(["whitaker-installer", "-vv"]);
        let args = cli.install_args();
        let workspace_root = Utf8PathBuf::from("/tmp");
        let toolchain = Toolchain::with_override(&workspace_root, "nightly-2025-09-18");
        let target_dir = Utf8PathBuf::from("/tmp/target");
        let context = RunContext {
            args,
            workspace_root: &workspace_root,
            toolchain: &toolchain,
            target_dir: &target_dir,
        };

        let config = build_config_for_context(&context);
        assert_eq!(config.verbosity, 2);
    }
}
