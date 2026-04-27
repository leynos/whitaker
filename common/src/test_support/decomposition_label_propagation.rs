//! Observable label-propagation seams for decomposition advice tests.

use crate::decomposition_advice::{
    MethodProfileBuilder, build_feature_vector,
    community::{
        LabelPropagationReport as RuntimeLabelPropagationReport,
        propagate_labels_report as runtime_propagate_labels_report,
    },
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
    runtime: RuntimeLabelPropagationReport,
    has_active_nodes: bool,
}

impl LabelPropagationReport {
    /// Returns the final label vector.
    #[must_use]
    pub fn labels(&self) -> &[usize] {
        &self.runtime.labels
    }

    /// Returns the propagated label for `node`, or `None` if it is out of
    /// range.
    #[must_use]
    pub fn label_of(&self, node: usize) -> Option<usize> {
        self.labels().get(node).copied()
    }

    /// Returns the number of propagation passes performed.
    #[must_use]
    pub fn iteration_count(&self) -> usize {
        self.runtime.iteration_count
    }

    /// Returns `true` when the graph contains at least one active node.
    #[must_use]
    pub fn has_active_nodes(&self) -> bool {
        self.has_active_nodes
    }

    /// Returns `true` when every label is a valid node index.
    #[must_use]
    pub fn all_labels_in_bounds(&self) -> bool {
        self.runtime
            .labels
            .iter()
            .all(|&label| label < self.runtime.labels.len())
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
        .map(|method_name| build_feature_vector(&MethodProfileBuilder::new(*method_name).build()))
        .collect::<Vec<_>>();
    let has_active_nodes = adjacency.iter().any(|neighbours| !neighbours.is_empty());
    let runtime = runtime_propagate_labels_report(&vectors, &adjacency, max_iterations);

    Ok(LabelPropagationReport {
        runtime,
        has_active_nodes,
    })
}
