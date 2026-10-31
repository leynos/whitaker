//! Unit tests for deterministic label propagation.

use rstest::{fixture, rstest};
use whitaker_test_macros::allow_fixture_expansion_lints;

use crate::decomposition_advice::{
    community::{SimilarityEdge, build_adjacency, detect_communities, propagate_labels_report},
    error::DecompositionError,
    minimal_feature_vector,
    vector::{MethodFeatureVector, test_feature_vector},
};

/// Create minimal vectors in the supplied method-name order.
fn vectors(method_names: &[&str]) -> Vec<MethodFeatureVector> {
    method_names
        .iter()
        .map(|method_name| minimal_feature_vector(method_name))
        .collect()
}

/// Create a weighted edge for propagation fixtures.
fn edge(left: usize, right: usize, weight: u64) -> SimilarityEdge {
    SimilarityEdge::new(left, right, weight)
}

/// Build a valid propagation fixture graph from checked edges.
fn adjacency(node_count: usize, edges: &[SimilarityEdge]) -> Vec<Vec<(usize, u64)>> {
    build_adjacency(node_count, edges).expect("test edges should be within node bounds")
}

/// Return three named vectors for the connected-triplet cases.
#[allow_fixture_expansion_lints]
#[fixture]
fn connected_triplet_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma"])
}

/// Return a three-node path adjacency graph.
#[fixture]
fn connected_triplet_adjacency() -> Vec<Vec<(usize, u64)>> {
    adjacency(3, &[edge(0, 1, 5), edge(1, 2, 5)])
}

/// Return four named vectors for the linear-quartet cases.
#[fixture]
fn linear_quartet_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma", "delta"])
}

/// Return a four-node path adjacency graph.
#[fixture]
fn linear_quartet_adjacency() -> Vec<Vec<(usize, u64)>> {
    adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)])
}

/// Return three named vectors for isolated-tail cases.
#[allow_fixture_expansion_lints]
#[fixture]
fn isolated_tail_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["alpha", "beta", "gamma"])
}

/// Return an edge and one isolated node.
#[allow_fixture_expansion_lints]
#[fixture]
fn isolated_tail_adjacency() -> Vec<Vec<(usize, u64)>> {
    adjacency(3, &[edge(0, 1, 5)])
}

/// Return names whose lexical order breaks a propagation tie.
#[allow_fixture_expansion_lints]
#[fixture]
fn lexical_tie_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["gamma", "alpha", "beta"])
}

/// Return a star graph for lexical tie-breaking cases.
#[fixture]
fn lexical_tie_adjacency() -> Vec<Vec<(usize, u64)>> {
    adjacency(3, &[edge(0, 1, 5), edge(0, 2, 5)])
}

/// Return names that make bounded propagation change across passes.
#[fixture]
fn non_converged_vectors() -> Vec<MethodFeatureVector> {
    vectors(&["delta", "charlie", "beta", "alpha"])
}

/// Return a four-node path for bounded propagation cases.
#[fixture]
fn non_converged_adjacency() -> Vec<Vec<(usize, u64)>> {
    adjacency(4, &[edge(0, 1, 5), edge(1, 2, 5), edge(2, 3, 5)])
}

#[derive(Debug)]
struct ConnectedTripletExpectation {
    iteration_bound: usize,
    expected_labels: Vec<usize>,
    expected_iteration_count: usize,
}

/// Report one label per vector for both zero and positive iteration bounds.
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
    )
    .expect("fixture adjacency should be valid");

    assert_eq!(report.labels.len(), connected_triplet_vectors.len());
}

/// Keep every propagated label inside the vector index range.
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
    )
    .expect("fixture adjacency should be valid");

    assert!(
        report
            .labels
            .iter()
            .all(|&label| label < linear_quartet_vectors.len())
    );
}

/// Preserve the initial label for nodes without incident edges.
#[rstest]
fn propagate_labels_leaves_isolated_nodes_with_original_labels(
    isolated_tail_vectors: Vec<MethodFeatureVector>,
    isolated_tail_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let report = propagate_labels_report(&isolated_tail_vectors, &isolated_tail_adjacency, 3)
        .expect("fixture adjacency should be valid");

    assert_eq!(report.labels.get(2), Some(&2));
}

/// Match deterministic labels and pass counts for connected triplets.
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
    )
    .expect("fixture adjacency should be valid");

    assert_eq!(report.labels, expected.expected_labels);
    assert_eq!(report.iteration_count, expected.expected_iteration_count);
}

/// Choose the lexically earlier method when candidate scores tie.
#[rstest]
fn propagate_labels_uses_lexical_tie_break_for_equal_scores(
    lexical_tie_vectors: Vec<MethodFeatureVector>,
    lexical_tie_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let report = propagate_labels_report(&lexical_tie_vectors, &lexical_tie_adjacency, 1)
        .expect("fixture adjacency should be valid");

    assert_eq!(report.labels.first(), Some(&1));
}

/// Prefer a heavier neighbour when competing edges have unequal weights.
#[rstest]
fn propagate_labels_prefers_heavier_star_neighbour_when_counts_match() {
    let vectors = vectors(&["hub", "zeta", "alpha"]);
    let adjacency = adjacency(3, &[edge(0, 1, 9), edge(0, 2, 1)]);

    let report = propagate_labels_report(&vectors, &adjacency, 1)
        .expect("fixture adjacency should be valid");

    assert_eq!(report.labels.first(), Some(&1));
    assert_eq!(report.labels.first(), report.labels.get(1));
}

/// Prefer triangle weight totals when neighbour counts tie.
#[rstest]
fn propagate_labels_prefers_triangle_weight_over_count_tie() {
    let vectors = vectors(&["hub", "zeta", "alpha"]);
    let adjacency = adjacency(3, &[edge(0, 1, 12), edge(0, 2, 1), edge(1, 2, 1)]);

    let report = propagate_labels_report(&vectors, &adjacency, 1)
        .expect("fixture adjacency should be valid");

    assert_eq!(report.labels.first(), Some(&1));
}

/// Stop propagation at the requested bound before convergence.
#[rstest]
fn propagate_labels_returns_after_bound_even_when_not_converged(
    non_converged_vectors: Vec<MethodFeatureVector>,
    non_converged_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let single_pass = propagate_labels_report(&non_converged_vectors, &non_converged_adjacency, 1)
        .expect("fixture adjacency should be valid");
    let two_passes = propagate_labels_report(&non_converged_vectors, &non_converged_adjacency, 2)
        .expect("fixture adjacency should be valid");

    assert_eq!(single_pass.iteration_count, 1);
    assert_eq!(single_pass.labels.len(), non_converged_vectors.len());
    assert_ne!(single_pass.labels, two_passes.labels);
}

/// Reject adjacency whose row count differs from the vector count.
#[rstest]
fn propagate_labels_rejects_extra_adjacency_rows(
    connected_triplet_vectors: Vec<MethodFeatureVector>,
    connected_triplet_adjacency: Vec<Vec<(usize, u64)>>,
) {
    let mut adjacency = connected_triplet_adjacency;
    adjacency.push(vec![(0, 5)]);

    assert!(matches!(
        propagate_labels_report(&connected_triplet_vectors, &adjacency, 1),
        Err(DecompositionError::AdjacencyRowCountMismatch {
            expected_rows: 3,
            actual_rows: 4,
        })
    ));
}

/// Reject a neighbour index outside the initial label vector.
#[test]
fn propagate_labels_rejects_out_of_range_neighbours() {
    let vectors = vectors(&["alpha", "beta", "gamma"]);
    let adjacency = vec![vec![(3, 5)], Vec::new(), Vec::new()];

    assert!(matches!(
        propagate_labels_report(&vectors, &adjacency, 1),
        Err(DecompositionError::InvalidNeighbourLabel { node_count: 3 })
    ));
}

/// Group connected vectors that share the same feature set.
#[test]
fn detect_communities_groups_connected_feature_vectors() {
    let vectors = vec![
        test_feature_vector("alpha", &[("shared", 1)]),
        test_feature_vector("beta", &[("shared", 1)]),
        test_feature_vector("gamma", &[("shared", 1)]),
    ];

    let communities = detect_communities(&vectors).expect("similarity graph should be valid");

    assert_eq!(communities, vec![vec![0, 1, 2]]);
}

/// Keep vectors without comparable features in separate communities.
#[test]
fn detect_communities_keeps_minimal_vectors_separate() {
    let vectors = vectors(&["alpha", "beta", "gamma"]);

    let communities = detect_communities(&vectors).expect("similarity graph should be valid");

    assert_eq!(communities, vec![vec![0], vec![1], vec![2]]);
}
