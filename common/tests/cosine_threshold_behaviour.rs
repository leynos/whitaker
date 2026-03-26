//! Behaviour-driven coverage for the decomposition cosine threshold.

use common::MethodProfileBuilder;
use common::test_support::decomposition::methods_meet_cosine_threshold;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
enum MethodSide {
    Left,
    Right,
}

impl MethodSide {
    fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

impl FromStr for MethodSide {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err(format!("unknown method side `{value}`")),
        }
    }
}

#[derive(Debug, Clone)]
struct CsvList(Vec<String>);

impl CsvList {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl FromStr for CsvList {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let items = value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(Self(items))
    }
}

#[derive(Debug, Default)]
struct CosineThresholdWorld {
    methods: RefCell<BTreeMap<String, MethodProfileBuilder>>,
    threshold_met: RefCell<Option<bool>>,
}

#[fixture]
fn world() -> CosineThresholdWorld {
    CosineThresholdWorld::default()
}

fn ensure_method_builder(world: &CosineThresholdWorld, side: MethodSide, method_name: &str) {
    world.methods.borrow_mut().insert(
        side.key().to_owned(),
        MethodProfileBuilder::new(method_name),
    );
}

fn with_method_builder(
    world: &CosineThresholdWorld,
    side: MethodSide,
    update: impl FnOnce(&mut MethodProfileBuilder),
) -> Result<(), String> {
    let mut methods = world.methods.borrow_mut();
    let builder = methods
        .get_mut(side.key())
        .ok_or_else(|| format!("{} method must be created before configuration", side.key()))?;
    update(builder);
    Ok(())
}

fn with_threshold_result(
    world: &CosineThresholdWorld,
    assert_fn: impl FnOnce(bool) -> Result<(), String>,
) -> Result<(), String> {
    let threshold_met = world.threshold_met.borrow();
    let value = threshold_met
        .as_ref()
        .copied()
        .ok_or_else(|| String::from("threshold must be evaluated before assertions"))?;
    assert_fn(value)
}

#[given("a {side} method named {name}")]
fn given_method(world: &CosineThresholdWorld, side: MethodSide, name: String) {
    ensure_method_builder(world, side, &name);
}

fn apply_csv_items_to_builder(
    world: &CosineThresholdWorld,
    side: MethodSide,
    items: CsvList,
    mut record: impl FnMut(&mut MethodProfileBuilder, &str),
) -> Result<(), String> {
    let parsed_items = items.into_vec();
    with_method_builder(world, side, |builder| {
        for item in &parsed_items {
            record(builder, item.as_str());
        }
    })
}

#[given("the {side} method accesses fields {fields}")]
fn given_fields(
    world: &CosineThresholdWorld,
    side: MethodSide,
    fields: CsvList,
) -> Result<(), String> {
    apply_csv_items_to_builder(world, side, fields, |builder, field| {
        builder.record_accessed_field(field);
    })
}

#[given("the {side} method uses external domains {domains}")]
fn given_domains(
    world: &CosineThresholdWorld,
    side: MethodSide,
    domains: CsvList,
) -> Result<(), String> {
    apply_csv_items_to_builder(world, side, domains, |builder, domain| {
        builder.record_external_domain(domain);
    })
}

#[when("the cosine threshold is evaluated")]
fn when_threshold_is_evaluated(world: &CosineThresholdWorld) -> Result<(), String> {
    let methods = world.methods.borrow();
    let left = methods
        .get("left")
        .ok_or_else(|| String::from("left method must be configured before evaluation"))?
        .clone()
        .build();
    let right = methods
        .get("right")
        .ok_or_else(|| String::from("right method must be configured before evaluation"))?
        .clone()
        .build();
    *world.threshold_met.borrow_mut() = Some(methods_meet_cosine_threshold(&left, &right));
    Ok(())
}

#[then("the methods are considered similar")]
fn then_methods_are_similar(world: &CosineThresholdWorld) -> Result<(), String> {
    with_threshold_result(world, |threshold_met| {
        if threshold_met {
            Ok(())
        } else {
            Err(String::from(
                "expected methods to satisfy the cosine threshold",
            ))
        }
    })
}

#[then("the methods are not considered similar")]
fn then_methods_are_not_similar(world: &CosineThresholdWorld) -> Result<(), String> {
    with_threshold_result(world, |threshold_met| {
        if threshold_met {
            Err(String::from(
                "expected methods to fail the cosine threshold check",
            ))
        } else {
            Ok(())
        }
    })
}

// Scenario indices must match declaration order in
// `tests/features/cosine_threshold.feature`.

#[scenario(path = "tests/features/cosine_threshold.feature", index = 0)]
fn scenario_strong_overlap(world: CosineThresholdWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cosine_threshold.feature", index = 1)]
fn scenario_below_threshold(world: CosineThresholdWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/cosine_threshold.feature", index = 2)]
fn scenario_zero_vector(world: CosineThresholdWorld) {
    let _ = world;
}
