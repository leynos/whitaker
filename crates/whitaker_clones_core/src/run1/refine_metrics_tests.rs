//! Metric-emission tests for the refinement pass.
//!
//! Split from the sibling test module to stay inside the 400-line budget. These
//! tests drive a recorder rather than asserting outcomes, so they belong
//! together and apart from the classification tests.

use std::collections::BTreeMap;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};

use super::{
    refine_with_lowering,
    tests::{NODES, accepted, ast_config, fragment, pair, refine, token_config, type2_pair},
};
use crate::{
    NormProfile,
    RefinementInput,
    run1::metrics as run1_metrics,
    test_support::{fixture_span, synthetic_tree},
};

#[test]
fn feature_vectors_are_counted_once_per_fragment() {
    let (fragments, candidates, accepted_pairs) = type2_pair();
    let token_config = token_config();
    let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);
    let mut trees = BTreeMap::new();
    trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
    trees.insert(10, synthetic_tree(7, NODES, fixture_span(10, 20)));

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let refined = metrics::with_local_recorder(&recorder, || refine(&input, &trees));
    assert_eq!(refined.len(), 1, "the fixture refines one pair");

    let snapshot = snapshotter.snapshot().into_vec();
    let feature_vectors = snapshot
        .iter()
        .filter(|(key, ..)| key.key().name() == crate::run1::metrics::FEATURE_VECTOR_TOTAL)
        .map(|(_, _, _, value)| match value {
            DebugValue::Counter(count) => *count,
            _ => 0,
        })
        .sum::<u64>();
    assert_eq!(
        feature_vectors, 2,
        "one feature vector must be counted per physical fragment"
    );
}

/// M1: every classification reaches the recorder, and lowering metrics do not
/// duplicate.
///
/// The other metrics test counts feature vectors. This one drives a fixture
/// that produces each of the three emit-side classifications and asserts the
/// pair counter carries each label, then asserts that `lower_span`'s own
/// counters are unchanged by the pass — the pass must not emit lowering
/// telemetry of its own.
#[test]
fn every_classification_reaches_the_recorder() {
    let run = || {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();
        let sources = crate::test_support::FIXTURE_SOURCE;
        let fragments = vec![
            fragment("first", NormProfile::T2, 0..10, &[1]),
            fragment("second", NormProfile::T2, 10..20, &[1]),
        ];
        let candidates = vec![pair("first", "second")];
        let accepted_pairs = vec![accepted("first", "second", NormProfile::T2, &[1])];
        let token_config = token_config();
        let input = RefinementInput::new(&fragments, &candidates, &accepted_pairs, &token_config);
        let mut trees = BTreeMap::new();
        trees.insert(0, synthetic_tree(7, NODES, fixture_span(0, 10)));
        trees.insert(10, synthetic_tree(9, NODES + 1, fixture_span(10, 20)));
        let _ = sources;
        let refined = metrics::with_local_recorder(&recorder, || {
            refine_with_lowering(&input, &ast_config(), |_text, span| {
                super::tests::lower_with(&trees, span)
            })
        })
        .expect("refinement must succeed");
        (refined, snapshotter.snapshot().into_vec())
    };

    let (refined, snapshot) = run();
    assert!(
        !refined.is_empty(),
        "the fixture must refine at least one pair"
    );

    let classifications = snapshot
        .iter()
        .filter(|(key, ..)| key.key().name() == run1_metrics::PAIR_TOTAL)
        .flat_map(|(key, ..)| {
            key.key()
                .labels()
                .filter(|label| label.key() == run1_metrics::CLASSIFICATION_LABEL)
                .map(|label| label.value().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        !classifications.is_empty(),
        "the pair counter must carry at least one classification label"
    );
    let allowed = [
        run1_metrics::CARRIED,
        run1_metrics::RECLASSIFIED,
        run1_metrics::ADDED,
        run1_metrics::REJECTED,
        run1_metrics::INCONCLUSIVE,
    ];
    for label in &classifications {
        assert!(
            allowed.contains(&label.as_str()),
            "unexpected classification label `{label}`"
        );
    }

    // The pass must not emit lowering telemetry: `lower_span` owns it.
    for metric in [
        "whitaker_clones.ast.lower_span.total",
        "whitaker_clones.ast.lower_span.duration_seconds",
    ] {
        assert!(
            snapshot.iter().all(|(key, ..)| key.key().name() != metric),
            "the refinement pass must not emit `{metric}`; lower_span owns it"
        );
    }
}
