//! Utilities for working with source locations and spans.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

use std::ops::RangeInclusive;

/// Errors produced when constructing spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanError {
    /// Indicates the start location occurs after the end location.
    StartAfterEnd,
}

/// Represents a location in source code using one-based line and column numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    line: usize,
    column: usize,
}

impl SourceLocation {
    /// Builds a new location.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::span::SourceLocation;
    ///
    /// let location = SourceLocation::new(3, 5);
    /// assert_eq!(location.line(), 3);
    /// ```
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Returns the one-based line number.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the one-based column number.
    #[must_use]
    pub const fn column(self) -> usize {
        self.column
    }

    /// Returns true when this location is positioned after other.
    #[must_use]
    pub const fn is_after(self, other: SourceLocation) -> bool {
        self.line > other.line || (self.line == other.line && self.column > other.column)
    }
}

/// Represents a span of source code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    start: SourceLocation,
    end: SourceLocation,
}

impl SourceSpan {
    /// Constructs a new span from two locations.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::span::{SourceLocation, SourceSpan};
    ///
    /// let span = SourceSpan::new(SourceLocation::new(1, 0), SourceLocation::new(3, 2)).expect("valid span for example");
    /// assert_eq!(span.start().line(), 1);
    /// ```
    #[must_use = "Inspect the span creation result to handle invalid ranges"]
    pub fn new(start: SourceLocation, end: SourceLocation) -> Result<Self, SpanError> {
        if start.is_after(end) {
            Err(SpanError::StartAfterEnd)
        } else {
            Ok(Self { start, end })
        }
    }

    /// Returns the start location.
    #[must_use]
    pub const fn start(self) -> SourceLocation {
        self.start
    }

    /// Returns the end location.
    #[must_use]
    pub const fn end(self) -> SourceLocation {
        self.end
    }
}

/// Converts a span into the inclusive range of line numbers it covers.
///
/// # Examples
///
/// ```
/// use common::span::{SourceLocation, SourceSpan, span_to_lines};
///
/// let span = SourceSpan::new(SourceLocation::new(4, 0), SourceLocation::new(6, 5)).expect("valid span for example");
/// assert_eq!(span_to_lines(span), 4..=6);
/// ```
#[must_use]
pub fn span_to_lines(span: SourceSpan) -> RangeInclusive<usize> {
    span.start.line()..=span.end.line()
}

/// Calculates the number of lines covered by the span (inclusive).
///
/// # Examples
///
/// ```
/// use common::span::{SourceLocation, SourceSpan, span_line_count};
///
/// let span = SourceSpan::new(SourceLocation::new(2, 0), SourceLocation::new(5, 1)).expect("valid span for example");
/// assert_eq!(span_line_count(span), 4);
/// ```
#[must_use]
pub fn span_line_count(span: SourceSpan) -> usize {
    span.end.line() - span.start.line() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn span_construction_validates_order() {
        let err = SourceSpan::new(SourceLocation::new(3, 1), SourceLocation::new(2, 0));
        assert!(matches!(err, Err(SpanError::StartAfterEnd)));
    }

    #[rstest]
    fn calculates_line_ranges() {
        let span = SourceSpan::new(SourceLocation::new(5, 0), SourceLocation::new(7, 3))
            .expect("valid span for line range test");
        assert_eq!(span_to_lines(span), 5..=7);
        assert_eq!(span_line_count(span), 3);
    }
}
