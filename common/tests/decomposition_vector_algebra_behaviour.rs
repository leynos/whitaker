//! Behaviour-driven coverage for decomposition vector algebra helpers.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::str::FromStr;
use whitaker_common::MethodProfileBuilder;
use whitaker_common::test_support::decomposition::{
    MethodVectorAlgebraReport, method_vector_algebra,
};

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
struct VectorAlgebraWorld {
    methods: RefCell<BTreeMap<String, MethodProfileBuilder>>,
    report: RefCell<Option<MethodVectorAlgebraReport>>,
}

#[fixture]
fn world() -> VectorAlgebraWorld {
    VectorAlgebraWorld::default()
}

fn ensure_method_builder(world: &VectorAlgebraWorld, side: MethodSide, method_name: &str) {
    world.methods.borrow_mut().insert(
        side.key().to_owned(),
        MethodProfileBuilder::new(method_name),
    );
}

fn with_method_builder(
    world: &VectorAlgebraWorld,
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

fn apply_csv_items_to_builder(
    world: &VectorAlgebraWorld,
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

fn with_report(
    world: &VectorAlgebraWorld,
    assert_fn: impl FnOnce(MethodVectorAlgebraReport) -> Result<(), String>,
) -> Result<(), String> {
    let report = world.report.borrow();
    let value = report
        .as_ref()
        .copied()
        .ok_or_else(|| String::from("vector algebra must be evaluated before assertions"))?;
    assert_fn(value)
}

#[given("a {side} method named {name}")]
fn given_method(world: &VectorAlgebraWorld, side: MethodSide, name: String) {
    ensure_method_builder(world, side, &name);
}

#[given("the {side} method accesses fields {fields}")]
fn given_fields(
    world: &VectorAlgebraWorld,
    side: MethodSide,
    fields: CsvList,
) -> Result<(), String> {
    apply_csv_items_to_builder(world, side, fields, |builder, field| {
        builder.record_accessed_field(field);
    })
}

#[given("the {side} method uses external domains {domains}")]
fn given_domains(
    world: &VectorAlgebraWorld,
    side: MethodSide,
    domains: CsvList,
) -> Result<(), String> {
    apply_csv_items_to_builder(world, side, domains, |builder, domain| {
        builder.record_external_domain(domain);
    })
}

#[when("the vector algebra is evaluated")]
fn when_vector_algebra_is_evaluated(world: &VectorAlgebraWorld) -> Result<(), String> {
    let methods = world.methods.borrow();
    let left = methods
        .get(MethodSide::Left.key())
        .ok_or_else(|| String::from("left method must be configured before evaluation"))?
        .clone()
        .build();
    let right = methods
        .get(MethodSide::Right.key())
        .ok_or_else(|| String::from("right method must be configured before evaluation"))?
        .clone()
        .build();
    *world.report.borrow_mut() = Some(method_vector_algebra(&left, &right));
    Ok(())
}

fn assert_on_report(
    world: &VectorAlgebraWorld,
    predicate: impl FnOnce(MethodVectorAlgebraReport) -> bool,
    message: &'static str,
) -> Result<(), String> {
    with_report(world, |report| {
        if predicate(report) {
            Ok(())
        } else {
            Err(String::from(message))
        }
    })
}

#[then("the dot product is commutative")]
fn then_dot_product_is_commutative(world: &VectorAlgebraWorld) -> Result<(), String> {
    assert_on_report(
        world,
        |report| report.left_dot_right() == report.right_dot_left(),
        "expected dot product to be commutative",
    )
}

#[then("the {side} squared norm is {expected}")]
fn then_squared_norm_matches(
    world: &VectorAlgebraWorld,
    side: MethodSide,
    expected: u64,
) -> Result<(), String> {
    with_report(world, |report| {
        let actual = match side {
            MethodSide::Left => report.left_norm_squared(),
            MethodSide::Right => report.right_norm_squared(),
        };
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "expected {} squared norm to be {expected}, got {actual}",
                side.key()
            ))
        }
    })
}

#[then("the dot product is zero")]
fn then_dot_product_is_zero(world: &VectorAlgebraWorld) -> Result<(), String> {
    assert_on_report(
        world,
        |report| report.left_dot_right() == 0 && report.right_dot_left() == 0,
        "expected the methods to have zero dot product",
    )
}

// Scenario indices must match declaration order in
// `tests/features/decomposition_vector_algebra.feature`.

#[scenario(
    path = "tests/features/decomposition_vector_algebra.feature",
    index = 0
)]
fn scenario_shared_field_preserves_commutativity(world: VectorAlgebraWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_vector_algebra.feature",
    index = 1
)]
fn scenario_empty_method_has_non_negative_norm(world: VectorAlgebraWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_vector_algebra.feature",
    index = 2
)]
fn scenario_disjoint_positive_features_have_zero_dot_product(world: VectorAlgebraWorld) {
    let _ = world;
}
