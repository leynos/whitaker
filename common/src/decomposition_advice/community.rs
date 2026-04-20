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

/// Observable output from deterministic label propagation.
///
/// `labels` contains one final community label per input method vector. Each
/// label is always a valid node index because propagation starts from the
/// identity labelling `0..vectors.len()` and only adopts labels already owned
/// by neighbours.
///
/// `iteration_count` records how many full passes over the active-node set
/// were executed. The count increments once per attempted propagation pass,
/// including the final pass that detects convergence. A value of `0` therefore
/// means either `max_iterations == 0` or the graph had no active nodes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LabelPropagationReport {
    pub(crate) labels: Vec<usize>,
    pub(crate) iteration_count: usize,
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
    propagate_labels_report(vectors, adjacency, max_iterations).labels
}

/// Runs deterministic weighted label propagation and reports its final state.
///
/// The returned report owns the final labels and the number of propagation
/// passes that were actually executed. Callers inside the crate can therefore
/// inspect the labels without re-running propagation or borrowing the input
/// graph.
///
/// The function never errors. If `adjacency` contains no active nodes, or if
/// `max_iterations` is `0`, it returns the initial self labelling with an
/// `iteration_count` of `0`.
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

    let mut scores = BTreeMap::new();
    let mut best: Option<(usize, u64)> = None;

    for &(neighbour, weight) in neighbours {
        let label = labels[neighbour];
        let score = score_label(&mut scores, label, weight);

        if should_replace_best(best, label, score, vectors) {
            best = Some((label, score));
        }
    }

    best.map(|(label, _)| label)
}

fn score_label(scores: &mut BTreeMap<usize, u64>, label: usize, weight: u64) -> u64 {
    let score = scores.entry(label).or_default();
    *score += weight;
    *score
}

fn should_replace_best(
    current_best: Option<(usize, u64)>,
    candidate_label: usize,
    candidate_score: u64,
    vectors: &[MethodFeatureVector],
) -> bool {
    match current_best {
        None => true,
        Some((best_label, best_score)) => {
            // Prefer higher score; on tie, pick the lexically earlier method
            // name and then the smaller label index to keep runs deterministic.
            if candidate_score != best_score {
                candidate_score > best_score
            } else {
                let candidate_name = vectors[candidate_label].method_name();
                let best_name = vectors[best_label].method_name();

                candidate_name < best_name
                    || (candidate_name == best_name && candidate_label < best_label)
            }
        }
    }
}

#[cfg(kani)]
#[path = "community_kani/mod.rs"]
mod verify;
