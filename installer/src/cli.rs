//! CLI argument definitions for the Whitaker installer.
//!
//! This module defines the command-line interface using clap. It is separated
//! from the main entrypoint to keep the binary small and focused on
//! orchestration.

use camino::Utf8PathBuf;
use clap::Parser;

/// Install Whitaker Dylint lint libraries.
#[derive(Parser, Debug)]
#[command(name = "whitaker-installer")]
#[command(version, about)]
#[command(long_about = concat!(
    "Install Whitaker Dylint lint libraries.\n\n",
    "Whitaker is a collection of opinionated Dylint lints for Rust. This installer ",
    "builds, links, and stages the lint libraries for local use, avoiding the need ",
    "to rebuild from source on each `cargo dylint` invocation.\n\n",
    "By default, the aggregated suite (all lints in a single library) is built. ",
    "Use --individual-lints to build separate libraries, or -l/--lint to select ",
    "specific lints.\n\n",
    "After installation, set DYLINT_LIBRARY_PATH to the staged directory and run ",
    "`cargo dylint --all` to use the lints.",
))]
#[command(after_help = concat!(
    "DEFAULT LINTS:\n",
    "  conditional_max_n_branches    Limit boolean branches in conditionals\n",
    "  function_attrs_follow_docs    Doc comments must precede other attributes\n",
    "  module_max_lines              Warn when modules exceed line threshold\n",
    "  module_must_have_inner_docs   Require inner doc comments on modules\n",
    "  no_expect_outside_tests       Forbid .expect() outside test contexts\n",
    "  no_std_fs_operations          Enforce capability-based filesystem access\n",
    "  no_unwrap_or_else_panic       Deny panicking unwrap_or_else fallbacks\n\n",
    "EXPERIMENTAL LINTS (requires --experimental):\n",
    "  bumpy_road_function           Detect high nesting depth in functions\n\n",
    "EXAMPLES:\n",
    "  Build and stage the aggregated suite:\n",
    "    $ whitaker-installer\n\n",
    "  Build specific lints:\n",
    "    $ whitaker-installer -l module_max_lines -l no_expect_outside_tests\n\n",
    "  Build all individual lint crates:\n",
    "    $ whitaker-installer --individual-lints\n\n",
    "  Include experimental lints in the suite:\n",
    "    $ whitaker-installer --experimental\n\n",
    "  Preview without building:\n",
    "    $ whitaker-installer --dry-run\n\n",
    "For more information, see: https://github.com/leynos/whitaker",
))]
pub struct Cli {
    /// Staging directory for built libraries [default: platform-specific].
    #[arg(short, long, value_name = "DIR")]
    pub target_dir: Option<Utf8PathBuf>,

    /// Build a specific lint by name (can be repeated).
    #[arg(short, long, value_name = "NAME")]
    pub lint: Vec<String>,

    /// Build all individual lint crates instead of the aggregated suite.
    #[arg(long, conflicts_with = "lint")]
    pub individual_lints: bool,

    /// Include experimental lints (e.g., bumpy_road_function).
    #[arg(long)]
    pub experimental: bool,

    /// Number of parallel cargo build jobs.
    #[arg(short, long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Override the toolchain detected from rust-toolchain.toml.
    #[arg(long, value_name = "TOOLCHAIN")]
    pub toolchain: Option<String>,

    /// Show configuration and exit without building.
    #[arg(long)]
    pub dry_run: bool,

    /// Increase cargo output verbosity (repeatable: -v, -vv, -vvv).
    #[arg(
        short,
        long = "verbose",
        alias = "verbosity",
        action = clap::ArgAction::Count,
        conflicts_with = "quiet"
    )]
    pub verbosity: u8,

    /// Suppress progress output (errors still shown).
    #[arg(short, long, conflicts_with = "verbosity")]
    pub quiet: bool,

    /// Skip installation of cargo-dylint and dylint-link.
    #[arg(long)]
    pub skip_deps: bool,

    /// Skip wrapper script generation.
    #[arg(long)]
    pub skip_wrapper: bool,

    /// Do not update existing repository clone.
    #[arg(long)]
    pub no_update: bool,
}

impl Default for Cli {
    /// Creates a `Cli` instance with all flags disabled and no lints selected.
    ///
    /// This is useful for testing or programmatic construction where only
    /// specific fields need to be set.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::Cli;
    ///
    /// let cli = Cli::default();
    /// assert!(!cli.individual_lints);
    /// assert!(!cli.skip_deps);
    /// assert!(cli.lint.is_empty());
    /// ```
    fn default() -> Self {
        Self {
            target_dir: None,
            lint: Vec::new(),
            individual_lints: false,
            experimental: false,
            jobs: None,
            toolchain: None,
            dry_run: false,
            verbosity: 0,
            quiet: false,
            skip_deps: false,
            skip_wrapper: false,
            no_update: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn cli_parses_defaults() {
        let cli = Cli::parse_from(["whitaker-installer"]);
        assert!(cli.target_dir.is_none());
        assert!(cli.lint.is_empty());
        assert!(!cli.individual_lints);
        assert!(!cli.experimental);
        assert!(!cli.dry_run);
        assert_eq!(cli.verbosity, 0);
        assert!(!cli.quiet);
        assert!(!cli.skip_deps);
        assert!(!cli.skip_wrapper);
        assert!(!cli.no_update);
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
    #[case::experimental(&["whitaker-installer", "--experimental"], |cli: &Cli| cli.experimental)]
    #[case::dry_run(&["whitaker-installer", "--dry-run"], |cli: &Cli| cli.dry_run)]
    #[case::verbose(&["whitaker-installer", "-v"], |cli: &Cli| cli.verbosity > 0)]
    #[case::quiet(&["whitaker-installer", "-q"], |cli: &Cli| cli.quiet)]
    #[case::skip_deps(&["whitaker-installer", "--skip-deps"], |cli: &Cli| cli.skip_deps)]
    #[case::skip_wrapper(&["whitaker-installer", "--skip-wrapper"], |cli: &Cli| cli.skip_wrapper)]
    #[case::no_update(&["whitaker-installer", "--no-update"], |cli: &Cli| cli.no_update)]
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

    #[rstest]
    #[case::individual_lints_with_lint(&["whitaker-installer", "--individual-lints", "--lint", "module_max_lines"])]
    #[case::verbose_with_quiet(&["whitaker-installer", "--verbose", "--quiet"])]
    fn cli_rejects_conflicting_flags(#[case] args: &[&str]) {
        Cli::try_parse_from(args).expect_err("expected clap to reject conflicting flags");
    }

    /// Verify the Default impl produces a valid baseline configuration.
    #[test]
    fn cli_default_is_valid() {
        let cli = Cli::default();
        assert!(!cli.individual_lints);
        assert!(!cli.experimental);
        assert!(!cli.skip_deps);
    }
}
