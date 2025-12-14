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
#[command(name = "whitaker-install")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Target directory for staged libraries.
    #[arg(short, long, value_name = "DIR")]
    target_dir: Option<Utf8PathBuf>,

    /// Build specific lints (can be repeated).
    #[arg(short, long, value_name = "NAME")]
    lint: Vec<String>,

    /// Build only the aggregated suite.
    #[arg(long, conflicts_with = "lint", conflicts_with = "no_suite")]
    suite_only: bool,

    /// Exclude the aggregated suite from the build.
    #[arg(long, conflicts_with = "suite_only")]
    no_suite: bool,

    /// Number of parallel build jobs.
    #[arg(short, long, value_name = "N")]
    jobs: Option<usize>,

    /// Override the detected toolchain.
    #[arg(long, value_name = "TOOLCHAIN")]
    toolchain: Option<String>,

    /// Show what would be done without executing.
    #[arg(long)]
    dry_run: bool,

    /// Increase verbosity.
    #[arg(short, long)]
    verbose: bool,

    /// Suppress output except errors (does not affect --dry-run output).
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();
    let mut stderr = std::io::stderr();
    let exit_code = exit_code_for_run_result(run(cli), &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(cli: Cli) -> Result<()> {
    let workspace_root = determine_workspace_root()?;
    let crates = resolve_requested_crates(&cli)?;
    let toolchain = resolve_toolchain(&workspace_root, cli.toolchain.as_deref())?;
    let target_dir = determine_target_dir(cli.target_dir.clone())?;

    if cli.dry_run {
        eprintln!("Dry run - no files will be modified\n");
        eprintln!("Workspace root: {workspace_root}");
        eprintln!("Toolchain: {}", toolchain.channel());
        eprintln!("Target directory: {target_dir}");
        eprintln!("Verbose: {}", cli.verbose);
        eprintln!("Quiet: {}", cli.quiet);

        if let Some(jobs) = cli.jobs {
            eprintln!("Parallel jobs: {jobs}");
        }

        eprintln!("\nCrates to build:");
        for crate_name in &crates {
            eprintln!("  - {crate_name}");
        }

        return Ok(());
    }

    let build_results = perform_build(&cli, &workspace_root, &toolchain, &crates)?;
    stage_and_output(&cli, &toolchain, &target_dir, &build_results)
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
/// This converts lint names into `CrateName` values, validates any provided
/// names, and applies suite-only / no-suite policy to determine the final build
/// list.
fn resolve_requested_crates(cli: &Cli) -> Result<Vec<CrateName>> {
    if cli.suite_only {
        return Ok(resolve_crates(&[], true, cli.no_suite));
    }

    let lint_crates: Vec<CrateName> = cli
        .lint
        .iter()
        .map(|name| CrateName::from(name.as_str()))
        .collect();

    if !lint_crates.is_empty() {
        validate_crate_names(&lint_crates)?;
    }

    Ok(resolve_crates(&lint_crates, cli.suite_only, cli.no_suite))
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
    cli: &Cli,
    workspace_root: &Utf8Path,
    toolchain: &Toolchain,
    crates: &[CrateName],
) -> Result<Vec<whitaker_installer::builder::BuildResult>> {
    if !cli.quiet {
        eprintln!(
            "Building {} lint crate(s) with toolchain {}...",
            crates.len(),
            toolchain.channel()
        );
    }

    let config = BuildConfig {
        toolchain: toolchain.clone(),
        target_dir: workspace_root.join("target"),
        jobs: cli.jobs,
        verbose: cli.verbose,
    };

    Builder::new(config).build_all(crates)
}

/// Stages built libraries and outputs success information.
fn stage_and_output(
    cli: &Cli,
    toolchain: &Toolchain,
    target_dir: &Utf8Path,
    build_results: &[whitaker_installer::builder::BuildResult],
) -> Result<()> {
    if !cli.quiet {
        eprintln!("Staging libraries to {}...", target_dir);
    }

    let stager = Stager::new(target_dir.to_owned(), toolchain.channel());
    stager.prepare()?;
    stager.stage_all(build_results)?;

    if !cli.quiet {
        eprintln!();
        eprintln!("{}", success_message(build_results.len(), target_dir));
        eprintln!();
        let snippet = ShellSnippet::new(target_dir);
        eprintln!("{}", snippet.display_text());
    }

    Ok(())
}

fn exit_code_for_run_result(result: Result<()>, stderr: &mut dyn Write) -> i32 {
    match result {
        Ok(()) => 0,
        Err(err) => {
            let _ = writeln!(stderr, "{err}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn cli_parses_defaults() {
        let cli = Cli::parse_from(["whitaker-install"]);
        assert!(cli.target_dir.is_none());
        assert!(cli.lint.is_empty());
        assert!(!cli.suite_only);
        assert!(!cli.no_suite);
        assert!(!cli.dry_run);
        assert!(!cli.verbose);
        assert!(!cli.quiet);
    }

    #[test]
    fn cli_parses_target_dir() {
        let cli = Cli::parse_from(["whitaker-install", "-t", "/tmp/dylint"]);
        assert_eq!(cli.target_dir, Some(Utf8PathBuf::from("/tmp/dylint")));
    }

    #[test]
    fn cli_parses_multiple_lints() {
        let cli = Cli::parse_from([
            "whitaker-install",
            "-l",
            "module_max_lines",
            "-l",
            "no_expect_outside_tests",
        ]);
        assert_eq!(cli.lint.len(), 2);
    }

    /// Parameterised tests for boolean CLI flags.
    #[rstest]
    #[case::suite_only(&["whitaker-install", "--suite-only"], |cli: &Cli| cli.suite_only)]
    #[case::dry_run(&["whitaker-install", "--dry-run"], |cli: &Cli| cli.dry_run)]
    #[case::verbose(&["whitaker-install", "-v"], |cli: &Cli| cli.verbose)]
    #[case::quiet(&["whitaker-install", "-q"], |cli: &Cli| cli.quiet)]
    #[case::no_suite(&["whitaker-install", "--no-suite"], |cli: &Cli| cli.no_suite)]
    fn cli_parses_boolean_flags(#[case] args: &[&str], #[case] check: fn(&Cli) -> bool) {
        let cli = Cli::parse_from(args);
        assert!(check(&cli));
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

        let stderr = String::from_utf8(stderr).expect("stderr was not UTF-8");
        assert!(stderr.contains("lint crate nonexistent_lint not found"));
    }

    #[rstest]
    #[case::default(
        Cli {
            target_dir: None,
            lint: Vec::new(),
            suite_only: false,
            no_suite: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbose: false,
            quiet: false,
        },
        true,
        true
    )]
    #[case::no_suite(
        Cli {
            target_dir: None,
            lint: Vec::new(),
            suite_only: false,
            no_suite: true,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbose: false,
            quiet: false,
        },
        true,
        false
    )]
    #[case::suite_only(
        Cli {
            target_dir: None,
            lint: Vec::new(),
            suite_only: true,
            no_suite: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbose: false,
            quiet: false,
        },
        false,
        true
    )]
    fn resolve_requested_crates_respects_suite_and_lint_flags(
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
        let cli = Cli {
            target_dir: None,
            lint: vec!["module_max_lines".to_owned()],
            suite_only: false,
            no_suite: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbose: false,
            quiet: false,
        };

        let crates = resolve_requested_crates(&cli).expect("expected crate resolution to succeed");
        assert_eq!(crates, vec![CrateName::from("module_max_lines")]);
    }

    #[test]
    fn resolve_requested_crates_rejects_unknown_lints() {
        let cli = Cli {
            target_dir: None,
            lint: vec!["nonexistent_lint".to_owned()],
            suite_only: false,
            no_suite: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbose: false,
            quiet: false,
        };

        let err = resolve_requested_crates(&cli).expect_err("expected crate resolution to fail");
        assert!(matches!(
            err,
            InstallerError::LintCrateNotFound { name } if name == CrateName::from("nonexistent_lint")
        ));
    }

    #[rstest]
    #[case::suite_only_with_lint(&["whitaker-install", "--suite-only", "--lint", "module_max_lines"])]
    #[case::suite_only_with_no_suite(&["whitaker-install", "--suite-only", "--no-suite"])]
    #[case::verbose_with_quiet(&["whitaker-install", "--verbose", "--quiet"])]
    fn cli_rejects_conflicting_flags(#[case] args: &[&str]) {
        Cli::try_parse_from(args).expect_err("expected clap to reject conflicting flags");
    }
}
