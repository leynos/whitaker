//! Call-site collection for helper invocations inside `rstest` tests.
//!
//! The collector stores passive evidence only. It deliberately avoids
//! diagnostics so later roadmap items can apply thresholds and message policy
//! without changing how call sites are discovered.

use std::collections::{BTreeMap, BTreeSet};

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
        }
    }
}

/// Deterministic store of helper call-site evidence keyed by callee.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallSiteCollector {
    by_callee: BTreeMap<String, Vec<CallSiteRecord>>,
    seen: BTreeSet<DedupKey>,
}

/// Source location used to key deduplicated call-site evidence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CallSiteLocation {
    callee_key: String,
    file_name: FileName,
    lo: BytePos,
    hi: BytePos,
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
    ///     rustc_span::BytePos(0),
    ///     rustc_span::BytePos(4),
    /// );
    /// ```
    pub(crate) const fn new(
        callee_key: String,
        file_name: FileName,
        lo: BytePos,
        hi: BytePos,
    ) -> Self {
        Self {
            callee_key,
            file_name,
            lo,
            hi,
        }
    }

    fn callee_key(&self) -> &str {
        &self.callee_key
    }
}

impl CallSiteCollector {
    /// Records one helper call, returning whether it was newly inserted.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use crate::collector::{CallSiteCollector, CallSiteRecord};
    /// # fn example(mut collector: CallSiteCollector, record: CallSiteRecord) {
    /// let inserted = collector.record(record, "src/lib.rs".into(), 0.into(), 4.into());
    /// assert!(inserted);
    /// # }
    /// ```
    pub(crate) fn record(&mut self, record: CallSiteRecord, location: CallSiteLocation) -> bool {
        let key = DedupKey::from_location(&location);
        if !self.seen.insert(key) {
            return false;
        }

        self.by_callee
            .entry(location.callee_key().to_string())
            .or_default()
            .push(record);
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DedupKey {
    callee_key: String,
    file_name: FileName,
    lo: BytePos,
    hi: BytePos,
}

impl DedupKey {
    fn from_location(location: &CallSiteLocation) -> Self {
        Self {
            callee_key: location.callee_key.clone(),
            file_name: location.file_name.clone(),
            lo: location.lo,
            hi: location.hi,
        }
    }
}

/// Lowers a HIR argument expression to the pure fingerprint model.
#[must_use]
pub(crate) fn lower_arg_atom<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
    fixture_locals: &BTreeSet<String>,
) -> ArgAtom {
    if whitaker::hir::recover_user_editable_hir_span(expr.span).is_none() {
        return ArgAtom::unsupported();
    }

    match &expr.kind {
        hir::ExprKind::Path(qpath) => lower_path_arg(cx, expr, qpath, fixture_locals),
        hir::ExprKind::Lit(lit) => literal_atom(cx, expr.span, lit),
        _ => ArgAtom::unsupported(),
    }
}

fn lower_path_arg<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx hir::Expr<'tcx>,
    qpath: &hir::QPath<'tcx>,
    fixture_locals: &BTreeSet<String>,
) -> ArgAtom {
    match cx.qpath_res(qpath, expr.hir_id) {
        Res::Local(_) => local_fixture_atom(qpath, fixture_locals),
        Res::Def(DefKind::Const | DefKind::AssocConst | DefKind::Static { .. }, def_id) => {
            ArgAtom::const_path(cx.tcx.def_path_str(def_id))
        }
        _ => ArgAtom::unsupported(),
    }
}

fn local_fixture_atom(qpath: &hir::QPath<'_>, fixture_locals: &BTreeSet<String>) -> ArgAtom {
    let hir::QPath::Resolved(None, path) = qpath else {
        return ArgAtom::unsupported();
    };
    let Some(segment) = path.segments.first() else {
        return ArgAtom::unsupported();
    };
    let name = segment.ident.as_str();
    if fixture_locals.contains(name) {
        ArgAtom::fixture_local(name)
    } else {
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
        _ => None,
    }?;

    is_local_function(cx, def_id).then_some(def_id)
}

fn resolve_direct_call<'tcx>(
    cx: &LateContext<'tcx>,
    callee: &'tcx hir::Expr<'tcx>,
) -> Option<DefId> {
    let hir::ExprKind::Path(qpath) = &callee.kind else {
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
