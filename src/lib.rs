//! Core Whitaker library surfaces shared configuration and helpers for lint crates.
#![feature(rustc_private)]

pub mod config;
pub mod lints;
pub mod testing;

pub use config::{ModuleMaxLinesConfig, SharedConfig};
pub use lints::{LintCrateTemplate, TemplateError, TemplateFiles};

/// Returns a greeting for the library.
#[must_use]
pub const fn greet() -> &'static str {
    "Hello from Whitaker!"
}
