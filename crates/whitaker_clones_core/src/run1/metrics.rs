//! Bounded operational metrics for AST-pass feature consumption.
//!
//! `lower_span` owns lowering latency, outcome and parser-recovery metrics.
//! This module adds only what the 7.3.2 boundary itself consumes: how many
//! feature vectors were built and how each physical pair was classified. Every
//! label is drawn from a closed vocabulary, so no path, span or source text
//! can reach a metric.

/// Total AST feature vectors built by the refinement boundary.
pub(crate) const FEATURE_VECTOR_TOTAL: &str = "whitaker_clones.ast.feature_vector.total";

/// Total physical pairs classified, labelled by outcome.
pub(crate) const PAIR_TOTAL: &str = "whitaker_clones.run1.pair.total";

/// Label key for the pair classification.
pub(crate) const CLASSIFICATION_LABEL: &str = "classification";

/// A token-pass result was carried forward unchanged.
pub(crate) const CARRIED: &str = "carried";

/// A token-pass result was reclassified as a near-miss.
pub(crate) const RECLASSIFIED: &str = "reclassified";

/// The AST pass added a result.
pub(crate) const ADDED: &str = "added";

/// A candidate pair was reported by neither pass.
pub(crate) const REJECTED: &str = "rejected";

/// The evidence was inconclusive, so the token verdict stands.
pub(crate) const INCONCLUSIVE: &str = "inconclusive";

/// Record that one feature vector was built for a physical fragment.
pub(crate) fn record_feature_vector() {
    describe_metrics();
    metrics::counter!(FEATURE_VECTOR_TOTAL).increment(1);
}

/// Record one pair classification under its bounded label.
pub(crate) fn record_pair_classification(classification: &'static str) {
    describe_metrics();
    metrics::counter!(PAIR_TOTAL, CLASSIFICATION_LABEL => classification).increment(1);
}

/// Describe the run-1 metrics to the active recorder.
fn describe_metrics() {
    // Descriptions belong to the active recorder; a process-wide guard could
    // register them only with a no-op or test-local recorder.
    metrics::describe_counter!(
        FEATURE_VECTOR_TOTAL,
        "AST feature vectors built by the Type-3 refinement boundary"
    );
    metrics::describe_counter!(
        PAIR_TOTAL,
        "Physical fragment pairs classified by the AST pass, by outcome"
    );
}

#[cfg(test)]
mod tests {
    //! Metric-name and emission tests against a debugging recorder.

    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    use super::*;

    /// Check that the metric names keep their documented spelling.
    #[test]
    fn metric_names_are_stable() {
        assert_eq!(
            FEATURE_VECTOR_TOTAL, "whitaker_clones.ast.feature_vector.total",
            "the feature-vector metric name is part of the operational contract"
        );
        assert_eq!(
            PAIR_TOTAL, "whitaker_clones.run1.pair.total",
            "the pair metric name is part of the operational contract"
        );
    }

    /// Check that every classification label is distinct and bounded.
    #[test]
    fn classification_labels_are_distinct() {
        let labels = [CARRIED, RECLASSIFIED, ADDED, REJECTED, INCONCLUSIVE];
        let unique = labels
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(
            unique.len(),
            labels.len(),
            "classification labels must be distinct, got {labels:?}"
        );
    }

    /// Check that a recorded classification reaches the active recorder with
    /// its label, and that the feature-vector counter increments once.
    #[test]
    fn metrics_reach_the_active_recorder() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        metrics::with_local_recorder(&recorder, || {
            record_feature_vector();
            record_pair_classification(ADDED);
        });

        let snapshot = snapshotter.snapshot().into_vec();
        assert!(
            snapshot.iter().any(|(key, _, _, value)| {
                key.key().name() == FEATURE_VECTOR_TOTAL && matches!(value, DebugValue::Counter(1))
            }),
            "the feature-vector counter must increment once"
        );
        assert!(
            snapshot.iter().any(|(key, _, _, value)| {
                key.key().name() == PAIR_TOTAL
                    && key
                        .key()
                        .labels()
                        .any(|label| label.key() == CLASSIFICATION_LABEL && label.value() == ADDED)
                    && matches!(value, DebugValue::Counter(1))
            }),
            "the pair counter must carry the classification label"
        );
    }
}
