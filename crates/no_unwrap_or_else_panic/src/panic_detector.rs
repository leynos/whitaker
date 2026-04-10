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

/// Summarises whether a closure contains panics and distinguishes between
/// plain (non-interpolated) and interpolated panic sites.
///
/// A closure may contain multiple panic paths; tracking both kinds prevents
/// incorrectly suppressing the lint when a test contains both interpolated
/// diagnostic panics and plain unconditional panics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PanicInfo {
    pub(crate) panics: bool,
    pub(crate) has_plain_panic: bool,
    pub(crate) has_interpolated_panic: bool,
}

impl PanicInfo {
    /// Returns `true` when the closure has at least one interpolated panic
    /// and no plain (non-interpolating) panic.
    #[must_use]
    pub(crate) fn is_interpolated_only(&self) -> bool {
        self.has_interpolated_panic && !self.has_plain_panic
    }
}

/// Analyses the closure referenced by `body_id` and returns a [`PanicInfo`]
/// describing whether it panics and distinguishing plain vs interpolated panics.
#[must_use]
pub(crate) fn closure_panics<'tcx>(cx: &LateContext<'tcx>, body_id: hir::BodyId) -> PanicInfo {
    let mut detector = PanicDetector {
        cx,
        panics: false,
        has_plain_panic: false,
        has_interpolated_panic: false,
    };
    let body = cx.tcx.hir_body(body_id);
    rustc_hir::intravisit::Visitor::visit_body(&mut detector, body);
    PanicInfo {
        panics: detector.panics,
        has_plain_panic: detector.has_plain_panic,
        has_interpolated_panic: detector.has_interpolated_panic,
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
    has_plain_panic: bool,
    has_interpolated_panic: bool,
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for PanicDetector<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if is_panic_call(self.cx, expr) {
            self.panics = true;
            if panic_args_use_interpolation(self.cx, expr) {
                self.has_interpolated_panic = true;
            } else {
                self.has_plain_panic = true;
            }
        } else if is_unwrap_or_expect(self.cx, expr) {
            self.panics = true;
            self.has_plain_panic = true;
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
/// inspects only the panic message argument (the first argument to the panic
/// entry point) for `Arguments::new_v1` or `Arguments::new_v1_formatted`,
/// which are only used when format arguments are present.
///
/// The check examines the message argument's call tree for fmt::Arguments
/// constructors, but only considers calls that are part of the compiler-
/// generated format_args expansion. This avoids false positives from unrelated
/// user code like `panic_any(MyType::new_v1())` where `MyType::new_v1()` is
/// the payload, not a format_args constructor.
fn panic_args_use_interpolation<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    // Extract the panic message argument (first argument to the panic call).
    let ExprKind::Call(_, args) = expr.kind else {
        return false;
    };

    let Some(message_arg) = args.first() else {
        return false;
    };

    // Check if the message argument contains fmt::Arguments runtime constructors.
    // This walks the expression tree to find nested calls (e.g., inside blocks
    // from format_args expansion), but only matches calls that represent the
    // actual format_args construction, not arbitrary user types with similar
    // method names.
    let mut finder = RuntimeArgsFinder { cx, found: false };
    rustc_hir::intravisit::walk_expr(&mut finder, message_arg);
    finder.found
}

struct RuntimeArgsFinder<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    found: bool,
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for RuntimeArgsFinder<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.found {
            return;
        }

        if let ExprKind::Call(callee, _) = expr.kind
            && is_fmt_args_runtime_call(self.cx, callee)
        {
            self.found = true;
            return;
        }

        rustc_hir::intravisit::walk_expr(self, expr);
    }
}

/// Returns `true` when the callee is a `QPath::TypeRelative` call whose
/// method segment matches a runtime `Arguments` constructor (`new_v1` or
/// `new_v1_formatted`) and the receiver type is `core::fmt::Arguments`
/// (verified via the `format_arguments` lang item).
///
/// Compiler-generated `format_arguments::new_v1(...)` expressions use a
/// type-relative qualified path whose method segment isn't resolvable via
/// `qpath_res` (it resolves to `Err`). This function inspects both the
/// segment identifier and the receiver type to avoid false positives from
/// unrelated user types with `new_v1` methods (e.g., `MyType::new_v1()`).
fn is_fmt_args_runtime_call<'tcx>(cx: &LateContext<'tcx>, callee: &Expr<'tcx>) -> bool {
    let ExprKind::Path(hir::QPath::TypeRelative(ty, segment)) = callee.kind else {
        return false;
    };

    if !matches!(segment.ident.name, sym::new_v1 | sym::new_v1_formatted) {
        return false;
    }

    // Verify the receiver type is core::fmt::Arguments to avoid false positives
    // from user types with similar method names.
    let receiver_ty = cx.typeck_results().node_type(ty.hir_id);
    let Some(fmt_args_did) = cx.tcx.lang_items().format_arguments() else {
        return false;
    };
    receiver_ty
        .ty_adt_def()
        .is_some_and(|adt| adt.did() == fmt_args_did)
}

fn def_id_of_callee(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<DefId> {
    cx.typeck_results()
        .type_dependent_def_id(callee.hir_id)
        .or_else(|| match callee.kind {
            ExprKind::Path(qpath) => cx.qpath_res(&qpath, callee.hir_id).opt_def_id(),
            _ => None,
        })
}
