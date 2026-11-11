//! Process entry point for the root `whitaker` command.

use std::{io::Write as _, process};

use ortho_config::NoOpLocalizer;
use whitaker::{adapters::SystemServices, cli, diagnostics};

/// Run the CLI, converting its outcome into the process exit code.
///
/// Diagnostics are configured before routing so the forwarded Dylint route,
/// which never reaches the clap parser, honours the same `-q`/`-v` policy.
fn main() {
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let services = SystemServices::new();
    let ports = services.ports();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    if let Err(error) = diagnostics::configure(cli::diagnostic_settings(&arguments)) {
        writeln!(stderr, "whitaker: {error}").unwrap_or(());
    }
    let localizer = NoOpLocalizer::new();
    let outcome = cli::run_from(
        arguments,
        &ports,
        &localizer,
        &mut cli::CommandStreams::new(&mut stdout, &mut stderr),
    );
    let exit_code = cli::exit::exit_code_for(&outcome);
    if exit_code != 0 {
        process::exit(exit_code);
    }
}
