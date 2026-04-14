//! Unit tests for deterministic label propagation.

use crate::decomposition_advice::community::{
    SimilarityEdge, build_adjacency, propagate_labels_report,
};
use crate::decomposition_advice::vector::{MethodFeatureVector, minimal_feature_vector};

fn vectors(method_names: &[&str]) -> Vec<MethodFeatureVector> {
    method_names
        .iter()
        .map(|method_name| minimal_feature_vector(method_name))
        .collect()
}

fn edge(left: usize, right: usize, weight: u64) -> SimilarityEdge {
    SimilarityEdge::new(left, right, weight)
}

#[test]
fn propagate_labels_returns_one_label_per_vector() {
    let vectors = vectors(&["alpha", "beta", "gamma"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 5), edge(1, 2, 5)]);

    let report = propagate_labels_report(&vectors, &adjacency, 3);

    assert_eq!(report.labels().len(), vectors.len());
}

#[test]
fn propagate_labels_keeps_labels_in_range_for_connected_graph() {
    let vectors = vectors(&["alpha", "beta", "gamma", "delta"]);
    let adjacency = build_adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)]);

    let report = propagate_labels_report(&vectors, &adjacency, 4);

    assert!(report.labels().iter().all(|&label| label < vectors.len()));
}

#[test]
fn propagate_labels_leaves_isolated_nodes_with_original_labels() {
    let vectors = vectors(&["alpha", "beta", "gamma"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 5)]);

    let report = propagate_labels_report(&vectors, &adjacency, 3);

    assert_eq!(report.labels()[2], 2);
}

#[test]
fn propagate_labels_respects_zero_iteration_bound() {
    let vectors = vectors(&["alpha", "beta", "gamma"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 5), edge(1, 2, 5)]);

    let report = propagate_labels_report(&vectors, &adjacency, 0);

    assert_eq!(report.labels(), &[0, 1, 2]);
    assert_eq!(report.iteration_count(), 0);
}

#[test]
fn propagate_labels_uses_lexical_tie_break_for_equal_scores() {
    let vectors = vectors(&["gamma", "alpha", "beta"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 5), edge(0, 2, 5)]);

    let report = propagate_labels_report(&vectors, &adjacency, 1);

    assert_eq!(report.labels()[0], 1);
}

#[test]
fn propagate_labels_returns_after_bound_even_when_not_converged() {
    let vectors = vectors(&["delta", "charlie", "beta", "alpha"]);
    let adjacency = build_adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)]);

    let single_pass = propagate_labels_report(&vectors, &adjacency, 1);
    let two_passes = propagate_labels_report(&vectors, &adjacency, 2);

    assert_eq!(single_pass.iteration_count(), 1);
    assert_eq!(single_pass.labels().len(), vectors.len());
    assert_ne!(single_pass.labels(), two_passes.labels());
}
