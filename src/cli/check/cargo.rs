//! The Cargo package, target and feature selection `whitaker check` accepts.
//!
//! The surface deliberately mirrors `cargo clippy`: a user who knows Cargo's
//! selectors should not have to learn a Whitaker spelling. Every option here
//! is re-emitted to Cargo unchanged, after Whitaker's own `--` separator, and
//! Whitaker never decides what a selector means. The one exception is the
//! deprecated `--all` alias, which is forwarded as itself so that Cargo — not
//! Whitaker — prints the deprecation warning the user already knows.
//!
//! Options that belong to Cargo's output or driver behaviour rather than to
//! selection (for example `--message-format` or `-q`) intentionally stay after
//! the separator. Rule selection, which roadmap item 3.6 owns, must use a
//! spelling such as `--select ALL` and must never take a Cargo flag name.

use std::ffi::OsString;

use camino::{Utf8Path, Utf8PathBuf};
use clap::{ArgAction, Args};

/// The notice `check` prints when `--all` is used.
///
/// Cargo documents `--all` as a deprecated alias for `--workspace` but emits no
/// runtime warning, so `whitaker check --all` names the alias itself. The
/// wording is Cargo's own description of the flag, and the notice goes to the
/// diagnostic stream so a piped standard output stays clean.
pub const ALL_DEPRECATION_NOTICE: &str = "warning: --all is deprecated in favour of --workspace";

/// Cargo's selection options, as `cargo clippy` accepts them.
///
/// The default value selects nothing, which is what makes `whitaker check`
/// forward no selection argument at all and leave Cargo's own default — the
/// current package, the default members, or the whole virtual workspace —
/// exactly as `cargo clippy` computes it.
///
/// # Examples
///
/// ```
/// use ortho_config::NoOpLocalizer;
/// use whitaker::cli::{Cli, Command};
///
/// let cli = Cli::try_parse_localized_from(
///     [
///         "whitaker",
///         "check",
///         "-p",
///         "member_a",
///         "--workspace",
///         "--exclude",
///         "member_b",
///         "--all-features",
///         "--",
///         "-q",
///     ],
///     &NoOpLocalizer::new(),
/// )?;
/// let Command::Check(check) = cli.command else {
///     panic!("expected the check subcommand");
/// };
/// assert_eq!(check.cargo.packages, ["member_a"]);
/// assert!(check.cargo.workspace, "`--workspace` must be retained");
/// assert_eq!(check.cargo.excludes, ["member_b"]);
/// assert!(
///     check.cargo.all_features,
///     "`--all-features` must be retained"
/// );
/// assert_eq!(check.cargo_args, ["-q"], "the suffix is Cargo's alone");
/// # Ok::<(), clap::Error>(())
/// ```
#[expect(
    clippy::struct_excessive_bools,
    reason = "Cargo's selection surface is a flat set of independent flags, not a state machine; \
              each bool is one documented clap flag, and pairing them into enums would \
              misrepresent the command line"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct CargoOptions {
    /// Check only the packages matching SPEC (repeatable; globs allowed).
    #[arg(short = 'p', long = "package", value_name = "SPEC")]
    pub packages: Vec<String>,
    /// Check every package in the workspace.
    #[arg(long)]
    pub workspace: bool,
    /// Exclude packages matching SPEC; Cargo requires `--workspace`.
    #[arg(long = "exclude", value_name = "SPEC")]
    pub excludes: Vec<String>,
    /// Deprecated alias for `--workspace`; Whitaker prints the warning.
    #[arg(long)]
    pub all: bool,
    /// Check the library target.
    #[arg(long)]
    pub lib: bool,
    /// Check all binary targets.
    #[arg(long)]
    pub bins: bool,
    /// Check the binary target named NAME (repeatable).
    #[arg(long, value_name = "NAME")]
    pub bin: Vec<String>,
    /// Check all test targets.
    #[arg(long)]
    pub tests: bool,
    /// Check the test target named NAME (repeatable).
    #[arg(long, value_name = "NAME")]
    pub test: Vec<String>,
    /// Check all example targets.
    #[arg(long)]
    pub examples: bool,
    /// Check the example target named NAME (repeatable).
    #[arg(long, value_name = "NAME")]
    pub example: Vec<String>,
    /// Check all benchmark targets.
    #[arg(long)]
    pub benches: bool,
    /// Check the benchmark target named NAME (repeatable).
    #[arg(long, value_name = "NAME")]
    pub bench: Vec<String>,
    /// Check all targets.
    #[arg(long)]
    pub all_targets: bool,
    /// Activate the given features (repeatable, space- or comma-separated).
    #[arg(short = 'F', long, value_name = "FEATURES")]
    pub features: Vec<String>,
    /// Activate every feature of every selected package.
    #[arg(long)]
    pub all_features: bool,
    /// Do not activate `default` features.
    #[arg(long)]
    pub no_default_features: bool,
    /// Read the workspace from PATH instead of the current directory.
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<Utf8PathBuf>,
    /// Check for the given target triple (repeatable).
    #[arg(long, value_name = "TRIPLE", action = ArgAction::Append)]
    pub target: Vec<String>,
    /// Write build artefacts to DIR.
    #[arg(long, value_name = "DIR")]
    pub target_dir: Option<Utf8PathBuf>,
    /// Check with the given profile.
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Check with the `release` profile.
    #[arg(long)]
    pub release: bool,
    /// Require `Cargo.lock` to be up to date.
    #[arg(long)]
    pub locked: bool,
    /// Require `Cargo.lock` and the cache to be up to date; implies offline.
    #[arg(long)]
    pub frozen: bool,
    /// Run without accessing the network.
    #[arg(long)]
    pub offline: bool,
    /// Run the given number of parallel jobs.
    #[arg(short = 'j', long = "jobs", value_name = "N")]
    pub jobs: Option<String>,
}

impl CargoOptions {
    /// Render the options as Cargo arguments, in a fixed order.
    ///
    /// None of these options is order-sensitive to Cargo, so a fixed order is
    /// safe and keeps the assembled vector deterministic for tests. The values
    /// themselves are never rewritten: a package spec, feature list or target
    /// triple reaches Cargo exactly as the caller typed it.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker::cli::check::CargoOptions;
    ///
    /// let options = CargoOptions {
    ///     packages: vec!["member_a".to_owned()],
    ///     all_features: true,
    ///     ..CargoOptions::default()
    /// };
    /// assert_eq!(options.tokens(), ["-p", "member_a", "--all-features"]);
    /// ```
    #[must_use]
    pub fn tokens(&self) -> Vec<OsString> {
        let mut tokens = Vec::new();
        push_repeated(&mut tokens, "-p", &self.packages);
        push_flag(&mut tokens, "--workspace", self.workspace);
        // Forwarded as itself so Cargo prints its own deprecation warning.
        push_flag(&mut tokens, "--all", self.all);
        push_repeated(&mut tokens, "--exclude", &self.excludes);
        push_flag(&mut tokens, "--lib", self.lib);
        push_flag(&mut tokens, "--bins", self.bins);
        push_repeated(&mut tokens, "--bin", &self.bin);
        push_flag(&mut tokens, "--tests", self.tests);
        push_repeated(&mut tokens, "--test", &self.test);
        push_flag(&mut tokens, "--examples", self.examples);
        push_repeated(&mut tokens, "--example", &self.example);
        push_flag(&mut tokens, "--benches", self.benches);
        push_repeated(&mut tokens, "--bench", &self.bench);
        push_flag(&mut tokens, "--all-targets", self.all_targets);
        push_repeated(&mut tokens, "--features", &self.features);
        push_flag(&mut tokens, "--all-features", self.all_features);
        push_flag(
            &mut tokens,
            "--no-default-features",
            self.no_default_features,
        );
        push_optional(
            &mut tokens,
            "--manifest-path",
            self.manifest_path.as_deref().map(Utf8Path::as_str),
        );
        push_repeated(&mut tokens, "--target", &self.target);
        push_optional(
            &mut tokens,
            "--target-dir",
            self.target_dir.as_deref().map(Utf8Path::as_str),
        );
        push_optional(&mut tokens, "--profile", self.profile.as_deref());
        push_flag(&mut tokens, "--release", self.release);
        push_flag(&mut tokens, "--locked", self.locked);
        push_flag(&mut tokens, "--frozen", self.frozen);
        push_flag(&mut tokens, "--offline", self.offline);
        push_optional(&mut tokens, "-j", self.jobs.as_deref());
        tokens
    }
}

/// Append `flag` when `present`.
fn push_flag(tokens: &mut Vec<OsString>, flag: &str, present: bool) {
    if present {
        tokens.push(OsString::from(flag));
    }
}

/// Append `flag value` once for each value.
fn push_repeated(tokens: &mut Vec<OsString>, flag: &str, values: &[String]) {
    for value in values {
        push_optional(tokens, flag, Some(value.as_str()));
    }
}

/// Append `flag value` when a value is present.
fn push_optional(tokens: &mut Vec<OsString>, flag: &str, candidate: Option<&str>) {
    if let Some(value) = candidate {
        tokens.push(OsString::from(flag));
        tokens.push(OsString::from(value));
    }
}

#[cfg(test)]
mod tests {
    //! The rendering contract: every accepted option reaches Cargo once.

    use rstest::rstest;

    use super::CargoOptions;

    /// Render a set of options and compare it with the literal Cargo vector.
    #[rstest]
    #[case::empty(CargoOptions::default(), &[])]
    #[case::workspace(
        CargoOptions { workspace: true, ..CargoOptions::default() },
        &["--workspace"]
    )]
    #[case::deprecated_all(
        CargoOptions { all: true, ..CargoOptions::default() },
        &["--all"]
    )]
    #[case::packages(
        CargoOptions {
            packages: vec!["a".to_owned(), "b*".to_owned()],
            ..CargoOptions::default()
        },
        &["-p", "a", "-p", "b*"]
    )]
    #[case::excludes(
        CargoOptions {
            workspace: true,
            excludes: vec!["b".to_owned()],
            ..CargoOptions::default()
        },
        &["--workspace", "--exclude", "b"]
    )]
    #[case::targets(
        CargoOptions {
            lib: true,
            bins: true,
            bin: vec!["tool".to_owned()],
            tests: true,
            test: vec!["smoke".to_owned()],
            examples: true,
            example: vec!["demo".to_owned()],
            benches: true,
            bench: vec!["load".to_owned()],
            all_targets: true,
            ..CargoOptions::default()
        },
        &[
            "--lib", "--bins", "--bin", "tool", "--tests", "--test", "smoke",
            "--examples", "--example", "demo", "--benches", "--bench", "load",
            "--all-targets",
        ]
    )]
    #[case::features(
        CargoOptions {
            features: vec!["one two".to_owned(), "three".to_owned()],
            all_features: true,
            no_default_features: true,
            ..CargoOptions::default()
        },
        &[
            "--features", "one two", "--features", "three", "--all-features",
            "--no-default-features",
        ]
    )]
    #[case::build(
        CargoOptions {
            manifest_path: Some("app/Cargo.toml".into()),
            target: vec!["x86_64-unknown-linux-gnu".to_owned()],
            target_dir: Some("target".into()),
            profile: Some("ci".to_owned()),
            release: false,
            locked: true,
            frozen: false,
            offline: true,
            jobs: Some("4".to_owned()),
            ..CargoOptions::default()
        },
        &[
            "--manifest-path", "app/Cargo.toml", "--target",
            "x86_64-unknown-linux-gnu", "--target-dir", "target", "--profile",
            "ci", "--locked", "--offline", "-j", "4",
        ]
    )]
    fn options_render_as_cargo_arguments(#[case] options: CargoOptions, #[case] expected: &[&str]) {
        let rendered = options.tokens();
        let wanted: Vec<std::ffi::OsString> = expected.iter().map(|s| (*s).into()).collect();

        assert_eq!(
            rendered, wanted,
            "the rendered vector must match Cargo's own spelling"
        );
    }

    /// Keep `--release` available as its own selector, not only `--profile`.
    #[test]
    fn the_release_selector_renders_as_itself() {
        let options = CargoOptions {
            release: true,
            ..CargoOptions::default()
        };

        assert_eq!(
            options.tokens(),
            ["--release"],
            "`--release` must reach Cargo unchanged"
        );
    }
}
