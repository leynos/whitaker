//! Similarity-graph construction and deterministic community detection.

use std::collections::BTreeMap;

use super::vector::{MethodFeatureVector, cosine_threshold_met, dot_product};

const MIN_SIMILARITY_NUMERATOR: u64 = 1;
const MIN_SIMILARITY_DENOMINATOR: u64 = 25;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SimilarityEdge {
    left: usize,
    right: usize,
    weight: u64,
}

impl SimilarityEdge {
    #[cfg(test)]
    pub(crate) fn left(&self) -> usize {
        self.left
    }

    #[cfg(test)]
    pub(crate) fn right(&self) -> usize {
        self.right
    }

    #[cfg(test)]
    pub(crate) fn weight(&self) -> u64 {
        self.weight
    }
}

pub(crate) fn build_similarity_edges(vectors: &[MethodFeatureVector]) -> Vec<SimilarityEdge> {
    let mut edges = Vec::new();

    for left in 0..vectors.len() {
        for right in (left + 1)..vectors.len() {
            if !cosine_threshold_met(
                &vectors[left],
                &vectors[right],
                MIN_SIMILARITY_NUMERATOR,
                MIN_SIMILARITY_DENOMINATOR,
            ) {
                continue;
            }

            let weight = dot_product(vectors[left].weights(), vectors[right].weights());
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

pub(crate) fn detect_communities(vectors: &[MethodFeatureVector]) -> Vec<Vec<usize>> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let edges = build_similarity_edges(vectors);
    let adjacency = build_adjacency(vectors.len(), &edges);
    let max_iterations = vectors.len().saturating_mul(2).max(1);
    let labels = propagate_labels(vectors, &adjacency, max_iterations);

    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (node, label) in labels.into_iter().enumerate() {
        groups.entry(label).or_default().push(node);
    }

    let mut communities: Vec<Vec<usize>> = groups.into_values().collect();
    for community in &mut communities {
        community.sort_by(|left, right| {
            vectors[*left]
                .method_name()
                .cmp(vectors[*right].method_name())
        });
    }

    communities.sort_by(|left, right| {
        right.len().cmp(&left.len()).then_with(|| {
            vectors[left[0]]
                .method_name()
                .cmp(vectors[right[0]].method_name())
        })
    });
    communities
}

fn build_adjacency(node_count: usize, edges: &[SimilarityEdge]) -> Vec<Vec<(usize, u64)>> {
    let mut adjacency = vec![Vec::new(); node_count];

    for edge in edges {
        adjacency[edge.left].push((edge.right, edge.weight));
        adjacency[edge.right].push((edge.left, edge.weight));
    }

    for neighbours in &mut adjacency {
        neighbours.sort_by(|left, right| left.0.cmp(&right.0));
    }

    adjacency
}

fn propagate_labels(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    max_iterations: usize,
) -> Vec<usize> {
    let mut labels: Vec<usize> = (0..vectors.len()).collect();

    for _ in 0..max_iterations {
        let mut changed = false;

        for node in 0..vectors.len() {
            let Some(best_label) = best_neighbour_label(node, &labels, adjacency, vectors) else {
                continue;
            };

            if best_label != labels[node] {
                labels[node] = best_label;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    labels
}

fn best_neighbour_label(
    node: usize,
    labels: &[usize],
    adjacency: &[Vec<(usize, u64)>],
    vectors: &[MethodFeatureVector],
) -> Option<usize> {
    let neighbours = &adjacency[node];
    if neighbours.is_empty() {
        return None;
    }

    let mut scores: BTreeMap<usize, u64> = BTreeMap::new();
    for &(neighbour, weight) in neighbours {
        let label = labels[neighbour];
        *scores.entry(label).or_insert(0) += weight;
    }

    scores
        .into_iter()
        .max_by(|(left_label, left_score), (right_label, right_score)| {
            left_score.cmp(right_score).then_with(|| {
                vectors[*right_label]
                    .method_name()
                    .cmp(vectors[*left_label].method_name())
            })
        })
        .map(|(label, _)| label)
}
