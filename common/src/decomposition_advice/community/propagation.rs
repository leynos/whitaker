//! Run deterministic weighted label propagation over checked adjacency rows.

use super::super::error::DecompositionError;
use super::super::vector::MethodFeatureVector;
use super::selection::best_neighbour_label;

/// Report the final labels and number of propagation passes.
///
/// `labels` contains one final community label per input method vector. Each
/// label is a valid node index because propagation starts from the identity
/// labelling `0..vectors.len()` and only adopts checked neighbour labels.
///
/// `iteration_count` records how many full passes over the active-node set were
/// executed. The count includes the final pass that detects convergence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LabelPropagationReport {
    pub(crate) labels: Vec<usize>,
    pub(crate) iteration_count: usize,
}

/// Carry the active nodes and pass bound for one propagation invocation.
struct PropagationPlan<'a> {
    active_nodes: &'a [usize],
    max_iterations: usize,
}

/// Propagate labels and report the completed passes.
///
/// If adjacency contains no active nodes, or `max_iterations` is zero, the
/// function returns the initial self-labelling with zero iterations.
///
/// # Errors
///
/// Returns an error when adjacency rows, neighbour labels, or label slots do
/// not match the supplied vectors.
pub(crate) fn propagate_labels_report(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    max_iterations: usize,
) -> Result<LabelPropagationReport, DecompositionError> {
    if adjacency.len() != vectors.len() {
        return Err(DecompositionError::AdjacencyRowCountMismatch {
            expected_rows: vectors.len(),
            actual_rows: adjacency.len(),
        });
    }

    let mut labels: Vec<usize> = (0..vectors.len()).collect();
    let active_nodes = active_nodes(adjacency);
    if active_nodes.is_empty() {
        log::debug!(
            "label propagation: no active nodes, skipping (total_nodes={})",
            vectors.len(),
        );
        return Ok(LabelPropagationReport {
            labels,
            iteration_count: 0,
        });
    }

    let plan = PropagationPlan {
        active_nodes: &active_nodes,
        max_iterations,
    };
    let iteration_count = run_iterations(vectors, adjacency, &plan, &mut labels)?;
    Ok(LabelPropagationReport {
        labels,
        iteration_count,
    })
}

/// Collect nodes that have at least one neighbour.
fn active_nodes(adjacency: &[Vec<(usize, u64)>]) -> Vec<usize> {
    adjacency
        .iter()
        .enumerate()
        .filter_map(|(node, neighbours)| (!neighbours.is_empty()).then_some(node))
        .collect()
}

/// Run passes until labels stabilize or the configured limit is reached.
///
/// # Errors
///
/// Returns an error when an active node or one of its propagated labels is
/// invalid.
fn run_iterations(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    plan: &PropagationPlan<'_>,
    labels: &mut [usize],
) -> Result<usize, DecompositionError> {
    let mut iteration_count = 0;
    for _ in 0..plan.max_iterations {
        iteration_count += 1;
        if !run_propagation_pass(vectors, adjacency, plan.active_nodes, labels)? {
            log::debug!(
                "label propagation converged: nodes={}, active_nodes={}, iterations={}",
                vectors.len(),
                plan.active_nodes.len(),
                iteration_count,
            );
            break;
        }
    }

    if iteration_count == plan.max_iterations {
        log::debug!(
            "label propagation reached iteration limit: nodes={}, active_nodes={}, max_iterations={}",
            vectors.len(),
            plan.active_nodes.len(),
            plan.max_iterations,
        );
    }
    Ok(iteration_count)
}

/// Run one label-propagation pass over the active nodes.
///
/// Return `true` when a node adopts a new label.
///
/// # Errors
///
/// Returns an error when an active node has no corresponding label slot or
/// one of its neighbours has an invalid label.
pub(super) fn run_propagation_pass(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    active_nodes: &[usize],
    labels: &mut [usize],
) -> Result<bool, DecompositionError> {
    let mut changed = false;
    for &node in active_nodes {
        let Some(best_label) = best_neighbour_label(node, labels, adjacency, vectors)? else {
            continue;
        };
        let node_count = labels.len();
        let label_slot = labels
            .get_mut(node)
            .ok_or(DecompositionError::InvalidLabelSlot { node_count })?;
        if *label_slot != best_label {
            *label_slot = best_label;
            changed = true;
        }
    }
    Ok(changed)
}

/// Check that every active node already holds its best neighbour label.
///
/// # Errors
///
/// Returns an error when adjacency references a missing node or label.
pub(super) fn labels_are_stable(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    labels: &[usize],
) -> Result<bool, DecompositionError> {
    for (node, neighbours) in adjacency.iter().enumerate() {
        if !neighbours.is_empty() && !node_holds_best_label(node, vectors, adjacency, labels)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Compare a node's current label with its best adjacent label.
fn node_holds_best_label(
    node: usize,
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    labels: &[usize],
) -> Result<bool, DecompositionError> {
    let Some(best_label) = best_neighbour_label(node, labels, adjacency, vectors)? else {
        return Ok(true);
    };
    let current_label = labels
        .get(node)
        .ok_or(DecompositionError::InvalidLabelSlot {
            node_count: labels.len(),
        })?;
    Ok(*current_label == best_label)
}
