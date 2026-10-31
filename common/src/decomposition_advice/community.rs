//! Similarity-graph construction and deterministic community detection.

use std::collections::BTreeMap;

mod propagation;
mod selection;

use super::error::DecompositionError;
pub(crate) use super::graph::{SimilarityEdge, build_similarity_edges};
use super::vector::MethodFeatureVector;
use propagation::labels_are_stable;
#[cfg(test)]
use propagation::run_propagation_pass;
pub(crate) use propagation::{LabelPropagationReport, propagate_labels_report};

/// Detect communities and reject malformed intermediate graph state.
///
/// # Errors
///
/// Returns an error when adjacency construction or label propagation finds
/// invalid node or label state.
pub(crate) fn detect_communities(
    vectors: &[MethodFeatureVector],
) -> Result<Vec<Vec<usize>>, DecompositionError> {
    if vectors.is_empty() {
        return Ok(Vec::new());
    }

    let edges = build_similarity_edges(vectors);
    let adjacency = build_adjacency(vectors.len(), &edges)?;
    let max_iterations = vectors.len().saturating_mul(2).max(1);
    let report = propagate_labels_report(vectors, &adjacency, max_iterations)?;
    if log::log_enabled!(log::Level::Debug) {
        let converged = labels_are_stable(vectors, &adjacency, &report.labels)?;
        log::debug!(
            "label propagation complete: nodes={}, iterations={}, converged={}",
            vectors.len(),
            report.iteration_count,
            converged,
        );
    }

    let groups = group_nodes_by_label(report.labels);
    Ok(sort_communities(groups, vectors))
}

/// Build symmetric adjacency lists with neighbours ordered by node index.
///
/// # Errors
///
/// Returns an error without exposing a partial graph when an endpoint is
/// outside `0..node_count`.
pub(crate) fn build_adjacency(
    node_count: usize,
    edges: &[SimilarityEdge],
) -> Result<Vec<Vec<(usize, u64)>>, DecompositionError> {
    let mut adjacency = build_adjacency_rows_unordered(node_count, edges)?;

    for neighbours in &mut adjacency {
        neighbours.sort_by_key(|left| left.0);
    }
    Ok(adjacency)
}

/// Build symmetric adjacency rows before deterministic neighbour ordering.
///
/// Keep this helper private to community graph construction. Production callers
/// use `build_adjacency` for ordered rows, and generated graph properties check
/// ordering through the production function.
///
/// # Errors
///
/// Returns an error without exposing partial rows when an endpoint is outside
/// `0..node_count`.
fn build_adjacency_rows_unordered(
    node_count: usize,
    edges: &[SimilarityEdge],
) -> Result<Vec<Vec<(usize, u64)>>, DecompositionError> {
    if edges
        .iter()
        .any(|edge| edge.left >= node_count || edge.right >= node_count)
    {
        return Err(DecompositionError::InvalidEdgeEndpoint {
            node_count,
            edge_count: edges.len(),
        });
    }

    let mut adjacency = vec![Vec::new(); node_count];
    for edge in edges {
        let Some(left_neighbours) = adjacency.get_mut(edge.left) else {
            return Err(DecompositionError::InvalidEdgeEndpoint {
                node_count,
                edge_count: edges.len(),
            });
        };
        left_neighbours.push((edge.right, edge.weight));
        let Some(right_neighbours) = adjacency.get_mut(edge.right) else {
            return Err(DecompositionError::InvalidEdgeEndpoint {
                node_count,
                edge_count: edges.len(),
            });
        };
        right_neighbours.push((edge.left, edge.weight));
    }

    Ok(adjacency)
}

/// Group each node index under its propagated community label.
fn group_nodes_by_label(labels: Vec<usize>) -> Vec<Vec<usize>> {
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for (node, label) in labels.into_iter().enumerate() {
        groups.entry(label).or_default().push(node);
    }
    groups.into_values().collect()
}

/// Sort nodes and communities by their stable method-name ordering.
fn sort_communities(
    mut communities: Vec<Vec<usize>>,
    vectors: &[MethodFeatureVector],
) -> Vec<Vec<usize>> {
    // Node indices come from `0..vectors.len()`, so these lookups cannot miss.
    let method_name = |node: usize| vectors.get(node).map(MethodFeatureVector::method_name);

    for community in &mut communities {
        community.sort_by(|left, right| method_name(*left).cmp(&method_name(*right)));
    }

    communities.sort_by(|left, right| {
        right.len().cmp(&left.len()).then_with(|| {
            let left_name = left.first().and_then(|&node| method_name(node));
            let right_name = right.first().and_then(|&node| method_name(node));
            left_name.cmp(&right_name)
        })
    });
    communities
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
#[path = "community_kani/mod.rs"]
mod verify;
