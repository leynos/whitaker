//! Behaviour-driven coverage for decomposition adjacency construction.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use whitaker_common::test_support::decomposition::{AdjacencyReport, EdgeInput, adjacency_report};

#[derive(Debug, Default)]
struct AdjacencyWorld {
    node_count: RefCell<usize>,
    edges: RefCell<Vec<EdgeInput>>,
    result: RefCell<Option<Result<AdjacencyReport, String>>>,
}

#[fixture]
fn world() -> AdjacencyWorld {
    AdjacencyWorld::default()
}

#[given("a graph with {count} nodes")]
fn given_graph_with_nodes(world: &AdjacencyWorld, count: usize) {
    *world.node_count.borrow_mut() = count;
}

#[given("an edge from {left} to {right} with weight {weight}")]
fn given_edge(world: &AdjacencyWorld, left: usize, right: usize, weight: u64) {
    world.edges.borrow_mut().push(EdgeInput {
        left,
        right,
        weight,
    });
}

#[when("adjacency is built")]
fn when_adjacency_is_built(world: &AdjacencyWorld) {
    let node_count = *world.node_count.borrow();
    let edges = world.edges.borrow();
    let result = adjacency_report(node_count, &edges);
    *world.result.borrow_mut() = Some(result);
}

fn with_report(
    world: &AdjacencyWorld,
    assert_fn: impl FnOnce(&AdjacencyReport) -> Result<(), String>,
) -> Result<(), String> {
    let result = world.result.borrow();
    let report = result
        .as_ref()
        .ok_or_else(|| String::from("adjacency must be built before assertions"))?;
    match report {
        Ok(report) => assert_fn(report),
        Err(message) => Err(format!("expected successful build, got error: {message}")),
    }
}

#[then("the adjacency is symmetric")]
fn then_adjacency_is_symmetric(world: &AdjacencyWorld) -> Result<(), String> {
    with_report(world, |report| {
        if report.is_symmetric() {
            Ok(())
        } else {
            Err(String::from("expected adjacency to be symmetric"))
        }
    })
}

#[then("all neighbour indices are in bounds")]
fn then_all_indices_in_bounds(world: &AdjacencyWorld) -> Result<(), String> {
    with_report(world, |report| {
        if report.all_indices_in_bounds() {
            Ok(())
        } else {
            Err(String::from(
                "expected all neighbour indices to be in bounds",
            ))
        }
    })
}

#[then("the build is rejected")]
fn then_build_is_rejected(world: &AdjacencyWorld) -> Result<(), String> {
    let result = world.result.borrow();
    let outcome = result
        .as_ref()
        .ok_or_else(|| String::from("adjacency must be built before assertions"))?;
    match outcome {
        Err(_) => Ok(()),
        Ok(_) => Err(String::from("expected build to be rejected")),
    }
}

#[then("node {node} has no neighbours")]
fn then_node_has_no_neighbours(world: &AdjacencyWorld, node: usize) -> Result<(), String> {
    with_report(world, |report| {
        let neighbours = report
            .neighbours_of(node)
            .ok_or_else(|| format!("node {node} is out of bounds"))?;
        if neighbours.is_empty() {
            Ok(())
        } else {
            Err(format!("expected node {node} to have no neighbours"))
        }
    })
}

#[then("the neighbours of node {node} are sorted")]
fn then_neighbours_of_node_are_sorted(world: &AdjacencyWorld, node: usize) -> Result<(), String> {
    with_report(world, |report| {
        let neighbours = report
            .neighbours_of(node)
            .ok_or_else(|| format!("node {node} is out of bounds"))?;
        let is_sorted = neighbours.windows(2).all(|pair| pair[0].0 <= pair[1].0);
        if is_sorted {
            Ok(())
        } else {
            Err(format!("expected neighbours of node {node} to be sorted"))
        }
    })
}

// Scenario indices must match declaration order in
// `tests/features/decomposition_adjacency.feature`.

#[scenario(path = "tests/features/decomposition_adjacency.feature", index = 0)]
fn scenario_valid_edges_produce_symmetric_neighbour_lists(world: AdjacencyWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_adjacency.feature", index = 1)]
fn scenario_malformed_edge_input_is_rejected(world: AdjacencyWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_adjacency.feature", index = 2)]
fn scenario_isolated_nodes_have_empty_neighbour_lists(world: AdjacencyWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/decomposition_adjacency.feature", index = 3)]
fn scenario_multiple_neighbours_appear_in_sorted_order(world: AdjacencyWorld) {
    let _ = world;
}
