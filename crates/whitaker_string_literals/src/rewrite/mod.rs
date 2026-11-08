//! Builds and validates source replacements for classified continuations.

use crate::{
    continuation::{Continuation, joins_only},
    facts::{BodyOffset, LiteralBody},
};

pub mod gate;
#[cfg(test)]
mod props;
mod source_gate;
#[cfg(test)]
mod tests;
mod validate;

use source_gate::{is_selected_join, is_single_cooked_string_token};
use validate::is_concat_macro_expression;

/// Recovers the body of a snippet that is exactly one cooked string literal.
///
/// The adapter uses this to turn a source span back into the bytes the scanner
/// and the rewriter operate on. Returning `None` for anything that is not one
/// complete, suffix-free cooked literal is what keeps a stale or partial span
/// from being classified as if it were the literal the caller expected.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::rewrite::cooked_literal_body;
///
/// assert_eq!(
///     cooked_literal_body("\"a\\\nb\"").map(|body| body.as_str()),
///     Some("a\\\nb")
/// );
/// assert!(cooked_literal_body("r\"a\"").is_none());
/// ```
#[must_use]
pub fn cooked_literal_body(snippet: &str) -> Option<LiteralBody<'_>> {
    if !is_single_cooked_string_token(snippet) {
        return None;
    }
    let body = snippet.get(1..snippet.len().checked_sub(1)?)?;
    Some(LiteralBody::new(body))
}

/// A source-preserving `concat!` rewrite that passed the domain checks.
///
/// The fragments borrow from the body they came from. The single constructor
/// keeps each replacement tied to that source and records the offsets of all
/// selected joins.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::{facts::LiteralBody, rewrite::ConcatRewrite};
///
/// let body = LiteralBody::new("alpha \\\nbeta");
/// let joins = body.scan();
/// let rewrite = ConcatRewrite::build(body, &joins).expect("the join is safe");
/// assert_eq!(rewrite.fragments(), &["alpha ", "beta"]);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConcatRewrite<'a> {
    fragments: Vec<&'a str>,
    join_offsets: Vec<BodyOffset>,
}

impl<'a> ConcatRewrite<'a> {
    /// Builds a replacement when the body and every selected split are valid.
    ///
    /// The selected continuations must equal every scanner result classified
    /// as an interior join. Their escape and consumed-whitespace ranges are
    /// removed; all remaining source spelling is copied into string fragments.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::{facts::LiteralBody, rewrite::ConcatRewrite};
    ///
    /// let body = LiteralBody::new("red\\\n blue");
    /// let continuations = body.scan();
    /// let rewrite = ConcatRewrite::build(body, &continuations).expect("valid source split");
    /// assert_eq!(rewrite.fragments(), &["red", "blue"]);
    /// ```
    #[must_use]
    pub fn build(body: LiteralBody<'a>, joins: &[Continuation]) -> Option<Self> {
        let source = body.as_str();
        let original = format!("\"{source}\"");
        if !is_single_cooked_string_token(&original) {
            return None;
        }

        let scanned_joins = joins_only(body.scan());
        if joins.is_empty() || joins != scanned_joins.as_slice() {
            return None;
        }

        let mut fragments = Vec::with_capacity(joins.len().saturating_add(1));
        let mut join_offsets = Vec::with_capacity(joins.len());
        let mut fragment_start = 0;
        let mut previous_end = 0;

        for join in joins {
            if !is_selected_join(source, join, &scanned_joins, previous_end) {
                return None;
            }

            let split = join.escape_range.start.get();
            let next_start = join.skipped_whitespace_range.end.get();
            fragments.push(source.get(fragment_start..split)?);
            join_offsets.push(join.escape_range.start);
            fragment_start = next_start;
            previous_end = next_start;
        }
        fragments.push(source.get(fragment_start..source.len())?);

        let rewrite = Self {
            fragments,
            join_offsets,
        };
        is_concat_macro_expression(&rewrite.render(0, Newline::Lf)).then_some(rewrite)
    }

    /// Returns the copied string-literal bodies in replacement order.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::{facts::LiteralBody, rewrite::ConcatRewrite};
    ///
    /// let body = LiteralBody::new("left\\\nright");
    /// let joins = body.scan();
    /// let rewrite = ConcatRewrite::build(body, &joins).expect("safe join");
    /// assert_eq!(rewrite.fragments(), &["left", "right"]);
    /// ```
    #[must_use]
    pub fn fragments(&self) -> &[&'a str] { &self.fragments }

    /// Returns the body-relative offsets of the selected unpaired backslashes.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::{facts::LiteralBody, rewrite::ConcatRewrite};
    ///
    /// let body = LiteralBody::new("a\\\nb");
    /// let joins = body.scan();
    /// let rewrite = ConcatRewrite::build(body, &joins).expect("safe join");
    /// assert_eq!(rewrite.join_offsets()[0].get(), 1);
    /// ```
    #[must_use]
    pub fn join_offsets(&self) -> &[BodyOffset] { &self.join_offsets }

    /// Renders the replacement at the original literal's indentation.
    ///
    /// A single-line form is used when it fits within rustfmt's default width
    /// of 100 columns. Otherwise the expression is expanded with four spaces
    /// of continuation indentation and the requested line ending.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::{
    ///     facts::LiteralBody,
    ///     rewrite::{ConcatRewrite, Newline},
    /// };
    ///
    /// let body = LiteralBody::new("left\\\nright");
    /// let joins = body.scan();
    /// let rewrite = ConcatRewrite::build(body, &joins).expect("safe join");
    /// assert_eq!(
    ///     rewrite.render(0, Newline::Lf),
    ///     "concat!(\"left\", \"right\")"
    /// );
    /// ```
    #[must_use]
    pub fn render(&self, indent: usize, newline: Newline) -> String {
        let one_line = self.render_single_line();
        if indent.saturating_add(one_line.len()) <= FORMATTER_WIDTH {
            return one_line;
        }

        self.render_multi_line(indent, newline)
    }

    /// Renders the whole rewrite as one `concat!(...)` line.
    fn render_single_line(&self) -> String {
        let mut replacement = String::from("concat!(");
        for (index, fragment) in self.fragments.iter().enumerate() {
            if index > 0 {
                replacement.push_str(", ");
            }
            push_quoted_fragment(&mut replacement, fragment);
        }
        replacement.push(')');
        replacement
    }

    /// Renders one fragment per line at the original indentation plus four.
    fn render_multi_line(&self, indent: usize, newline: Newline) -> String {
        let line_ending = newline.as_str();
        let inner_indent = indent.saturating_add(CONTINUATION_INDENT);
        let mut replacement = String::from("concat!(");
        replacement.push_str(line_ending);

        for fragment in &self.fragments {
            push_spaces(&mut replacement, inner_indent);
            push_quoted_fragment(&mut replacement, fragment);
            replacement.push(',');
            replacement.push_str(line_ending);
        }

        push_spaces(&mut replacement, indent);
        replacement.push(')');
        replacement
    }
}

/// A line ending to use inside a multi-line replacement.
///
/// # Examples
///
/// ```
/// use whitaker_string_literals::rewrite::Newline;
///
/// assert_eq!(Newline::Lf.as_str(), "\n");
/// assert_eq!(Newline::CrLf.as_str(), "\r\n");
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Newline {
    /// Line feed.
    Lf,
    /// Carriage return followed by line feed.
    CrLf,
}

impl Newline {
    /// Returns the source bytes for this line ending.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_string_literals::rewrite::Newline;
    ///
    /// assert_eq!(Newline::Lf.as_str(), "\n");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

const FORMATTER_WIDTH: usize = 100;
const CONTINUATION_INDENT: usize = 4;

/// Appends `fragment` wrapped in one pair of double quotes.
fn push_quoted_fragment(replacement: &mut String, fragment: &str) {
    replacement.push('"');
    replacement.push_str(fragment);
    replacement.push('"');
}

/// Appends `count` spaces to `replacement`.
fn push_spaces(replacement: &mut String, count: usize) {
    for _ in 0..count {
        replacement.push(' ');
    }
}
