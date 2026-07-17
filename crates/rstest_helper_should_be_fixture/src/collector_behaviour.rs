//! Behaviour-driven tests for call-site collector state transitions.
//!
//! These scenarios describe how the `collector` module behaves when macro
//! expansion generates duplicate source call sites versus genuinely distinct
//! helper invocations. They complement the lower-level unit and property tests
//! by naming the externally useful behaviour that later thresholding code will
//! consume from the passive evidence store.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_span::{BytePos, DUMMY_SP, FileName};
use whitaker_common::rstest::{ArgAtom, ArgFingerprint};

use super::{CallSiteCollector, CallSiteLocation, CallSiteRecord};

#[derive(Default)]
struct CollectorWorld {
    collector: CallSiteCollector,
    inserted: Vec<bool>,
}

#[fixture]
fn world() -> CollectorWorld {
    CollectorWorld::default()
}

#[given("two generated rstest cases share the same source helper call")]
fn given_same_source_call(world: &mut CollectorWorld) {
    insert_two_calls(world, 10, 18);
}

#[given("two helper calls use different source spans")]
fn given_distinct_source_calls(world: &mut CollectorWorld) {
    insert_two_calls(world, 20, 28);
}

fn insert_two_calls(world: &mut CollectorWorld, lo2: u32, hi2: u32) {
    world.inserted.push(world.collector.record(
        record(def_id(1), ArgAtom::fixture_local("fixture")),
        location("crate::helper", 10, 18),
    ));
    world.inserted.push(world.collector.record(
        record(def_id(1), ArgAtom::fixture_local("fixture")),
        location("crate::helper", lo2, hi2),
    ));
}

#[when("the collector stores the call-site evidence")]
fn when_collector_stores_evidence() {}

#[then("one deduplicated record is retained")]
fn then_one_record(world: &CollectorWorld) {
    assert_eq!(world.inserted, [true, false]);
    assert_eq!(world.collector.callee_count(), 1);
    assert_eq!(world.collector.record_count(), 1);
}

#[then("both source records are retained")]
fn then_two_records(world: &CollectorWorld) {
    assert_eq!(world.inserted, [true, true]);
    assert_eq!(world.collector.callee_count(), 1);
    assert_eq!(world.collector.record_count(), 2);
}

#[scenario(path = "tests/features/collection.feature", index = 0)]
fn scenario_generated_cases_deduplicate(world: CollectorWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/collection.feature", index = 1)]
fn scenario_distinct_calls_remain_distinct(world: CollectorWorld) {
    let _ = world;
}

fn record(callee_def_id: DefId, atom: ArgAtom) -> CallSiteRecord {
    CallSiteRecord::new(
        callee_def_id,
        ArgFingerprint::new([atom]),
        def_id(99),
        DUMMY_SP,
    )
}

fn def_id(index: u32) -> DefId {
    DefId::local(DefIndex::from_u32(index))
}

fn location(callee: &str, lo: u32, hi: u32) -> CallSiteLocation {
    CallSiteLocation::new(
        callee.to_string(),
        FileName::Custom("src/lib.rs".to_string()),
        rustc_span::Span::with_root_ctxt(BytePos(lo), BytePos(hi)),
        rustc_hir::ItemLocalId::ZERO,
    )
}
