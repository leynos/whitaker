//! Lint pass detecting multiple "bumpy road" complexity clusters inside a function.
//!
//! The detector converts nested control-flow and predicate complexity into a
//! per-line signal, applies moving-average smoothing, then identifies two or
//! more separated bumps above a configurable threshold. The warning highlights
//! the two largest bump intervals with labelled spans.

use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::RangeInclusive;

use crate::analysis::{BumpInterval, Settings, detect_bumps, normalise_settings, top_two_bumps};
use common::complexity_signal::{LineSegment, rasterize_signal, smooth_moving_average};
use common::i18n::MessageKey;
use common::{
    Arguments, Localizer, MessageResolution, get_localizer_for_lint, safe_resolve_message_set,
};
use fluent_templates::fluent_bundle::FluentValue;
use log::debug;
use rustc_hir as hir;
use rustc_hir::{BinOpKind, ExprKind, LoopSource, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::symbol::Symbol;
use rustc_span::{BytePos, DesugaringKind, Span};
use serde::Deserialize;
use whitaker::SharedConfig;

const LINT_NAME: &str = "bumpy_road_function";
const MESSAGE_KEY: MessageKey<'static> = MessageKey::new(LINT_NAME);

dylint_linting::impl_late_lint! {
    pub BUMPY_ROAD_FUNCTION,
    Warn,
    "functions should avoid multiple separated clusters of complex conditional logic",
    BumpyRoadFunction::default()
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    threshold: f64,
    window: usize,
    min_bump_lines: usize,
    include_closures: bool,
    weights: WeightsConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WeightsConfig {
    depth: f64,
    predicate: f64,
    flow: f64,
}

impl Default for WeightsConfig {
    fn default() -> Self {
        let defaults = Settings::default().weights;
        Self {
            depth: defaults.depth,
            predicate: defaults.predicate,
            flow: defaults.flow,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let defaults = Settings::default();
        Self {
            threshold: defaults.threshold,
            window: defaults.window,
            min_bump_lines: defaults.min_bump_lines,
            include_closures: defaults.include_closures,
            weights: WeightsConfig::default(),
        }
    }
}

impl Config {
    fn into_settings(self) -> Settings {
        Settings {
            threshold: self.threshold,
            window: self.window,
            min_bump_lines: self.min_bump_lines,
            include_closures: self.include_closures,
            weights: crate::analysis::Weights {
                depth: self.weights.depth,
                predicate: self.weights.predicate,
                flow: self.weights.flow,
            },
        }
    }
}

/// Lint pass that caches configuration and localisation for a crate.
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
    fn analyse_if_not_expanded(
        &self,
        cx: &LateContext<'_>,
        span: Span,
        target: AnalysisTarget<'_>,
    ) {
        if span.from_expansion() {
            return;
        }

        analyse_body(cx, target, &self.settings, &self.localizer);
    }
}

fn load_configuration() -> Config {
    match dylint_linting::config::<Config>(LINT_NAME) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(error) => {
            debug!(
                target: LINT_NAME,
                "failed to parse `{LINT_NAME}` configuration: {error}; using defaults"
            );
            Config::default()
        }
    }
}

fn extract_item_target<'hir>(item: &'hir hir::Item<'hir>) -> Option<AnalysisTarget<'hir>> {
    let hir::ItemKind::Fn { ident, body, .. } = item.kind else {
        return None;
    };

    Some(AnalysisTarget {
        name: ident.name,
        primary_span: ident.span,
        body_id: body,
        _marker: PhantomData,
    })
}

fn extract_impl_item_target<'hir>(item: &'hir hir::ImplItem<'hir>) -> Option<AnalysisTarget<'hir>> {
    let hir::ImplItemKind::Fn(_, body_id) = item.kind else {
        return None;
    };

    Some(AnalysisTarget {
        name: item.ident.name,
        primary_span: item.ident.span,
        body_id,
        _marker: PhantomData,
    })
}

fn extract_trait_item_target<'hir>(
    item: &'hir hir::TraitItem<'hir>,
) -> Option<AnalysisTarget<'hir>> {
    let hir::TraitItemKind::Fn(_, trait_fn) = item.kind else {
        return None;
    };

    let hir::TraitFn::Provided(body_id) = trait_fn else {
        return None;
    };

    Some(AnalysisTarget {
        name: item.ident.name,
        primary_span: item.ident.span,
        body_id,
        _marker: PhantomData,
    })
}

fn extract_expr_target<'hir>(expr: &'hir hir::Expr<'hir>) -> Option<AnalysisTarget<'hir>> {
    let ExprKind::Closure(hir::Closure { body, .. }) = expr.kind else {
        return None;
    };

    Some(AnalysisTarget {
        name: Symbol::intern("closure"),
        primary_span: expr.span,
        body_id: *body,
        _marker: PhantomData,
    })
}

struct AnalysisTarget<'a> {
    name: Symbol,
    primary_span: Span,
    body_id: hir::BodyId,
    _marker: PhantomData<&'a ()>,
}

fn analyse_body(
    cx: &LateContext<'_>,
    target: AnalysisTarget<'_>,
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
    let mut builder = SegmentBuilder {
        cx,
        settings,
        function_lines: function_lines.clone(),
        segments: &mut segments,
    };
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

struct SegmentBuilder<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    settings: &'a Settings,
    function_lines: RangeInclusive<usize>,
    segments: &'a mut Vec<LineSegment>,
}

impl<'a, 'tcx> SegmentBuilder<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        match expr.kind {
            ExprKind::If(cond, then_expr, else_expr) => {
                if expr.span.desugaring_kind() == Some(DesugaringKind::WhileLoop) {
                    rustc_hir::intravisit::walk_expr(self, expr);
                    return;
                }
                self.push_predicate_segment(cond);

                self.visit_expr_with_depth(then_expr);

                if let Some(other) = else_expr {
                    self.push_depth_segment(other.span);
                    self.visit_expr(other);
                }
            }
            ExprKind::Loop(block, _, source, _) => {
                if self.handle_while_loop(source, block) {
                    return;
                }
                self.visit_block_with_depth(block);
            }
            ExprKind::Match(scrutinee, arms, _) => {
                self.visit_expr(scrutinee);
                self.visit_match_arms(arms);
            }
            ExprKind::Block(block, _) => {
                self.visit_block(block);
            }
            ExprKind::Closure(..) => {
                // Closure bodies are analysed separately when configured.
            }
            _ => {
                rustc_hir::intravisit::walk_expr(self, expr);
            }
        }
    }

    fn handle_while_loop(&mut self, source: LoopSource, block: &'tcx hir::Block<'tcx>) -> bool {
        if source != LoopSource::While {
            return false;
        }

        let Some((cond, body_expr)) = extract_while_components(block) else {
            return false;
        };

        self.push_predicate_segment(cond);
        self.visit_expr_with_depth(body_expr);
        true
    }

    fn visit_match_arms(&mut self, arms: &'tcx [hir::Arm<'tcx>]) {
        for arm in arms {
            self.visit_match_arm(arm);
        }
    }

    fn visit_match_arm(&mut self, arm: &'tcx hir::Arm<'tcx>) {
        if let Some(guard) = arm.guard {
            self.push_predicate_segment(guard);
        }

        self.push_depth_segment(arm.body.span);
        self.push_flow_segment(arm.body.span);
        self.visit_expr(arm.body);
    }

    fn visit_block(&mut self, block: &'tcx hir::Block<'tcx>) {
        for stmt in block.stmts {
            rustc_hir::intravisit::walk_stmt(self, stmt);
        }
        if let Some(expr) = block.expr {
            self.visit_expr(expr);
        }
    }

    fn visit_block_with_depth(&mut self, block: &'tcx hir::Block<'tcx>) {
        self.push_depth_segment(block.span);
        self.visit_block(block);
    }

    fn visit_expr_with_depth(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        self.push_depth_segment(expr.span);
        self.visit_expr(expr);
    }

    fn push_depth_segment(&mut self, span: Span) {
        self.push_segment(span, self.settings.weights.depth);
    }

    fn push_flow_segment(&mut self, span: Span) {
        self.push_segment(span, self.settings.weights.flow);
    }

    fn push_predicate_segment(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        if matches!(expr.kind, ExprKind::Let(..)) {
            return;
        }

        let branches = count_branches(expr) as f64;
        let value = branches * self.settings.weights.predicate;
        self.push_segment(expr.span, value);
    }

    fn push_segment(&mut self, span: Span, value: f64) {
        let source_map = self.cx.tcx.sess.source_map();
        let Some(lines) = span_line_range(source_map, span) else {
            return;
        };

        if lines.end() < self.function_lines.start() || lines.start() > self.function_lines.end() {
            self.cx.tcx.sess.dcx().span_delayed_bug(
                span,
                format!(
                    "bumpy-road segment lines lie outside function range (segment={segment_start}..={segment_end}, function={function_start}..={function_end})",
                    segment_start = lines.start(),
                    segment_end = lines.end(),
                    function_start = self.function_lines.start(),
                    function_end = self.function_lines.end(),
                ),
            );
            return;
        }

        let segment = match LineSegment::new(*lines.start(), *lines.end(), value) {
            Ok(segment) => segment,
            Err(error) => {
                self.cx
                    .tcx
                    .sess
                    .dcx()
                    .span_delayed_bug(span, format!("invalid bumpy-road line segment: {error}"));
                return;
            }
        };
        self.segments.push(segment);
    }
}

impl<'a, 'tcx> rustc_hir::intravisit::Visitor<'tcx> for SegmentBuilder<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        Self::visit_expr(self, expr);
    }

    fn visit_block(&mut self, block: &'tcx hir::Block<'tcx>) {
        Self::visit_block(self, block);
    }
}

fn extract_while_components<'hir>(
    block: &'hir hir::Block<'hir>,
) -> Option<(&'hir hir::Expr<'hir>, &'hir hir::Expr<'hir>)> {
    let expr = block.expr?;
    if let ExprKind::If(cond, then_expr, ..) = expr.kind {
        Some((cond, then_expr))
    } else {
        None
    }
}

fn count_branches(expr: &hir::Expr<'_>) -> usize {
    match expr.kind {
        ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
            count_branches(lhs) + count_branches(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) => count_branches(inner),
        ExprKind::DropTemps(inner) => count_branches(inner),
        ExprKind::Block(block, _) => match block.expr {
            Some(inner) => count_branches(inner),
            None => 1,
        },
        ExprKind::If(cond, ..) => count_branches(cond),
        _ => 1,
    }
}

fn span_line_range(
    source_map: &rustc_span::source_map::SourceMap,
    span: Span,
) -> Option<RangeInclusive<usize>> {
    let info = source_map.span_to_lines(span).ok()?;
    let first = info.lines.first()?;
    let last = info.lines.last()?;

    let contiguous = info
        .lines
        .windows(2)
        .all(|pair| pair[1].line_index == pair[0].line_index + 1);
    if !contiguous {
        return None;
    }

    Some((first.line_index + 1)..=(last.line_index + 1))
}

struct DiagnosticInput<'a> {
    name: &'a str,
    primary_span: Span,
    body_span: Span,
    function_lines: RangeInclusive<usize>,
    bumps: Vec<BumpInterval>,
    settings: &'a Settings,
}

fn emit_diagnostic(cx: &LateContext<'_>, input: DiagnosticInput<'_>, localizer: &Localizer) {
    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("name"), FluentValue::from(input.name));
    args.insert(
        Cow::Borrowed("count"),
        FluentValue::from(input.bumps.len() as i64),
    );
    args.insert(
        Cow::Borrowed("threshold"),
        FluentValue::from(input.settings.threshold),
    );

    let resolution = MessageResolution {
        lint_name: LINT_NAME,
        key: MESSAGE_KEY,
        args: &args,
    };

    let messages = safe_resolve_message_set(
        localizer,
        resolution,
        |message| {
            cx.tcx
                .sess
                .dcx()
                .span_delayed_bug(input.primary_span, message);
        },
        || fallback_messages(input.name, input.bumps.len(), input.settings.threshold),
    );

    let bump_spans = build_bump_spans(cx, input.body_span, &input.function_lines, &input.bumps);
    let highlighted = top_two_bumps(input.bumps);

    cx.span_lint(BUMPY_ROAD_FUNCTION, input.primary_span, |lint| {
        lint.primary_message(messages.primary().to_string());
        lint.span_note(input.primary_span, messages.note().to_string());

        for (ordinal, interval) in highlighted.iter().enumerate() {
            let Some(span) = bump_spans.get(ordinal).copied().flatten() else {
                continue;
            };
            let label = resolve_bump_label(localizer, (ordinal + 1) as i64, interval.len() as i64);
            lint.span_label(span, label);
        }

        lint.help(messages.help().to_string());
    });
}

fn build_bump_spans(
    cx: &LateContext<'_>,
    body_span: Span,
    function_lines: &RangeInclusive<usize>,
    bumps: &[BumpInterval],
) -> Vec<Option<Span>> {
    let source_map = cx.tcx.sess.source_map();
    let Ok(snippet) = source_map.span_to_snippet(body_span) else {
        return vec![None; 2];
    };

    let line_starts = line_start_offsets(&snippet);
    let body_start_line = *function_lines.start();
    let mapper = LineSpanMapper::new(body_span, snippet.len(), body_start_line, line_starts);

    let top = top_two_bumps(bumps.to_vec());
    top.iter()
        .map(|interval| {
            let start_line = body_start_line + interval.start_index();
            let end_line = body_start_line + interval.end_index();
            mapper.span_for_range(start_line, end_line)
        })
        .collect()
}

fn line_start_offsets(snippet: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in snippet.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

struct LineSpanMapper {
    base_span: Span,
    snippet_len: usize,
    base_line: usize,
    line_starts: Vec<usize>,
}

impl LineSpanMapper {
    fn new(base_span: Span, snippet_len: usize, base_line: usize, line_starts: Vec<usize>) -> Self {
        Self {
            base_span,
            snippet_len,
            base_line,
            line_starts,
        }
    }

    fn span_for_range(&self, start_line: usize, end_line: usize) -> Option<Span> {
        if start_line < self.base_line || end_line < start_line {
            return None;
        }

        let start_index = start_line - self.base_line;
        let end_index = end_line - self.base_line;

        let start_offset = *self.line_starts.get(start_index)?;
        let end_offset = self
            .line_starts
            .get(end_index + 1)
            .copied()
            .unwrap_or(self.snippet_len);

        let base = self.base_span.shrink_to_lo();
        let lo = base.lo() + BytePos(start_offset as u32);
        let mut hi = base.lo() + BytePos(end_offset as u32);
        if hi <= lo {
            hi = lo + BytePos(1);
        }
        Some(base.with_lo(lo).with_hi(hi))
    }
}

fn resolve_bump_label(localizer: &Localizer, index: i64, lines: i64) -> String {
    let mut args: Arguments<'_> = Arguments::default();
    args.insert(Cow::Borrowed("index"), FluentValue::from(index));
    args.insert(Cow::Borrowed("lines"), FluentValue::from(lines));

    let label = localizer
        .attribute_with_args(LINT_NAME, "label", &args)
        .unwrap_or_else(|_| format!("Complexity bump {index} spans {lines} lines."));

    label
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}' | '\u{FFFD}'))
        .collect()
}

fn fallback_messages(
    name: &str,
    count: usize,
    threshold: f64,
) -> common::i18n::DiagnosticMessageSet {
    common::i18n::DiagnosticMessageSet::new(
        format!("Multiple clusters of nested conditional logic in `{name}`."),
        format!("Detected {count} complexity bumps above the threshold {threshold}."),
        String::from(
            "Extract helper functions from the highlighted regions to reduce clustered complexity.",
        ),
    )
}
