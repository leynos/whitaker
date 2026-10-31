//! Process entry point for the root `whitaker` command.

use std::process;

use ortho_config::NoOpLocalizer;
use whitaker::{adapters::SystemServices, cli};

fn main() {
    let services = SystemServices::new();
    let ports = services.ports();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let localizer = NoOpLocalizer::new();
    let outcome = cli::run_from(
        std::env::args_os(),
        &ports,
        &localizer,
        &mut cli::CommandStreams::new(&mut stdout, &mut stderr),
    );
    let exit_code = cli::exit::exit_code_for(&outcome);
    if exit_code != 0 {
        process::exit(i32::from(exit_code));
    }
}
