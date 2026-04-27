//! Behaviour-driven coverage for decomposition label propagation.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::str::FromStr;
use whitaker_common::test_support::decomposition::{
    AdjacencyError, EdgeInput, LabelPropagationReport, label_propagation_report,
};

#[derive(Clone, Debug)]
struct CsvList(Vec<String>);

impl CsvList {
    fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl FromStr for CsvList {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        ))
    }
}

#[derive(Clone, Debug)]
struct CsvLabels(Vec<usize>);

impl CsvLabels {
    fn as_slice(&self) -> &[usize] {
        &self.0
    }
}

impl FromStr for CsvLabels {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let labels = value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                entry
                    .parse::<usize>()
                    .map_err(|error| format!("invalid label `{entry}`: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(labels))
    }
}

#[derive(Debug, Default)]
struct LabelPropagationWorld {
    method_names: RefCell<Vec<String>>,
    edges: RefCell<Vec<EdgeInput>>,
    max_iterations: RefCell<usize>,
    result: RefCell<Option<Result<LabelPropagationReport, AdjacencyError>>>,
}

#[fixture]
fn world() -> LabelPropagationWorld {
    LabelPropagationWorld::default()
}

#[given("methods named {method_names} are tracked")]
fn given_methods(world: &LabelPropagationWorld, method_names: CsvList) {
    *world.method_names.borrow_mut() = method_names.into_vec();
}

#[given("an edge from {left} to {right} with weight {weight}")]
fn given_edge(world: &LabelPropagationWorld, left: usize, right: usize, weight: u64) {
    world.edges.borrow_mut().push(EdgeInput {
        left,
        right,
        weight,
    });
}

#[given("the maximum iteration count is {max_iterations}")]
fn given_max_iterations(world: &LabelPropagationWorld, max_iterations: usize) {
    *world.max_iterations.borrow_mut() = max_iterations;
}

#[when("label propagation is run")]
fn when_label_propagation_runs(world: &LabelPropagationWorld) {
    let method_names = world.method_names.borrow();
    let names = method_names.iter().map(String::as_str).collect::<Vec<_>>();
    let result = label_propagation_report(
        &names,
        &world.edges.borrow(),
        *world.max_iterations.borrow(),
    );
    *world.result.borrow_mut() = Some(result);
}

fn with_report(
    world: &LabelPropagationWorld,
    assert_fn: impl FnOnce(&LabelPropagationReport) -> Result<(), String>,
) -> Result<(), String> {
    let result = world.result.borrow();
    let outcome = result
        .as_ref()
        .ok_or_else(|| String::from("label propagation must run before assertions"))?;
    match outcome {
        Ok(report) => assert_fn(report),
        Err(error) => Err(format!(
            "expected successful propagation, got error: {error}"
        )),
    }
}

#[then("the labels are {labels}")]
fn then_labels_match(world: &LabelPropagationWorld, labels: CsvLabels) -> Result<(), String> {
    with_report(world, |report| {
        if report.labels() == labels.as_slice() {
            Ok(())
        } else {
            Err(format!(
                "expected labels {:?}, got {:?}",
                labels.as_slice(),
                report.labels()
            ))
        }
    })
}

#[then("all propagated labels are in bounds")]
fn then_labels_are_in_bounds(world: &LabelPropagationWorld) -> Result<(), String> {
    with_report(world, |report| {
        if report.all_labels_in_bounds() {
            Ok(())
        } else {
            Err(String::from(
                "expected all propagated labels to be in bounds",
            ))
        }
    })
}

#[then("the propagation uses {iterations} iterations")]
fn then_iteration_count_matches(
    world: &LabelPropagationWorld,
    iterations: usize,
) -> Result<(), String> {
    with_report(world, |report| {
        if report.iteration_count() == iterations {
            Ok(())
        } else {
            Err(format!(
                "expected {iterations} iterations, got {}",
                report.iteration_count()
            ))
        }
    })
}

#[then("the graph has no active nodes")]
fn then_graph_has_no_active_nodes(world: &LabelPropagationWorld) -> Result<(), String> {
    with_report(world, |report| {
        if report.has_active_nodes() {
            Err(String::from("expected the graph to have no active nodes"))
        } else {
            Ok(())
        }
    })
}

#[then("the propagation input is rejected")]
fn then_propagation_input_is_rejected(world: &LabelPropagationWorld) -> Result<(), String> {
    let result = world.result.borrow();
    let outcome = result
        .as_ref()
        .ok_or_else(|| String::from("label propagation must run before assertions"))?;
    match outcome {
        Err(_) => Ok(()),
        Ok(_) => Err(String::from("expected propagation input to be rejected")),
    }
}

#[scenario(
    path = "tests/features/decomposition_label_propagation.feature",
    index = 0
)]
fn scenario_disconnected_pairs_settle_to_shared_labels(world: LabelPropagationWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_label_propagation.feature",
    index = 1
)]
fn scenario_isolated_nodes_keep_their_own_labels(world: LabelPropagationWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_label_propagation.feature",
    index = 2
)]
fn scenario_zero_iteration_bound_keeps_initial_labels(world: LabelPropagationWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_label_propagation.feature",
    index = 3
)]
fn scenario_equal_scores_break_ties_lexically(world: LabelPropagationWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/decomposition_label_propagation.feature",
    index = 4
)]
fn scenario_invalid_edge_input_is_rejected(world: LabelPropagationWorld) {
    let _ = world;
}
