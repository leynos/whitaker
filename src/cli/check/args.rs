//! The `whitaker check` argument surface.
//!
//! `check` owns two things only: whether it may provision Whitaker, and the
//! arguments it forwards. Everything else about a lint run is deliberately
//! absent, so the design surface stays as the design document states it.

use std::ffi::OsString;

use clap::Args;

/// The exit-status contract, printed by `check --help`.
///
/// The codes are the binary-wide contract from ADR 0017. A user who has just
/// seen a non-zero status can find its meaning without leaving the terminal.
pub const EXIT_TABLE: &str = concat!(
    "Exit codes:\n",
    "  0     Dylint and Cargo both succeeded\n",
    "  1     a lint denial, a compile error, or a rejected Cargo argument\n",
    "  2     a Whitaker usage error\n",
    "  3     a configuration error\n",
    "  4     Whitaker is not provisioned, and repair was refused, failed, or did not converge\n",
    "  5     a host or execution failure\n",
    "\n",
    "Arguments after `--` are passed unchanged to `cargo check`.\n",
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
///     ["whitaker", "check", "--no-install", "--", "-p", "x"],
///     &NoOpLocalizer::new(),
/// )?;
/// let Command::Check(check) = cli.command else {
///     panic!("expected the check subcommand");
/// };
/// assert!(check.no_install);
/// assert_eq!(check.cargo_args, ["-p", "x"]);
/// # Ok::<(), clap::Error>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
#[command(after_help = EXIT_TABLE)]
pub struct CheckArgs {
    /// Refuse to install missing or broken Whitaker components.
    #[arg(long)]
    pub no_install: bool,
    /// Arguments passed unchanged to `cargo check`.
    ///
    /// They follow a `--` so an option such as `-p` is never mistaken for a
    /// Whitaker option; `whitaker check -p x` is a usage error that says so.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo_args: Vec<OsString>,
}
