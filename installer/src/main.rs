//! Whitaker installer CLI entrypoint.
//!
//! This binary builds, links, and stages Dylint lint libraries for local use.
//! After installation, it prints shell configuration snippets for enabling
//! library discovery.

use camino::Utf8PathBuf;
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

    /// Suppress output except errors.
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    // Determine workspace root
    let cwd = std::env::current_dir()?;
    let cwd_utf8 = Utf8PathBuf::try_from(cwd).map_err(|e| InstallerError::ToolchainDetection {
        reason: format!("current directory is not valid UTF-8: {e}"),
    })?;
    let workspace_root = find_workspace_root(&cwd_utf8)?;

    // Detect or override toolchain
    let toolchain = match &cli.toolchain {
        Some(channel) => Toolchain::with_override(&workspace_root, channel)?,
        None => Toolchain::detect(&workspace_root)?,
    };

    // Verify toolchain is installed
    toolchain.verify_installed()?;

    // Convert lint names to CrateName
    let lint_names: Vec<CrateName> = cli
        .lint
        .iter()
        .map(|s| CrateName::from(s.as_str()))
        .collect();

    // Validate lint names if specific lints were requested
    if !lint_names.is_empty() {
        validate_crate_names(&lint_names)?;
    }

    // Resolve which crates to build
    let crates = resolve_crates(&lint_names, cli.suite_only, cli.no_suite);

    // Determine target directory
    let target_dir = cli
        .target_dir
        .clone()
        .or_else(default_target_dir)
        .ok_or_else(|| InstallerError::StagingFailed {
            reason: "could not determine default target directory".to_owned(),
        })?;

    // Handle dry run
    if cli.dry_run {
        let config = DryRunConfig {
            workspace_root: &workspace_root,
            toolchain: toolchain.channel(),
            crates: &crates,
            target_dir: &target_dir,
        };
        return dry_run_output(&cli, config);
    }

    // Build crates
    if !cli.quiet {
        eprintln!(
            "Building {} lint crate(s) with toolchain {}...",
            crates.len(),
            toolchain.channel()
        );
    }

    let build_target_dir = workspace_root.join("target");
    let config = BuildConfig {
        toolchain: toolchain.clone(),
        target_dir: build_target_dir,
        jobs: cli.jobs,
        verbose: cli.verbose,
    };

    let builder = Builder::new(config);
    let build_results = builder.build_all(&crates)?;

    // Stage libraries
    if !cli.quiet {
        eprintln!("Staging libraries to {}...", target_dir);
    }

    let stager = Stager::new(target_dir.clone(), toolchain.channel());
    stager.prepare()?;
    stager.stage_all(&build_results)?;

    // Output success message and shell snippet
    if !cli.quiet {
        eprintln!();
        eprintln!("{}", success_message(build_results.len(), &target_dir));
        eprintln!();
        let snippet = ShellSnippet::new(&target_dir);
        eprintln!("{}", snippet.display_text());
    }

    Ok(())
}

/// Configuration for dry run output.
struct DryRunConfig<'a> {
    workspace_root: &'a Utf8PathBuf,
    toolchain: &'a str,
    crates: &'a [CrateName],
    target_dir: &'a Utf8PathBuf,
}

fn dry_run_output(cli: &Cli, config: DryRunConfig<'_>) -> Result<()> {
    eprintln!("Dry run - no files will be modified\n");
    eprintln!("Workspace root: {}", config.workspace_root);
    eprintln!("Toolchain: {}", config.toolchain);
    eprintln!("Target directory: {}", config.target_dir);
    eprintln!("Verbose: {}", cli.verbose);

    if let Some(jobs) = cli.jobs {
        eprintln!("Parallel jobs: {jobs}");
    }

    eprintln!("\nCrates to build:");
    for crate_name in config.crates {
        eprintln!("  - {crate_name}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn cli_parses_suite_only() {
        let cli = Cli::parse_from(["whitaker-install", "--suite-only"]);
        assert!(cli.suite_only);
    }

    #[test]
    fn cli_parses_dry_run() {
        let cli = Cli::parse_from(["whitaker-install", "--dry-run"]);
        assert!(cli.dry_run);
    }
}
