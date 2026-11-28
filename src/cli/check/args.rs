//! The `whitaker check` argument surface.
//!
//! `check` owns three things only: whether it may provision Whitaker, the
//! Cargo selection flags it re-emits unchanged, and the arguments it forwards
//! after `--`. Everything else about a lint run is deliberately absent, so the
//! design surface stays as the design document states it.

use std::ffi::OsString;

use clap::Args;

use super::cargo::CargoOptions;

/// The exit-status contract, printed by `check --help`.
///
/// The codes are the binary-wide contract from ADR 0017. A user who has just
/// seen a non-zero status can find its meaning without leaving the terminal.
pub const EXIT_TABLE: &str = concat!(
    "Exit codes:\n",
    "  0     Dylint and Cargo both succeeded\n",
    "  1     a lint denial, a compile error, or a rejected Cargo argument\n",
    "  2     a Whitaker usage error\n",
    "  3     reserved for a configuration error until roadmap item 3.5.4\n",
    "  4     Whitaker is not provisioned, and repair was refused, failed, or did not converge;\n",
    "        or the user-managed library path does not name exactly one Whitaker suite\n",
    "  5     a host or execution failure\n",
    "\n",
    "Package, target and feature flags before `--` mean what they mean to\n",
    "`cargo clippy`; they are passed unchanged to `cargo check`.\n",
    "Arguments after `--` are passed unchanged to `cargo check` too.\n",
    "Statuses 0, 1, 2, 101 and 128+n come from `cargo dylint` itself.\n",
);

/// Options for `whitaker check`.
///
/// # Examples
///
/// ```
/// use ortho_config::NoOpLocalizer;
/// use whitaker::cli::{Cli, Command};
///
/// let cli = Cli::try_parse_localized_from(
///     ["whitaker", "check", "--no-install", "-p", "x", "--", "-q"],
///     &NoOpLocalizer::new(),
/// )?;
/// let Command::Check(check) = cli.command else {
///     panic!("expected the check subcommand");
/// };
/// assert!(check.no_install, "`--no-install` must set the refusal flag");
/// assert_eq!(
///     check.cargo.packages,
///     ["x"],
///     "`-p x` before `--` is Whitaker's own Cargo selector",
/// );
/// assert_eq!(
///     check.cargo_args,
///     ["-q"],
///     "the parser must preserve `-q` in the Cargo suffix",
/// );
/// # Ok::<(), clap::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
#[command(after_help = EXIT_TABLE)]
pub struct CheckArgs {
    /// Refuse to install missing or broken Whitaker components.
    #[arg(long)]
    pub no_install: bool,
    /// Cargo's package, target and feature selectors.
    ///
    /// They are parsed here so `whitaker check --all` can mean what
    /// `cargo clippy --all` means, then re-emitted to Cargo unchanged after
    /// Whitaker's own separator.
    #[command(flatten)]
    pub cargo: CargoOptions,
    /// Arguments passed unchanged to `cargo check`.
    ///
    /// They follow a `--` so Whitaker's own selectors are never confused with
    /// Cargo arguments that happen to share a name.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo_args: Vec<OsString>,
}
