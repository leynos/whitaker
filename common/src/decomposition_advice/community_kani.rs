//! Bounded model-checking harnesses for [`super::build_adjacency`].
//!
//! These Kani harnesses verify that `build_adjacency` preserves every valid
//! similarity edge, never emits out-of-bounds neighbour indices, always
//! produces symmetric adjacency lists, and emits *only* edges that were
//! present in the input (no spurious edges). The harnesses use symbolic
//! fixed-size arrays with an active-length field rather than symbolic `Vec`
//! values, giving Kani a small explicit search space.

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
        edges.push(SimilarityEdge::new(left, right, weight));
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

/// Returns the number of input edges incident on `node` (as either endpoint).
fn incident_degree(edges: &[SimilarityEdge], node: usize) -> usize {
    edges
        .iter()
        .filter(|e| e.left == node || e.right == node)
        .count()
}

/// Returns `true` when `(node → neighbour, weight)` is backed by an input
/// edge (considering both orientations).
fn is_edge_in_input(edges: &[SimilarityEdge], node: usize, neighbour: usize, weight: u64) -> bool {
    edges.iter().any(|e| {
        (e.left == node && e.right == neighbour && e.weight == weight)
            || (e.right == node && e.left == neighbour && e.weight == weight)
    })
}

/// Verifies that `build_adjacency` returns exactly `node_count` buckets.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_length() {
    let (node_count, _edges, adjacency) = symbolic_adjacency();

    assert_eq!(adjacency.len(), node_count);
}

/// Verifies that every input edge is preserved in both directions.
///
/// The `node_count > 0` guard documents intent: when `node_count = 0` the
/// `right < node_count` constraint inside `constrained_edges` is
/// unsatisfiable, so `edges` is always empty and the assertion loop never
/// executes. The proof outcome is unchanged without the guard, but we
/// retain it for readability.
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
///
/// The `node_count > 0` guard documents intent — see
/// [`verify_build_adjacency_preserves_edges`] for the rationale.
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

/// Verifies that `build_adjacency` emits only edges present in the input.
///
/// Without this exclusion property, a buggy implementation that injected
/// extra symmetric, in-bounds edges would pass the other five harnesses.
/// This harness closes that gap by asserting that every adjacency entry
/// traces back to an input edge.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_no_spurious_edges() {
    let (node_count, edges, adjacency) = symbolic_adjacency();
    kani::assume(node_count > 0);

    for (node, bucket) in adjacency.iter().enumerate() {
        assert_eq!(bucket.len(), incident_degree(&edges, node));
        for &(neighbour, weight) in bucket {
            assert!(is_edge_in_input(&edges, node, neighbour, weight));
        }
    }
}

/// Verifies that each per-node neighbour list is sorted by neighbour index
/// after construction.
///
/// With `MAX_NODES = 3`, no node can have more than 2 neighbours, so this
/// harness only exercises 0-, 1-, and 2-element bucket sorts. A sort defect
/// that manifests only at 3+ elements is invisible at this bound. Raising
/// `MAX_NODES` to 4 (C(4,2) = 6 edges) would expose such bugs, but Rust's
/// standard `sort_by` generates deeply nested control-flow paths that cause
/// CBMC state-space explosion at that scale. The current bound is a
/// conscious engineering trade-off documented in
/// `docs/brain-trust-lints-design.md`.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_sorted_neighbours() {
    let (_node_count, _edges, adjacency) = symbolic_adjacency();

    for bucket in &adjacency {
        for window in bucket.windows(2) {
            // NOTE: With `MAX_NODES = 3`, `bucket.windows(2)` only observes
            // 0-, 1-, and 2-element neighbour lists, so
            // `assert!(window[0].0 <= window[1].0)` cannot expose sort defects
            // that manifest only on buckets of length 3 or greater. See the
            // function-level comment above for the rationale and trade-off.
            assert!(window[0].0 <= window[1].0);
        }
    }
}
