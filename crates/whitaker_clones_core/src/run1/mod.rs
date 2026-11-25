//! AST-pass (SARIF Run 1) Type-3 scoring and emission.
//!
//! Run 1 refines the token pass's results with structural evidence. It
//! compares canonical AST hashes, scores cosine similarity over
//! canonical trigram-production multisets, and either carries a token-pass
//! result forward, reclassifies it as `WHK003`, or adds a `WHK003` result the
//! token pass did not report.
//!
//! The pass is positive-evidence only: AST evidence may add or strengthen a
//! result, never remove or weaken one. Every decision is integer-exact.

#[cfg(all(test, feature = "parser"))]
#[path = "calibration_tests.rs"]
mod calibration_tests;
mod classify;
mod config;
mod cosine;
mod emit;
mod error;
mod evidence;
mod kani;
mod metrics;
mod outcome;
mod physical;
mod refine;
mod tables;

pub use classify::{ThresholdKind, validate_decimal_threshold, validate_threshold_order};
pub use config::{AstPassConfig, RefinementInput};
pub use cosine::CosineMicros;
pub use emit::emit_run1;
pub use error::{Run1Error, Run1Result};
pub use outcome::{AstEvidence, RefinedPair};
pub use refine::refine_candidate_pairs;

/// Widen a `usize` to `u128` exactly.
///
/// `From<usize>` is not implemented for `u128`, so the cast is the only
/// lossless conversion available; `usize` is at most 64 bits wide.
pub(super) const fn widen(value: usize) -> u128 { value as u128 }
