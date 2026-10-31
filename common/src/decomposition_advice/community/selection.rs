//! Score neighbouring labels and apply deterministic tie-breaking.

use std::collections::BTreeMap;

use super::super::error::DecompositionError;
use super::super::vector::MethodFeatureVector;

/// Select the highest-scoring label among a node's neighbours.
///
/// # Errors
///
/// Returns an error when a neighbour or its propagated label is outside the
/// vector range.
pub(super) fn best_neighbour_label(
    node: usize,
    labels: &[usize],
    adjacency: &[Vec<(usize, u64)>],
    vectors: &[MethodFeatureVector],
) -> Result<Option<usize>, DecompositionError> {
    let neighbours = adjacency
        .get(node)
        .ok_or(DecompositionError::InvalidLabelSlot {
            node_count: labels.len(),
        })?;
    if neighbours.is_empty() {
        return Ok(None);
    }

    let mut scores = BTreeMap::new();
    let mut best: Option<(usize, u64)> = None;
    for &(neighbour, weight) in neighbours {
        let label = checked_neighbour_label(neighbour, labels, vectors.len())?;
        let score = score_label(&mut scores, label, weight);
        if should_replace_best(best, label, score, vectors)? {
            best = Some((label, score));
        }
    }
    Ok(best.map(|(label, _)| label))
}

/// Return a neighbour's propagated label after checking both index domains.
///
/// # Errors
///
/// Returns an error when either the neighbour index or its label is invalid.
fn checked_neighbour_label(
    neighbour: usize,
    labels: &[usize],
    vector_count: usize,
) -> Result<usize, DecompositionError> {
    let label = *labels
        .get(neighbour)
        .ok_or(DecompositionError::InvalidNeighbourLabel {
            node_count: labels.len(),
        })?;
    if label >= vector_count {
        return Err(DecompositionError::InvalidNeighbourLabel {
            node_count: vector_count,
        });
    }
    Ok(label)
}

/// Add `weight` to a label's accumulated score and return the new total.
fn score_label(scores: &mut BTreeMap<usize, u64>, label: usize, weight: u64) -> u64 {
    let score = scores.entry(label).or_default();
    *score += weight;
    *score
}

/// Compare candidates by score, method name, and label index.
///
/// # Errors
///
/// Returns an error if either label does not identify a method vector.
fn should_replace_best(
    current_best: Option<(usize, u64)>,
    candidate_label: usize,
    candidate_score: u64,
    vectors: &[MethodFeatureVector],
) -> Result<bool, DecompositionError> {
    let Some((best_label, best_score)) = current_best else {
        return Ok(true);
    };
    if candidate_score != best_score {
        return Ok(candidate_score > best_score);
    }
    Ok(label_order_key(candidate_label, vectors)? < label_order_key(best_label, vectors)?)
}

/// Return the lexical method-name and label-index tie-break key.
///
/// # Errors
///
/// Returns an error when the label does not identify a method vector.
fn label_order_key(
    label: usize,
    vectors: &[MethodFeatureVector],
) -> Result<(&str, usize), DecompositionError> {
    let vector = vectors
        .get(label)
        .ok_or(DecompositionError::InvalidNeighbourLabel {
            node_count: vectors.len(),
        })?;
    Ok((vector.method_name(), label))
}
