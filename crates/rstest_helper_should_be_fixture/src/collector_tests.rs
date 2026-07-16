//! Unit tests for passive rstest helper call-site collection.
//!
//! This module focuses on the pure storage contract in `collector`: stable
//! callee ordering, source-span deduplication, and insertion-order
//! independence. Compiler-facing HIR lowering remains in the production
//! collector module, while these tests keep the record store cheap to exercise
//! without constructing a rustc lint context.

use rstest::rstest;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_span::{BytePos, DUMMY_SP, FileName, Span};
use whitaker_common::rstest::{ArgAtom, ArgFingerprint};

use super::{
    CallSiteCollector, CallSiteLocation, CallSiteRecord, literal_text_atom,
    should_skip_arg_for_unrecoverable_span,
};

use proptest::prelude::*;

#[test]
fn collector_iterates_callees_in_definition_path_order() {
    let mut collector = CallSiteCollector::default();

    collector.record(
        record(def_id(2)),
        location("crate::z_helper", BytePos(10), BytePos(18)),
    );
    collector.record(
        record(def_id(1)),
        location("crate::a_helper", BytePos(20), BytePos(28)),
    );

    let keys = collector
        .iter()
        .map(|(callee, _)| callee.to_string())
        .collect::<Vec<_>>();

    assert_eq!(keys, ["crate::a_helper", "crate::z_helper"]);
}

#[rstest]
#[case(10, 18, [true, false], 1)]
#[case(20, 28, [true, true], 2)]
fn collector_records_calls_by_source_span(
    #[case] lo2: u32,
    #[case] hi2: u32,
    #[case] expected_inserted: [bool; 2],
    #[case] expected_record_count: usize,
) {
    let (collector, inserted) = collect_two_calls(lo2, hi2);

    assert_eq!(inserted[0], expected_inserted[0]);
    assert_eq!(inserted[1], expected_inserted[1]);
    assert_eq!(collector.callee_count(), 1);
    assert_eq!(collector.record_count(), expected_record_count);
}

#[test]
fn literal_lowering_records_const_lit_atom() {
    assert_eq!(
        literal_text_atom("\"literal\"".to_string()),
        ArgAtom::const_lit("\"literal\""),
    );
}

#[test]
fn unrecoverable_argument_span_is_unsupported() {
    assert!(should_skip_arg_for_unrecoverable_span(DUMMY_SP));
}

fn collect_two_calls(lo2: u32, hi2: u32) -> (CallSiteCollector, [bool; 2]) {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    let inserted = [
        collector.record(
            record(callee),
            location("crate::helper", BytePos(10), BytePos(18)),
        ),
        collector.record(
            record(callee),
            location("crate::helper", BytePos(lo2), BytePos(hi2)),
        ),
    ];

    (collector, inserted)
}

fn record(callee_def_id: DefId) -> CallSiteRecord {
    record_at(callee_def_id, DUMMY_SP)
}

fn record_at(callee_def_id: DefId, span: Span) -> CallSiteRecord {
    CallSiteRecord::new(callee_def_id, ArgFingerprint::default(), def_id(99), span)
}

fn def_id(index: u32) -> DefId {
    DefId::local(DefIndex::from_u32(index))
}

fn location(callee: &str, lo: BytePos, hi: BytePos) -> CallSiteLocation {
    CallSiteLocation::new(callee.to_string(), source_file(), lo, hi)
}

fn source_file() -> FileName {
    FileName::Custom("src/lib.rs".to_string())
}

proptest! {
    #[test]
    fn collector_order_is_independent_of_insertion_order(mut spans in span_specs()) {
        spans.sort_unstable();
        spans.dedup();
        let reversed = spans.iter().copied().rev().collect::<Vec<_>>();

        let forward = collect_spans(&spans);
        let backward = collect_spans(&reversed);

        prop_assert_eq!(forward, backward);
    }
}

fn collect_spans(spans: &[(u32, u32, u32)]) -> Vec<(String, Vec<(u32, u32)>)> {
    let mut collector = CallSiteCollector::default();
    for (callee_index, lo, hi) in spans.iter().copied() {
        let callee = format!("crate::helper_{callee_index}");
        collector.record(
            record_at(
                def_id(callee_index),
                Span::with_root_ctxt(BytePos(lo), BytePos(hi)),
            ),
            location(&callee, BytePos(lo), BytePos(hi)),
        );
    }

    collector
        .iter()
        .map(|(callee, records)| {
            let spans = records
                .iter()
                .map(|record| (record.span.lo().0, record.span.hi().0))
                .collect();
            (callee.to_string(), spans)
        })
        .collect()
}

fn span_specs() -> impl Strategy<Value = Vec<(u32, u32, u32)>> {
    proptest::collection::vec((1_u32..5, 1_u32..100, 101_u32..200), 1..24)
}
