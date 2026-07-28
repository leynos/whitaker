//! Lint crate enforcing capability-based filesystem access by forbidding
//! `std::fs` operations.

use crate::config::{LINT_NAME, load_configuration};
use crate::diagnostics::emit_diagnostic;
use crate::exclusion::PathExclusions;
use crate::usage::{
    StdFsUsage, UsageCategory, classify_def_id, classify_qpath, classify_res, label_is_std_fs,
};
use log::{debug, info};
use rustc_hir as hir;
use rustc_hir::AmbigArg;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::{Span, sym};
use whitaker::SharedConfig;
use whitaker_common::SimplePath;
use whitaker_common::i18n::Localizer;
use whitaker_common::i18n::get_localizer_for_lint;

pub struct NoStdFsOperations {
    localizer: Localizer,
    excluded: bool,
    path_exclusions: PathExclusions,
}

impl Default for NoStdFsOperations {
    fn default() -> Self {
        Self {
            localizer: Localizer::new(None),
            excluded: false,
            path_exclusions: PathExclusions::default(),
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

        self.excluded = config.is_excluded(crate_name);
        self.path_exclusions = config.path_exclusions();

        if self.excluded {
            info!(
                target: LINT_NAME,
                "crate `{crate_name}` is excluded from no_std_fs_operations lint"
            );
        } else if !self.path_exclusions.is_empty() {
            debug!(
                target: LINT_NAME,
                "crate `{crate_name}` has module-path exclusions configured for no_std_fs_operations"
            );
        }
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if self.should_skip() {
            return;
        }
        if let hir::ItemKind::Use(path, ..) = item.kind {
            let site = LintSite {
                hir_id: item.hir_id(),
                span: path.span,
            };
            for res in path.res.present_items() {
                let usage = classify_res(cx, res, UsageCategory::Import);
                self.emit_optional(cx, site, usage);
            }
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        if self.should_skip() {
            return;
        }
        let site = LintSite {
            hir_id: expr.hir_id,
            span: expr.span,
        };
        match &expr.kind {
            hir::ExprKind::Path(qpath) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, site, usage);
            }
            hir::ExprKind::Struct(qpath, ..) => {
                let usage = classify_qpath(cx, qpath, expr.hir_id, UsageCategory::Call);
                self.emit_optional(cx, site, usage);
            }
            hir::ExprKind::MethodCall(segment, receiver, ..) => {
                let mut usage = cx
                    .typeck_results()
                    .type_dependent_def_id(expr.hir_id)
                    .and_then(|def_id| classify_def_id(cx, def_id, UsageCategory::Call));

                if usage.is_none() {
                    usage = self.receiver_usage_for_method(cx, receiver, segment.ident.as_str());
                }

                self.emit_optional(cx, site, usage);
            }
            _ => {}
        }
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx hir::Ty<'tcx, AmbigArg>) {
        if self.should_skip() {
            return;
        }
        if let hir::TyKind::Path(qpath) = &ty.kind {
            let usage = classify_qpath(cx, qpath, ty.hir_id, UsageCategory::Type);
            self.emit_optional(
                cx,
                LintSite {
                    hir_id: ty.hir_id,
                    span: ty.span,
                },
                usage,
            );
        }
    }
}

/// The HIR location of a candidate `std::fs` usage.
///
/// Bundling the node's `HirId` with its span keeps `emit_optional` to a single
/// site argument: the `HirId` resolves the enclosing module path and the span
/// anchors the emitted diagnostic.
#[derive(Clone, Copy)]
struct LintSite {
    hir_id: hir::HirId,
    span: Span,
}

impl NoStdFsOperations {
    /// Centralizes exclusion logic for all lint pass methods.
    #[inline]
    fn should_skip(&self) -> bool {
        self.excluded
    }

    fn emit_optional(&self, cx: &LateContext<'_>, site: LintSite, usage: Option<StdFsUsage>) {
        if self.should_skip() {
            return;
        }
        let Some(usage) = usage else {
            return;
        };
        // Resolve the enclosing item's path only for genuine `std::fs` hits
        // (rare), and only when path exclusions are configured, so the common
        // path pays no lookup cost.
        if self.is_path_excluded(cx, site.hir_id) {
            return;
        }
        self.emit(cx, site.span, usage);
    }

    /// Returns `true` when the item enclosing `hir_id` falls within a
    /// configured `excluded_paths` entry.
    fn is_path_excluded(&self, cx: &LateContext<'_>, hir_id: hir::HirId) -> bool {
        if self.path_exclusions.is_empty() {
            return false;
        }
        self.path_exclusions
            .excludes(&enclosing_item_path(cx, hir_id))
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

/// Resolve the fully qualified path of the item enclosing `hir_id`.
///
/// The path is anchored at the crate identifier (for example
/// `my_app::legacy_io::reader`), matching the convention used by
/// `excluded_paths` entries. `def_path_str` renders the local crate root as the
/// `crate` keyword rather than the crate name, so the path is assembled from the
/// crate name plus the named def-path segments instead. Unnamed segments (such
/// as `impl` blocks or closures) carry no module identity and are skipped.
fn enclosing_item_path(cx: &LateContext<'_>, hir_id: hir::HirId) -> SimplePath {
    let owner = cx.tcx.hir_get_parent_item(hir_id).to_def_id();
    let def_path = cx.tcx.def_path(owner);
    let mut segments = Vec::with_capacity(def_path.data.len() + 1);
    segments.push(cx.tcx.crate_name(owner.krate).to_string());
    segments.extend(
        def_path
            .data
            .iter()
            .filter_map(|segment| segment.data.get_opt_name())
            .map(|name| name.to_string()),
    );
    SimplePath::new(segments)
}
