//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

// Guard against accidentally linking rustc_driver when compiling tests with the
// dylint-driver feature enabled.
#[cfg(all(feature = "dylint-driver", test))]
compile_error!("rustc_driver must remain excluded from unit test builds");

// Link against `rustc_driver` only when consumers need the dylint driver runtime.
// Unit tests of this crate should not pull the compiler driver to avoid the
// duplicated `std`/`core` link errors seen during all-features test runs.
#[cfg(all(feature = "dylint-driver", not(test)))]
extern crate rustc_driver;

pub mod config;
#[cfg(feature = "dylint-driver")]
pub mod hir;
pub mod lints;
pub mod testing;

pub use config::{ModuleMaxLinesConfig, SharedConfig};
#[cfg(feature = "dylint-driver")]
pub use hir::{module_body_span, module_header_span};
pub use lints::{LintCrateTemplate, TemplateError, TemplateFiles};

/// Returns a greeting for the library.
#[must_use]
pub const fn greet() -> &'static str {
    "Hello from Whitaker!"
}
