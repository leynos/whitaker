//! Lint crate enforcing capability-based filesystem access by forbidding
//! `std::fs` operations.

use crate::diagnostics::emit_diagnostic;
use crate::usage::{StdFsUsage, UsageCategory, classify_def_id, classify_qpath, classify_res};
use common::i18n::Localizer;
use common::i18n::get_localizer_for_lint;
use rustc_hir as hir;
use rustc_hir::AmbigArg;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use whitaker::SharedConfig;

pub struct NoStdFsOperations {
    localizer: Localizer,
}

impl Default for NoStdFsOperations {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
        }
    }
}

dylint_linting::impl_late_lint! {
    pub NO_STD_FS_OPERATIONS,
    Deny,
    "std::fs operations bypass Whitaker's capability-based filesystem policy",
    NoStdFsOperations::default()
}

impl<'tcx> LateLintPass<'tcx> for NoStdFsOperations {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint("no_std_fs_operations", shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Use(path, ..) = item.kind {
            for res in path.res.present_items() {
                let usage = classify_res(cx, res, UsageCategory::Import);
                self.emit_optional(cx, path.span, usage);
            }
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        match &expr.kind {
            hir::ExprKind::Path(qpath) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, expr.span, usage);
            }
            hir::ExprKind::Struct(qpath, ..) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, expr.span, usage);
            }
            hir::ExprKind::MethodCall(..) => {
                if let Some(def_id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) {
                    let usage = classify_def_id(cx, def_id, UsageCategory::Call);
                    self.emit_optional(cx, expr.span, usage);
                }
            }
            _ => {}
        }
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx hir::Ty<'tcx, AmbigArg>) {
        if let hir::TyKind::Path(qpath) = &ty.kind {
            let usage = classify_qpath(cx, qpath, ty.hir_id, UsageCategory::Type);
            self.emit_optional(cx, ty.span, usage);
        }
    }
}

impl NoStdFsOperations {
    fn emit_optional(
        &self,
        cx: &LateContext<'_>,
        span: Span,
        usage: Option<StdFsUsage>,
    ) {
        if let Some(usage) = usage {
            self.emit(cx, span, usage);
        }
    }

    fn emit(&self, cx: &LateContext<'_>, span: Span, usage: StdFsUsage) {
        emit_diagnostic(cx, span, usage, &self.localizer);
    }
}
