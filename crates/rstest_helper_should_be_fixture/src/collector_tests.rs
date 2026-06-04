//! Unit tests for passive rstest helper call-site collection.

use rustc_hir::def_id::{DefId, DefIndex};
use rustc_span::{BytePos, DUMMY_SP, FileName};
use whitaker_common::rstest::ArgFingerprint;

use super::{CallSiteCollector, CallSiteLocation, CallSiteRecord};

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

#[test]
fn collector_deduplicates_identical_callee_and_source_span() {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    let first = collector.record(
        record(callee),
        location("crate::helper", BytePos(10), BytePos(18)),
    );
    let second = collector.record(
        record(callee),
        location("crate::helper", BytePos(10), BytePos(18)),
    );

    assert!(first);
    assert!(!second);
    assert_eq!(collector.callee_count(), 1);
    assert_eq!(collector.record_count(), 1);
}

#[test]
fn collector_keeps_distinct_source_spans_for_same_callee() {
    let mut collector = CallSiteCollector::default();
    let callee = def_id(1);

    collector.record(
        record(callee),
        location("crate::helper", BytePos(10), BytePos(18)),
    );
    collector.record(
        record(callee),
        location("crate::helper", BytePos(20), BytePos(28)),
    );

    assert_eq!(collector.callee_count(), 1);
    assert_eq!(collector.record_count(), 2);
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
