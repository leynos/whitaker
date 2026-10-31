//! Build weighted similarity edges for decomposition community detection.
//!
//! Keep this module private to decomposition advice. It owns vector-pair edge
//! construction; adjacency storage and propagation remain in `community`.

use super::vector::{
    MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED, MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
    MethodFeatureVector, cosine_threshold_met, dot_product,
};

/// Weighted connection between two method feature vectors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SimilarityEdge {
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) weight: u64,
}

impl SimilarityEdge {
    /// Create an edge with its node endpoints and similarity weight.
    pub(crate) const fn new(left: usize, right: usize, weight: u64) -> Self {
        Self {
            left,
            right,
            weight,
        }
    }

    /// Return the left endpoint.
    #[cfg(test)]
    pub(crate) const fn left(&self) -> usize {
        self.left
    }

    /// Return the right endpoint.
    #[cfg(test)]
    pub(crate) const fn right(&self) -> usize {
        self.right
    }

    /// Return the edge weight.
    #[cfg(test)]
    pub(crate) const fn weight(&self) -> u64 {
        self.weight
    }
}

/// Connect vector pairs that meet the configured cosine similarity threshold.
pub(crate) fn build_similarity_edges(vectors: &[MethodFeatureVector]) -> Vec<SimilarityEdge> {
    let mut edges = Vec::new();

    for (left, left_vector) in vectors.iter().enumerate() {
        for (right, right_vector) in vectors.iter().enumerate().skip(left + 1) {
            if !cosine_threshold_met(
                left_vector,
                right_vector,
                MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
                MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
            ) {
                continue;
            }

            let weight = dot_product(left_vector.weights(), right_vector.weights());
            if weight == 0 {
                continue;
            }

            edges.push(SimilarityEdge {
                left,
                right,
                weight,
            });
        }
    }

    edges
}
