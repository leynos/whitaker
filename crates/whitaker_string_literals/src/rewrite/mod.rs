//! Builds and validates source replacements for classified continuations.

use rustc_lexer::{LiteralKind, Token, TokenKind, tokenize};

use crate::{
    continuation::{Continuation, ContinuationKind},
    facts::{BodyOffset, LiteralBody},
};

#[cfg(test)]
mod props;
#[cfg(test)]
mod tests;

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

        let scanned_joins: Vec<_> = body
            .scan()
            .into_iter()
            .filter(|continuation| continuation.kind == ContinuationKind::Join)
            .collect();
        if joins.is_empty() || joins != scanned_joins.as_slice() {
            return None;
        }

        let mut fragments = Vec::with_capacity(joins.len().saturating_add(1));
        let mut join_offsets = Vec::with_capacity(joins.len());
        let mut fragment_start = 0;
        let mut previous_end = 0;

        for join in joins {
            if join.kind != ContinuationKind::Join {
                return None;
            }
            if !scanned_joins.contains(join) {
                return None;
            }
            if join.escape_range.start.get() < previous_end {
                return None;
            }
            if !is_valid_join_range(source, join) {
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

fn is_single_cooked_string_token(source: &str) -> bool {
    let mut tokens = tokenize(source);
    let Some(token) = tokens.next() else {
        return false;
    };
    let TokenKind::Literal {
        kind: LiteralKind::Str { terminated: true },
        suffix_start,
    } = token.kind
    else {
        return false;
    };

    token.len == source.len() && suffix_start == token.len && tokens.next().is_none()
}

fn is_valid_join_range(source: &str, join: &Continuation) -> bool {
    let escape_start = join.escape_range.start.get();
    let escape_end = join.escape_range.end.get();
    let skipped_start = join.skipped_whitespace_range.start.get();
    let skipped_end = join.skipped_whitespace_range.end.get();
    let Some(escape) = source.as_bytes().get(escape_start..escape_end) else {
        return false;
    };
    let Some(skipped) = source.as_bytes().get(skipped_start..skipped_end) else {
        return false;
    };

    escape == b"\\\n"
        && escape_end == skipped_start
        && skipped
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
}

fn is_concat_macro_expression(source: &str) -> bool {
    let mut tokens = tokenize(source).filter(|token| token.kind != TokenKind::Whitespace);
    if !has_concat_macro_head(source, &mut tokens) {
        return false;
    }
    if !has_concat_macro_arguments(&mut tokens) {
        return false;
    }

    tokens.next().is_none()
}

fn has_concat_macro_head(source: &str, tokens: &mut impl Iterator<Item = Token>) -> bool {
    let Some(ident) = tokens.next() else {
        return false;
    };
    let Some(ident_source) = source.get(..ident.len) else {
        return false;
    };
    if ident.kind != TokenKind::Ident || ident_source != "concat" {
        return false;
    }
    if !matches!(tokens.next().map(|token| token.kind), Some(TokenKind::Not))
        || !matches!(
            tokens.next().map(|token| token.kind),
            Some(TokenKind::OpenParen)
        )
    {
        return false;
    }

    true
}

fn has_concat_macro_arguments(tokens: &mut impl Iterator<Item = Token>) -> bool {
    let mut literal_count = 0_usize;
    let mut expects_literal = true;
    let mut can_close = false;
    loop {
        let Some(token) = tokens.next() else {
            return false;
        };

        match token.kind {
            TokenKind::Literal {
                kind: LiteralKind::Str { terminated: true },
                suffix_start,
            } if expects_literal && suffix_start == token.len => {
                literal_count = literal_count.saturating_add(1);
                expects_literal = false;
                can_close = true;
            }
            TokenKind::Comma if !expects_literal => expects_literal = true,
            TokenKind::CloseParen if can_close => break,
            _ => return false,
        }
    }

    literal_count >= 2
}

fn push_quoted_fragment(replacement: &mut String, fragment: &str) {
    replacement.push('"');
    replacement.push_str(fragment);
    replacement.push('"');
}

fn push_spaces(replacement: &mut String, count: usize) {
    for _ in 0..count {
        replacement.push(' ');
    }
}
