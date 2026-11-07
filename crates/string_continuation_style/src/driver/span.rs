//! Converts literal-body byte ranges into exact compiler spans.
//!
//! The domain crate measures offsets from the first byte after the opening
//! quote. rustc spans measure from the opening quote, so this module owns the
//! checked one-byte translation and rejects any range outside the token.

use std::ops::Range;

use rustc_span::{InnerSpan, Span};

/// Maps a body-relative byte range into the corresponding token subspan.
pub(crate) fn body_range_span(
    token_span: Span,
    body_len: usize,
    body_range: Range<usize>,
) -> Option<Span> {
    let token_len = usize::try_from(token_span.hi().0.checked_sub(token_span.lo().0)?).ok()?;
    if token_len != body_len.checked_add(2)? || body_range.start > body_range.end {
        return None;
    }

    let start = TokenOffset::from_body_index(body_range.start, body_len)?;
    let end = TokenOffset::from_body_index(body_range.end, body_len)?;
    Some(token_span.from_inner(InnerSpan::new(start.0, end.0)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenOffset(usize);

impl TokenOffset {
    /// Converts a body-relative byte index to a token offset.
    ///
    /// The body starts one byte after the opening quote, so the conversion
    /// adds one. An index past the end, or one that would overflow, is
    /// rejected rather than clamped: a clamped span would point at a real but
    /// wrong token, which is worse than declining the rewrite.
    fn from_body_index(index: usize, body_len: usize) -> Option<Self> {
        (index <= body_len)
            .then(|| index.checked_add(1).map(Self))
            .flatten()
    }
}

#[cfg(test)]
#[path = "../../tests/support/span_tests.rs"]
mod tests;
