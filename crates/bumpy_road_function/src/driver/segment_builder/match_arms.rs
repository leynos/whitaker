//! Score a `match` expression, recognizing lowerings that are not authored.

use rustc_hir::{self as hir, MatchSource};

use super::SegmentBuilder;

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Visit a `match`, treating a `?` desugaring as a try rather than a match.
    pub(super) fn visit_try_or_match(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        let hir::ExprKind::Match(scrutinee, arms, source) = expr.kind else {
            return;
        };
        // `?` and `for` are distinct lowerings, so the order of these
        // two checks does not matter.
        if !self.visit_try(expr.span, scrutinee, source) {
            self.visit_match(scrutinee, arms, source);
        }
    }

    /// Score a `match`, recognizing the lowerings that are not authored `match`.
    ///
    /// rustc lowers both a `for` loop and a `?` operator into a `match`.
    /// Desugarings are transparent to the provenance classifier, so without
    /// this dispatch the generic arm walk would charge their scaffolding as
    /// authored branching. Each recognized lowering returns `true` once it has
    /// recorded its own segments; anything else is an authored `match`, scored
    /// arm by arm. `visit_expr` offers the `?` lowering to `visit_try` first,
    /// because that charge needs the whole expression's span.
    pub(super) fn visit_match(
        &mut self,
        scrutinee: &'tcx hir::Expr<'tcx>,
        arms: &'tcx [hir::Arm<'tcx>],
        source: MatchSource,
    ) {
        if source == MatchSource::ForLoopDesugar && self.visit_for_loop(scrutinee, arms) {
            return;
        }
        self.visit_expr(scrutinee);
        self.visit_match_arms(arms);
    }

    fn visit_match_arms(&mut self, arms: &'tcx [hir::Arm<'tcx>]) {
        for arm in arms {
            self.visit_match_arm(arm);
        }
    }

    fn visit_match_arm(&mut self, arm: &'tcx hir::Arm<'tcx>) {
        if let Some(guard) = arm.guard {
            self.visit_predicate(guard);
        }

        self.push_depth_segment(arm.body.span);
        self.push_flow_segment(arm.body.span);
        self.visit_expr(arm.body);
    }
}
