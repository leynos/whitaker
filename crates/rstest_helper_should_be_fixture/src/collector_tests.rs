//! Unit tests for passive rstest helper call-site collection.

use rstest::rstest;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_span::{BytePos, DUMMY_SP, FileName};
use whitaker_common::rstest::ArgFingerprint;

use super::{CallSiteCollector, CallSiteLocation, CallSiteRecord};

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
#[case(BytePos(10), BytePos(18), false, 1)]
#[case(BytePos(20), BytePos(28), true, 2)]
fn collector_handles_second_call_for_same_callee(
    #[case] second_lo: BytePos,
    #[case] second_hi: BytePos,
    #[case] second_inserted: bool,
    #[case] expected_record_count: usize,
) {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    let first = collector.record(
        record(callee),
        location("crate::helper", BytePos(10), BytePos(18)),
    );
    let second = collector.record(
        record(callee),
        location("crate::helper", second_lo, second_hi),
    );

    assert!(first);
    assert_eq!(second, second_inserted);
    assert_eq!(collector.callee_count(), 1);
    assert_eq!(collector.record_count(), expected_record_count);
}

fn record(callee_def_id: DefId) -> CallSiteRecord {
    CallSiteRecord::new(
        callee_def_id,
        ArgFingerprint::default(),
        def_id(99),
        DUMMY_SP,
    )
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

fn collect_spans(spans: &[(u32, u32, u32)]) -> Vec<(String, usize)> {
    let mut collector = CallSiteCollector::default();
    for (callee_index, lo, hi) in spans.iter().copied() {
        let callee = format!("crate::helper_{callee_index}");
        collector.record(
            record(def_id(callee_index)),
            location(&callee, BytePos(lo), BytePos(hi)),
        );
    }

    collector
        .iter()
        .map(|(callee, records)| (callee.to_string(), records.len()))
        .collect()
}

fn span_specs() -> impl Strategy<Value = Vec<(u32, u32, u32)>> {
    proptest::collection::vec((1_u32..5, 1_u32..100, 101_u32..200), 1..24)
}
