//! Body selection and event collection for the complexity policy.
//!
//! Selection mirrors pinned Clippy: an `async fn` is unwrapped out of its
//! coroutine closure before scoring, and every other function scores its own
//! body value. Collection records one event per visited expression in source
//! order, with match-arm guards charged independently of their match, and
//! deliberately descends into generated nodes so that return adjacency is
//! unchanged. A closure that a macro generated, and every `async` block, is
//! transparent (RFC 0010 section 7.5): the walker enters its body and charges
//! the authored contents to the enclosing function, because the author wrote no
//! closure and no unit scores the block. A closure written in source stays its
//! own unit and is not entered.

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
use rustc_span::{Span, sym};
use whitaker_lint_core::hir::node_origin::node_origin;

use crate::policy::{ChargeLedger, Event, Observed};

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

/// Report whether `expr` has type `core::result::Result`.
///
/// The diagnostic item is checked instead of a textual type name so aliases
/// and re-exports still classify correctly.
fn is_result_typed(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let Some(expr_type) = cx.typeck_results().node_type_opt(expr.hir_id) else {
        return false;
    };

    expr_type
        .ty_adt_def()
        .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Result, adt.did()))
}

/// Report whether the selected body's expression type is `core::result::Result`.
pub(super) fn body_returns_result(cx: &LateContext<'_>, root: &Expr<'_>) -> bool {
    is_result_typed(cx, root)
}

/// Reduce one body root to the ordered event stream the policy scores.
pub(super) fn collect_events<'tcx>(cx: &LateContext<'tcx>, root: &'tcx Expr<'tcx>) -> Vec<Event> {
    let mut visitor = ComplexityVisitor {
        cx,
        events: Vec::new(),
        ledger: ChargeLedger::default(),
        owners: Vec::new(),
    };
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

/// What the walker does after recording an expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Visit {
    /// Walk the expression's descendants.
    Descend,
    /// The expression repeats an authored node already charged; skip its copy.
    SkipRepeat,
}

/// Collect one charge event per visited expression.
struct ComplexityVisitor<'a, 'tcx> {
    /// Resolves the bodies of generated closures, which the walker enters.
    cx: &'a LateContext<'tcx>,
    events: Vec<Event>,
    /// The authored charges already recorded, so a node is charged once.
    ledger: ChargeLedger,
    /// For each transparent closure that encloses the expression being
    /// visited, outermost first, whether it yields a `Result`. A `return`
    /// belongs to the innermost one, or to the selected body when empty.
    owners: Vec<bool>,
}

impl<'tcx> ComplexityVisitor<'_, 'tcx> {
    /// Record one visited expression with its provenance.
    ///
    /// A macro that duplicates its argument puts the same authored node in the
    /// expansion more than once, as `tracing` does with its `log` feature. The
    /// author wrote it once, so only its first visit is charged; see
    /// [`ChargeLedger`]. A repeat records nothing and tells the caller to skip
    /// the copy's descendants, so the second copy leaves the event stream, and
    /// with it return adjacency, as if it had never been expanded.
    fn record(&mut self, observed: Observed, span: Span) -> Visit {
        let event = Event::new(observed, node_origin(span));
        if self.ledger.is_repeat(event, span.lo().0, span.hi().0) {
            return Visit::SkipRepeat;
        }

        self.events.push(event);
        Visit::Descend
    }

    /// Record a match as one branch when it has more than one arm.
    ///
    /// Guards are not recorded here. They arrive through [`Visitor::visit_arm`]
    /// so that each guard keeps its source position between the preceding arms
    /// and its own arm body; recording them up front would move authored
    /// events out of order and could separate returns that are adjacent in the
    /// source.
    fn record_match(&mut self, expr: &Expr<'_>, arms: &[Arm<'_>]) -> Visit {
        let observed = if arms.len() > 1 {
            Observed::Branch
        } else {
            Observed::Neutral
        };
        self.record(observed, expr.span)
    }

    /// Enter the body of an `async` block, or of a closure a macro generated.
    ///
    /// The default walk stops at every closure body, which is right for a
    /// closure the author wrote: `check_fn` scores that closure as its own
    /// unit. An `async` block is never scored as a unit, because `check_fn`
    /// skips every coroutine wrapper, and a generated closure is skipped for
    /// the same reason. Without this step an attribute macro that wraps a body
    /// in `async move { .. }` would hide the author's branching entirely.
    ///
    /// An authored `async` block is entered too, not only a generated one.
    /// `#[async_trait]` builds its block from the method body's own span, so
    /// provenance cannot tell that block from one the author wrote, and a
    /// block that no unit scores would otherwise be a blind spot.
    fn enter_transparent_closure(&mut self, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Closure(closure) = expr.kind else {
            return;
        };
        if !is_transparent(closure, expr.span) {
            return;
        }

        let closure_body = self.cx.tcx.hir_body(closure.body).value;
        self.owners.push(is_result_typed(self.cx, closure_body));
        self.visit_expr(closure_body);
        self.owners.pop();
    }

    /// Classify a `return` by the body that owns it.
    fn return_category(&self) -> Observed {
        match self.owners.last() {
            Some(&returns_result) => Observed::BlockReturn { returns_result },
            None => Observed::Return,
        }
    }
}

/// Report whether a closure is part of the enclosing function's reading.
///
/// An `async` block is never a unit, and neither is a closure a macro wrote.
fn is_transparent(closure: &Closure<'_>, span: Span) -> bool {
    matches!(closure.kind, ClosureKind::Coroutine(_)) || node_origin(span).is_generated()
}

impl<'tcx> Visitor<'tcx> for ComplexityVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let visit = match expr.kind {
            ExprKind::If(..) => self.record(Observed::Branch, expr.span),
            ExprKind::Match(_, arms, _) => self.record_match(expr, arms),
            ExprKind::Ret(..) => self.record(self.return_category(), expr.span),
            _ => self.record(Observed::Neutral, expr.span),
        };
        if visit == Visit::SkipRepeat {
            return;
        }

        intravisit::walk_expr(self, expr);
        self.enter_transparent_closure(expr);
    }

    /// Record the arm's guard before walking into the arm.
    ///
    /// A guard is always charged separately because its span is independently
    /// authored, and it must land in the event stream where it appears in the
    /// source: after the arms that precede it and before its own arm body.
    fn visit_arm(&mut self, arm: &'tcx Arm<'tcx>) {
        if let Some(guard) = arm.guard {
            // A guard repeats only inside a repeated `match`, which `visit_expr`
            // has already skipped, so there is no copy left to skip here.
            let _visit = self.record(Observed::Guard, guard.span);
        }

        intravisit::walk_arm(self, arm);
    }
}
