//! Typed failures for malformed decomposition-analysis state.
//!
//! Keep error reasons finite so diagnostics and metrics remain useful without
//! exposing method names, feature keys, or other unbounded values.

/// Identifies invalid graph or feature state rejected during advice analysis.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum DecompositionError {
    /// An edge endpoint lies outside the graph's node range.
    #[error("similarity edge endpoint is outside the graph's node range")]
    InvalidEdgeEndpoint {
        /// Number of nodes expected in the graph.
        node_count: usize,
        /// Number of edges supplied for validation.
        edge_count: usize,
    },
    /// The adjacency row count differs from the node count.
    #[error("adjacency row count differs from the node count")]
    AdjacencyRowCountMismatch {
        /// Number of graph nodes expected by propagation.
        expected_rows: usize,
        /// Number of adjacency rows supplied by the caller.
        actual_rows: usize,
    },
    /// A neighbour cannot be mapped to an existing label.
    #[error("neighbour index is outside the label range")]
    InvalidNeighbourLabel {
        /// Number of labels available to propagation.
        node_count: usize,
    },
    /// A node cannot be mapped to an existing label slot.
    #[error("node index is outside the label range")]
    InvalidLabelSlot {
        /// Number of labels available to propagation.
        node_count: usize,
    },
    /// A community contains an index outside the feature-vector range.
    #[error("community index is outside the feature-vector range")]
    InvalidCommunityIndex {
        /// Number of feature vectors available to the suggestion builder.
        vector_count: usize,
        /// Number of node indices in the malformed community.
        community_count: usize,
    },
    /// A feature weight has no corresponding display metadata.
    #[error("feature metadata is missing for a weighted feature")]
    MissingFeatureMetadata {
        /// Number of feature vectors considered during aggregation.
        vector_count: usize,
    },
}

impl DecompositionError {
    /// Return finite metric labels and bounded context for this failure.
    const fn context(&self) -> (&'static str, &'static str, Option<usize>, Option<usize>) {
        match self {
            Self::InvalidEdgeEndpoint {
                node_count,
                edge_count,
            } => (
                "build_adjacency",
                "invalid_edge_endpoint",
                Some(*node_count),
                Some(*edge_count),
            ),
            Self::AdjacencyRowCountMismatch {
                expected_rows,
                actual_rows,
            } => (
                "propagate_labels",
                "adjacency_row_count_mismatch",
                Some(*expected_rows),
                Some(*actual_rows),
            ),
            Self::InvalidNeighbourLabel { node_count } => (
                "propagate_labels",
                "invalid_neighbour_label",
                Some(*node_count),
                None,
            ),
            Self::InvalidLabelSlot { node_count } => (
                "propagate_labels",
                "invalid_label_slot",
                Some(*node_count),
                None,
            ),
            Self::InvalidCommunityIndex {
                vector_count,
                community_count,
            } => (
                "build_suggestion",
                "invalid_community_index",
                Some(*vector_count),
                Some(*community_count),
            ),
            Self::MissingFeatureMetadata { vector_count } => (
                "aggregate_features",
                "missing_feature_metadata",
                Some(*vector_count),
                None,
            ),
        }
    }
}

/// Emit a warning and fixed-cardinality counter for rejected analysis state.
pub(crate) fn report_decomposition_error(error: &DecompositionError) {
    let (operation, reason, node_count, item_count) = error.context();
    metrics::describe_counter!(
        "whitaker.decomposition.invalid_state",
        "Invalid decomposition analysis state rejected before advice is rendered"
    );
    metrics::counter!(
        "whitaker.decomposition.invalid_state",
        "operation" => operation,
        "reason" => reason
    )
    .increment(1);
    tracing::warn!(
        operation,
        reason,
        node_count = ?node_count,
        item_count = ?item_count,
        "decomposition analysis rejected invalid state"
    );
}

#[cfg(test)]
mod tests;
