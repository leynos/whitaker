//! Measure the authored structure beneath a charged predicate.
//!
//! A predicate — a match guard, an `if` condition, or a `while` condition — is
//! charged once, as a weighted segment over the logical spine that
//! [`super::count_branches`] walks. Everything below that spine — statement
//! lists, branch bodies, and arguments the spine stops at — is ordinary
//! authored structure, so [`SegmentBuilder::visit_predicate`] visits it once,
//! whatever the origin of the enclosing construct. Keeping one implementation
//! for authored and generated positions is what makes macro provenance
//! transparent: a predicate measures the same whether it is written directly
//! or supplied as a macro argument.

use rustc_hir::{BinOpKind, Expr, ExprKind, UnOp, intravisit::walk_stmt};

use super::{SegmentBuilder, is_generated};

impl<'tcx> SegmentBuilder<'_, 'tcx> {
    /// Charge `predicate` once, then visit the authored structure it contains.
    ///
    /// The charge covers the predicate's own decision, so
    /// [`Self::visit_below_predicate_spine`] never re-enters that spine; doing
    /// so would weight the same predicate twice. A predicate whose span yields
    /// no segment, such as one the enclosing macro wrote, has charged nothing,
    /// so it is visited as an ordinary expression instead and the authored
    /// parts inside it are still measured exactly once.
    pub(super) fn visit_predicate(&mut self, predicate: &'tcx Expr<'tcx>) {
        if self.push_predicate_segment(predicate) {
            self.visit_below_predicate_spine(predicate);
        } else {
            self.visit_expr(predicate);
        }
    }

    /// Visit the parts of a predicate that its charge does not reach.
    ///
    /// [`super::count_branches`] tallies only a predicate's logical spine: the
    /// operands of `&&`/`||`, the inner expression of `!` and `DropTemps`, a
    /// block's tail, and an `if`'s condition. Branch bodies, statement lists,
    /// and the subexpressions the spine stops at stay unmeasured, so each is
    /// visited here as ordinary authored structure. An expression off the spine
    /// is handed to [`SegmentBuilder::visit_expr`], which records its own
    /// segment exactly once.
    fn visit_below_predicate_spine(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
                self.visit_below_predicate_spine(lhs);
                self.visit_below_predicate_spine(rhs);
            }
            ExprKind::Unary(UnOp::Not, inner) | ExprKind::DropTemps(inner) => {
                self.visit_below_predicate_spine(inner);
            }
            ExprKind::Block(block, _) => {
                for stmt in block.stmts {
                    walk_stmt(self, stmt);
                }
                if let Some(tail) = block.expr {
                    self.visit_below_predicate_spine(tail);
                }
            }
            ExprKind::If(cond, then_expr, else_expr) => {
                self.visit_below_predicate_spine(cond);
                // The charge counted this `if`'s condition as one branch, so
                // only its branch bodies are left to measure. Nesting depth
                // belongs to the `if` itself: a macro-written `if` contributes
                // none of its own, and its authored branch bodies are reached
                // here without inheriting depth from the expansion.
                let should_charge_depth = !is_generated(expr.span);
                self.visit_branch(then_expr, should_charge_depth);
                if let Some(other) = else_expr {
                    self.visit_branch(other, should_charge_depth);
                }
            }
            _ => self.visit_expr(expr),
        }
    }

    /// Visit one branch body of a predicate's `if`, charging its nesting depth
    /// only when the `if` is authored.
    fn visit_branch(&mut self, branch: &'tcx Expr<'tcx>, should_charge_depth: bool) {
        if should_charge_depth {
            self.push_depth_segment(branch.span);
        }
        self.visit_expr(branch);
    }
}
