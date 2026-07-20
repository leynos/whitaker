//! Unit tests for passive rstest helper call-site collection.
//!
//! This module focuses on the pure storage contract in `collector`: stable
//! callee ordering, source-span deduplication, and insertion-order
//! independence. Compiler-facing HIR lowering remains in the production
//! collector module, while these tests keep the record store cheap to exercise
//! without constructing a rustc lint context.

use rstest::rstest;
use rustc_hir::ItemLocalId;
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
fn collector_keeps_macro_only_calls_with_distinct_hir_ids() {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    let inserted = [
        collector.record(
            record(callee),
            location_with_hir_id("crate::helper", BytePos(10), BytePos(18), 1),
        ),
        collector.record(
            record(callee),
            location_with_hir_id("crate::helper", BytePos(10), BytePos(18), 2),
        ),
    ];

    assert_eq!(inserted, [true, true]);
    assert_eq!(collector.record_count(), 2);
}

#[test]
fn collector_orders_large_single_callee_bucket_by_span() {
    // Exercises the deferred `finalize` sort at scale: many call sites for a
    // single callee, inserted out of order, must still read back in ascending
    // span order without the per-insertion shifting the sorted store avoids.
    const CALL_COUNT: u32 = 4_096;
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    // Insert in descending span order so every record is maximally out of place.
    for index in (0..CALL_COUNT).rev() {
        let lo = index * 10;
        let inserted = collector.record(
            record_at(callee, Span::with_root_ctxt(BytePos(lo), BytePos(lo + 8))),
            location("crate::helper", BytePos(lo), BytePos(lo + 8)),
        );
        assert!(inserted);
    }
    collector.finalize();

    let ordered_los = collector
        .iter()
        .next()
        .map(|(_, records)| records.iter().map(|r| r.span.lo().0).collect::<Vec<_>>())
        .unwrap_or_default();

    let expected = (0..CALL_COUNT).map(|index| index * 10).collect::<Vec<_>>();
    assert_eq!(collector.record_count(), CALL_COUNT as usize);
    assert_eq!(ordered_los, expected);
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
    location_with_hir_id(callee, lo, hi, 0)
}

fn location_with_hir_id(
    callee: &str,
    lo: BytePos,
    hi: BytePos,
    hir_local_id: u32,
) -> CallSiteLocation {
    CallSiteLocation::new(
        callee.to_string(),
        source_file(),
        Span::with_root_ctxt(lo, hi),
        ItemLocalId::from_u32(hir_local_id),
    )
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

    #[test]
    fn collector_orders_equal_span_calls_by_hir_id(
        first_hir_id in 1_u32..100,
        second_hir_id in 1_u32..100,
        first_literal in 1_u32..100,
        second_literal in 1_u32..100,
    ) {
        prop_assume!(first_hir_id != second_hir_id);
        prop_assume!(first_literal != second_literal);

        let calls = [
            (first_hir_id, first_literal),
            (second_hir_id, second_literal),
        ];
        let forward = collect_equal_span_calls(&calls);
        let backward = collect_equal_span_calls(&[calls[1], calls[0]]);
        let mut expected = calls
            .iter()
            .map(|(hir_local_id, literal)| {
                (*hir_local_id, ArgFingerprint::new([ArgAtom::const_lit(literal.to_string())]))
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(|(hir_local_id, _)| *hir_local_id);

        prop_assert_eq!(&forward, &expected);
        prop_assert_eq!(&backward, &expected);
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
    collector.finalize();

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

fn collect_equal_span_calls(calls: &[(u32, u32)]) -> Vec<(u32, ArgFingerprint)> {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);
    let span = Span::with_root_ctxt(BytePos(10), BytePos(18));

    for (hir_local_id, literal) in calls {
        collector.record(
            CallSiteRecord::new(
                callee,
                ArgFingerprint::new([ArgAtom::const_lit(literal.to_string())]),
                def_id(99),
                span,
            ),
            location_with_hir_id("crate::helper", BytePos(10), BytePos(18), *hir_local_id),
        );
    }
    collector.finalize();

    collector
        .iter()
        .next()
        .map(|(_, records)| {
            records
                .iter()
                .map(|record| (record.hir_local_id.as_u32(), record.fingerprint.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn span_specs() -> impl Strategy<Value = Vec<(u32, u32, u32)>> {
    proptest::collection::vec((1_u32..5, 1_u32..100, 101_u32..200), 1..24)
}
