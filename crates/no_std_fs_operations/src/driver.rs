//! Lint crate enforcing capability-based filesystem access by forbidding
//! `std::fs` operations.

use crate::diagnostics::emit_diagnostic;
use crate::usage::{
    StdFsUsage, UsageCategory, classify_def_id, classify_qpath, classify_res, label_is_std_fs,
};
use common::i18n::Localizer;
use common::i18n::get_localizer_for_lint;
use log::debug;
use rustc_hir as hir;
use rustc_hir::AmbigArg;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::{Span, sym};
use serde::Deserialize;
use whitaker::SharedConfig;

const LINT_NAME: &str = "no_std_fs_operations";

/// Configuration for the `no_std_fs_operations` lint.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NoStdFsConfig {
    /// Crate names excluded from the lint. These crates are allowed to use
    /// `std::fs` operations without triggering diagnostics.
    pub excluded_crates: Vec<String>,
}

pub struct NoStdFsOperations {
    localizer: Localizer,
    excluded: bool,
}

impl Default for NoStdFsOperations {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
            excluded: false,
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
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());

        let config = load_configuration();
        let crate_name_sym = cx.tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
        let crate_name = crate_name_sym.as_str();

        self.excluded = config
            .excluded_crates
            .iter()
            .any(|excluded| excluded == crate_name);

        if self.excluded {
            debug!(
                target: LINT_NAME,
                "crate `{crate_name}` is excluded from no_std_fs_operations lint"
            );
        }
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
            hir::ExprKind::MethodCall(segment, receiver, ..) => {
                let mut usage = cx
                    .typeck_results()
                    .type_dependent_def_id(expr.hir_id)
                    .and_then(|def_id| classify_def_id(cx, def_id, UsageCategory::Call));

                if usage.is_none() {
                    usage = self.receiver_usage_for_method(cx, receiver, segment.ident.as_str());
                }

                self.emit_optional(cx, expr.span, usage);
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
    fn emit_optional(&self, cx: &LateContext<'_>, span: Span, usage: Option<StdFsUsage>) {
        if self.excluded {
            return;
        }
        if let Some(usage) = usage {
            self.emit(cx, span, usage);
        }
    }

    fn emit(&self, cx: &LateContext<'_>, span: Span, usage: StdFsUsage) {
        emit_diagnostic(cx, span, usage, &self.localizer);
    }

    fn receiver_usage_for_method(
        &self,
        cx: &LateContext<'_>,
        receiver: &hir::Expr<'_>,
        method: &str,
    ) -> Option<StdFsUsage> {
        let ty = cx.typeck_results().expr_ty(receiver).peel_refs();

        let ty::Adt(adt, _) = ty.kind() else {
            return None;
        };

        let def_id = adt.did();
        if cx.tcx.crate_name(def_id.krate) != sym::std {
            return None;
        }

        let label = cx.tcx.def_path_str(def_id);
        if !label_is_std_fs(&label) {
            return None;
        }

        let operation = format!("{label}::{method}");
        Some(StdFsUsage::new(operation, UsageCategory::Call))
    }
}

fn load_configuration() -> NoStdFsConfig {
    match dylint_linting::config::<NoStdFsConfig>(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => NoStdFsConfig::default(),
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            NoStdFsConfig::default()
        }
    }
}
