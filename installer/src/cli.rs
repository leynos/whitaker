//! CLI argument definitions for the Whitaker installer.
//!
//! This module defines the command-line interface using clap. It is separated
//! from the main entrypoint to keep the binary small and focused on
//! orchestration.

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

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
    "  List installed lints:\n",
    "    $ whitaker-installer list\n\n",
    "  Preview without building:\n",
    "    $ whitaker-installer --dry-run\n\n",
    "For more information, see: https://github.com/leynos/whitaker",
))]
pub struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Install arguments (used when no subcommand is given).
    #[command(flatten)]
    pub install: InstallArgs,
}

/// Available subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Install lint libraries (default when no subcommand given).
    Install(InstallArgs),

    /// List installed lints.
    List(ListArgs),
}

/// Arguments for the install command.
#[derive(Parser, Debug, Clone)]
pub struct InstallArgs {
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

    /// Skip prebuilt artefact download and build from source.
    #[arg(long)]
    pub build_only: bool,
}

/// Arguments for the list command.
#[derive(Parser, Debug, Clone)]
pub struct ListArgs {
    /// Output in JSON format for scripting.
    #[arg(long)]
    pub json: bool,

    /// Staging directory to scan [default: platform-specific].
    #[arg(short, long, value_name = "DIR")]
    pub target_dir: Option<Utf8PathBuf>,
}

impl Default for InstallArgs {
    /// Creates an `InstallArgs` instance with all flags disabled and no lints selected.
    ///
    /// This is useful for testing or programmatic construction where only
    /// specific fields need to be set.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::InstallArgs;
    ///
    /// let args = InstallArgs::default();
    /// assert!(!args.individual_lints);
    /// assert!(!args.skip_deps);
    /// assert!(args.lint.is_empty());
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
            build_only: false,
        }
    }
}

impl Default for ListArgs {
    /// Creates a `ListArgs` instance with default settings.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::ListArgs;
    ///
    /// let args = ListArgs::default();
    /// assert!(!args.json);
    /// assert!(args.target_dir.is_none());
    /// ```
    fn default() -> Self {
        Self {
            json: false,
            target_dir: None,
        }
    }
}

impl Cli {
    /// Returns the effective install arguments.
    ///
    /// If an `Install` subcommand was provided, returns those arguments.
    /// Otherwise returns the flattened install arguments for backwards
    /// compatibility.
    ///
    /// # Note
    ///
    /// When `Command::List` is active, this returns the default flattened
    /// install arguments. Callers should check `self.command` before calling
    /// this method if the `List` case needs different handling.
    #[must_use]
    pub fn install_args(&self) -> &InstallArgs {
        match &self.command {
            Some(Command::Install(args)) => args,
            Some(Command::List(_)) | None => &self.install,
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
        assert!(cli.command.is_none());
        assert!(cli.install.target_dir.is_none());
        assert!(cli.install.lint.is_empty());
        assert!(!cli.install.individual_lints);
        assert!(!cli.install.experimental);
        assert!(!cli.install.dry_run);
        assert_eq!(cli.install.verbosity, 0);
        assert!(!cli.install.quiet);
        assert!(!cli.install.skip_deps);
        assert!(!cli.install.skip_wrapper);
        assert!(!cli.install.no_update);
        assert!(!cli.install.build_only);
    }

    #[test]
    fn cli_parses_target_dir() {
        let cli = Cli::parse_from(["whitaker-installer", "-t", "/tmp/dylint"]);
        assert_eq!(
            cli.install.target_dir,
            Some(Utf8PathBuf::from("/tmp/dylint"))
        );
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
        assert_eq!(cli.install.lint.len(), 2);
    }

    #[test]
    fn cli_parses_list_subcommand() {
        let cli = Cli::parse_from(["whitaker-installer", "list"]);
        assert!(matches!(cli.command, Some(Command::List(_))));
    }

    #[test]
    fn cli_parses_list_with_json() {
        let cli = Cli::parse_from(["whitaker-installer", "list", "--json"]);
        match cli.command {
            Some(Command::List(args)) => assert!(args.json),
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn cli_parses_list_with_target_dir() {
        let cli = Cli::parse_from(["whitaker-installer", "list", "-t", "/custom/path"]);
        match cli.command {
            Some(Command::List(args)) => {
                assert_eq!(args.target_dir, Some(Utf8PathBuf::from("/custom/path")));
            }
            _ => panic!("expected List command"),
        }
    }

    #[test]
    fn cli_parses_install_subcommand() {
        let cli = Cli::parse_from(["whitaker-installer", "install"]);
        assert!(matches!(cli.command, Some(Command::Install(_))));
    }

    #[test]
    fn cli_parses_install_with_args() {
        let cli = Cli::parse_from([
            "whitaker-installer",
            "install",
            "--experimental",
            "-l",
            "module_max_lines",
        ]);
        match cli.command {
            Some(Command::Install(args)) => {
                assert!(args.experimental);
                assert_eq!(args.lint, vec!["module_max_lines"]);
            }
            _ => panic!("expected Install command"),
        }
    }

    /// Parameterised tests for boolean CLI flags (backwards compatibility).
    #[rstest]
    #[case::individual_lints(&["whitaker-installer", "--individual-lints"], |cli: &Cli| cli.install.individual_lints)]
    #[case::experimental(&["whitaker-installer", "--experimental"], |cli: &Cli| cli.install.experimental)]
    #[case::dry_run(&["whitaker-installer", "--dry-run"], |cli: &Cli| cli.install.dry_run)]
    #[case::verbose(&["whitaker-installer", "-v"], |cli: &Cli| cli.install.verbosity > 0)]
    #[case::quiet(&["whitaker-installer", "-q"], |cli: &Cli| cli.install.quiet)]
    #[case::skip_deps(&["whitaker-installer", "--skip-deps"], |cli: &Cli| cli.install.skip_deps)]
    #[case::skip_wrapper(&["whitaker-installer", "--skip-wrapper"], |cli: &Cli| cli.install.skip_wrapper)]
    #[case::no_update(&["whitaker-installer", "--no-update"], |cli: &Cli| cli.install.no_update)]
    #[case::build_only(&["whitaker-installer", "--build-only"], |cli: &Cli| cli.install.build_only)]
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
        assert_eq!(cli.install.verbosity, expected);
    }

    #[rstest]
    #[case::individual_lints_with_lint(&["whitaker-installer", "--individual-lints", "--lint", "module_max_lines"])]
    #[case::verbose_with_quiet(&["whitaker-installer", "--verbose", "--quiet"])]
    fn cli_rejects_conflicting_flags(#[case] args: &[&str]) {
        Cli::try_parse_from(args).expect_err("expected clap to reject conflicting flags");
    }

    /// Verify the Default impl produces a valid baseline configuration.
    #[test]
    fn install_args_default_is_valid() {
        let args = InstallArgs::default();
        assert!(!args.individual_lints);
        assert!(!args.experimental);
        assert!(!args.skip_deps);
    }

    #[test]
    fn list_args_default_is_valid() {
        let args = ListArgs::default();
        assert!(!args.json);
        assert!(args.target_dir.is_none());
    }

    #[test]
    fn install_args_returns_flattened_when_no_subcommand() {
        let cli = Cli::parse_from(["whitaker-installer", "--experimental"]);
        let args = cli.install_args();
        assert!(args.experimental);
    }

    #[test]
    fn install_args_returns_subcommand_args_when_present() {
        let cli = Cli::parse_from(["whitaker-installer", "install", "--dry-run"]);
        let args = cli.install_args();
        assert!(args.dry_run);
    }
}
