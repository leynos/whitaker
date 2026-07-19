//! Call-site collection for helper invocations inside `rstest` tests.
//!
//! The collector stores passive evidence only. It deliberately avoids
//! diagnostics so later roadmap items can apply thresholds and message policy
//! without changing how call sites are discovered.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use log::debug;
use rustc_hir as hir;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_lint::LateContext;
use rustc_span::{BytePos, FileName, Span};
use whitaker_common::rstest::{ArgAtom, ArgFingerprint};

/// Evidence for one collected helper call inside an `rstest` test.
#[derive(Clone, Debug)]
pub(crate) struct CallSiteRecord {
    pub(crate) callee_def_id: DefId,
    pub(crate) fingerprint: ArgFingerprint,
    pub(crate) test_source_def_id: DefId,
    pub(crate) span: Span,
    pub(crate) hir_local_id: hir::ItemLocalId,
}

impl CallSiteRecord {
    /// Builds a call-site record for a local helper invocation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use rustc_span::DUMMY_SP;
    /// # use whitaker_common::rstest::ArgFingerprint;
    /// # use crate::collector::CallSiteRecord;
    /// # fn example(callee: rustc_hir::def_id::DefId, test: rustc_hir::def_id::DefId) {
    /// let record = CallSiteRecord::new(callee, ArgFingerprint::default(), test, DUMMY_SP);
    /// assert_eq!(record.callee_def_id, callee);
    /// # }
    /// ```
    pub(crate) fn new(
        callee_def_id: DefId,
        fingerprint: ArgFingerprint,
        test_source_def_id: DefId,
        span: Span,
    ) -> Self {
        Self {
            callee_def_id,
            fingerprint,
            test_source_def_id,
            span,
            hir_local_id: hir::ItemLocalId::ZERO,
        }
    }
}

/// Deterministic store of helper call-site evidence keyed by callee.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallSiteCollector {
    by_callee: BTreeMap<String, Vec<CallSiteRecord>>,
    seen: BTreeSet<CallSiteLocation>,
}

/// Source location used to key deduplicated call-site evidence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CallSiteLocation {
    callee_key: String,
    file_name: FileName,
    lo: BytePos,
    hi: BytePos,
    hir_local_id: hir::ItemLocalId,
}

impl CallSiteLocation {
    /// Builds a deduplication location for one helper call.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use crate::collector::CallSiteLocation;
    /// let location = CallSiteLocation::new(
    ///     "crate::helper".to_string(),
    ///     rustc_span::FileName::Custom("src/lib.rs".to_string()),
    ///     rustc_span::Span::with_root_ctxt(
    ///         rustc_span::BytePos(0),
    ///         rustc_span::BytePos(4),
    ///     ),
    ///     rustc_hir::ItemLocalId::ZERO,
    /// );
    /// ```
    pub(crate) fn new(
        callee_key: String,
        file_name: FileName,
        span: Span,
        hir_local_id: hir::ItemLocalId,
    ) -> Self {
        Self {
            callee_key,
            file_name,
            lo: span.lo(),
            hi: span.hi(),
            hir_local_id,
        }
    }
}

impl CallSiteCollector {
    /// Records one helper call, returning whether it was newly inserted.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use crate::collector::{CallSiteCollector, CallSiteLocation, CallSiteRecord};
    /// # fn example(mut collector: CallSiteCollector, record: CallSiteRecord) {
    /// let location = CallSiteLocation::new(
    ///     "crate::helper".to_string(),
    ///     rustc_span::FileName::Custom("src/lib.rs".to_string()),
    ///     rustc_span::Span::with_root_ctxt(
    ///         rustc_span::BytePos(0),
    ///         rustc_span::BytePos(4),
    ///     ),
    ///     rustc_hir::ItemLocalId::ZERO,
    /// );
    /// let inserted = collector.record(record, location);
    /// assert!(inserted);
    /// # }
    /// ```
    pub(crate) fn record(
        &mut self,
        mut record: CallSiteRecord,
        location: CallSiteLocation,
    ) -> bool {
        let callee_key = location.callee_key.clone();
        let lo = location.lo;
        let hi = location.hi;
        record.hir_local_id = location.hir_local_id;
        if !self.seen.insert(location) {
            debug!(
                target: "rstest_helper_should_be_fixture",
                "dropping duplicate rstest helper call-site evidence: callee={}, lo={:?}, hi={:?}",
                callee_key,
                lo,
                hi,
            );
            return false;
        }

        let records = self.by_callee.entry(callee_key).or_default();
        // Preserve iteration order without re-sorting the complete callee bucket.
        let insertion_index = records.partition_point(|existing| {
            (
                existing.span.lo(),
                existing.span.hi(),
                existing.hir_local_id,
            ) < (record.span.lo(), record.span.hi(), record.hir_local_id)
        });
        records.insert(insertion_index, record);
        true
    }

    /// Returns collected records in deterministic callee order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &[CallSiteRecord])> {
        self.by_callee
            .iter()
            .map(|(callee, records)| (callee.as_str(), records.as_slice()))
    }

    /// Returns the number of distinct callees with collected evidence.
    pub(crate) fn callee_count(&self) -> usize {
        self.by_callee.len()
    }

    /// Returns the number of deduplicated call-site records.
    pub(crate) fn record_count(&self) -> usize {
        self.by_callee.values().map(Vec::len).sum()
    }

    /// Removes all stored evidence from the collector.
    pub(crate) fn clear(&mut self) {
        self.by_callee.clear();
        self.seen.clear();
    }
}

/// Lowers a HIR argument expression to the pure fingerprint model.
#[must_use]
pub(crate) fn lower_arg_atom<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
    fixture_local_ids: &HashSet<hir::HirId>,
) -> ArgAtom {
    if should_skip_arg_for_unrecoverable_span(expr.span) {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "lowering unsupported argument: user-editable span recovery failed for {:?}",
            expr.span,
        );
        return ArgAtom::unsupported();
    }

    match &expr.kind {
        hir::ExprKind::Path(qpath) => lower_path_arg(cx, expr, qpath, fixture_local_ids),
        hir::ExprKind::Lit(lit) => literal_atom(cx, expr.span, lit),
        _ => {
            debug!(
                target: "rstest_helper_should_be_fixture",
                "lowering unsupported argument expression at {:?}",
                expr.span,
            );
            ArgAtom::unsupported()
        }
    }
}

fn should_skip_arg_for_unrecoverable_span(span: Span) -> bool {
    whitaker::hir::recover_user_editable_hir_span(span).is_none()
}

fn lower_path_arg<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
    qpath: &hir::QPath<'tcx>,
    fixture_local_ids: &HashSet<hir::HirId>,
) -> ArgAtom {
    match cx.qpath_res(qpath, expr.hir_id) {
        Res::Local(binding_id) => local_fixture_atom(qpath, binding_id, fixture_local_ids),
        Res::Def(
            DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. },
            def_id,
        ) => ArgAtom::const_path(cx.tcx.def_path_str(def_id)),
        _ => {
            debug!(
                target: "rstest_helper_should_be_fixture",
                "lowering unsupported path argument at {:?}",
                expr.span,
            );
            ArgAtom::unsupported()
        }
    }
}

fn local_fixture_atom(
    qpath: &hir::QPath<'_>,
    binding_id: hir::HirId,
    fixture_local_ids: &HashSet<hir::HirId>,
) -> ArgAtom {
    let hir::QPath::Resolved(None, path) = qpath else {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "lowering unsupported local argument: qualified path is not a plain resolved path",
        );
        return ArgAtom::unsupported();
    };
    let Some(segment) = path.segments.first() else {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "lowering unsupported local argument: resolved path has no first segment",
        );
        return ArgAtom::unsupported();
    };
    let name = segment.ident.as_str();
    if fixture_local_ids.contains(&binding_id) {
        ArgAtom::fixture_local(name)
    } else {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "lowering unsupported local argument: `{}` is not an rstest fixture local",
            name,
        );
        ArgAtom::unsupported()
    }
}

fn literal_atom(cx: &LateContext<'_>, span: Span, lit: &hir::Lit) -> ArgAtom {
    let text = cx
        .tcx
        .sess
        .source_map()
        .span_to_snippet(span)
        .unwrap_or_else(|_| lit.node.to_string());
    literal_text_atom(text)
}

fn literal_text_atom(text: String) -> ArgAtom {
    ArgAtom::const_lit(text)
}

/// Resolves a helper call expression to a local function definition.
#[must_use]
pub(crate) fn resolve_local_callee<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
) -> Option<DefId> {
    let def_id = match &expr.kind {
        hir::ExprKind::Call(callee, _) => resolve_direct_call(cx, callee),
        hir::ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id),
        _ => {
            debug!(
                target: "rstest_helper_should_be_fixture",
                "callee resolution skipped for non-call expression at {:?}",
                expr.span,
            );
            None
        }
    }?;

    if is_local_function(cx, def_id) {
        Some(def_id)
    } else {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "callee resolution skipped non-local or non-function callee: {:?}",
            def_id,
        );
        None
    }
}

fn resolve_direct_call<'tcx>(
    cx: &LateContext<'tcx>,
    callee: &'tcx hir::Expr<'tcx>,
) -> Option<DefId> {
    let hir::ExprKind::Path(qpath) = &callee.kind else {
        debug!(
            target: "rstest_helper_should_be_fixture",
            "direct call callee resolution failed: callee expression is not a path at {:?}",
            callee.span,
        );
        return None;
    };
    cx.qpath_res(qpath, callee.hir_id).opt_def_id()
}

fn is_local_function(cx: &LateContext<'_>, def_id: DefId) -> bool {
    def_id.krate == LOCAL_CRATE && matches!(cx.tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn)
}

#[cfg(test)]
#[path = "collector_tests.rs"]
mod tests;
