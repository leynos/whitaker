//! Bounded model-checking harnesses for `propagate_labels`.

use crate::decomposition_advice::minimal_feature_vector;

use super::super::propagate_labels_report;
use super::shared::bounded_iteration_count;

const NODE_COUNT: usize = 3;

fn symbolic_vectors() -> [crate::decomposition_advice::vector::MethodFeatureVector; NODE_COUNT] {
    [
        minimal_feature_vector("gamma"),
        minimal_feature_vector("alpha"),
        minimal_feature_vector("beta"),
    ]
}

fn symbolic_adjacency() -> [Vec<(usize, u64)>; NODE_COUNT] {
    let mut node_0 = Vec::new();
    let mut node_1 = Vec::new();
    let mut node_2 = Vec::new();

    add_symbolic_edge(&mut node_0, 0, &mut node_1, 1);
    add_symbolic_edge(&mut node_0, 0, &mut node_2, 2);
    add_symbolic_edge(&mut node_1, 1, &mut node_2, 2);

    [node_0, node_1, node_2]
}

fn add_symbolic_edge(
    left: &mut Vec<(usize, u64)>,
    left_index: usize,
    right: &mut Vec<(usize, u64)>,
    right_index: usize,
) {
    let is_active: bool = kani::any();
    if !is_active {
        return;
    }

    let weight: u64 = kani::any();
    kani::assume(weight > 0);
    left.push((right_index, weight));
    right.push((left_index, weight));
}

/// Verifies that propagation returns exactly one label per input vector.
#[kani::proof]
#[kani::unwind(10)]
fn verify_propagate_labels_returns_vector_per_input() {
    let adjacency = symbolic_adjacency();
    let vectors = symbolic_vectors();

    let report = propagate_labels_report(&vectors, &adjacency, bounded_iteration_count());

    assert_eq!(report.labels().len(), NODE_COUNT);
}

/// Verifies that every propagated label remains a valid node index.
#[kani::proof]
#[kani::unwind(10)]
fn verify_propagate_labels_preserves_label_indices() {
    let adjacency = symbolic_adjacency();
    let vectors = symbolic_vectors();

    let report = propagate_labels_report(&vectors, &adjacency, bounded_iteration_count());

    for &label in report.labels() {
        assert!(label < NODE_COUNT);
    }
}

/// Verifies that zero iterations keep the initial self labels unchanged.
#[kani::proof]
#[kani::unwind(10)]
fn verify_propagate_labels_zero_iterations_keeps_initial_labels() {
    let adjacency = symbolic_adjacency();
    let vectors = symbolic_vectors();

    let report = propagate_labels_report(&vectors, &adjacency, 0);

    assert_eq!(report.iteration_count(), 0);
    for (index, &label) in report.labels().iter().enumerate() {
        assert_eq!(label, index);
    }
}

/// Verifies that the reported iteration count never exceeds the supplied
/// bound.
#[kani::proof]
#[kani::unwind(10)]
fn verify_propagate_labels_bounded_return_for_any_max_iterations() {
    let adjacency = symbolic_adjacency();
    let vectors = symbolic_vectors();
    let max_iterations = bounded_iteration_count();

    let report = propagate_labels_report(&vectors, &adjacency, max_iterations);

    assert_eq!(report.labels().len(), NODE_COUNT);
    assert!(report.iteration_count() <= max_iterations);
}
