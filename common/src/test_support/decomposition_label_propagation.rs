//! Observable label-propagation seams for decomposition advice tests.

use crate::decomposition_advice::{
    community::propagate_labels_report as runtime_propagate_labels_report, minimal_feature_vector,
};

use super::adjacency::{AdjacencyError, EdgeInput, validate_edges};

/// Observable label-propagation results for declarative graph input.
///
/// # Examples
///
/// ```rust
/// use whitaker_common::test_support::decomposition::{
///     EdgeInput, label_propagation_report,
/// };
///
/// let report = label_propagation_report(
///     &["gamma", "alpha", "beta"],
///     &[EdgeInput {
///         left: 0,
///         right: 1,
///         weight: 5,
///     }],
///     1,
/// )
/// .expect("valid input");
///
/// assert_eq!(report.labels().len(), 3);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabelPropagationReport {
    labels: Vec<usize>,
    iteration_count: usize,
    has_active_nodes: bool,
}

impl LabelPropagationReport {
    /// Returns the final label vector.
    #[must_use]
    pub fn labels(&self) -> &[usize] {
        &self.labels
    }

    /// Returns the propagated label for `node`, or `None` if it is out of
    /// range.
    #[must_use]
    pub fn label_of(&self, node: usize) -> Option<usize> {
        self.labels.get(node).copied()
    }

    /// Returns the number of propagation passes performed.
    #[must_use]
    pub fn iteration_count(&self) -> usize {
        self.iteration_count
    }

    /// Returns `true` when the graph contains at least one active node.
    #[must_use]
    pub fn has_active_nodes(&self) -> bool {
        self.has_active_nodes
    }

    /// Returns `true` when every label is a valid node index.
    #[must_use]
    pub fn all_labels_in_bounds(&self) -> bool {
        self.labels.iter().all(|&label| label < self.labels.len())
    }
}

/// Runs label propagation over declarative graph input.
///
/// # Errors
///
/// Returns a typed validation error when any edge violates the production
/// adjacency contract.
pub fn label_propagation_report(
    method_names: &[&str],
    edges: &[EdgeInput],
    max_iterations: usize,
) -> Result<LabelPropagationReport, AdjacencyError> {
    let similarity_edges = validate_edges(method_names.len(), edges)?;
    let adjacency = crate::decomposition_advice::community::build_adjacency(
        method_names.len(),
        &similarity_edges,
    );
    let vectors = method_names
        .iter()
        .map(|method_name| minimal_feature_vector(method_name))
        .collect::<Vec<_>>();
    let report = runtime_propagate_labels_report(&vectors, &adjacency, max_iterations);

    Ok(LabelPropagationReport {
        labels: report.labels().to_vec(),
        iteration_count: report.iteration_count(),
        has_active_nodes: report.has_active_nodes(),
    })
}
