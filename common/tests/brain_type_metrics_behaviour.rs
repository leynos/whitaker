//! Behaviour-driven coverage for brain type metric collection.

use common::brain_type_metrics::{
    ForeignReferenceSet, MethodMetrics, TypeMetricsBuilder, brain_methods, foreign_reach_count,
    weighted_methods_count,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};

#[derive(Debug)]
struct MetricsWorld {
    methods: RefCell<Vec<MethodMetrics>>,
    cc_threshold: Cell<usize>,
    loc_threshold: Cell<usize>,
    lcom4: Cell<Option<usize>>,
    foreign_reach_value: Cell<Option<usize>>,
    wmc_result: Cell<Option<usize>>,
    brain_method_names: RefCell<Vec<String>>,
    foreign_refs: RefCell<ForeignReferenceSet>,
    raw_foreign_pairs: RefCell<Vec<(String, bool)>>,
    foreign_reach_result: Cell<Option<usize>>,
    type_metrics_wmc: Cell<Option<usize>>,
    type_metrics_brain_count: Cell<Option<usize>>,
    type_metrics_lcom4: Cell<Option<usize>>,
    type_metrics_foreign_reach: Cell<Option<usize>>,
}

impl Default for MetricsWorld {
    fn default() -> Self {
        Self {
            methods: RefCell::new(Vec::new()),
            cc_threshold: Cell::new(25),
            loc_threshold: Cell::new(80),
            lcom4: Cell::new(None),
            foreign_reach_value: Cell::new(None),
            wmc_result: Cell::new(None),
            brain_method_names: RefCell::new(Vec::new()),
            foreign_refs: RefCell::new(ForeignReferenceSet::new()),
            raw_foreign_pairs: RefCell::new(Vec::new()),
            foreign_reach_result: Cell::new(None),
            type_metrics_wmc: Cell::new(None),
            type_metrics_brain_count: Cell::new(None),
            type_metrics_lcom4: Cell::new(None),
            type_metrics_foreign_reach: Cell::new(None),
        }
    }
}

#[fixture]
fn world() -> MetricsWorld {
    MetricsWorld::default()
}

// --- Helpers ---

fn record_foreign_ref(world: &MetricsWorld, path: &str, is_from_expansion: bool) {
    world
        .foreign_refs
        .borrow_mut()
        .record_reference(path, is_from_expansion);
    world
        .raw_foreign_pairs
        .borrow_mut()
        .push((path.to_owned(), is_from_expansion));
}

fn assert_brain_count(world: &MetricsWorld, n: usize) {
    assert_eq!(world.type_metrics_brain_count.get(), Some(n));
}

// --- Given steps ---

#[given("a method called {name} with CC {cc} and LOC {loc}")]
fn given_method(world: &MetricsWorld, name: String, cc: usize, loc: usize) {
    world
        .methods
        .borrow_mut()
        .push(MethodMetrics::new(name, cc, loc));
}

#[given("the brain method CC threshold is {threshold}")]
fn given_cc_threshold(world: &MetricsWorld, threshold: usize) {
    world.cc_threshold.set(threshold);
}

#[given("the brain method LOC threshold is {threshold}")]
fn given_loc_threshold(world: &MetricsWorld, threshold: usize) {
    world.loc_threshold.set(threshold);
}

#[given("the LCOM4 value is {value}")]
fn given_lcom4(world: &MetricsWorld, value: usize) {
    world.lcom4.set(Some(value));
}

#[given("the foreign reach count is {count}")]
fn given_foreign_reach_count(world: &MetricsWorld, count: usize) {
    world.foreign_reach_value.set(Some(count));
}

#[given("a foreign reference to {path}")]
fn given_foreign_ref(world: &MetricsWorld, path: String) {
    record_foreign_ref(world, &path, false);
}

#[given("a foreign reference to {path} from expansion")]
fn given_foreign_ref_expanded(world: &MetricsWorld, path: String) {
    record_foreign_ref(world, &path, true);
}

#[given("a foreign reference to {path} not from expansion")]
fn given_foreign_ref_not_expanded(world: &MetricsWorld, path: String) {
    record_foreign_ref(world, &path, false);
}

// --- When steps ---

#[when("WMC is computed")]
fn when_compute_wmc(world: &MetricsWorld) {
    let methods = world.methods.borrow();
    world.wmc_result.set(Some(weighted_methods_count(&methods)));
}

#[when("brain methods are identified")]
fn when_identify_brain_methods(world: &MetricsWorld) {
    let methods = world.methods.borrow();
    let brains = brain_methods(
        &methods,
        world.cc_threshold.get(),
        world.loc_threshold.get(),
    );
    *world.brain_method_names.borrow_mut() = brains.iter().map(|m| m.name().to_owned()).collect();
}

#[when("type metrics are built for {name}")]
fn when_build_type_metrics(world: &MetricsWorld, name: String) {
    let mut builder =
        TypeMetricsBuilder::new(name, world.cc_threshold.get(), world.loc_threshold.get());
    for m in world.methods.borrow().iter() {
        builder.add_method(m.name(), m.cognitive_complexity(), m.lines_of_code());
    }
    if let Some(lcom4) = world.lcom4.get() {
        builder.set_lcom4(lcom4);
    }
    if let Some(fr) = world.foreign_reach_value.get() {
        builder.set_foreign_reach(fr);
    }
    let metrics = builder.build();
    world.type_metrics_wmc.set(Some(metrics.wmc()));
    world
        .type_metrics_brain_count
        .set(Some(metrics.brain_method_count()));
    world.type_metrics_lcom4.set(Some(metrics.lcom4()));
    world
        .type_metrics_foreign_reach
        .set(Some(metrics.foreign_reach()));
}

#[when("foreign reach is computed")]
fn when_compute_foreign_reach(world: &MetricsWorld) {
    let refs = world.foreign_refs.borrow();
    world.foreign_reach_result.set(Some(refs.count()));
}

#[when("foreign reach is computed using the convenience function")]
fn when_compute_foreign_reach_convenience(world: &MetricsWorld) {
    let pairs = world.raw_foreign_pairs.borrow().clone();
    world
        .foreign_reach_result
        .set(Some(foreign_reach_count(pairs)));
}

// --- Then steps ---

#[then("the WMC is {value}")]
fn then_wmc_is(world: &MetricsWorld, value: usize) {
    assert_eq!(world.wmc_result.get(), Some(value));
}

#[then("{name} is a brain method")]
fn then_is_brain_method(world: &MetricsWorld, name: String) {
    let names = world.brain_method_names.borrow();
    assert!(
        names.contains(&name),
        "expected '{name}' to be a brain method, found: {names:?}"
    );
}

#[then("there are no brain methods")]
fn then_no_brain_methods(world: &MetricsWorld) {
    let names = world.brain_method_names.borrow();
    assert!(
        names.is_empty(),
        "expected no brain methods, found: {names:?}"
    );
}

#[then("the type WMC is {value}")]
fn then_type_wmc(world: &MetricsWorld, value: usize) {
    assert_eq!(world.type_metrics_wmc.get(), Some(value));
}

#[then("the type has {n} brain method")]
fn then_type_brain_count_singular(world: &MetricsWorld, n: usize) {
    assert_brain_count(world, n);
}

#[then("the type has {n} brain methods")]
fn then_type_brain_count_plural(world: &MetricsWorld, n: usize) {
    assert_brain_count(world, n);
}

#[then("the type LCOM4 is {value}")]
fn then_type_lcom4(world: &MetricsWorld, value: usize) {
    assert_eq!(world.type_metrics_lcom4.get(), Some(value));
}

#[then("the type foreign reach is {count}")]
fn then_type_foreign_reach(world: &MetricsWorld, count: usize) {
    assert_eq!(world.type_metrics_foreign_reach.get(), Some(count));
}

#[then("the foreign reach is {count}")]
fn then_foreign_reach(world: &MetricsWorld, count: usize) {
    assert_eq!(world.foreign_reach_result.get(), Some(count));
}

// Scenario indices must match their declaration order in
// `tests/features/brain_type_metrics.feature`. Adding, removing, or
// reordering scenarios in the feature file requires updating the indices
// here.

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 0)]
fn scenario_wmc_sum(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 1)]
fn scenario_brain_method_qualifies(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 2)]
fn scenario_below_both_thresholds(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 3)]
fn scenario_only_cc_threshold(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 4)]
fn scenario_only_loc_threshold(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 5)]
fn scenario_empty_type_zero_wmc(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 6)]
fn scenario_type_metrics_aggregate(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 7)]
fn scenario_foreign_refs_deduplicated(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 8)]
fn scenario_macro_expanded_foreign_filtered(world: MetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_type_metrics.feature", index = 9)]
fn scenario_foreign_reach_convenience(world: MetricsWorld) {
    let _ = world;
}
