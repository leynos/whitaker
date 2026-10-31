//! Tests for propagation failures that cannot arise from validated graphs.

use crate::decomposition_advice::{
    community::run_propagation_pass, error::DecompositionError, minimal_feature_vector,
};

/// Reject a propagation pass whose active node has no label slot.
#[test]
fn propagation_rejects_an_invalid_label_slot_without_partial_updates() {
    let vectors = [
        minimal_feature_vector("alpha"),
        minimal_feature_vector("beta"),
        minimal_feature_vector("gamma"),
    ];
    let adjacency = vec![Vec::new(), Vec::new(), vec![(1, 5)]];
    let active_nodes = [2];
    let mut labels = vec![0, 1];

    let result = run_propagation_pass(&vectors, &adjacency, &active_nodes, &mut labels);

    assert!(matches!(
        result,
        Err(DecompositionError::InvalidLabelSlot { node_count: 2 })
    ));
    assert_eq!(labels, [0, 1]);
}
