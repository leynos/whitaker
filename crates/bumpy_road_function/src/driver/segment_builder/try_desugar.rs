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
//! charges the `?` itself, on the `?` token, three weights: one predicate
//! branch, one nesting depth for the early-return body, and one control-flow
//! weight for the return that leaves the function. An authored
//! `if cond { return … }` earns the first two only, because `SegmentBuilder`
//! charges flow to match arms alone; the flow weight is the maintainer's ruling
//! for `?` (ADR 0016). The lowered arms add nothing further, and neither does
//! the generated `return`.

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
        // the three weights of the written-out early return.
        let branch = self.question_mark_span(span, scrutinee);
        self.push_segment(
            branch,
            SegmentWeight::Predicate {
                branches: 1,
                per_branch: self.settings.weights.predicate,
            },
        );
        self.push_depth_segment(branch);
        self.push_flow_segment(branch);
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
        let Some(offset) = text.find('?').and_then(|index| u32::try_from(index).ok()) else {
            return last_byte;
        };
        let start = after_operand.lo() + BytePos(offset);
        span.with_lo(start)
            .with_hi(start + BytePos(QUESTION_MARK_WIDTH))
    }
}
