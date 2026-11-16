//! Charge an authored `for` loop once, rather than its lowered scaffolding.
//!
//! rustc lowers `for value in iter { body }` into
//!
//! ```text
//! match IntoIterator::into_iter(iter) {
//!     mut iter => loop {
//!         match Iterator::next(&mut iter) {
//!             None => break,
//!             Some(value) => body,
//!         }
//!     },
//! }
//! ```
//!
//! Desugarings are transparent to the shared provenance classifier, so every
//! node above is authored and the default walk charged each one: the outer
//! match arm, the loop body, and the inner match arm each contributed a depth
//! segment, and the two match arms also contributed a flow segment. Those
//! segments all cover the same authored lines, so a flat `for` loop
//! accumulated roughly three times the weight of the equivalent `while` loop.
//!
//! [`super::SegmentBuilder::visit_for_loop`] instead charges the construct the
//! author wrote: the iterator expression, then one nesting level over the
//! authored body. That matches [`super::SegmentBuilder::handle_while_loop`],
//! which charges a `while` loop's condition and one level over its body.

use rustc_hir::{self as hir, ExprKind, LoopSource, MatchSource};

use super::SegmentBuilder;

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Charge a desugared `for` loop as the single authored construct.
    ///
    /// Returns `false`, recording nothing, when the expression does not have
    /// the lowered shape this module recognizes, so the caller falls back to
    /// the ordinary match walk rather than dropping authored structure.
    pub(super) fn visit_for_loop(
        &mut self,
        scrutinee: &'tcx hir::Expr<'tcx>,
        arms: &'tcx [hir::Arm<'tcx>],
    ) -> bool {
        let Some(body) = extract_for_loop_body(arms) else {
            return false;
        };

        // The scrutinee is the synthetic `into_iter` call; walking it reaches
        // the authored iterable without charging the call itself.
        self.visit_expr(scrutinee);
        self.visit_expr_with_depth(body);
        true
    }
}

/// Return the authored body of a lowered `for` loop.
///
/// The body is the arm of the inner `Iterator::next` match that runs the
/// author's block; its sibling arm breaks out of the loop when the iterator is
/// exhausted. That sibling, the bindings, and both matches are scaffolding the
/// author did not write, so they contribute no segments.
fn extract_for_loop_body<'hir>(arms: &'hir [hir::Arm<'hir>]) -> Option<&'hir hir::Expr<'hir>> {
    let loop_expr = arms.first()?.body;
    let ExprKind::Loop(block, _, LoopSource::ForLoop, _) = loop_expr.kind else {
        return None;
    };

    // The lowered `next()` match is the loop block's single statement; it is
    // a statement rather than the block's tail expression.
    let inner = block.stmts.iter().find_map(|stmt| match stmt.kind {
        hir::StmtKind::Expr(expr) | hir::StmtKind::Semi(expr) => Some(expr),
        _ => None,
    })?;
    let ExprKind::Match(_, inner_arms, MatchSource::ForLoopDesugar) = inner.kind else {
        return None;
    };

    // Select the authored arm by shape rather than by position: lowering emits
    // the exit arm and the body arm in a fixed order today, but the exit arm is
    // identifiable as the one that breaks, which keeps this robust if that
    // order changes.
    inner_arms
        .iter()
        .map(|arm| arm.body)
        .find(|body| !matches!(body.kind, ExprKind::Break(..)))
}
