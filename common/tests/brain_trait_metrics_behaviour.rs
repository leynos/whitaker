//! Behaviour-driven coverage for brain trait metric collection.

use common::brain_trait_metrics::{TraitMetrics, TraitMetricsBuilder};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Clone, Debug)]
enum PendingTraitItem {
    RequiredMethod(String),
    DefaultMethod {
        name: String,
        cc: usize,
        is_from_expansion: bool,
    },
    AssociatedType(String),
    AssociatedConst(String),
}

#[derive(Debug, Default)]
struct TraitMetricsWorld {
    trait_name: RefCell<String>,
    items: RefCell<Vec<PendingTraitItem>>,
    metrics: RefCell<Option<TraitMetrics>>,
}

#[fixture]
fn world() -> TraitMetricsWorld {
    TraitMetricsWorld::default()
}

fn with_metrics(world: &TraitMetricsWorld, assert_fn: impl FnOnce(&TraitMetrics)) {
    let metrics = world.metrics.borrow();
    match metrics.as_ref() {
        Some(metrics) => assert_fn(metrics),
        None => panic!("metrics must be built before running assertions"),
    }
}

#[given("a trait named {name}")]
fn given_trait_name(world: &TraitMetricsWorld, name: String) {
    *world.trait_name.borrow_mut() = name;
}

#[given("a required method {name}")]
fn given_required_method(world: &TraitMetricsWorld, name: String) {
    world
        .items
        .borrow_mut()
        .push(PendingTraitItem::RequiredMethod(name));
}

#[given("a default method {name} with CC {cc}")]
fn given_default_method(world: &TraitMetricsWorld, name: String, cc: usize) {
    world
        .items
        .borrow_mut()
        .push(PendingTraitItem::DefaultMethod {
            name,
            cc,
            is_from_expansion: false,
        });
}

#[given("a default method {name} with CC {cc} from expansion")]
fn given_default_method_from_expansion(world: &TraitMetricsWorld, name: String, cc: usize) {
    world
        .items
        .borrow_mut()
        .push(PendingTraitItem::DefaultMethod {
            name,
            cc,
            is_from_expansion: true,
        });
}

#[given("an associated type {name}")]
fn given_associated_type(world: &TraitMetricsWorld, name: String) {
    world
        .items
        .borrow_mut()
        .push(PendingTraitItem::AssociatedType(name));
}

#[given("an associated const {name}")]
fn given_associated_const(world: &TraitMetricsWorld, name: String) {
    world
        .items
        .borrow_mut()
        .push(PendingTraitItem::AssociatedConst(name));
}

#[when("trait metrics are built")]
fn when_metrics_are_built(world: &TraitMetricsWorld) {
    let trait_name = world.trait_name.borrow().clone();
    let mut builder = TraitMetricsBuilder::new(trait_name);

    for item in world.items.borrow().iter() {
        match item {
            PendingTraitItem::RequiredMethod(name) => {
                builder.add_required_method(name.as_str());
            }
            PendingTraitItem::DefaultMethod {
                name,
                cc,
                is_from_expansion,
            } => {
                builder.add_default_method(name.as_str(), *cc, *is_from_expansion);
            }
            PendingTraitItem::AssociatedType(name) => {
                builder.add_associated_type(name.as_str());
            }
            PendingTraitItem::AssociatedConst(name) => {
                builder.add_associated_const(name.as_str());
            }
        }
    }

    *world.metrics.borrow_mut() = Some(builder.build());
}

#[then("total trait items is {count}")]
fn then_total_trait_items(world: &TraitMetricsWorld, count: usize) {
    with_metrics(world, |metrics| {
        assert_eq!(metrics.total_item_count(), count);
    });
}

#[then("required method count is {count}")]
fn then_required_method_count(world: &TraitMetricsWorld, count: usize) {
    with_metrics(world, |metrics| {
        assert_eq!(metrics.required_method_count(), count);
    });
}

#[then("default method count is {count}")]
fn then_default_method_count(world: &TraitMetricsWorld, count: usize) {
    with_metrics(world, |metrics| {
        assert_eq!(metrics.default_method_count(), count);
    });
}

#[then("default method CC sum is {sum}")]
fn then_default_method_cc_sum(world: &TraitMetricsWorld, sum: usize) {
    with_metrics(world, |metrics| {
        assert_eq!(metrics.default_method_cc_sum(), sum);
    });
}

#[then("implementor burden is {count}")]
fn then_implementor_burden(world: &TraitMetricsWorld, count: usize) {
    with_metrics(world, |metrics| {
        assert_eq!(metrics.implementor_burden(), count);
    });
}

// Scenario indices must match declaration order in
// `tests/features/brain_trait_metrics.feature`.

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 0)]
fn scenario_mixed_trait_items(world: TraitMetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 1)]
fn scenario_without_default_methods(world: TraitMetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 2)]
fn scenario_empty_trait(world: TraitMetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 3)]
fn scenario_expansion_filter(world: TraitMetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 4)]
fn scenario_implementor_burden(world: TraitMetricsWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/brain_trait_metrics.feature", index = 5)]
fn scenario_only_default_methods(world: TraitMetricsWorld) {
    let _ = world;
}
