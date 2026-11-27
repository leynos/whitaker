//! Metric-emission tests for the refinement pass.
//!
//! Split from the sibling test module to stay inside the 400-line budget. Every
//! case drives a debugging recorder and asserts the *exact* label-to-count map
//! for the pair counter, so a duplicate event, a missing event or a
//! mislabelled event all fail. The pair counter's documented unit is the
//! physical pair, which is what the dual-profile case pins.

use std::collections::BTreeMap;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use rstest::rstest;

use super::{
    refine_with_lowering,
    tests::{NODES, accepted, ast_config, fragment, lower_with, pair, token_config, type2_pair},
};
use crate::{
    AcceptedPair,
    CandidatePair,
    NormProfile,
    NormalizedTree,
    RefinementInput,
    TokenFragment,
    record_refinement_metrics,
    run1::metrics as run1_metrics,
    test_support::{fixture_span, synthetic_tree},
};

/// One refinement scenario: its inputs, its lowering table and what the pair
/// counter must record.
///
/// Keeping all five inputs together is what lets the case table stay a table:
/// each row states a classification and the situation that produces it.
struct Scenario {
    fragments: Vec<TokenFragment>,
    candidates: Vec<CandidatePair>,
    accepted: Vec<AcceptedPair>,
    trees: BTreeMap<u32, NormalizedTree>,
    /// The one label the pair counter must carry, and how many events it must
    /// record. Exactly one physical pair is refined in every scenario, so the
    /// count is the assertion that guards against double counting.
    expected: (&'static str, u64),
}

/// Build the standard two-fragment fixture with the given lowering table.
///
/// `was_accepted` decides which half of the pass the scenario reaches: keeping
/// the accepted pair exercises table A, and dropping it exercises table B, the
/// candidate-only path where no profile accepted the pair. The one physical pair
/// means the pair counter sees exactly one event either way.
fn fixture(trees: BTreeMap<u32, NormalizedTree>, was_accepted: bool) -> Scenario {
    let (fragments, candidates, accepted) = type2_pair();
    let expected = if was_accepted {
        (run1_metrics::CARRIED, 1)
    } else {
        (run1_metrics::REJECTED, 1)
    };
    Scenario {
        fragments,
        candidates,
        accepted: if was_accepted { accepted } else { Vec::new() },
        trees,
        expected,
    }
}

/// Build a lowering table for the two fixture spans.
fn trees_at(left: NormalizedTree, right: NormalizedTree) -> BTreeMap<u32, NormalizedTree> {
    let mut trees = BTreeMap::new();
    trees.insert(0, left);
    trees.insert(10, right);
    trees
}

/// The scenarios, one per classification the pass can report.
///
/// `reclassified` needs equal-ish trees under one extra node, `rejected` needs
/// a cosine below the threshold, `added` needs a candidate no profile accepted
/// whose trees nonetheless meet it, and `inconclusive` needs lowering to fail.
fn scenarios() -> Vec<(&'static str, Scenario)> {
    let carried = fixture(
        trees_at(
            synthetic_tree(7, NODES, fixture_span(0, 10)),
            synthetic_tree(7, NODES, fixture_span(10, 20)),
        ),
        true,
    );

    let mut reclassified = fixture(
        trees_at(
            synthetic_tree(7, NODES, fixture_span(0, 10)),
            synthetic_tree(7, NODES + 1, fixture_span(10, 20)),
        ),
        true,
    );
    reclassified.expected = (run1_metrics::RECLASSIFIED, 1);

    let mut added = fixture(
        trees_at(
            synthetic_tree(7, NODES, fixture_span(0, 10)),
            synthetic_tree(7, NODES + 1, fixture_span(10, 20)),
        ),
        false,
    );
    added.expected = (run1_metrics::ADDED, 1);

    let mut inconclusive = fixture(BTreeMap::new(), true);
    inconclusive.expected = (run1_metrics::INCONCLUSIVE, 1);

    vec![
        ("carried", carried),
        ("reclassified", reclassified),
        ("added", added),
        (
            "rejected",
            fixture(
                trees_at(
                    synthetic_tree(1, NODES, fixture_span(0, 10)),
                    synthetic_tree(100, NODES, fixture_span(10, 20)),
                ),
                true,
            ),
        ),
        ("inconclusive", inconclusive),
    ]
}

/// Run one scenario under a local recorder and return the pair counts.
fn pair_counts(scenario: &Scenario) -> BTreeMap<String, u64> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let token_config = token_config();
    let input = RefinementInput::new(
        &scenario.fragments,
        &scenario.candidates,
        &scenario.accepted,
        &token_config,
    );
    let trees = &scenario.trees;
    // The pass is a pure query; reporting is the explicit call the boundary
    // makes, so the recorder sees exactly the counts the pass returned.
    let refined =
        refine_with_lowering(&input, &ast_config(), |_text, span| lower_with(trees, span))
            .expect("refinement must succeed");
    metrics::with_local_recorder(&recorder, || record_refinement_metrics(&refined));

    assert!(
        !refined.pairs().is_empty() || scenario.expected.0 == run1_metrics::REJECTED,
        "a {:?} scenario must either refine a pair or report a rejection",
        scenario.expected.0
    );

    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, ..)| key.key().name() == run1_metrics::PAIR_TOTAL)
        .filter_map(|(key, _, _, value)| {
            let DebugValue::Counter(count) = value else {
                return None;
            };
            let label = key
                .key()
                .labels()
                .find(|label| label.key() == run1_metrics::CLASSIFICATION_LABEL)?
                .value()
                .to_owned();
            Some((label, count))
        })
        .collect()
}

/// Check that each classification records exactly one event under its own label.
#[rstest]
fn each_classification_records_exactly_one_event(
    #[values("carried", "reclassified", "added", "rejected", "inconclusive")] name: &str,
) {
    let cases = scenarios();
    let Some((_, scenario)) = cases.iter().find(|(label, _)| *label == name) else {
        panic!("the case table must define a `{name}` scenario");
    };

    let counts = pair_counts(scenario);

    let expected = BTreeMap::from([(scenario.expected.0.to_owned(), scenario.expected.1)]);
    assert_eq!(
        counts, expected,
        "the `{name}` scenario must record exactly one {expected:?} event"
    );
}

/// Check that one physical pair accepted under both profiles records one event.
///
/// The T1 and T2 fragments of one region share a physical location, so the pass
/// groups them into a single pair. Counting per refined result made such a pair
/// contribute two events to a counter documented in physical pairs.
#[test]
fn dual_profile_pair_records_one_event() {
    let fragments = vec![
        fragment("left_t1", NormProfile::T1, 0..10, &[1, 2]),
        fragment("right_t1", NormProfile::T1, 10..20, &[1, 2]),
        fragment("left_t2", NormProfile::T2, 0..10, &[1, 2]),
        fragment("right_t2", NormProfile::T2, 10..20, &[1, 2]),
    ];
    let candidates = vec![pair("left_t1", "right_t1"), pair("left_t2", "right_t2")];
    let accepted = vec![
        accepted("left_t1", "right_t1", NormProfile::T1, &[1, 2]),
        accepted("left_t2", "right_t2", NormProfile::T2, &[1, 2]),
    ];
    let trees = trees_at(
        synthetic_tree(7, NODES, fixture_span(0, 10)),
        synthetic_tree(7, NODES, fixture_span(10, 20)),
    );
    let scenario = Scenario {
        fragments,
        candidates,
        accepted,
        trees,
        expected: (run1_metrics::CARRIED, 1),
    };

    let counts = pair_counts(&scenario);

    assert_eq!(
        counts,
        BTreeMap::from([(run1_metrics::CARRIED.to_owned(), 1)]),
        "one physical pair must contribute one event however many profiles accept it"
    );
}

/// Check that each physical fragment yields exactly one feature vector.
#[test]
fn feature_vectors_are_counted_once_per_fragment() {
    let scenario = fixture(
        trees_at(
            synthetic_tree(7, NODES, fixture_span(0, 10)),
            synthetic_tree(7, NODES, fixture_span(10, 20)),
        ),
        true,
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let token_config = token_config();
    let input = RefinementInput::new(
        &scenario.fragments,
        &scenario.candidates,
        &scenario.accepted,
        &token_config,
    );
    let trees = &scenario.trees;
    let refined =
        refine_with_lowering(&input, &ast_config(), |_text, span| lower_with(trees, span))
            .expect("refinement must succeed");
    assert_eq!(refined.pairs().len(), 1, "the fixture refines one pair");
    metrics::with_local_recorder(&recorder, || record_refinement_metrics(&refined));

    let feature_vectors = snapshotter
        .snapshot()
        .into_vec()
        .iter()
        .filter(|(key, ..)| key.key().name() == run1_metrics::FEATURE_VECTOR_TOTAL)
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

/// Check the pass emits no lowering telemetry of its own.
///
/// `lower_span` owns lowering metrics; a pass that also recorded them would
/// double-count every lowering when both are wired to the same recorder.
#[test]
fn the_pass_emits_no_lowering_metrics() {
    let scenario = fixture(
        trees_at(
            synthetic_tree(7, NODES, fixture_span(0, 10)),
            synthetic_tree(7, NODES, fixture_span(10, 20)),
        ),
        true,
    );
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let token_config = token_config();
    let input = RefinementInput::new(
        &scenario.fragments,
        &scenario.candidates,
        &scenario.accepted,
        &token_config,
    );
    let trees = &scenario.trees;
    let refined =
        refine_with_lowering(&input, &ast_config(), |_text, span| lower_with(trees, span))
            .expect("refinement must succeed");
    metrics::with_local_recorder(&recorder, || record_refinement_metrics(&refined));

    let snapshot = snapshotter.snapshot().into_vec();
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
