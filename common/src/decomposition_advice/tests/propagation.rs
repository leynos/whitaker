//! Unit tests for deterministic label propagation.

use rstest::{fixture, rstest};

use crate::decomposition_advice::community::{
    SimilarityEdge, build_adjacency, propagate_labels_report,
};
use crate::decomposition_advice::minimal_feature_vector;
use crate::decomposition_advice::vector::MethodFeatureVector;

fn vectors(method_names: &[&str]) -> Vec<MethodFeatureVector> {
    method_names
        .iter()
        .map(|method_name| minimal_feature_vector(method_name))
        .collect()
}

fn edge(left: usize, right: usize, weight: u64) -> SimilarityEdge {
    SimilarityEdge::new(left, right, weight)
}

#[fixture]
fn connected_triplet_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma"])
}

#[fixture]
fn connected_triplet_adjacency() -> Vec<Vec<(usize, u64)>> {
    build_adjacency(3, &[edge(0, 1, 5), edge(1, 2, 5)])
}

#[fixture]
fn linear_quartet_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma", "delta"])
}

#[fixture]
fn linear_quartet_adjacency() -> Vec<Vec<(usize, u64)>> {
    build_adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)])
}

#[fixture]
fn isolated_tail_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma"])
}

#[fixture]
fn isolated_tail_adjacency() -> Vec<Vec<(usize, u64)>> {
    build_adjacency(3, &[edge(0, 1, 5)])
}

#[fixture]
fn lexical_tie_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["gamma", "alpha", "beta"])
}

#[fixture]
fn lexical_tie_adjacency() -> Vec<Vec<(usize, u64)>> {
    build_adjacency(3, &[edge(0, 1, 5), edge(0, 2, 5)])
}

#[fixture]
fn non_converged_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["delta", "charlie", "beta", "alpha"])
}

#[fixture]
fn non_converged_adjacency() -> Vec<Vec<(usize, u64)>> {
    build_adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)])
}

#[derive(Debug)]
struct ConnectedTripletExpectation {
    iteration_bound: usize,
    expected_labels: Vec<usize>,
    expected_iteration_count: usize,
}

#[rstest]
#[case(3)]
#[case(0)]
fn propagate_labels_reports_expected_length(
    connected_triplet_vectors: Vec<MethodFeatureVector>,
    connected_triplet_adjacency: Vec<Vec<(usize, u64)>>,
    #[case] iteration_bound: usize,
) {
    let report = propagate_labels_report(
        &connected_triplet_vectors,
        &connected_triplet_adjacency,
        iteration_bound,
    );

    assert_eq!(report.labels.len(), connected_triplet_vectors.len());
}

#[rstest]
#[case(4)]
#[case(1)]
fn propagate_labels_keeps_labels_in_range(
    linear_quartet_vectors: Vec<MethodFeatureVector>,
    linear_quartet_adjacency: Vec<Vec<(usize, u64)>>,
    #[case] iteration_bound: usize,
) {
    let report = propagate_labels_report(
        &linear_quartet_vectors,
        &linear_quartet_adjacency,
        iteration_bound,
    );

    assert!(
        report
            .labels
            .iter()
            .all(|&label| label < linear_quartet_vectors.len())
    );
}

#[rstest]
fn propagate_labels_leaves_isolated_nodes_with_original_labels(
    isolated_tail_vectors: Vec<MethodFeatureVector>,
    isolated_tail_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let report = propagate_labels_report(&isolated_tail_vectors, &isolated_tail_adjacency, 3);

    assert_eq!(report.labels[2], 2);
}

#[rstest]
#[case(ConnectedTripletExpectation {
    iteration_bound: 0,
    expected_labels: vec![0, 1, 2],
    expected_iteration_count: 0,
})]
#[case(ConnectedTripletExpectation {
    iteration_bound: 1,
    expected_labels: vec![1, 1, 1],
    expected_iteration_count: 1,
})]
fn propagate_labels_matches_expected_report_for_connected_triplet(
    connected_triplet_vectors: Vec<MethodFeatureVector>,
    connected_triplet_adjacency: Vec<Vec<(usize, u64)>>,
    #[case] expected: ConnectedTripletExpectation,
) {
    let report = propagate_labels_report(
        &connected_triplet_vectors,
        &connected_triplet_adjacency,
        expected.iteration_bound,
    );

    assert_eq!(report.labels, expected.expected_labels);
    assert_eq!(report.iteration_count, expected.expected_iteration_count);
}

#[rstest]
fn propagate_labels_uses_lexical_tie_break_for_equal_scores(
    lexical_tie_vectors: Vec<MethodFeatureVector>,
    lexical_tie_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let report = propagate_labels_report(&lexical_tie_vectors, &lexical_tie_adjacency, 1);

    assert_eq!(report.labels[0], 1);
}

#[rstest]
fn propagate_labels_prefers_heavier_star_neighbour_when_counts_match() {
    let vectors = vectors(&["hub", "zeta", "alpha"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 9), edge(0, 2, 1)]);

    let report = propagate_labels_report(&vectors, &adjacency, 1);

    assert_eq!(report.labels[0], 1);
    assert_eq!(report.labels[0], report.labels[1]);
}

#[rstest]
fn propagate_labels_prefers_triangle_weight_over_count_tie() {
    let vectors = vectors(&["hub", "zeta", "alpha"]);
    let adjacency = build_adjacency(3, &[edge(0, 1, 12), edge(0, 2, 1), edge(1, 2, 1)]);

    let report = propagate_labels_report(&vectors, &adjacency, 1);

    assert_eq!(report.labels[0], 1);
}

#[rstest]
fn propagate_labels_returns_after_bound_even_when_not_converged(
    non_converged_vectors: Vec<MethodFeatureVector>,
    non_converged_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let single_pass = propagate_labels_report(&non_converged_vectors, &non_converged_adjacency, 1);
    let two_passes = propagate_labels_report(&non_converged_vectors, &non_converged_adjacency, 2);

    assert_eq!(single_pass.iteration_count, 1);
    assert_eq!(single_pass.labels.len(), non_converged_vectors.len());
    assert_ne!(single_pass.labels, two_passes.labels);
}

#[rstest]
#[should_panic(expected = "propagate_labels_report requires adjacency rows to match vectors")]
fn propagate_labels_rejects_extra_adjacency_rows(
    connected_triplet_vectors: Vec<MethodFeatureVector>,
    connected_triplet_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let mut adjacency = connected_triplet_adjacency;
    adjacency.push(vec![(0, 5)]);

    propagate_labels_report(&connected_triplet_vectors, &adjacency, 1);
}
