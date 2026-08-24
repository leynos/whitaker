//! Composition root for the legacy `whitaker-installer` command.

use clap::Parser;
use whitaker_installer::cli::Cli;
use whitaker_installer::orchestration::{exit_code_for_run_result, run};
use whitaker_installer::output::write_stderr_line;

mod tracing_config;

fn main() {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    if let Err(error) = tracing_config::configure_tracing(cli.install_args()) {
        write_stderr_line(&mut stderr, error);
        std::process::exit(1);
    }
    let exit_code = exit_code_for_run_result(run(&cli, &mut stdout, &mut stderr), &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
