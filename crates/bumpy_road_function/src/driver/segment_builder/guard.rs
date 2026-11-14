//! Measure the authored structure inside a match guard.
//!
//! A guard is charged once, as a predicate over the logical spine that
//! [`super::count_branches`] walks. Everything below that spine — statement
//! lists, branch bodies, and arguments the spine stops at — is ordinary
//! authored structure, so [`SegmentBuilder::visit_match_guard`] visits it once,
//! whatever the origin of the enclosing `match`. Keeping one implementation for
//! authored and generated matches is what makes macro provenance transparent:
//! a guard measures the same whether it is written directly or supplied as a
//! macro argument.

use rustc_hir::{BinOpKind, Expr, ExprKind, UnOp, intravisit::walk_stmt};

use super::SegmentBuilder;

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Charge `guard` once, then visit the authored structure it contains.
    ///
    /// The charge covers the guard's own decision, so the traversal below never
    /// re-enters that spine; doing so would weight the same predicate twice.
    pub(super) fn visit_match_guard(&mut self, guard: &'tcx Expr<'tcx>) {
        self.push_predicate_segment(guard);
        self.visit_below_guard_spine(guard);
    }

    /// Visit the parts of a guard that its predicate charge does not reach.
    ///
    /// [`super::count_branches`] tallies only a guard's logical spine: the
    /// operands of `&&`/`||`, the inner expression of `!` and `DropTemps`, a
    /// block's tail, and an `if`'s condition. Branch bodies, statement lists,
    /// and the subexpressions the spine stops at stay unmeasured, so each is
    /// visited here as ordinary authored structure. An expression off the spine
    /// is handed to [`SegmentBuilder::visit_expr`], which records its own
    /// segment exactly once.
    fn visit_below_guard_spine(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.visit_below_guard_spine(lhs);
                self.visit_below_guard_spine(rhs);
            }
            ExprKind::Unary(UnOp::Not, inner) | ExprKind::DropTemps(inner) => {
                self.visit_below_guard_spine(inner);
            }
            ExprKind::Block(block, _) => {
                for stmt in block.stmts {
                    walk_stmt(self, stmt);
                }
                if let Some(tail) = block.expr {
                    self.visit_below_guard_spine(tail);
                }
            }
            ExprKind::If(cond, then_expr, else_expr) => {
                self.visit_below_guard_spine(cond);
                self.visit_expr_with_depth(then_expr);
                if let Some(other) = else_expr {
                    self.visit_expr_with_depth(other);
                }
            }
            _ => self.visit_expr(expr),
        }
    }
}
