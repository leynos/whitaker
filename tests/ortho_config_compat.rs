//! Compile-time compatibility check for future root CLI configuration.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "whitaker")]
#[ortho_config(prefix = "WHITAKER_")]
struct ConfigCompatibleCli {
    /// Enables verbose CLI output.
    #[arg(long)]
    verbose: bool,
}

#[test]
fn ortho_config_derives_alongside_clap_parser() {
    let parsed = ConfigCompatibleCli::try_parse_from(["whitaker", "--verbose"])
        .expect("the clap derive should parse the test argument");
    assert!(parsed.verbose, "the clap-derived flag should be retained");
}
