//! Detect panics inside `unwrap_or_else` fallback closures.

use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::sym;
use whitaker_common::SimplePath;

/// All known panic entry points (plain and formatted).
const PANIC_PATHS: &[&[&str]] = &[
    // core
    &["core", "panicking", "panic"],
    &["core", "panicking", "panic_fmt"],
    &["core", "panicking", "panic_nounwind"],
    &["core", "panicking", "panic_str"],
    &["core", "panicking", "panic_any"],
    &["core", "panicking", "begin_panic"],
    &["core", "panicking", "panic_display"],
    // std::panicking re-exports
    &["std", "panicking", "panic"],
    &["std", "panicking", "panic_fmt"],
    &["std", "panicking", "panic_any"],
    &["std", "panicking", "begin_panic"],
    &["std", "panicking", "panic_display"],
    // std::panic re-exports
    &["std", "panic", "panic_any"],
    // std::rt wrappers
    &["std", "rt", "panic_fmt"],
    &["std", "rt", "panic_display"],
    &["std", "rt", "begin_panic"],
    &["std", "rt", "begin_panic_fmt"],
];

/// Method names on `core::fmt::Arguments` that accept runtime format values.
const FMT_ARGS_RUNTIME_METHODS: &[&str] = &["new_v1", "new_v1_formatted"];

/// Summarises whether a closure contains a panic and whether that panic uses
/// format-string interpolation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PanicInfo {
    pub(crate) panics: bool,
    pub(crate) uses_interpolation: bool,
}

/// Analyses the closure referenced by `body_id` and returns a [`PanicInfo`]
/// describing whether it panics and whether the panic interpolates values.
#[must_use]
pub(crate) fn closure_panics<'tcx>(cx: &LateContext<'tcx>, body_id: hir::BodyId) -> PanicInfo {
    let mut detector = PanicDetector {
        cx,
        panics: false,
        uses_interpolation: false,
    };
    let body = cx.tcx.hir_body(body_id);
    rustc_hir::intravisit::Visitor::visit_body(&mut detector, body);
    PanicInfo {
        panics: detector.panics,
        uses_interpolation: detector.uses_interpolation,
    }
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
    uses_interpolation: bool,
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for PanicDetector<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.panics {
            return;
        }

        if is_panic_call(self.cx, expr) {
            self.panics = true;
            self.uses_interpolation = panic_args_use_interpolation(expr);
            return;
        }

        if is_unwrap_or_expect(self.cx, expr) {
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

/// Returns `true` when `expr` calls a known panic entry point.
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
        .any(|candidate| whitaker_common::is_path_to(&path, candidate.iter().copied()))
}

/// Checks whether a panic call's `format_args!` construction uses runtime
/// values, indicating the panic message interpolates at least one expression.
///
/// In Rust 2021+, even `panic!("static")` routes through `panic_fmt`, so the
/// def-path alone cannot distinguish interpolation. Instead, this function
/// walks the call's argument sub-expressions looking for `Arguments::new_v1`
/// or `Arguments::new_v1_formatted`, which are only used when format arguments
/// are present.
fn panic_args_use_interpolation(expr: &Expr<'_>) -> bool {
    let mut finder = RuntimeArgsFinder { found: false };
    rustc_hir::intravisit::walk_expr(&mut finder, expr);
    finder.found
}

struct RuntimeArgsFinder {
    found: bool,
}

impl<'tcx> rustc_hir::intravisit::Visitor<'tcx> for RuntimeArgsFinder {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.found {
            return;
        }

        if let ExprKind::Call(callee, _) = expr.kind
            && is_fmt_args_runtime_call(callee)
        {
            self.found = true;
            return;
        }

        rustc_hir::intravisit::walk_expr(self, expr);
    }
}

/// Returns `true` when the callee is a `QPath::TypeRelative` call whose
/// method segment matches a runtime `Arguments` constructor (`new_v1` or
/// `new_v1_formatted`).
///
/// Compiler-generated `format_arguments::new_v1(...)` expressions use a
/// type-relative qualified path whose method segment isn't resolvable via
/// `qpath_res` (it resolves to `Err`). Inspecting the segment identifier
/// directly is the reliable way to detect these.
fn is_fmt_args_runtime_call(callee: &Expr<'_>) -> bool {
    let ExprKind::Path(hir::QPath::TypeRelative(_, segment)) = callee.kind else {
        return false;
    };
    let name = segment.ident.name.as_str();
    FMT_ARGS_RUNTIME_METHODS.contains(&name)
}

fn def_id_of_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<DefId> {
    cx.typeck_results()
        .type_dependent_def_id(callee.hir_id)
        .or_else(|| match callee.kind {
            ExprKind::Path(qpath) => cx.qpath_res(&qpath, callee.hir_id).opt_def_id(),
            _ => None,
        })
}
