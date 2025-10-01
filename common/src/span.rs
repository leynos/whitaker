use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty::TyCtxt;
use rustc_span::{Span, Symbol};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[must_use]
pub fn span_to_line_range<'tcx>(cx: &LateContext<'tcx>, span: Span) -> Option<LineRange> {
    let sm = cx.tcx.sess.source_map();
    sm.span_to_lines(span).ok().and_then(|info| {
        let start = info.lines.first()?.line_index + 1;
        let end = info.lines.last()?.line_index + 1;
        Some(LineRange { start, end })
    })
}

#[must_use]
pub fn module_line_count<'tcx>(cx: &LateContext<'tcx>, span: Span) -> Option<usize> {
    span_to_line_range(cx, span).map(|range| range.end.saturating_sub(range.start) + 1)
}

#[must_use]
pub fn def_id_of_expr_callee<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<rustc_hir::def_id::DefId> {
    match expr.kind {
        ExprKind::Call(callee, _) => match &callee.kind {
            ExprKind::Path(qpath) => cx.qpath_res(qpath, callee.hir_id).opt_def_id(),
            _ => None,
        },
        ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
        _ => None,
    }
}

#[must_use]
pub fn is_path_to(expr: &Expr<'_>, segments: &[&str]) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => {
            path.segments.len() == segments.len()
                && path
                    .segments
                    .iter()
                    .zip(segments.iter().copied())
                    .all(|(segment, expected)| segment.ident.name == Symbol::intern(expected))
        }
        ExprKind::Path(QPath::TypeRelative(_, segment)) => {
            segments.len() == 1 && segment.ident.name == Symbol::intern(segments[0])
        }
        _ => false,
    }
}

#[must_use]
pub fn tcx_from_late_context<'tcx>(cx: &LateContext<'tcx>) -> TyCtxt<'tcx> {
    cx.tcx
}
