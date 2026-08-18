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
    pub(crate) const fn new(left: usize, right: usize, weight: u64) -> Self {
        Self {
            left,
            right,
            weight,
        }
    }

    #[cfg(test)]
    pub(crate) const fn left(&self) -> usize {
        self.left
    }

    #[cfg(test)]
    pub(crate) const fn right(&self) -> usize {
        self.right
    }

    #[cfg(test)]
    pub(crate) const fn weight(&self) -> u64 {
        self.weight
    }
}

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

pub(crate) fn detect_communities(vectors: &[MethodFeatureVector]) -> Vec<Vec<usize>> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let edges = build_similarity_edges(vectors);
    let adjacency = build_adjacency(vectors.len(), &edges);
    let max_iterations = vectors.len().saturating_mul(2).max(1);
    let report = propagate_labels_report(vectors, &adjacency, max_iterations);
    if log::log_enabled!(log::Level::Debug) {
        let converged = labels_are_stable(vectors, &adjacency, &report.labels);
        log::debug!(
            "label propagation complete: nodes={}, iterations={}, converged={}",
            vectors.len(),
            report.iteration_count,
            converged,
        );
    }
    let labels = report.labels;

    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (node, label) in labels.into_iter().enumerate() {
        groups.entry(label).or_default().push(node);
    }

    // Node indices always come from `0..vectors.len()`, so the lookup never
    // misses; the `Option` ordering (`None` first) is only a type-level guard.
    let method_name = |node: usize| vectors.get(node).map(MethodFeatureVector::method_name);

    let mut communities: Vec<Vec<usize>> = groups.into_values().collect();
    for community in &mut communities {
        community.sort_by(|left, right| method_name(*left).cmp(&method_name(*right)));
    }

    communities.sort_by(|left, right| {
        right.len().cmp(&left.len()).then_with(|| {
            let left_name = left.first().and_then(|&node| method_name(node));
            let right_name = right.first().and_then(|&node| method_name(node));
            left_name.cmp(&right_name)
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

    // Edges referencing nodes outside `0..node_count` are ignored rather than
    // panicking; validated callers never produce them.
    for edge in edges {
        if let Some(neighbours) = adjacency.get_mut(edge.left) {
            neighbours.push((edge.right, edge.weight));
        }
        if let Some(neighbours) = adjacency.get_mut(edge.right) {
            neighbours.push((edge.left, edge.weight));
        }
    }

    for neighbours in &mut adjacency {
        neighbours.sort_by_key(|left| left.0);
    }

    adjacency
}

/// Runs deterministic weighted label propagation and reports its final state.
///
/// The returned report owns the final labels and the number of propagation
/// passes that were actually executed. Callers inside the crate can therefore
/// inspect the labels without re-running propagation or borrowing the input
/// graph.
///
/// Callers must ensure that every node index referenced from `adjacency` is in
/// bounds for the initial self labelling `0..vectors.len()`. The helper
/// assumes validated adjacency input and may panic if a neighbour index falls
/// outside that range.
///
/// If `adjacency` contains no active nodes, or if `max_iterations` is `0`, it
/// returns the initial self labelling with an `iteration_count` of `0`.
pub(crate) fn propagate_labels_report(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    max_iterations: usize,
) -> LabelPropagationReport {
    assert_eq!(
        adjacency.len(),
        vectors.len(),
        "propagate_labels_report requires adjacency rows to match vectors"
    );
    let mut labels: Vec<usize> = (0..vectors.len()).collect();
    // Each `node` comes from `adjacency`; keep rows aligned with `labels` so
    // `labels[node]` stays in bounds.
    let active_nodes: Vec<_> = adjacency
        .iter()
        .enumerate()
        .filter_map(|(node, neighbours)| (!neighbours.is_empty()).then_some(node))
        .collect();
    if active_nodes.is_empty() {
        log::debug!(
            "label propagation: no active nodes, skipping (total_nodes={})",
            vectors.len(),
        );
        return LabelPropagationReport {
            labels,
            iteration_count: 0,
        };
    }
    let mut iteration_count = 0;

    for _ in 0..max_iterations {
        iteration_count += 1;
        let changed = run_propagation_pass(vectors, adjacency, &active_nodes, &mut labels);

        if !changed {
            log::debug!(
                "label propagation converged: nodes={}, active_nodes={}, iterations={}",
                vectors.len(),
                active_nodes.len(),
                iteration_count,
            );
            break;
        }
    }

    if iteration_count == max_iterations {
        log::debug!(
            "label propagation reached iteration limit: nodes={}, active_nodes={}, max_iterations={}",
            vectors.len(),
            active_nodes.len(),
            max_iterations,
        );
    }

    LabelPropagationReport {
        labels,
        iteration_count,
    }
}

/// Runs one label-propagation pass over the active nodes.
///
/// Returns `true` when at least one node adopted a new label, which tells the
/// caller whether propagation has converged.
fn run_propagation_pass(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    active_nodes: &[usize],
    labels: &mut [usize],
) -> bool {
    let mut changed = false;

    for &node in active_nodes {
        let Some(best_label) = best_neighbour_label(node, labels, adjacency, vectors) else {
            continue;
        };

        // Active nodes are adjacency indices, so the label slot always exists.
        if let Some(label_slot) = labels.get_mut(node)
            && *label_slot != best_label
        {
            *label_slot = best_label;
            changed = true;
        }
    }

    changed
}

fn best_neighbour_label(
    node: usize,
    labels: &[usize],
    adjacency: &[Vec<(usize, u64)>],
    vectors: &[MethodFeatureVector],
) -> Option<usize> {
    let neighbours = adjacency.get(node)?;
    if neighbours.is_empty() {
        return None;
    }

    let mut scores = BTreeMap::new();
    let mut best: Option<(usize, u64)> = None;

    for &(neighbour, weight) in neighbours {
        // Neighbour indices come from validated adjacency rows, so the label
        // lookup never misses; skipping keeps the scan panic free regardless.
        let Some(&label) = labels.get(neighbour) else {
            continue;
        };
        let score = score_label(&mut scores, label, weight);

        if should_replace_best(best, label, score, vectors) {
            best = Some((label, score));
        }
    }

    best.map(|(label, _)| label)
}

fn labels_are_stable(
    vectors: &[MethodFeatureVector],
    adjacency: &[Vec<(usize, u64)>],
    labels: &[usize],
) -> bool {
    adjacency.iter().enumerate().all(|(node, neighbours)| {
        if neighbours.is_empty() {
            return true;
        }

        best_neighbour_label(node, labels, adjacency, vectors)
            .is_none_or(|best_label| labels.get(node) == Some(&best_label))
    })
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
            if candidate_score == best_score {
                // Labels are node indices, so both lookups always succeed; the
                // `Option` ordering (`None` first) is only a type-level guard.
                let candidate_name = vectors
                    .get(candidate_label)
                    .map(MethodFeatureVector::method_name);
                let best_name = vectors.get(best_label).map(MethodFeatureVector::method_name);

                candidate_name < best_name
                    || (candidate_name == best_name && candidate_label < best_label)
            } else {
                candidate_score > best_score
            }
        }
    }
}

#[cfg(kani)]
#[path = "community_kani/mod.rs"]
mod verify;
