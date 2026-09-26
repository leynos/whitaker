//! Compile contract for the Oxford-spelled public settings API.

// `make test` enables the Dylint driver for workspace dependencies.
#![feature(rustc_private)]

use bumpy_road_function::analysis::{Settings, normalize_settings};

fn main() {
    let settings = normalize_settings(Settings::default());
    assert!(settings.window > 0);
}
