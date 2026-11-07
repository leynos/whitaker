//! Composition root for the legacy `whitaker-installer` command.
//!
//! The binary parses arguments, configures tracing, and hands the request to
//! the installer library, so the same orchestration is reachable from the root
//! `whitaker` binary.

use clap::Parser;
use whitaker_installer::{
    cli::{Cli, Command},
    orchestration::{exit_code_for_run_result, run, run_bootstrap_command},
    output::write_stderr_line,
};

mod tracing_config;

fn main() {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    if let Err(error) = tracing_config::configure_tracing(cli.install_args()) {
        write_stderr_line(&mut stderr, error);
        std::process::exit(1);
    }
    // `run` hands the child's exit status back unchanged, lint failures included.
    if let Some(Command::Run(args)) = &cli.command {
        std::process::exit(run_bootstrap_command(&args.args, &mut stderr));
    }
    let exit_code = exit_code_for_run_result(run(&cli, &mut stdout, &mut stderr), &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
