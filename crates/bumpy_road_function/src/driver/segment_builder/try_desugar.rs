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
//! charges the `?` itself, on the `?` token, the three weights that
//! `if cond { return … }` earns: one predicate branch, one nesting depth for
//! the early-return body, and one control-flow weight for the return. The
//! lowered arms add nothing further, and neither does the generated `return`.

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
        let branch = question_mark_span(span);
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
}

/// Return the span of the trailing `?` token within `value?`.
fn question_mark_span(span: Span) -> Span {
    let start = span.hi() - BytePos(QUESTION_MARK_WIDTH);
    span.with_lo(start.max(span.lo()))
}
