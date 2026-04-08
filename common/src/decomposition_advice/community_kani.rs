//! Bounded model-checking harnesses for [`super::build_adjacency`].
//!
//! These Kani harnesses verify that `build_adjacency` preserves every valid
//! similarity edge, never emits out-of-bounds neighbour indices, and always
//! produces symmetric adjacency lists. The harnesses use symbolic fixed-size
//! arrays with an active-length field rather than symbolic `Vec` values,
//! giving Kani a small explicit search space.

use super::{SimilarityEdge, build_adjacency};

/// Maximum number of nodes in the bounded model.
///
/// Kept at 3 to keep the CBMC sort-unwinding tractable — Rust's standard
/// `sort_by` generates deep control-flow paths that explode combinatorially
/// at higher bounds.
const MAX_NODES: usize = 3;

/// Maximum number of edges in the bounded model. With 3 nodes the maximum
/// possible unique undirected edges (left < right) is C(3,2) = 3.
const MAX_EDGES: usize = 3;

/// Generates a symbolic node count constrained to `[0, MAX_NODES]`.
///
/// Use this helper to ensure consistent node-count generation and assumptions
/// across all harnesses.
fn constrained_node_count() -> usize {
    let node_count: usize = kani::any();
    kani::assume(node_count <= MAX_NODES);
    node_count
}

/// Materialises a concrete `Vec<SimilarityEdge>` from a symbolic fixed-size
/// array, constraining each active edge to the production contract established
/// by `build_similarity_edges`.
fn constrained_edges(node_count: usize) -> Vec<SimilarityEdge> {
    let active_count: usize = kani::any();
    kani::assume(active_count <= MAX_EDGES);

    let mut edges = Vec::new();
    let mut seen = [[false; MAX_NODES]; MAX_NODES];

    for _ in 0..active_count {
        let left: usize = kani::any();
        let right: usize = kani::any();
        let weight: u64 = kani::any();

        // Production contract: left < right < node_count, weight > 0,
        // no duplicate unordered pair.
        kani::assume(left < right);
        kani::assume(right < node_count);
        kani::assume(weight > 0);
        kani::assume(!seen[left][right]);

        seen[left][right] = true;
        edges.push(SimilarityEdge {
            left,
            right,
            weight,
        });
    }

    edges
}

/// Returns `(node_count, edges, adjacency)` from a fully constrained symbolic
/// model. Harnesses that need `node_count > 0` must apply that assumption
/// on the returned value after calling this helper.
fn symbolic_adjacency() -> (usize, Vec<SimilarityEdge>, Vec<Vec<(usize, u64)>>) {
    let node_count = constrained_node_count();
    let edges = constrained_edges(node_count);
    let adjacency = build_adjacency(node_count, &edges);
    (node_count, edges, adjacency)
}

/// Verifies that `build_adjacency` returns exactly `node_count` buckets.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_length() {
    let (node_count, _edges, adjacency) = symbolic_adjacency();

    assert!(adjacency.len() == node_count);
}

/// Verifies that every input edge is preserved in both directions.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_preserves_edges() {
    let (node_count, edges, adjacency) = symbolic_adjacency();
    kani::assume(node_count > 0);

    for edge in &edges {
        let forward_found = adjacency[edge.left]
            .iter()
            .any(|&(neighbour, weight)| neighbour == edge.right && weight == edge.weight);
        assert!(forward_found);

        let reverse_found = adjacency[edge.right]
            .iter()
            .any(|&(neighbour, weight)| neighbour == edge.left && weight == edge.weight);
        assert!(reverse_found);
    }
}

/// Verifies that every neighbour index in every adjacency bucket is within
/// bounds.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_indices_in_bounds() {
    let (node_count, _edges, adjacency) = symbolic_adjacency();

    for bucket in &adjacency {
        for &(neighbour, _weight) in bucket {
            assert!(neighbour < node_count);
        }
    }
}

/// Verifies that adjacency lists are symmetric: for every entry
/// `(node -> neighbour, weight)`, the mirrored entry
/// `(neighbour -> node, weight)` also exists.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_symmetry() {
    let (node_count, _edges, adjacency) = symbolic_adjacency();
    kani::assume(node_count > 0);

    for (node, bucket) in adjacency.iter().enumerate() {
        for &(neighbour, weight) in bucket {
            let mirror_found =
                adjacency[neighbour]
                    .iter()
                    .any(|&(mirror_neighbour, mirror_weight)| {
                        mirror_neighbour == node && mirror_weight == weight
                    });
            assert!(mirror_found);
        }
    }
}

/// Verifies that each per-node neighbour list is sorted by neighbour index
/// after construction.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_sorted_neighbours() {
    let (_node_count, _edges, adjacency) = symbolic_adjacency();

    for bucket in &adjacency {
        for window in bucket.windows(2) {
            assert!(window[0].0 <= window[1].0);
        }
    }
}
