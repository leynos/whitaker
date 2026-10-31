//! CLI argument definitions for the Whitaker installer.
//!
//! This module defines the command-line interface using clap. It is separated
//! from the main entrypoint to keep the binary small and focused on
//! orchestration.

use crate::artefact::suite_ref::SuiteRef;
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
    "  no_unwrap_or_else_panic       Deny panicking unwrap_or_else fallbacks\n",
    "  test_must_not_have_example    Forbid examples in test documentation\n\n",
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

    /// Run `cargo dylint` through the installed bootstrap.
    ///
    /// The `whitaker` and `whitaker-ls` wrappers call this. It checks the
    /// installation, exports `DYLINT_LIBRARY_PATH`, `DYLINT_CLIPPY_REPO`,
    /// and `DYLINT_DRIVER_PATH`, serializes the first driver build, and exits
    /// with `cargo dylint`'s status.
    Run(RunArgs),
}

/// Arguments for the run command.
#[derive(Parser, Debug, Clone)]
pub struct RunArgs {
    /// Arguments passed to `cargo dylint` unchanged, after `--`.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub args: Vec<std::ffi::OsString>,
}

/// Arguments for the install command.
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
pub struct InstallArgs {
    /// Staging directory for built libraries [default: platform-specific].
    #[arg(short, long, value_name = "DIR")]
    pub target_dir: Option<Utf8PathBuf>,

    /// Build a specific lint by name (can be repeated).
    #[arg(short, long, value_name = "NAME")]
    pub lint: Vec<String>,

    /// Flags selecting which lint crates are built.
    #[command(flatten)]
    pub lint_selection: LintSelectionFlags,

    /// Number of parallel cargo build jobs.
    #[arg(short, long, value_name = "N")]
    pub jobs: Option<usize>,

    /// Override the toolchain detected from rust-toolchain.toml.
    #[arg(long, value_name = "TOOLCHAIN")]
    pub toolchain: Option<String>,

    /// Flags adjusting how the installer executes the build.
    #[command(flatten)]
    pub execution: ExecutionFlags,

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

    /// Flags that skip individual installation steps.
    #[command(flatten)]
    pub skip: SkipFlags,

    /// Build the lint suite from this git reference instead of the branch tip.
    ///
    /// Accepts a tag, a branch or a commit. Without it the suite comes from
    /// whatever is at the default branch tip, so a change there alters lint
    /// results with no commit in the consuming repository.
    ///
    /// A pinned suite is built from source: prebuilt artefacts are published
    /// only for the tip, so pinning trades install time for reproducibility.
    #[arg(long = "suite-version", alias = "suite-ref", value_name = "REF")]
    pub suite_version: Option<SuiteRef>,

    /// Skip prebuilt artefact download and build from source.
    #[arg(long = "build-only")]
    pub is_build_only: bool,

    /// Install this Clippy snapshot bundle instead of downloading it.
    ///
    /// The bundle's `.sha256` sidecar must sit beside it. The bundle is
    /// verified exactly as a downloaded one is; there is no fallback to
    /// public Clippy when it is missing or invalid.
    #[arg(long = "clippy-snapshot-bundle", value_name = "FILE")]
    pub clippy_snapshot_bundle: Option<Utf8PathBuf>,

    /// Fail rather than build from source when a published artefact is absent.
    ///
    /// The installer's fallbacks are silent successes: a missing prebuilt lint
    /// library becomes a local compilation, and a missing Dylint tool archive
    /// becomes `cargo install`. Both work, so a run that took either looks
    /// healthy while having built something nobody pinned, slowly. In CI that
    /// is a defect rather than a degraded mode, so this turns each fallback
    /// into an error naming what was missing.
    ///
    /// Rejected alongside the flags that require a source build, because
    /// asking for both is a contradiction rather than a preference.
    #[arg(
        long = "no-source-fallback",
        conflicts_with_all = ["is_build_only", "experimental", "suite_version"]
    )]
    pub no_source_fallback: bool,
}

/// Flags selecting which lint crates are built.
///
/// Flattened into [`InstallArgs`] so the command-line surface is unchanged.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct LintSelectionFlags {
    /// Build all individual lint crates instead of the aggregated suite.
    #[arg(long, conflicts_with = "lint")]
    pub individual_lints: bool,

    /// Include experimental lints when available.
    #[arg(long)]
    pub experimental: bool,
}

/// Flags adjusting how the installer executes the build.
///
/// Flattened into [`InstallArgs`] so the command-line surface is unchanged.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionFlags {
    /// Install rustc-codegen-cranelift via rustup.
    #[arg(long, default_value_t = false)]
    pub cranelift: bool,

    /// Show configuration and exit without building.
    #[arg(long)]
    pub dry_run: bool,
}

/// Flags that skip individual installation steps.
///
/// Flattened into [`InstallArgs`] so the command-line surface is unchanged.
#[derive(clap::Args, Debug, Clone, Default, PartialEq, Eq)]
