//! Composition root for the legacy `whitaker-installer` command.

use clap::Parser;
use whitaker_installer::{
    cli::Cli,
    orchestration::{exit_code_for_run_result, run},
};

fn main() {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let exit_code = exit_code_for_run_result(run(&cli, &mut stdout, &mut stderr), &mut stderr);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}
