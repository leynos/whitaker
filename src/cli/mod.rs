//! Localized command parsing, routing, and process-facing command execution.

pub mod dispatch;
pub mod exit;

mod parser;
mod runner;

pub use parser::{Cli, Command};
pub use runner::{CommandStreams, run_from};
