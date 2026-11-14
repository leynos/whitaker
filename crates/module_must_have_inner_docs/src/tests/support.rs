//! Shared helpers for the lint's unit and behavioural suites.
//!
//! Both suites build synthetic attribute records at explicit byte offsets so
//! ordering decisions stay unambiguous. Keeping the helper here states the
//! span construction rule once instead of in each suite.

use rustc_span::{BytePos, DUMMY_SP, Span};

/// Builds a span at a distinct byte offset so record order is unambiguous.
///
/// `Span::new` swaps its bounds when `lo > hi`, so the high bound is set first;
/// setting the low bound on a dummy span would otherwise collapse back to zero.
pub(super) fn span_at(lo: u32) -> Span { DUMMY_SP.with_hi(BytePos(lo + 1)).with_lo(BytePos(lo)) }
