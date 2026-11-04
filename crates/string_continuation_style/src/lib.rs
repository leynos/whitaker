//! Dylint adapter for the string-literal continuation style lint.
//!
//! The adapter inspects rustc's expanded syntax tree and delegates byte-level
//! analysis to `whitaker_string_literals`.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]
#![deny(clippy::arithmetic_side_effects)]

#[cfg(feature = "dylint-driver")]
mod config;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod exclusion;

#[cfg(feature = "dylint-driver")]
pub use driver::{STRING_CONTINUATION_STYLE, StringContinuationStyle};

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "../tests/support/tests.rs"]
mod tests;
