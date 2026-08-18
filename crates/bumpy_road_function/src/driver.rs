//! Lint pass detecting multiple "bumpy road" complexity clusters inside a function.
//!
//! The detector converts nested control-flow and predicate complexity into a
//! per-line signal, applies moving-average smoothing, then identifies two or
//! more separated bumps above a configurable threshold. The warning highlights
//! the two largest bump intervals with labelled spans.

use rustc_hir as hir;
use rustc_hir::ExprKind;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Ident, Span, symbol::Symbol};
use whitaker::SharedConfig;
use whitaker_common::{
    Localizer,
    complexity_signal::{rasterize_signal, smooth_moving_average},
    get_localizer_for_lint,
    i18n::MessageKey,
};

use crate::analysis::{Settings, detect_bumps, normalise_settings};

const LINT_NAME: &str = "bumpy_road_function";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

mod config;
mod diagnostic;
mod segment_builder;

use self::{
    config::load_configuration,
    diagnostic::{DiagnosticInput, emit_diagnostic},
    segment_builder::{SegmentBuilder, span_line_range},
};

dylint_linting::impl_late_lint! {
    pub BUMPY_ROAD_FUNCTION,
    Warn,
    "functions should avoid multiple separated clusters of complex conditional logic",
    BumpyRoadFunction::default()
}

/// Lint pass that caches configuration and localization for a crate.
pub struct BumpyRoadFunction {
    settings: Settings,
    localizer: Localizer,
}

impl Default for BumpyRoadFunction {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            localizer: Localizer::new(None),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for BumpyRoadFunction {
    fn check_crate(&mut self, _cx: &LateContext<'tcx>) {
        self.settings = normalise_settings(load_configuration().into_settings());
        let shared_config = SharedConfig::load();
        self.localizer = get_localizer_for_lint(LINT_NAME, shared_config.locale());
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let Some(target) = extract_item_target(item) else {
            return;
        };

        self.analyse_if_not_expanded(cx, item.span, target);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        let Some(target) = extract_impl_item_target(item) else {
            return;
        };

        self.analyse_if_not_expanded(cx, item.span, target);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        let Some(target) = extract_trait_item_target(item) else {
            return;
        };

        self.analyse_if_not_expanded(cx, item.span, target);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx hir::Expr<'tcx>) {
        if !self.settings.include_closures {
            return;
        }

        let Some(target) = extract_expr_target(expr) else {
            return;
        };

        self.analyse_if_not_expanded(cx, expr.span, target);
    }
}

impl BumpyRoadFunction {
    fn analyse_if_not_expanded(&self, cx: &LateContext<'_>, span: Span, target: AnalysisTarget) {
        if span.from_expansion() {
            return;
        }

        analyse_body(cx, target, &self.settings, &self.localizer);
    }
}

const fn extract_item_target(item: &hir::Item<'_>) -> Option<AnalysisTarget> {
    let hir::ItemKind::Fn { ident, body, .. } = item.kind else {
        return None;
    };

    Some(make_ident_target(ident, body))
}

const fn extract_impl_item_target(item: &hir::ImplItem<'_>) -> Option<AnalysisTarget> {
    if let hir::ImplItemKind::Fn(_, body_id) = item.kind {
        return Some(make_analysis_target(
            item.ident.name,
            item.ident.span,
            body_id,
        ));
    }

    None
}

const fn extract_trait_item_target(item: &hir::TraitItem<'_>) -> Option<AnalysisTarget> {
    let hir::TraitItemKind::Fn(_, trait_fn) = item.kind else {
        return None;
    };

    let hir::TraitFn::Provided(body_id) = trait_fn else {
        return None;
    };

    Some(make_analysis_target(
        item.ident.name,
        item.ident.span,
        body_id,
    ))
}

fn extract_expr_target(expr: &hir::Expr<'_>) -> Option<AnalysisTarget> {
    let ExprKind::Closure(hir::Closure { body, .. }) = expr.kind else {
        return None;
    };

    Some(make_analysis_target(
        Symbol::intern("closure"),
        expr.span,
        *body,
    ))
}

const fn make_analysis_target(
    name: Symbol,
    primary_span: Span,
    body_id: hir::BodyId,
) -> AnalysisTarget {
    AnalysisTarget {
        name,
        primary_span,
        body_id,
    }
}

const fn make_ident_target(ident: Ident, body_id: hir::BodyId) -> AnalysisTarget {
    make_analysis_target(ident.name, ident.span, body_id)
}

struct AnalysisTarget {
    name: Symbol,
    primary_span: Span,
    body_id: hir::BodyId,
}

fn analyse_body(
    cx: &LateContext<'_>,
    target: AnalysisTarget,
    settings: &Settings,
    localizer: &Localizer,
) {
    let body = cx.tcx.hir_body(target.body_id);
    let body_span = body.value.span;
    if body_span.from_expansion() {
        return;
    }

    let source_map = cx.tcx.sess.source_map();
    let Some(function_lines) = span_line_range(source_map, body_span) else {
        return;
    };

    let mut segments = Vec::new();
    let mut builder = SegmentBuilder::new(cx, settings, function_lines.clone(), &mut segments);
    builder.visit_expr(body.value);

    let signal = match rasterize_signal(function_lines.clone(), &segments) {
        Ok(signal) => signal,
        Err(error) => {
            cx.tcx.sess.dcx().span_delayed_bug(
                body_span,
                format!("bumpy-road signal rasterisation failed: {error}"),
            );
            return;
        }
    };

    let smoothed = match smooth_moving_average(&signal, settings.window) {
        Ok(signal) => signal,
        Err(error) => {
            cx.tcx.sess.dcx().span_delayed_bug(
                body_span,
                format!("bumpy-road signal smoothing failed: {error}"),
            );
            return;
        }
    };

    let bumps = detect_bumps(&smoothed, settings.threshold, settings.min_bump_lines);
    if bumps.len() < 2 {
        return;
    }

    emit_diagnostic(
        cx,
        DiagnosticInput {
            name: target.name.as_str(),
            primary_span: target.primary_span,
            body_span,
            function_lines,
            bumps,
            settings,
        },
        localizer,
    );
}
