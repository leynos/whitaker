//! Bounded model-checking harnesses for `build_adjacency`.

use super::shared::{incident_degree, is_edge_in_input, symbolic_adjacency};

/// Verifies that `build_adjacency` returns exactly `node_count` buckets.
#[kani::proof]
#[kani::unwind(7)]
fn verify_build_adjacency_length() {
    let (node_count, _edges, adjacency) = symbolic_adjacency();

    assert_eq!(adjacency.len(), node_count);
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

/// Verifies that adjacency lists are symmetric.
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

/// Verifies that each per-node neighbour list is sorted by neighbour index.
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
