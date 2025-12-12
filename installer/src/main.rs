//! Whitaker installer CLI entrypoint.
//!
//! This binary builds, links, and stages Dylint lint libraries for local use.
//! After installation, it prints shell configuration snippets for enabling
//! library discovery.

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
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
    if let Err(err) = run(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let workspace_root = determine_workspace_root()?;
    let toolchain = resolve_toolchain(&workspace_root, cli.toolchain.as_deref())?;
    let crates = resolve_requested_crates(&cli)?;
    let target_dir = determine_target_dir(cli.target_dir.clone())?;

    if cli.dry_run {
        eprintln!("Dry run - no files will be modified\n");
        eprintln!("Workspace root: {workspace_root}");
        eprintln!("Toolchain: {}", toolchain.channel());
        eprintln!("Target directory: {target_dir}");
        eprintln!("Verbose: {}", cli.verbose);

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
    let lint_names: Vec<CrateName> = cli.lint.iter().cloned().map(CrateName::from).collect();

    if !lint_names.is_empty() {
        validate_crate_names(&lint_names)?;
    }

    Ok(resolve_crates(&lint_names, cli.suite_only, cli.no_suite))
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
}
