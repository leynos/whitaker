//! HIR traversal and parameter lowering for rstest helper call-site collection.
//!
//! This module keeps rustc HIR mechanics separate from the lint-pass bootstrap
//! so the driver remains focused on configuration and crate-level lifecycle.

use crate::collector::{
    CallSiteCollector, CallSiteLocation, CallSiteRecord, lower_arg_atom, resolve_local_callee,
};
use log::debug;
use rustc_ast::AttrStyle;
use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_lint::LateContext;
use rustc_span::Span;
use std::collections::HashSet;
use whitaker_common::attributes::{Attribute, AttributeKind, AttributePath};
use whitaker_common::rstest::{
    ArgAtom, ArgFingerprint, ParameterBinding, RstestDetectionOptions, RstestParameter,
    RstestParameterKind, classify_rstest_parameter,
};

const LINT_NAME: &str = "rstest_helper_should_be_fixture";

pub(crate) struct CallSiteVisitor<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    collector: &'a mut CallSiteCollector,
    test_source_def_id: DefId,
    fixture_local_ids: &'a HashSet<hir::HirId>,
    closure_span_fallbacks: Vec<Span>,
}

impl<'a, 'tcx> CallSiteVisitor<'a, 'tcx> {
    pub(crate) fn new(
        cx: &'a LateContext<'tcx>,
        collector: &'a mut CallSiteCollector,
        test_source_def_id: DefId,
        fixture_local_ids: &'a HashSet<hir::HirId>,
    ) -> Self {
        Self {
            cx,
            collector,
            test_source_def_id,
            fixture_local_ids,
            closure_span_fallbacks: Vec::new(),
        }
    }

    fn collect_call<I>(&mut self, expr: &'tcx hir::Expr<'tcx>, args: I)
    where
        I: IntoIterator<Item = &'tcx hir::Expr<'tcx>>,
    {
        let Some(span) = self.recover_call_span(expr.span) else {
            debug!(
                target: LINT_NAME,
                "skipping helper call-site collection: user-editable span recovery failed for {:?}",
                expr.span,
            );
            return;
        };
        let Some(callee_def_id) = resolve_local_callee(self.cx, expr) else {
            debug!(
                target: LINT_NAME,
                "skipping helper call-site collection: local callee resolution failed for {:?}",
                expr.span,
            );
            return;
        };

        let fingerprint = ArgFingerprint::new(
            args.into_iter()
                .map(|arg| lower_arg_atom(self.cx, arg, self.fixture_local_ids)),
        );
        let record = CallSiteRecord::new(callee_def_id, fingerprint, self.test_source_def_id, span);
        let source_map = self.cx.tcx.sess.source_map();
        self.collector.record(
            record,
            CallSiteLocation::new(
                self.cx.tcx.def_path_str(callee_def_id),
                source_map.span_to_filename(span),
                span,
                expr.hir_id.local_id,
            ),
        );
    }
}

impl<'tcx> Visitor<'tcx> for CallSiteVisitor<'_, 'tcx> {
    fn visit_nested_body(&mut self, body_id: hir::BodyId) {
        self.visit_body(self.cx.tcx.hir_body(body_id));
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            hir::ExprKind::Call(_, args) => self.collect_call(expr, args),
            hir::ExprKind::MethodCall(_, receiver, args, _) => {
                self.collect_call(expr, std::iter::once(receiver).chain(args))
            }
            hir::ExprKind::Closure(hir::Closure { .. }) => {
                self.closure_span_fallbacks.push(expr.span);
                intravisit::walk_expr(self, expr);
                self.closure_span_fallbacks.pop();
                return;
            }
            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}

impl CallSiteVisitor<'_, '_> {
    fn recover_call_span(&self, span: Span) -> Option<Span> {
        whitaker::hir::recover_user_editable_hir_span(span).or_else(|| {
            self.closure_span_fallbacks
                .last()
                .and_then(|span| whitaker::hir::recover_user_editable_hir_span(*span))
        })
    }
}

pub(crate) fn rstest_parameters(
    cx: &LateContext<'_>,
    body: &hir::Body<'_>,
) -> Vec<RstestParameter> {
    body.params
        .iter()
        .map(|param| {
            RstestParameter::new(
                parameter_binding(param.pat),
                parameter_attributes(cx, param.hir_id),
            )
        })
        .collect()
}

pub(crate) fn fixture_local_ids(
    cx: &LateContext<'_>,
    body: &hir::Body<'_>,
    options: &RstestDetectionOptions,
) -> HashSet<hir::HirId> {
    let parameters = rstest_parameters(cx, body);
    body.params
        .iter()
        .zip(parameters.iter())
        .filter_map(|(param, parameter)| {
            matches!(
                classify_rstest_parameter(parameter, options),
                RstestParameterKind::FixtureLocal { .. }
            )
            .then_some(param.pat.hir_id)
        })
        .collect()
}

fn parameter_binding(pat: &hir::Pat<'_>) -> ParameterBinding {
    match pat.kind {
        hir::PatKind::Binding(_, _, ident, None) => ParameterBinding::Ident(ident.to_string()),
        _ => ParameterBinding::Unsupported,
    }
}

fn parameter_attributes(cx: &LateContext<'_>, hir_id: hir::HirId) -> Vec<Attribute> {
    cx.tcx
        .hir_attrs(hir_id)
        .iter()
        .filter_map(attribute_from_hir)
        .collect()
}

pub(crate) fn attribute_from_hir(attr: &hir::Attribute) -> Option<Attribute> {
    Some(Attribute::new(attribute_path(attr)?, attribute_kind(attr)))
}

fn attribute_path(attr: &hir::Attribute) -> Option<AttributePath> {
    let hir::Attribute::Unparsed(_) = attr else {
        return None;
    };

    let mut names = attr.path().into_iter().map(|symbol| symbol.to_string());
    let first = names.next()?;
    Some(AttributePath::new(std::iter::once(first).chain(names)))
}

fn attribute_kind(attr: &hir::Attribute) -> AttributeKind {
    match attribute_style(attr) {
        AttrStyle::Inner => AttributeKind::Inner,
        AttrStyle::Outer => AttributeKind::Outer,
    }
}

fn attribute_style(attr: &hir::Attribute) -> AttrStyle {
    let hir::Attribute::Unparsed(item) = attr else {
        unreachable!("attribute_path filters parsed attributes");
    };
    item.style
}

pub(crate) fn redacted_fingerprint_shape(fingerprint: &ArgFingerprint) -> String {
    // Keep observability useful for tests and debugging without writing
    // literal values or source snippets to logs.
    fingerprint
        .atoms()
        .iter()
        .map(redacted_atom_shape)
        .collect::<Vec<_>>()
        .join(",")
}

fn redacted_atom_shape(atom: &ArgAtom) -> &'static str {
    match atom {
        ArgAtom::FixtureLocal { .. } => "fixture-local",
        ArgAtom::ConstLit { .. } => "const-lit",
        ArgAtom::ConstPath { .. } => "const-path",
        ArgAtom::Unsupported => "unsupported",
    }
}
