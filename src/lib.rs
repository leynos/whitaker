//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

pub mod config;
pub mod lints;
pub mod testing;

pub use config::{ModuleMax400LinesConfig, SharedConfig};
pub use lints::{LintCrateTemplate, TemplateError, TemplateFiles};

/// Returns a greeting for the library.
#[must_use]
pub fn greet() -> &'static str {
    "Hello from Whitaker!"
}
