//! Shared bounded symbolic models for decomposition-advice Kani proofs.

use super::super::{SimilarityEdge, build_adjacency};

/// Maximum number of nodes in the bounded model.
pub(super) const MAX_NODES: usize = 3;

/// Maximum number of edges in the bounded model.
pub(super) const MAX_EDGES: usize = 3;

/// Maximum iteration bound explored by propagation proofs.
pub(super) const MAX_ITERATIONS: usize = 3;

/// Generates a symbolic node count constrained to `[0, MAX_NODES]`.
pub(super) fn constrained_node_count() -> usize {
    let node_count: usize = kani::any();
    kani::assume(node_count <= MAX_NODES);
    node_count
}

/// Generates a symbolic iteration count constrained to `[0, MAX_ITERATIONS]`.
pub(super) fn bounded_iteration_count() -> usize {
    let max_iterations: usize = kani::any();
    kani::assume(max_iterations <= MAX_ITERATIONS);
    max_iterations
}

/// Materialises a concrete `Vec<SimilarityEdge>` from a symbolic fixed-size
/// array, constraining each active edge to the production contract.
pub(super) fn constrained_edges(node_count: usize) -> Vec<SimilarityEdge> {
    let active_count: usize = kani::any();
    kani::assume(active_count <= MAX_EDGES);

    let mut edges = Vec::new();
    let mut seen = [[false; MAX_NODES]; MAX_NODES];

    for _ in 0..active_count {
        let left: usize = kani::any();
        let right: usize = kani::any();
        let weight: u64 = kani::any();

        kani::assume(left < right);
        kani::assume(right < node_count);
        kani::assume(weight > 0);
        kani::assume(!seen[left][right]);

        seen[left][right] = true;
        edges.push(SimilarityEdge::new(left, right, weight));
    }

    edges
}

/// Returns `(node_count, edges, adjacency)` from a fully constrained symbolic
/// model.
pub(super) fn symbolic_adjacency() -> (usize, Vec<SimilarityEdge>, Vec<Vec<(usize, u64)>>) {
    let node_count = constrained_node_count();
    let edges = constrained_edges(node_count);
    let adjacency = build_adjacency(node_count, &edges);
    (node_count, edges, adjacency)
}

/// Returns the number of input edges incident on `node` (as either endpoint).
pub(super) fn incident_degree(edges: &[SimilarityEdge], node: usize) -> usize {
    edges
        .iter()
        .filter(|edge| edge.left == node || edge.right == node)
        .count()
}

/// Returns `true` when `(node -> neighbour, weight)` is backed by an input
/// edge in either orientation.
pub(super) fn is_edge_in_input(
    edges: &[SimilarityEdge],
    node: usize,
    neighbour: usize,
    weight: u64,
) -> bool {
    edges.iter().any(|edge| {
        (edge.left == node && edge.right == neighbour && edge.weight == weight)
            || (edge.right == node && edge.left == neighbour && edge.weight == weight)
    })
}
