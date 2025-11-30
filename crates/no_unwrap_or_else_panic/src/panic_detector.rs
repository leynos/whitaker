//! Detect panics inside `unwrap_or_else` fallback closures.

use common::SimplePath;
use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::sym;

/// Known panic entry points used by both clippy and non-clippy builds.
const PANIC_PATHS: &[&[&str]] = &[
    // core
    &["core", "panicking", "panic"],
    &["core", "panicking", "panic_fmt"],
    &["core", "panicking", "panic_nounwind"],
    &["core", "panicking", "panic_str"],
    &["core", "panicking", "panic_any"],
    &["core", "panicking", "begin_panic"],
    // std::panicking re-exports
    &["std", "panicking", "panic"],
    &["std", "panicking", "panic_fmt"],
    &["std", "panicking", "panic_any"],
    &["std", "panicking", "begin_panic"],
    // std::rt wrappers
    &["std", "rt", "panic_fmt"],
    &["std", "rt", "begin_panic"],
    &["std", "rt", "begin_panic_fmt"],
];

/// Returns `true` when the closure referenced by `body_id` contains a panic
/// invocation or an inner `unwrap`/`expect`.
#[must_use]
pub(crate) fn closure_panics<'tcx>(cx: &LateContext<'tcx>, body_id: hir::BodyId) -> bool {
    let mut detector = PanicDetector { cx, panics: false };
    let body = cx.tcx.hir_body(body_id);
    rustc_hir::intravisit::Visitor::visit_body(&mut detector, body);
    detector.panics
}

/// Returns `true` when the receiver resolves to `Option` or `Result`.
#[must_use]
pub(crate) fn receiver_is_option_or_result<'tcx>(
    cx: &LateContext<'tcx>,
    receiver: &'tcx hir::Expr<'tcx>,
) -> bool {
    let ty = cx.typeck_results().expr_ty(receiver);
    ty_is_option_or_result(cx, ty)
}

fn ty_is_option_or_result<'tcx>(cx: &LateContext<'tcx>, ty: ty::Ty<'tcx>) -> bool {
    let typing_env = ty::TypingEnv {
        typing_mode: ty::TypingMode::non_body_analysis(),
        param_env: cx.param_env,
    };
    let ty = cx.tcx.normalize_erasing_regions(typing_env, ty).peel_refs();

    let Some(adt) = ty.ty_adt_def() else {
        return false;
    };

    let def_id = adt.did();
    cx.tcx.is_diagnostic_item(sym::Option, def_id) || cx.tcx.is_diagnostic_item(sym::Result, def_id)
}

struct PanicDetector<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    panics: bool,
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for PanicDetector<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.panics {
            return;
        }

        if is_panic_call(self.cx, expr) || is_unwrap_or_expect(self.cx, expr) {
            self.panics = true;
            return;
        }

        rustc_hir::intravisit::walk_expr(self, expr);
    }
}

fn is_unwrap_or_expect<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    let ExprKind::MethodCall(segment, receiver, ..) = expr.kind else {
        return false;
    };

    matches!(segment.ident.name.as_str(), "unwrap" | "expect")
        && receiver_is_option_or_result(cx, receiver)
}

/// Returns `true` when `expr` calls a known panic entry point. Uses def-path
/// string matching because internal panic helpers lack stable diagnostic items.
fn is_panic_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let ExprKind::Call(callee, _) = expr.kind else {
        return false;
    };

    let Some(def_id) = def_id_of_callee(cx, callee) else {
        return false;
    };

    let path = SimplePath::from(cx.tcx.def_path_str(def_id).as_str());
    PANIC_PATHS
        .iter()
        .any(|candidate| common::is_path_to(&path, candidate.iter().copied()))
}

fn def_id_of_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<DefId> {
    cx.typeck_results()
        .type_dependent_def_id(callee.hir_id)
        .or_else(|| match callee.kind {
            ExprKind::Path(qpath) => cx.qpath_res(&qpath, callee.hir_id).opt_def_id(),
            _ => None,
        })
}
