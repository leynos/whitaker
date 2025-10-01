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
    /// let span = SourceSpan::new(SourceLocation::new(1, 0), SourceLocation::new(3, 2)).unwrap();
    /// assert_eq!(span.start().line(), 1);
    /// ```
    #[must_use]
    pub fn new(start: SourceLocation, end: SourceLocation) -> Result<Self, SpanError> {
        if start.line > end.line || (start.line == end.line && start.column > end.column) {
            return Err(SpanError::StartAfterEnd);
        }
        Ok(Self { start, end })
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
/// let span = SourceSpan::new(SourceLocation::new(4, 0), SourceLocation::new(6, 5)).unwrap();
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
/// use common::span::{SourceLocation, SourceSpan, module_line_count};
///
/// let span = SourceSpan::new(SourceLocation::new(2, 0), SourceLocation::new(5, 1)).unwrap();
/// assert_eq!(module_line_count(span), 4);
/// ```
#[must_use]
pub fn module_line_count(span: SourceSpan) -> usize {
    span.end.line() - span.start.line() + 1
}

/// A simplified representation of a Rust path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimplePath {
    segments: Vec<String>,
}

impl SimplePath {
    /// Builds a path from iterator segments.
    #[must_use]
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    /// Parses a path from the conventional `::`-separated string form.
    #[must_use]
    pub fn from(path: &str) -> Self {
        Self::new(path.split("::"))
    }

    /// Returns the path segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns `true` when the path matches the provided segments exactly.
    #[must_use]
    pub fn matches<'a, I>(&self, candidate: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.segments
            .iter()
            .map(String::as_str)
            .eq(candidate.into_iter())
    }
}

/// A tiny expression model used for helper functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    /// A call expression with a resolved callee path.
    Call { callee: SimplePath },
    /// A path expression.
    Path(SimplePath),
    /// Any other literal expression (placeholder for expansion).
    Literal(String),
}

/// Returns the callee path of a call expression, if one is present.
///
/// # Examples
///
/// ```
/// use common::span::{def_id_of_expr_callee, Expr, SimplePath};
///
/// let expr = Expr::Call { callee: SimplePath::from("std::mem::drop") };
/// assert_eq!(def_id_of_expr_callee(&expr).unwrap().segments(), &["std", "mem", "drop"]);
/// ```
#[must_use]
pub fn def_id_of_expr_callee(expr: &Expr) -> Option<&SimplePath> {
    match expr {
        Expr::Call { callee } => Some(callee),
        _ => None,
    }
}

/// Tests whether a path matches the provided candidate segments.
///
/// # Examples
///
/// ```
/// use common::span::{is_path_to, SimplePath};
///
/// let path = SimplePath::from("core::option::Option");
/// assert!(is_path_to(&path, ["core", "option", "Option"]));
/// ```
#[must_use]
pub fn is_path_to<'a, I>(path: &SimplePath, candidate: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    path.matches(candidate)
}

/// Returns `true` when the receiver is `Option` or `Result` regardless of
/// module path.
///
/// # Examples
///
/// ```
/// use common::span::{recv_is_option_or_result, SimplePath};
///
/// assert!(recv_is_option_or_result(&SimplePath::from("std::option::Option")));
/// assert!(recv_is_option_or_result(&SimplePath::from("Result")));
/// assert!(!recv_is_option_or_result(&SimplePath::from("crate::Thing")));
/// ```
#[must_use]
pub fn recv_is_option_or_result(path: &SimplePath) -> bool {
    matches!(
        path.segments().last().map(String::as_str),
        Some("Option" | "Result")
    )
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
        let span = SourceSpan::new(SourceLocation::new(5, 0), SourceLocation::new(7, 3)).unwrap();
        assert_eq!(span_to_lines(span), 5..=7);
        assert_eq!(module_line_count(span), 3);
    }

    #[rstest]
    fn callee_extraction() {
        let expr = Expr::Call {
            callee: SimplePath::from("std::mem::drop"),
        };
        assert!(def_id_of_expr_callee(&expr).is_some());
    }

    #[rstest]
    fn recognises_option_like_receivers() {
        let option_path = SimplePath::from("std::option::Option");
        let result_path = SimplePath::from("Result");
        let custom_path = SimplePath::from("crate::Thing");

        assert!(recv_is_option_or_result(&option_path));
        assert!(recv_is_option_or_result(&result_path));
        assert!(!recv_is_option_or_result(&custom_path));
    }

    #[rstest]
    fn path_comparison() {
        let path = SimplePath::from("crate::module::Item");
        assert!(is_path_to(&path, ["crate", "module", "Item"]));
        assert!(!is_path_to(&path, ["crate", "module", "Other"]));
    }
}
