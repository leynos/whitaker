//! Run 1 emission tests for result identity, provenance and merging.
//!
//! Split from the sibling test module to stay inside the 400-line module
//! budget; the fixtures are duplicated deliberately so each file can be
//! read on its own.

use whitaker_sarif::{
    AST_HASH_KEY,
    BaselineState,
    TOKEN_HASH_KEY,
    WHK001_ID,
    WHK003_ID,
    WhitakerProperties,
    merge_runs,
};

use super::tests::{
    CARRIED,
    RESHAPED,
    UNPARSABLE,
    accepted,
    all_fragments,
    all_kinds,
    emit_all,
    token_config,
};
use crate::{
    NormProfile,
    test_support::{fixture_span, synthetic_tree},
};

/// Check that an added result is keyed by physical identity and reports new.
#[test]
fn added_results_are_new_and_physically_keyed() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let added = run
        .results
        .iter()
        .find(|result| result.baseline_state == Some(BaselineState::New))
        .expect("the added result exists");

    assert_eq!(added.rule_id, WHK003_ID, "the added pair is a near-miss");
    assert!(
        added.partial_fingerprints.contains_key(TOKEN_HASH_KEY),
        "an added result still carries its token digest"
    );
    let properties =
        WhitakerProperties::try_from(added.properties.as_ref().expect("properties present"))
            .expect("properties extract");
    assert_eq!(
        properties.jaccard.to_bits(),
        0.75_f64.to_bits(),
        "an added result reports the representative candidate's Jaccard score"
    );
    assert_eq!(
        properties.token_rule_id, None,
        "an added result has no token-pass rule"
    );
}

/// Check that an inconclusive carried result records no AST data.
#[test]
fn inconclusive_results_have_no_ast_data() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let inconclusive = run
        .results
        .iter()
        .find(|result| result.rule_id == WHK001_ID)
        .expect("the inconclusive T1 result exists");

    assert!(
        !inconclusive.partial_fingerprints.contains_key(AST_HASH_KEY),
        "inconclusive evidence must not add an astHash"
    );
    let properties = WhitakerProperties::try_from(
        inconclusive
            .properties
            .as_ref()
            .expect("properties present"),
    )
    .expect("properties extract");
    assert_eq!(
        properties.cosine, None,
        "inconclusive evidence must omit the cosine"
    );
    assert_eq!(
        properties.ast_hashes_equal, None,
        "inconclusive evidence must omit hash equality"
    );
}

/// Check that emission is deterministic under permutation of its input.
#[test]
fn emission_is_deterministic_under_permutation() {
    let refined = all_kinds();
    let forward = emit_all(&refined).expect("Run 1 emission must succeed");
    let mut reversed = refined.clone();
    reversed.reverse();
    let backward = emit_all(&reversed).expect("Run 1 emission must succeed");
    let mut duplicated = refined.clone();
    duplicated.extend(refined.clone());
    let duplicated_run = emit_all(&duplicated).expect("Run 1 emission must succeed");

    let forward_json = serde_json::to_value(&forward).expect("runs serialize");
    let backward_json = serde_json::to_value(&backward).expect("runs serialize");
    let duplicated_json = serde_json::to_value(&duplicated_run).expect("runs serialize");
    assert_eq!(
        forward_json, backward_json,
        "permuting the refined input must not change the emitted log"
    );
    assert_eq!(
        forward_json, duplicated_json,
        "duplicating the refined input must not change the emitted log"
    );
}

/// Check that a merged log keeps Run 1's refinements when Run 1 comes first.
#[test]
fn merge_keeps_run_one_when_it_comes_first() {
    let run1 = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let run0 = similar_run_zero().expect("Run 0 emission must succeed");
    let merged = merge_runs(&[run1.clone(), run0.clone()]).expect("runs must merge");

    assert_eq!(
        merged.tool.driver.name, "whitaker_clones_cli@ast",
        "the merged tool name comes from the first run"
    );
    assert!(
        merged
            .results
            .iter()
            .any(|result| result.rule_id == WHK003_ID),
        "Run 1's refinements must survive the merge"
    );
}

/// Check that merging Run 0 first discards the reclassification, which is why
/// the consumer contract fixes the order.
#[test]
fn merge_loses_run_one_when_run_zero_comes_first() {
    let run1 = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let run0 = similar_run_zero().expect("Run 0 emission must succeed");
    let merged = merge_runs(&[run0, run1]).expect("runs must merge");

    assert!(
        !merged
            .results
            .iter()
            .any(|result| result.baseline_state == Some(BaselineState::Updated)),
        "merging Run 0 first must discard every reclassification"
    );
}

/// Build a Run 0 whose results mirror Run 1's carried and reclassified pairs.
pub(super) fn similar_run_zero() -> Result<whitaker_sarif::Run, super::Run1Error> {
    let fragments = all_fragments();
    let token = token_config();
    let accepted_pairs = vec![
        accepted(CARRIED, NormProfile::T2),
        accepted(RESHAPED, NormProfile::T2),
        accepted(UNPARSABLE, NormProfile::T1),
    ];

    crate::emit_run0(&fragments, &accepted_pairs, &token).map_err(super::Run1Error::from)
}

/// Check that the AST fixture trees are usable as evidence sources.
#[test]
fn synthetic_trees_hash_deterministically() {
    let first = synthetic_tree(7, 20, fixture_span(0, 10));
    let second = synthetic_tree(7, 20, fixture_span(0, 10));

    assert_eq!(
        crate::canonical_hash(&first).to_hex(),
        crate::canonical_hash(&second).to_hex(),
        "identical synthetic trees must hash identically"
    );
}
