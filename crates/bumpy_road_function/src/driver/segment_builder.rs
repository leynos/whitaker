//! Build per-line complexity segments for bump detection.
//!
//! The lint converts nested control-flow and predicate branching into weighted
//! line segments which are then rasterised into a per-line signal.

use std::ops::RangeInclusive;

use crate::analysis::Settings;
use common::complexity_signal::LineSegment;
use rustc_hir as hir;
use rustc_hir::{BinOpKind, ExprKind, LoopSource, UnOp};
use rustc_lint::LateContext;
use rustc_span::source_map::SourceMap;
use rustc_span::{DesugaringKind, Span};

pub(super) struct SegmentBuilder<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    settings: &'a Settings,
    function_lines: RangeInclusive<usize>,
    segments: &'a mut Vec<LineSegment>,
}

impl<'a, 'tcx> SegmentBuilder<'a, 'tcx> {
    pub(super) fn new(
        cx: &'a LateContext<'tcx>,
        settings: &'a Settings,
        function_lines: RangeInclusive<usize>,
        segments: &'a mut Vec<LineSegment>,
    ) -> Self {
        Self {
            cx,
            settings,
            function_lines,
            segments,
        }
    }

    pub(super) fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
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

pub(super) fn span_line_range(source_map: &SourceMap, span: Span) -> Option<RangeInclusive<usize>> {
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
