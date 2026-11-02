//! Serve prepared workspace metadata and route Cargo's Nextest call directly.
//!
//! The behaviour lives in [`dylint_testing::cargo_test_driver`] so it can be
//! tested as a library without spawning this binary.

use std::{
    io::{self, Write as _},
    process,
};

/// Run the driver and exit with the status its child reported.
fn main() {
    match dylint_testing::cargo_test_driver::run_from_process() {
        Ok(code) => process::exit(code),
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            process::exit(1);
        }
    }
}
