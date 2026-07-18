//! Dylint crate for collecting `rstest` helper-call evidence.
//!
//! The driver recognizes strict `#[rstest]` bodies and delegates HIR traversal
//! to the visitor, which lowers helper-call arguments into the collector's
//! deterministic per-callee records. Collection is diagnostic-silent: later
//! roadmap items evaluate thresholds and emit user-facing guidance.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod collector;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod visitor;

#[cfg(feature = "dylint-driver")]
pub use driver::*;
