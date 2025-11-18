//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
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
