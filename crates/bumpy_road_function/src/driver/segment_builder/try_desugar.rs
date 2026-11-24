//! Charge a `?` operator as the one early-return branch it abbreviates.
//!
//! **Decision: `?` is a branch.** See ADR 0016
//! (`docs/adrs/adr-0016-question-mark-is-a-branch.md`). A `?` is charged as
//! the handwritten branch it abbreviates, an `if` whose body returns early:
//! not as zero, and not as its lowered two-arm `match`. Changing that requires
//! a superseding ADR approved by the maintainer.
//!
//! rustc lowers `value?` into
//!
//! ```text
//! match Try::branch(value) {
//!     ControlFlow::Continue(inner) => inner,
//!     ControlFlow::Break(residual) => return FromResidual::from_residual(residual),
//! }
//! ```
//!
//! Desugarings are transparent to the shared provenance classifier, so both
//! arms are authored as far as [`super::SegmentBuilder`] is concerned. The
//! default walk charged each arm a nesting depth and a control-flow weight, so
//! a single `?` counted as two nested branches and a flat guard chain was
//! over-charged. Charging nothing, the opposite error, hid the propagation
//! points that make a function hard to read.
//!
//! [`super::SegmentBuilder::visit_try`] therefore measures the scrutinee, then
//! charges the `?` itself, on the `?` token, exactly what the authored
//! `if cond { return … }` it abbreviates earns: one predicate branch and one
//! nesting depth for the early-return body (1.5 at the default weights). If
//! `SegmentBuilder`'s charge for that written form ever changes, `?` follows
//! it (ADR 0016). The lowered arms add nothing further, and neither does the
//! generated `return`.

use rustc_hir::{self as hir, MatchSource};
use rustc_span::{BytePos, Span};

use super::{SegmentBuilder, SegmentWeight};

/// Width in bytes of the `?` token that ends a propagation.
const QUESTION_MARK_WIDTH: u32 = 1;

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Charge a desugared `?` operator as one branch.
    ///
    /// `span` covers the whole `value?` expression. Returns `false`, recording
    /// nothing, when the expression does not have the lowered shape this
    /// module recognizes, so the caller falls back to the ordinary match walk
    /// rather than dropping authored structure.
    pub(super) fn visit_try(
        &mut self,
        span: Span,
        scrutinee: &'tcx hir::Expr<'tcx>,
        source: MatchSource,
    ) -> bool {
        if !matches!(source, MatchSource::TryDesugar(_)) {
            return false;
        }

        // The scrutinee is whatever the author applied `?` to; walking it
        // reaches that expression's own authored structure.
        self.visit_expr(scrutinee);

        // The branch sits on the `?` token, so a `?` ending a multi-line method
        // chain charges its own line rather than every line of the operand.
        // The arms are lowering the author never wrote and add nothing beyond
        // the two weights of the written-out early-return `if`.
        let branch = self.question_mark_span(span, scrutinee);
        self.push_segment(
            branch,
            SegmentWeight::Predicate {
                branches: 1,
                per_branch: self.settings.weights.predicate,
            },
        );
        self.push_depth_segment(branch);
        true
    }

    /// Return the span of the `?` token of a desugared `value?`.
    ///
    /// The expression's span can extend past the token: HIR drops parentheses
    /// but keeps their extent, so `(value?\n)` ends on the closing parenthesis,
    /// on a later line. The token is therefore located in the source text after
    /// the operand, which is the first argument of the lowered `Try::branch`
    /// call. When the operand or the snippet is unavailable, the last byte of
    /// `span` is the best remaining guess.
    fn question_mark_span(&self, span: Span, scrutinee: &hir::Expr<'_>) -> Span {
        let last_byte = span.with_lo(span.hi() - BytePos(QUESTION_MARK_WIDTH));
        let hir::ExprKind::Call(_, [operand]) = scrutinee.kind else {
            return last_byte;
        };
        let after_operand = span.with_lo(operand.span.hi());
        let source_map = self.cx.tcx.sess.source_map();
        let Ok(text) = source_map.span_to_snippet(after_operand) else {
            return last_byte;
        };
        let Some(offset) = find_question_mark(&text).and_then(|index| u32::try_from(index).ok())
        else {
            return last_byte;
        };
        let start = after_operand.lo() + BytePos(offset);
        span.with_lo(start)
            .with_hi(start + BytePos(QUESTION_MARK_WIDTH))
    }
}

/// Return the byte offset of the first `?` outside a comment in `text`.
///
/// `text` is the source between a `?` operand and the end of its expression:
/// whitespace, comments, closing parentheses and the operator. A `?` inside a
/// line or (nested) block comment is not the operator, so those are skipped.
/// String literals cannot occur there and are not handled.
fn find_question_mark(text: &str) -> Option<usize> {
    let mut cursor = Cursor::new(text);
    while let Some(rest) = cursor.rest() {
        if rest.starts_with('?') {
            return Some(cursor.offset);
        }
        if rest.starts_with("//") {
            cursor.skip_line_comment();
        } else if rest.starts_with("/*") {
            cursor.skip_block_comment();
        } else {
            cursor.skip_char();
        }
    }
    None
}

/// A byte position in the text between a `?` operand and its operator.
///
/// Keeping the text and position together leaves the scanning steps free of
/// text arguments: each reads `rest` and moves `offset`.
struct Cursor<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(text: &'a str) -> Self { Self { text, offset: 0 } }

    /// Return the unread text, or `None` at the end.
    fn rest(&self) -> Option<&'a str> {
        self.text.get(self.offset..).filter(|rest| !rest.is_empty())
    }

    /// Step over one character.
    fn skip_char(&mut self) {
        self.offset += self
            .rest()
            .and_then(|rest| rest.chars().next())
            .map_or(1, char::len_utf8);
    }

    /// Step over a line comment, including its newline when it has one.
    fn skip_line_comment(&mut self) {
        let width = self.rest().map_or(0, |rest| {
            rest.find('\n').map_or(rest.len(), |newline| newline + 1)
        });
        self.offset += width;
    }

    /// Step over a block comment. Block comments nest in Rust, and an
    /// unterminated comment runs to the end.
    fn skip_block_comment(&mut self) {
        let mut depth = 0_usize;
        while let Some(rest) = self.rest() {
            let (width, nesting_change) = block_comment_token(rest);
            depth = depth.saturating_add_signed(nesting_change);
            self.offset += width;
            if depth == 0 {
                return;
            }
        }
    }
}

/// Return the width of the token at the start of `tail` inside a block comment
/// and how it changes the nesting depth: `/*` opens, `*/` closes, and any other
/// character leaves the depth unchanged.
fn block_comment_token(tail: &str) -> (usize, isize) {
    if tail.starts_with("/*") {
        (2, 1)
    } else if tail.starts_with("*/") {
        (2, -1)
    } else {
        (tail.chars().next().map_or(1, char::len_utf8), 0)
    }
}

#[cfg(test)]
mod tests {
    //! Verify the `?` token search skips comments.

    use rstest::rstest;

    use super::find_question_mark;

    #[rstest]
    #[case::bare("?", Some(0))]
    #[case::after_parenthesis(")\n  ?", Some(4))]
    #[case::line_comment(" // really?\n ?", Some(13))]
    #[case::block_comment(" /* is it? */ ?", Some(14))]
    #[case::nested_block_comment(" /* a /* b? */ c? */ ?", Some(21))]
    #[case::multiline_block_comment(" /* why?\n*/ ?", Some(12))]
    #[case::unterminated_line_comment(" // what?", None)]
    #[case::unterminated_block_comment(" /* what?", None)]
    #[case::none(")", None)]
    fn finds_the_operator_outside_comments(#[case] text: &str, #[case] expected: Option<usize>) {
        assert_eq!(find_question_mark(text), expected);
    }
}
