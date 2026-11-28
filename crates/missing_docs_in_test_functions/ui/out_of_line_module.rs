//! An out-of-line module file gets its own pre-expansion run.
//!
//! Rustc runs a pre-expansion pass once for the crate root and once more for
//! each out-of-line module as expansion loads it. The module-file run walks only
//! that module's items, and the lint-level stack it builds starts from the
//! command line plus the module's own inner attributes. So the crate-root
//! `#![allow(missing_docs_in_test_functions)]` below does not reach the module
//! file, and the diagnostic appears in the module rather than here. The
//! crate-root control function is silent precisely because the allow does reach
//! the root run.

#![allow(missing_docs_in_test_functions)]

#[path = "auxiliary/out_of_line_helpers.rs.inc"]
mod helpers;

fn crate_root_control() { helpers::module_level_function(); }

fn main() {}