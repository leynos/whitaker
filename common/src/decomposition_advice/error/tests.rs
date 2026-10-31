//! Tests for the bounded observability context attached to analysis errors.

use super::DecompositionError;
use rstest::rstest;

/// Keep every error mapped to a finite operation and reason pair.
#[rstest]
#[case(
    DecompositionError::InvalidEdgeEndpoint { node_count: 4, edge_count: 3 },
    ("build_adjacency", "invalid_edge_endpoint", Some(4), Some(3))
)]
#[case(
    DecompositionError::AdjacencyRowCountMismatch { expected_rows: 4, actual_rows: 3 },
    ("propagate_labels", "adjacency_row_count_mismatch", Some(4), Some(3))
)]
#[case(
    DecompositionError::InvalidNeighbourLabel { node_count: 4 },
    ("propagate_labels", "invalid_neighbour_label", Some(4), None)
)]
#[case(
    DecompositionError::InvalidLabelSlot { node_count: 4 },
    ("propagate_labels", "invalid_label_slot", Some(4), None)
)]
#[case(
    DecompositionError::InvalidCommunityIndex { vector_count: 4, community_count: 3 },
    ("build_suggestion", "invalid_community_index", Some(4), Some(3))
)]
#[case(
    DecompositionError::MissingFeatureMetadata { vector_count: 4 },
    ("aggregate_features", "missing_feature_metadata", Some(4), None)
)]
fn errors_use_bounded_observability_labels(
    #[case] error: DecompositionError,
    #[case] expected: (&'static str, &'static str, Option<usize>, Option<usize>),
) {
    assert_eq!(error.context(), expected);
}
