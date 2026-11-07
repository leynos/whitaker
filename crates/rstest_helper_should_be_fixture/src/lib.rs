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

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{RSTEST_HELPER_SHOULD_BE_FIXTURE, RstestHelperShouldBeFixture};
