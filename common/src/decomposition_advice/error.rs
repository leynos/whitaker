//! Typed failures for malformed decomposition-analysis state.
//!
//! The analysis queries return these errors to their callers rather than
//! logging or metering them inside the domain, so a diagnostic adapter can
//! choose its own observability policy. Keep the reasons finite; they must not
//! expose method names, feature keys, or other unbounded values.

/// Identifies invalid graph or feature state rejected during advice analysis.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DecompositionError {
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
