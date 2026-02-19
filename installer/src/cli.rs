//! CLI argument definitions for the Whitaker installer.
//!
//! This module defines the command-line interface using clap. It is separated
//! from the main entrypoint to keep the binary small and focused on
//! orchestration.

use crate::crate_name::CrateName;
use crate::resolution::EXPERIMENTAL_LINT_CRATES;
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
    "  bumpy_road_function           Detect multiple complexity clusters in functions\n",
    "  conditional_max_n_branches    Limit boolean branches in conditionals\n",
    "  function_attrs_follow_docs    Doc comments must precede other attributes\n",
    "  module_max_lines              Warn when modules exceed line threshold\n",
    "  module_must_have_inner_docs   Require inner doc comments on modules\n",
    "  no_expect_outside_tests       Forbid .expect() outside test contexts\n",
    "  no_std_fs_operations          Enforce capability-based filesystem access\n",
    "  no_unwrap_or_else_panic       Deny panicking unwrap_or_else fallbacks\n\n",
    "EXPERIMENTAL LINTS (requires --experimental):\n",
    "  (none currently)\n\n",
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

    /// Include experimental lints when available.
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
    #[arg(long = "build-only")]
    pub is_build_only: bool,
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

impl InstallArgs {
    /// Return true when installer settings permit a prebuilt download attempt.
    ///
    /// Prebuilt artefacts are skipped when:
    /// - `--build-only` is set, or
    /// - experimental lint behaviour is requested, either via
    ///   `--experimental` (suite build) or explicit experimental crates when
    ///   the experimental crate list is non-empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::cli::InstallArgs;
    /// use whitaker_installer::crate_name::CrateName;
    ///
    /// let requested = vec![CrateName::from("whitaker_suite")];
    ///
    /// let default_args = InstallArgs::default();
    /// assert!(default_args.should_attempt_prebuilt(&requested));
    ///
    /// let build_only_args = InstallArgs {
    ///     is_build_only: true,
    ///     ..InstallArgs::default()
    /// };
    /// assert!(!build_only_args.should_attempt_prebuilt(&requested));
    /// ```
    #[must_use]
    pub fn should_attempt_prebuilt(&self, requested_crates: &[CrateName]) -> bool {
        if self.is_build_only || self.experimental {
            return false;
        }
        !requested_crates
            .iter()
            .any(|crate_name| EXPERIMENTAL_LINT_CRATES.contains(&crate_name.as_str()))
    }
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
            is_build_only: false,
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
#[path = "cli_tests.rs"]
mod tests;
