//! Unit tests for `build_adjacency`.
//!
//! Validates that adjacency construction preserves input edges in both
//! directions, keeps neighbour indices in bounds, produces symmetric
//! adjacency lists, and sorts each per-node neighbour list by neighbour
//! index.

use super::super::community::{SimilarityEdge, build_adjacency};

/// Convenience constructor for test `SimilarityEdge` values.
fn edge(left: usize, right: usize, weight: u64) -> SimilarityEdge {
    SimilarityEdge::new(left, right, weight)
}

#[test]
fn empty_edges_yield_empty_neighbour_lists() {
    let adjacency = build_adjacency(3, &[]);

    assert_eq!(adjacency.len(), 3);
    for bucket in &adjacency {
        assert!(bucket.is_empty());
    }
}

#[test]
fn single_edge_inserted_in_both_directions() {
    let adjacency = build_adjacency(3, &[edge(0, 2, 10)]);

    assert_eq!(adjacency.len(), 3);
    assert_eq!(adjacency[0], vec![(2, 10)]);
    assert!(adjacency[1].is_empty());
    assert_eq!(adjacency[2], vec![(0, 10)]);
}

#[test]
fn multiple_edges_produce_sorted_neighbour_lists() {
    // Node 1 connects to 0, 2, and 3.
    let edges = vec![edge(0, 1, 5), edge(1, 2, 8), edge(1, 3, 3)];
    let adjacency = build_adjacency(4, &edges);

    // Node 1's neighbours should be sorted by neighbour index.
    assert_eq!(adjacency[1], vec![(0, 5), (2, 8), (3, 3)]);
}

#[test]
fn sparse_graph_preserves_isolated_nodes() {
    // Nodes 1 and 3 are isolated; only 0-2 has an edge.
    let adjacency = build_adjacency(4, &[edge(0, 2, 7)]);

    assert_eq!(adjacency.len(), 4);
    assert_eq!(adjacency[0], vec![(2, 7)]);
    assert!(adjacency[1].is_empty());
    assert_eq!(adjacency[2], vec![(0, 7)]);
    assert!(adjacency[3].is_empty());
}

#[test]
fn multi_edge_graph_is_symmetric() {
    let edges = vec![edge(0, 1, 4), edge(2, 3, 9)];
    let adjacency = build_adjacency(4, &edges);

    // Every (node -> neighbour, weight) pair has its mirror.
    for (node, bucket) in adjacency.iter().enumerate() {
        for &(neighbour, weight) in bucket {
            assert!(
                adjacency[neighbour]
                    .iter()
                    .any(|&(mirror, mirror_weight)| mirror == node && mirror_weight == weight),
                "missing mirror for ({node} -> {neighbour}, weight {weight})",
            );
        }
    }
}
