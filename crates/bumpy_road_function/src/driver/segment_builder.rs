//! Build per-line complexity segments for bump detection.
//!
//! The lint converts nested control-flow and predicate branching into weighted
//! line segments which are then rasterized into a per-line signal.

use std::ops::RangeInclusive;

use crate::analysis::Settings;
use rustc_hir as hir;
use rustc_hir::{BinOpKind, ExprKind, LoopSource, UnOp};
use rustc_lint::LateContext;
use rustc_span::source_map::SourceMap;
use rustc_span::{DesugaringKind, Span};
use whitaker_common::complexity_signal::LineSegment;

use super::numeric::predicate_segment_weight;

/// Describe a fixed or branch-scaled contribution to a line segment.
#[derive(Clone, Copy)]
enum SegmentWeight {
    /// Use the supplied value unchanged.
    Fixed(f64),
    /// Scale by a branch count after clamping it to the supported range.
    Predicate { branches: usize, per_branch: f64 },
}

impl SegmentWeight {
    /// Convert the selected production weight to the value stored in a segment.
    fn value(self) -> f64 {
        match self {
            Self::Fixed(value) => value,
            Self::Predicate {
                branches,
                per_branch,
            } => predicate_segment_weight(branches, per_branch),
        }
    }
}

/// Construct the line segment after applying the production weight policy.
///
/// # Errors
///
/// Returns an error when the inclusive line range is not one-based and ordered.
fn line_segment(
    lines: RangeInclusive<usize>,
    weight: SegmentWeight,
) -> Result<LineSegment, whitaker_common::complexity_signal::SegmentError> {
    LineSegment::new(*lines.start(), *lines.end(), weight.value())
}

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

    /// Add the configured nesting-depth weight for `span`.
    fn push_depth_segment(&mut self, span: Span) {
        self.push_segment(span, SegmentWeight::Fixed(self.settings.weights.depth));
    }

    /// Add the configured control-flow weight for `span`.
    fn push_flow_segment(&mut self, span: Span) {
        self.push_segment(span, SegmentWeight::Fixed(self.settings.weights.flow));
    }

    /// Add a predicate-weighted segment unless the condition is a `let` test.
    ///
    /// Branch counts above `u32::MAX` are saturated before conversion to the
    /// floating-point weight; spans rejected by `push_segment` are omitted.
    fn push_predicate_segment(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        if matches!(expr.kind, ExprKind::Let(..)) {
            return;
        }

        self.push_segment(
            expr.span,
            SegmentWeight::Predicate {
                branches: count_branches(expr),
                per_branch: self.settings.weights.predicate,
            },
        );
    }

    /// Add a weighted segment when `span` maps to lines within this function.
    ///
    /// Ignore macro-expanded or unmappable spans; report an internal delayed
    /// bug for invalid segments or lines outside the function range.
    fn push_segment(&mut self, span: Span, weight: SegmentWeight) {
        // Skip spans from macro expansions; their line numbers can point to
        // locations outside the function (e.g., the macro definition site).
        if span.from_expansion() {
            return;
        }

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

        let segment = match line_segment(lines, weight) {
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

/// Count logical branches, treating `&&` and `||` as sums of their operands.
///
/// Negation, blocks, and `if` expressions are transparent; every other
/// expression contributes one branch.
fn count_branches(expr: &hir::Expr<'_>) -> usize {
    match expr.kind {
        ExprKind::Binary(op, lhs, rhs) if matches!(op.node, BinOpKind::And | BinOpKind::Or) => {
            count_branches(lhs) + count_branches(rhs)
        }
        ExprKind::Unary(UnOp::Not, inner) | ExprKind::DropTemps(inner) => count_branches(inner),
        ExprKind::Block(block, _) => block.expr.map_or(1, count_branches),
        ExprKind::If(cond, ..) => count_branches(cond),
        _ => 1,
    }
}

/// Return the contiguous 1-based source-line range covered by a compiler span.
///
/// Returns `None` when the span cannot be mapped to source lines or maps to a
/// non-contiguous set of lines.
pub(super) fn span_line_range(source_map: &SourceMap, span: Span) -> Option<RangeInclusive<usize>> {
    let info = source_map.span_to_lines(span).ok()?;
    let first = info.lines.first()?;
    let last = info.lines.last()?;

    let contiguous = info
        .lines
        .iter()
        .zip(info.lines.iter().skip(1))
        .all(|(previous, next)| next.line_index == previous.line_index + 1);
    if !contiguous {
        return None;
    }

    Some((first.line_index + 1)..=(last.line_index + 1))
}

#[cfg(test)]
mod tests {
    //! Verify the production segment builder applies bounded predicate weights.

    use super::{SegmentWeight, line_segment};

    /// Verify oversized branch counts saturate in the constructed line segment.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn predicate_segment_saturates_oversized_branch_counts() {
        let oversized =
            usize::try_from(u64::from(u32::MAX) + 1).expect("oversized branch count fits usize");

        let segment = line_segment(
            1..=1,
            SegmentWeight::Predicate {
                branches: oversized,
                per_branch: 0.5,
            },
        )
        .expect("one-based line range should build");

        assert_eq!(segment.value(), f64::from(u32::MAX) * 0.5);
    }
}
