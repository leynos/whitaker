//! Similarity-graph construction and deterministic community detection.

use std::collections::BTreeMap;

use super::vector::{
    MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED, MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
    MethodFeatureVector, cosine_threshold_met, dot_product,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SimilarityEdge {
    left: usize,
    right: usize,
    weight: u64,
}

impl SimilarityEdge {
    // Used by test_support::decomposition::adjacency_report and unit tests.
    pub(crate) fn new(left: usize, right: usize, weight: u64) -> Self {
        Self {
            left,
            right,
            weight,
        }
    }

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
                MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
                MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LabelPropagationReport {
    labels: Vec<usize>,
    iteration_count: usize,
    has_active_nodes: bool,
}

impl LabelPropagationReport {
    pub(crate) fn labels(&self) -> &[usize] {
        &self.labels
    }

    pub(crate) fn iteration_count(&self) -> usize {
        self.iteration_count
    }

    pub(crate) fn has_active_nodes(&self) -> bool {
        self.has_active_nodes
    }

    fn into_labels(self) -> Vec<usize> {
        self.labels
    }
}

pub(crate) fn build_adjacency(
    node_count: usize,
    edges: &[SimilarityEdge],
) -> Vec<Vec<(usize, u64)>> {
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

pub(crate) fn propagate_labels(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    max_iterations: usize,
) -> Vec<usize> {
    propagate_labels_report(vectors, adjacency, max_iterations).into_labels()
}

pub(crate) fn propagate_labels_report(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    max_iterations: usize,
) -> LabelPropagationReport {
    let mut labels: Vec<usize> = (0..vectors.len()).collect();
    let active_nodes: Vec<_> = adjacency
        .iter()
        .enumerate()
        .filter_map(|(node, neighbours)| (!neighbours.is_empty()).then_some(node))
        .collect();
    if active_nodes.is_empty() {
        return LabelPropagationReport {
            labels,
            iteration_count: 0,
            has_active_nodes: false,
        };
    }
    let mut iteration_count = 0;

    for _ in 0..max_iterations {
        iteration_count += 1;
        let mut changed = false;

        for &node in &active_nodes {
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

    LabelPropagationReport {
        labels,
        iteration_count,
        has_active_nodes: true,
    }
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

    let mut scores = vec![0; labels.len()];
    let mut best: Option<(usize, u64)> = None;

    for &(neighbour, weight) in neighbours {
        let label = labels[neighbour];
        scores[label] += weight;
        let score = scores[label];

        if should_replace_best(best, label, score, vectors) {
            best = Some((label, score));
        }
    }

    best.map(|(label, _)| label)
}

fn should_replace_best(
    current_best: Option<(usize, u64)>,
    candidate_label: usize,
    candidate_score: u64,
    vectors: &[MethodFeatureVector],
) -> bool {
    match current_best {
        None => true,
        Some(best) => is_better_label((candidate_label, candidate_score), best, vectors),
    }
}

fn is_better_label(
    candidate: (usize, u64),
    best: (usize, u64),
    vectors: &[MethodFeatureVector],
) -> bool {
    candidate.1 > best.1
        || (candidate.1 == best.1 && breaks_label_tie(candidate.0, best.0, vectors))
}

fn breaks_label_tie(
    candidate_label: usize,
    best_label: usize,
    vectors: &[MethodFeatureVector],
) -> bool {
    let candidate_name = vectors[candidate_label].method_name();
    let best_name = vectors[best_label].method_name();

    candidate_name < best_name || (candidate_name == best_name && candidate_label < best_label)
}

#[cfg(kani)]
#[path = "community_kani/mod.rs"]
mod verify;
