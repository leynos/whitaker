//! Body selection and event collection for the complexity policy.
//!
//! Selection mirrors pinned Clippy: an `async fn` is unwrapped out of its
//! coroutine closure before scoring, and every other function scores its own
//! body value. Collection records one event per visited expression, with
//! match-arm guards charged independently of their match, and deliberately
//! descends into generated nodes so that return adjacency is unchanged.

use rustc_hir::{
    Arm,
    Block,
    Body,
    Closure,
    ClosureKind,
    CoroutineDesugaring,
    CoroutineKind,
    Expr,
    ExprKind,
    intravisit::{self, Visitor},
};
use rustc_lint::LateContext;
use rustc_span::sym;
use whitaker_lint_core::hir::node_origin::node_origin;

use crate::policy::{Event, Observed};

/// Select the expression that roots scoring for `body`.
///
/// An `async fn` hides its authored block inside a compiler-generated
/// coroutine closure. Return `None` when that shape is not found, matching
/// pinned Clippy's refusal to score an unexpected lowering.
pub(super) fn body_root<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    is_async: bool,
) -> Option<&'tcx Expr<'tcx>> {
    if is_async {
        async_fn_body(cx, body)
    } else {
        Some(body.value)
    }
}

/// Report whether the selected body's expression type is `core::result::Result`.
///
/// The diagnostic item is checked instead of a textual type name so aliases
/// and re-exports still classify correctly.
pub(super) fn body_returns_result(cx: &LateContext<'_>, root: &Expr<'_>) -> bool {
    let Some(body_type) = cx.typeck_results().node_type_opt(root.hir_id) else {
        return false;
    };

    body_type
        .ty_adt_def()
        .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Result, adt.did()))
}

/// Reduce one body root to the ordered event stream the policy scores.
pub(super) fn collect_events<'tcx>(root: &'tcx Expr<'tcx>) -> Vec<Event> {
    let mut visitor = ComplexityVisitor::default();
    visitor.visit_expr(root);
    visitor.events
}

/// Unwrap the authored block of an `async fn` body.
///
/// This mirrors pinned Clippy's `get_async_fn_body`; the shape must be
/// rechecked whenever the rustc pin changes.
fn async_fn_body<'tcx>(cx: &LateContext<'tcx>, body: &'tcx Body<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    if let ExprKind::Closure(&Closure {
        body: coroutine_body,
        kind: ClosureKind::Coroutine(CoroutineKind::Desugared(CoroutineDesugaring::Async, _)),
        ..
    }) = body.value.kind
        && let ExprKind::Block(
            Block {
                expr:
                    Some(Expr {
                        kind: ExprKind::DropTemps(inner),
                        ..
                    }),
                ..
            },
            _,
        ) = cx.tcx.hir_body(coroutine_body).value.kind
    {
        Some(inner)
    } else {
        None
    }
}

/// Collect one charge event per visited expression.
#[derive(Default)]
struct ComplexityVisitor {
    events: Vec<Event>,
}

impl ComplexityVisitor {
    /// Record one visited expression with its provenance.
    fn record(&mut self, observed: Observed, span: rustc_span::Span) {
        self.events.push(Event::new(observed, node_origin(span)));
    }

    /// Record a match and each of its arm guards.
    ///
    /// A match is one branch only when it has more than one arm; a guard is
    /// always charged separately because its span is independently authored.
    fn record_match(&mut self, expr: &Expr<'_>, arms: &[Arm<'_>]) {
        let observed = if arms.len() > 1 {
            Observed::Branch
        } else {
            Observed::Neutral
        };
        self.record(observed, expr.span);

        for guard in arms.iter().filter_map(|arm| arm.guard) {
            self.record(Observed::Guard, guard.span);
        }
    }
}

impl<'v> Visitor<'v> for ComplexityVisitor {
    fn visit_expr(&mut self, expr: &'v Expr<'v>) {
        match expr.kind {
            ExprKind::If(..) => self.record(Observed::Branch, expr.span),
            ExprKind::Match(_, arms, _) => self.record_match(expr, arms),
            ExprKind::Ret(..) => self.record(Observed::Return, expr.span),
            _ => self.record(Observed::Neutral, expr.span),
        }

        intravisit::walk_expr(self, expr);
    }
}
