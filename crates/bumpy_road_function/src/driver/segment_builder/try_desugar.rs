//! Charge a `?` operator once, rather than its lowered `match` scaffolding.
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
//! a flat guard chain — six `?` in a row, a plain statement, six more — was
//! reported as "multiple clusters of nested conditional logic" although it
//! contains no nesting at all.
//!
//! The author wrote a propagation, not a branch: `?` hands the residual to the
//! enclosing function and yields the success value in place. There is no
//! authored body inside either arm, so
//! [`super::SegmentBuilder::visit_try`] measures the scrutinee and charges
//! nothing for the match. That is what the pre-provenance detector did, by
//! skipping the whole desugared match through `Span::from_expansion`; walking
//! the scrutinee keeps the authored structure it used to miss.

use rustc_hir::{self as hir, MatchSource};

use super::SegmentBuilder;

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Charge a desugared `?` operator as the single authored construct.
    ///
    /// Returns `false`, recording nothing, when the expression does not have
    /// the lowered shape this module recognizes, so the caller falls back to
    /// the ordinary match walk rather than dropping authored structure.
    pub(super) fn visit_try(
        &mut self,
        scrutinee: &'tcx hir::Expr<'tcx>,
        source: MatchSource,
    ) -> bool {
        if !matches!(source, MatchSource::TryDesugar(_)) {
            return false;
        }

        // The scrutinee is whatever the author applied `?` to; walking it
        // reaches that expression's own authored structure. The arms are
        // lowering — a binding and a `return` the author never wrote — so they
        // contribute neither nesting nor flow.
        self.visit_expr(scrutinee);
        true
    }
}
