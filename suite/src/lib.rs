//! Aggregated Whitaker Dylint suite.
//!
//! This crate bundles the individual Whitaker lint crates into a single
//! cdylib. Consumers can load `whitaker_suite` to enable every shipped lint without
//! configuring each crate separately. The exported helpers mirror the
//! dylint entrypoint so the library can register itself through
//! `register_lints` while also exposing a pure-Rust view of the wiring for
//! tests and documentation.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

mod lints;

pub use lints::{LintDescriptor, SUITE_LINTS, suite_lint_names};

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::{register_suite_lints, suite_lint_decls};
