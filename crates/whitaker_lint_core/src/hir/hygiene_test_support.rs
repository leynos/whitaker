//! Synthetic hygiene contexts shared by HIR adapter tests.
//!
//! These helpers create expansion marks without a compiler session. They model
//! the mark chains rustc records, so tests can cover every mark kind and
//! parent context; real lowering is covered separately by compiler fixtures.

use rustc_data_structures::stable_hash::{
    RawDefId,
    RawDefPathHash,
    RawSpan,
    StableHashControls,
    StableHashCtxt,
    StableHasher,
};
use rustc_span::{
    BytePos,
    Span,
    SyntaxContext,
    edition::Edition,
    hygiene::{ExpnData, ExpnKind, LocalExpnId, MacroKind, Transparency},
    sym,
};

/// Create a span with a root syntax context.
pub(super) fn test_span(lo: u32, hi: u32) -> Span { Span::with_root_ctxt(BytePos(lo), BytePos(hi)) }

#[derive(Clone, Copy)]
struct TestHashStableContext;

impl StableHashCtxt for TestHashStableContext {
    fn stable_hash_span(&mut self, _span: RawSpan, _hasher: &mut StableHasher) {}

    fn def_path_hash(&self, _def_id: RawDefId) -> RawDefPathHash { RawDefPathHash([0; 16]) }

    fn stable_hash_controls(&self) -> StableHashControls {
        StableHashControls { hash_spans: false }
    }

    fn assert_default_stable_hash_controls(&self, _msg: &str) {}
}

/// Apply `kind` as the new outermost transparent mark on `ctxt`.
pub(super) fn mark(ctxt: SyntaxContext, kind: ExpnKind, call_site: Span) -> SyntaxContext {
    let expn_id = LocalExpnId::fresh_empty();
    expn_id.set_expn_data(
        ExpnData::default(kind, call_site, Edition::Edition2024, None, None),
        TestHashStableContext,
    );

    ctxt.apply_mark(expn_id.to_expn_id(), Transparency::Transparent)
}

/// Build a span carrying a single bang-macro mark over the root context.
pub(super) fn expanded_span(span: Span, call_site: Span) -> Span {
    span.with_ctxt(mark(
        SyntaxContext::root(),
        ExpnKind::Macro(MacroKind::Bang, sym::include),
        call_site,
    ))
}
