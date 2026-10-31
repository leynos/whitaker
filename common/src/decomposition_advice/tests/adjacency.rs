//! Unit tests for `build_adjacency`.
//!
//! Validates that adjacency construction preserves input edges in both
//! directions, keeps neighbour indices in bounds, produces symmetric
//! adjacency lists, and sorts each per-node neighbour list by neighbour
//! index.

use super::super::{
    community::{SimilarityEdge, build_adjacency},
    error::DecompositionError,
};
use proptest::{prelude::*, test_runner::TestRunner};
use std::collections::BTreeSet;

/// Create a test edge with the supplied endpoints and weight.
fn edge(left: usize, right: usize, weight: u64) -> SimilarityEdge {
    SimilarityEdge::new(left, right, weight)
}

/// Build a valid test graph and explain invalid fixture endpoints.
fn adjacency(node_count: usize, edges: &[SimilarityEdge]) -> Vec<Vec<(usize, u64)>> {
    build_adjacency(node_count, edges).expect("test edges should be within node bounds")
}

/// Preserve the requested number of isolated adjacency rows.
#[test]
fn empty_edges_yield_empty_neighbour_lists() {
    let adjacency = adjacency(3, &[]);

    assert_eq!(adjacency.len(), 3);
    for bucket in &adjacency {
        assert!(bucket.is_empty());
    }
}

/// Insert each valid edge in both endpoint rows.
#[test]
fn single_edge_inserted_in_both_directions() {
    let adjacency = adjacency(3, &[edge(0, 2, 10)]);

    assert_eq!(adjacency.len(), 3);
    assert_eq!(adjacency.first(), Some(&vec![(2, 10)]));
    assert_eq!(adjacency.get(1), Some(&Vec::new()));
    assert_eq!(adjacency.get(2), Some(&vec![(0, 10)]));
}

/// Sort each neighbour row by node index.
#[test]
fn multiple_edges_produce_sorted_neighbour_lists() {
    // Node 1 connects to 0, 2, and 3.
    let edges = vec![edge(0, 1, 5), edge(1, 2, 8), edge(1, 3, 3)];
    let adjacency = adjacency(4, &edges);

    // Node 1's neighbours should be sorted by neighbour index.
    assert_eq!(adjacency.get(1), Some(&vec![(0, 5), (2, 8), (3, 3)]));
}

/// Check graph invariants for generated valid similarity edges.
#[test]
fn generated_valid_graphs_satisfy_adjacency_contract() {
    let strategy = (
        0usize..=8,
        prop::collection::vec((any::<usize>(), any::<usize>(), any::<u64>()), 0..24),
    );
    let mut runner = TestRunner::default();

    runner
        .run(&strategy, |(node_count, raw_edges)| {
            let edges: Vec<_> = if node_count == 0 {
                Vec::new()
            } else {
                let mut seen = BTreeSet::new();
                raw_edges
                    .into_iter()
                    .filter_map(|(left, right, weight)| {
                        let left = left % node_count;
                        let right = right % node_count;
                        let endpoints = (left.min(right), left.max(right));
                        (left != right && seen.insert(endpoints))
                            .then(|| edge(endpoints.0, endpoints.1, weight))
                    })
                    .collect()
            };
            let adjacency = adjacency(node_count, &edges);
            let emitted_entries: usize = adjacency.iter().map(Vec::len).sum();
            let expected_entries: BTreeSet<(usize, usize, u64)> = edges
                .iter()
                .flat_map(|edge| {
                    [
                        (edge.left(), edge.right(), edge.weight()),
                        (edge.right(), edge.left(), edge.weight()),
                    ]
                })
                .collect();
            let mut actual_entries = BTreeSet::new();

            prop_assert_eq!(adjacency.len(), node_count);
            prop_assert_eq!(emitted_entries, edges.len() * 2);

            for (node, bucket) in adjacency.iter().enumerate() {
                prop_assert!(bucket.windows(2).all(|window| window[0].0 <= window[1].0));

                for &(neighbour, weight) in bucket {
                    actual_entries.insert((node, neighbour, weight));
                }
            }

            prop_assert_eq!(actual_entries, expected_entries);
            Ok(())
        })
        .expect("generated valid graph cases should satisfy adjacency invariants");
}

/// Keep isolated nodes alongside connected components.
#[test]
fn sparse_graph_preserves_isolated_nodes() {
    // Nodes 1 and 3 are isolated; only 0-2 has an edge.
    let adjacency = adjacency(4, &[edge(0, 2, 7)]);

    assert_eq!(adjacency.len(), 4);
    assert_eq!(adjacency.first(), Some(&vec![(2, 7)]));
    assert_eq!(adjacency.get(1), Some(&Vec::new()));
    assert_eq!(adjacency.get(2), Some(&vec![(0, 7)]));
    assert_eq!(adjacency.get(3), Some(&Vec::new()));
}

/// Preserve symmetry for every valid input edge.
#[test]
fn multi_edge_graph_is_symmetric() {
    let edges = vec![edge(0, 1, 4), edge(2, 3, 9)];
    let adjacency = adjacency(4, &edges);

    // Every (node -> neighbour, weight) pair has its mirror.
    for (node, bucket) in adjacency.iter().enumerate() {
        for &(neighbour, weight) in bucket {
            let mirror_bucket = adjacency
                .get(neighbour)
                .expect("neighbour index should be within adjacency bounds");
            assert!(
                mirror_bucket
                    .iter()
                    .any(|&(mirror, mirror_weight)| mirror == node && mirror_weight == weight),
                "missing mirror for ({node} -> {neighbour}, weight {weight})",
            );
        }
    }
}

/// Reject an invalid endpoint before returning any adjacency output.
#[test]
fn invalid_endpoint_rejects_the_complete_adjacency() {
    let result = build_adjacency(3, &[edge(0, 1, 5), edge(1, 3, 7)]);

    assert!(matches!(
        result,
        Err(DecompositionError::InvalidEdgeEndpoint {
            node_count: 3,
            edge_count: 2,
        })
    ));
}
