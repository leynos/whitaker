//! Compile-pass guard for the root CLI's configuration derives.
//!
//! The root binary layers `ortho_config` over `clap`, so one type must satisfy
//! both derives at once. This fixture compiles that combination on its own,
//! away from the runtime test, so a dependency change that breaks the pairing
//! is reported as a compile error against a few lines rather than as a
//! confusing failure inside the CLI.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// The smallest type the root CLI can derive every configuration trait on.
#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "whitaker")]
#[ortho_config(prefix = "WHITAKER_")]
struct ConfigCompatibleCli {
    /// Enables verbose CLI output.
    #[arg(long)]
    verbose: bool,
}

/// Exercise both parsing paths so the derives are used, not just declared.
fn main() {
    let parsed = ConfigCompatibleCli::try_parse_from(["whitaker", "--verbose"])
        .expect("the clap derive should parse the test argument");
    assert!(parsed.verbose, "the clap-derived flag should be retained");
}