//! Whitaker installer CLI entrypoint.
//!
//! This binary builds, links, and stages Dylint lint libraries for local use.
//! After installation, it prints shell configuration snippets for enabling
//! library discovery.

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::io::Write;
use whitaker_installer::builder::{
    BuildConfig, Builder, CrateName, find_workspace_root, resolve_crates, validate_crate_names,
};
use whitaker_installer::error::{InstallerError, Result};
use whitaker_installer::output::{ShellSnippet, success_message};
use whitaker_installer::stager::{Stager, default_target_dir};
use whitaker_installer::toolchain::Toolchain;

/// Install Whitaker Dylint lint libraries.
#[derive(Parser, Debug)]
#[command(name = "whitaker-installer")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target directory for staged libraries.
    #[arg(short, long, value_name = "DIR")]
    target_dir: Option<Utf8PathBuf>,

    /// Build specific lints (can be repeated).
    #[arg(short, long, value_name = "NAME")]
    lint: Vec<String>,

    /// Build all individual lint crates instead of the aggregated suite.
    #[arg(long, conflicts_with = "lint")]
    individual_lints: bool,

    /// Number of parallel build jobs.
    #[arg(short, long, value_name = "N")]
    jobs: Option<usize>,

    /// Override the detected toolchain.
    #[arg(long, value_name = "TOOLCHAIN")]
    toolchain: Option<String>,

    /// Show what would be done without executing.
    #[arg(long)]
    dry_run: bool,

    /// Increase output verbosity (repeatable).
    #[arg(
        short,
        long = "verbose",
        alias = "verbosity",
        action = clap::ArgAction::Count,
        conflicts_with = "quiet"
    )]
    verbosity: u8,

    /// Suppress output except errors (does not affect --dry-run output).
    #[arg(short, long, conflicts_with = "verbosity")]
    quiet: bool,
}

struct RunContext<'a> {
    cli: &'a Cli,
    workspace_root: &'a Utf8Path,
    toolchain: &'a Toolchain,
    target_dir: &'a Utf8Path,
}

fn main() {
    let cli = Cli::parse();
    let mut stderr = std::io::stderr();
    let run_result = run(&cli, &mut stderr);
    let exit_code = exit_code_for_run_result(run_result, &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(cli: &Cli, stderr: &mut dyn Write) -> Result<()> {
    let workspace_root = determine_workspace_root()?;
    let crates = resolve_requested_crates(cli)?;
    let toolchain = resolve_toolchain(&workspace_root, cli.toolchain.as_deref())?;
    let target_dir = determine_target_dir(cli.target_dir.clone())?;

    if cli.dry_run {
        write_stderr_line(stderr, "Dry run - no files will be modified");
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, format!("Workspace root: {workspace_root}"));
        write_stderr_line(stderr, format!("Toolchain: {}", toolchain.channel()));
        write_stderr_line(stderr, format!("Target directory: {target_dir}"));
        write_stderr_line(stderr, format!("Verbosity level: {}", cli.verbosity));
        write_stderr_line(stderr, format!("Quiet: {}", cli.quiet));

        if let Some(jobs) = cli.jobs {
            write_stderr_line(stderr, format!("Parallel jobs: {jobs}"));
        }

        write_stderr_line(stderr, "");
        write_stderr_line(stderr, "Crates to build:");
        for crate_name in &crates {
            write_stderr_line(stderr, format!("  - {crate_name}"));
        }

        return Ok(());
    }

    let context = RunContext {
        cli,
        workspace_root: &workspace_root,
        toolchain: &toolchain,
        target_dir: &target_dir,
    };
    let build_results = perform_build(&context, &crates, stderr)?;
    stage_and_output(&context, &build_results, stderr)
}

/// Locates the workspace root from the current directory.
fn determine_workspace_root() -> Result<Utf8PathBuf> {
    let cwd = std::env::current_dir()?;
    let cwd_utf8 = Utf8PathBuf::try_from(cwd).map_err(|e| InstallerError::ToolchainDetection {
        reason: format!("current directory is not valid UTF-8: {e}"),
    })?;
    find_workspace_root(&cwd_utf8)
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
///
/// Converts lint names into `CrateName` values, validates any provided names,
/// and applies the individual-lints flag to determine the final build list.
fn resolve_requested_crates(cli: &Cli) -> Result<Vec<CrateName>> {
    let lint_crates: Vec<CrateName> = cli
        .lint
        .iter()
        .map(|name| CrateName::from(name.as_str()))
        .collect();

    if !lint_crates.is_empty() {
        validate_crate_names(&lint_crates)?;
    }

    Ok(resolve_crates(&lint_crates, cli.individual_lints))
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
    if !context.cli.quiet {
        write_stderr_line(
            stderr,
            format!(
                "Building {} lint crate(s) with toolchain {}...",
                crates.len(),
                context.toolchain.channel()
            ),
        );
    }

    let config = build_config_for_cli(context);

    Builder::new(config).build_all(crates)
}

/// Stages built libraries and outputs success information.
fn stage_and_output(
    context: &RunContext<'_>,
    build_results: &[whitaker_installer::builder::BuildResult],
    stderr: &mut dyn Write,
) -> Result<()> {
    let stager = Stager::new(context.target_dir.to_owned(), context.toolchain.channel());
    let staging_path = stager.staging_path();

    if !context.cli.quiet {
        write_stderr_line(stderr, format!("Staging libraries to {staging_path}..."));
    }

    stager.prepare()?;
    stager.stage_all(build_results)?;

    if !context.cli.quiet {
        write_stderr_line(stderr, "");
        write_stderr_line(stderr, success_message(build_results.len(), &staging_path));
        write_stderr_line(stderr, "");
        let snippet = ShellSnippet::new(&staging_path);
        write_stderr_line(stderr, snippet.display_text());
    }

    Ok(())
}

fn build_config_for_cli(context: &RunContext<'_>) -> BuildConfig {
    BuildConfig {
        toolchain: context.toolchain.clone(),
        target_dir: context.workspace_root.join("target"),
        jobs: context.cli.jobs,
        verbosity: context.cli.verbosity,
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

    fn base_cli() -> Cli {
        Cli {
            target_dir: None,
            lint: Vec::new(),
            individual_lints: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbosity: 0,
            quiet: false,
        }
    }

    #[test]
    fn cli_parses_defaults() {
        let cli = Cli::parse_from(["whitaker-installer"]);
        assert!(cli.target_dir.is_none());
        assert!(cli.lint.is_empty());
        assert!(!cli.individual_lints);
        assert!(!cli.dry_run);
        assert_eq!(cli.verbosity, 0);
        assert!(!cli.quiet);
    }

    #[test]
    fn cli_parses_target_dir() {
        let cli = Cli::parse_from(["whitaker-installer", "-t", "/tmp/dylint"]);
        assert_eq!(cli.target_dir, Some(Utf8PathBuf::from("/tmp/dylint")));
    }

    #[test]
    fn cli_parses_multiple_lints() {
        let cli = Cli::parse_from([
            "whitaker-installer",
            "-l",
            "module_max_lines",
            "-l",
            "no_expect_outside_tests",
        ]);
        assert_eq!(cli.lint.len(), 2);
    }

    /// Parameterised tests for boolean CLI flags.
    #[rstest]
    #[case::individual_lints(&["whitaker-installer", "--individual-lints"], |cli: &Cli| cli.individual_lints)]
    #[case::dry_run(&["whitaker-installer", "--dry-run"], |cli: &Cli| cli.dry_run)]
    #[case::verbose(&["whitaker-installer", "-v"], |cli: &Cli| cli.verbosity > 0)]
    #[case::quiet(&["whitaker-installer", "-q"], |cli: &Cli| cli.quiet)]
    fn cli_parses_boolean_flags(#[case] args: &[&str], #[case] check: fn(&Cli) -> bool) {
        let cli = Cli::parse_from(args);
        assert!(check(&cli));
    }

    /// Parameterised tests for repeatable verbosity flags.
    #[rstest]
    #[case::double_short(&["whitaker-installer", "-vv"], 2)]
    #[case::triple_short(&["whitaker-installer", "-vvv"], 3)]
    #[case::double_long(&["whitaker-installer", "--verbose", "--verbose"], 2)]
    #[case::double_alias(&["whitaker-installer", "--verbosity", "--verbosity"], 2)]
    fn cli_parses_repeatable_verbosity_flags(#[case] args: &[&str], #[case] expected: u8) {
        let cli = Cli::parse_from(args);
        assert_eq!(cli.verbosity, expected);
    }

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
    #[case::default_suite_only(base_cli(), false, true)]
    #[case::individual_lints(
        {
            let mut cli = base_cli();
            cli.individual_lints = true;
            cli
        },
        true,
        false
    )]
    fn resolve_requested_crates_respects_individual_lints_flag(
        #[case] cli: Cli,
        #[case] expect_lint: bool,
        #[case] expect_suite: bool,
    ) {
        let crates = resolve_requested_crates(&cli).expect("expected crate resolution to succeed");
        assert_eq!(
            crates.contains(&CrateName::from("module_max_lines")),
            expect_lint
        );
        assert_eq!(crates.contains(&CrateName::from("suite")), expect_suite);
    }

    #[test]
    fn resolve_requested_crates_returns_specific_lints_when_provided() {
        let mut cli = base_cli();
        cli.lint = vec!["module_max_lines".to_owned()];

        let crates = resolve_requested_crates(&cli).expect("expected crate resolution to succeed");
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[test]
    fn resolve_requested_crates_rejects_unknown_lints() {
        let mut cli = base_cli();
        cli.lint = vec!["nonexistent_lint".to_owned()];

        let err = resolve_requested_crates(&cli).expect_err("expected crate resolution to fail");
        assert!(matches!(
            err,
            InstallerError::LintCrateNotFound { name } if name == CrateName::from("nonexistent_lint")
        ));
    }

    #[rstest]
    #[case::individual_lints_with_lint(&["whitaker-installer", "--individual-lints", "--lint", "module_max_lines"])]
    #[case::verbose_with_quiet(&["whitaker-installer", "--verbose", "--quiet"])]
    fn cli_rejects_conflicting_flags(#[case] args: &[&str]) {
        Cli::try_parse_from(args).expect_err("expected clap to reject conflicting flags");
    }

    #[test]
    fn build_config_propagates_verbosity_level() {
        let cli = Cli::parse_from(["whitaker-installer", "-vv"]);
        let workspace_root = Utf8PathBuf::from("/tmp");
        let toolchain = Toolchain::with_override(&workspace_root, "nightly-2025-09-18");
        let target_dir = Utf8PathBuf::from("/tmp/target");
        let context = RunContext {
            cli: &cli,
            workspace_root: &workspace_root,
            toolchain: &toolchain,
            target_dir: &target_dir,
        };

        let config = build_config_for_cli(&context);
        assert_eq!(config.verbosity, 2);
    }
}
