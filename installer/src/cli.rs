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

    /// Install rustc-codegen-cranelift via rustup.
    #[arg(long, default_value_t = false)]
    pub cranelift: bool,

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

/// Name of the environment variable that forbids a source build.
pub const NO_SOURCE_FALLBACK_ENV: &str = "WHITAKER_NO_SOURCE_FALLBACK";

/// Whether the environment forbids falling back to a source build.
///
/// Any value other than the empty string, `0` or `false` enables the rule.
/// A caller who exported the variable at all meant something by it, and
/// reading an unrecognized value as "off" would silently disable a
/// protection.
fn environment_forbids_source_fallback() -> bool {
    match std::env::var(NO_SOURCE_FALLBACK_ENV) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false"
        ),
        Err(_) => false,
    }
}

impl InstallArgs {
    /// Whether a missing published artefact must fail rather than build.
    ///
    /// The environment variable exists for callers that cannot easily add a
    /// flag, such as a composite action invoking the installer through a
    /// wrapper. Either source enables the rule; neither disables the other,
    /// because a caller who set one meant it.
    #[must_use]
    pub fn forbids_source_fallback(&self) -> bool {
        self.no_source_fallback || environment_forbids_source_fallback()
    }

    /// How this run should react to a missing published artefact.
    #[must_use]
    pub fn source_policy(&self) -> crate::deps::SourcePolicy {
        crate::deps::SourcePolicy {
            quiet: self.quiet,
            no_source_fallback: self.forbids_source_fallback(),
        }
    }

    /// Whether the arguments alone rule out a prebuilt artefact.
    ///
    /// Three unrelated reasons, so they are named here rather than read as one
    /// condition: the caller asked to build, the caller asked for experimental
    /// lints which are never published, or the caller pinned a suite that the
    /// rolling release cannot carry.
    fn arguments_force_a_source_build(&self) -> bool {
        self.is_build_only || self.experimental || self.suite_version.is_some()
    }

    /// Return true when installer settings permit a prebuilt download attempt.
    ///
    /// Prebuilt artefacts are skipped when:
    /// - `--build-only` is set,
    /// - `--suite-version` pins the suite, because prebuilt artefacts are
    ///   published only for the branch tip and so can never satisfy a pin, or
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
    ///
    /// let pinned_args = InstallArgs {
    ///     suite_version: Some("v0.2.7".try_into().expect("valid reference")),
    ///     ..InstallArgs::default()
    /// };
    /// assert!(!pinned_args.should_attempt_prebuilt(&requested));
    /// ```
    #[must_use]
    pub fn should_attempt_prebuilt(&self, requested_crates: &[CrateName]) -> bool {
        if self.arguments_force_a_source_build() {
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
            cranelift: false,
            dry_run: false,
            verbosity: 0,
            quiet: false,
            skip_deps: false,
            skip_wrapper: false,
            no_update: false,
            suite_version: None,
            no_source_fallback: false,
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

#[cfg(test)]
mod no_source_fallback_tests {
    use super::*;
    use rstest::rstest;

    fn install_args(no_source_fallback: bool) -> InstallArgs {
        InstallArgs {
            no_source_fallback,
            ..InstallArgs::default()
        }
    }

    #[rstest]
    #[case::unset(None, false, false)]
    #[case::flag_alone(None, true, true)]
    #[case::empty_is_off(Some(""), false, false)]
    #[case::zero_is_off(Some("0"), false, false)]
    #[case::false_is_off(Some("false"), false, false)]
    #[case::one_is_on(Some("1"), false, true)]
    #[case::true_is_on(Some("true"), false, true)]
    #[case::yes_is_on(Some("yes"), false, true)]
    #[case::mixed_case_false_is_off(Some("FALSE"), false, false)]
    #[case::flag_wins_over_off_value(Some("false"), true, true)]
    fn the_environment_and_the_flag_agree_on_the_rule(
        #[case] environment: Option<&str>,
        #[case] flag: bool,
        #[case] expected: bool,
    ) {
        // A caller who exported the variable meant something by it, so an
        // unrecognized value enables the rule rather than silently disabling
        // a protection. `0`, `false` and empty are the three shapes that
        // conventionally mean "off".
        temp_env::with_var(NO_SOURCE_FALLBACK_ENV, environment, || {
            assert_eq!(install_args(flag).forbids_source_fallback(), expected);
        });
    }
}
