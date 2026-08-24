//! Utilities for constructing new Whitaker lint crates.
//!
//! The `template` module provides helpers that assemble the boilerplate needed
//! by individual lint crates. These helpers generate `Cargo.toml` manifests
//! referencing shared workspace dependencies and emit source files that wire in
//! the shared UI test harness.

pub mod template;

pub use template::{LintCrateTemplate, TemplateError, TemplateFiles};
