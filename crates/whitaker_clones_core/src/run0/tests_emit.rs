//! Tests for SARIF Run 0 emission specifics.

use whitaker_sarif::{TOKEN_HASH_KEY, WHITAKER_FRAGMENT_KEY, WHK002_ID, WhitakerProperties};

use super::{AcceptedPair, Run0Error, SimilarityRatio, emit_run0, score::jaccard_similarity};
use crate::{
    Fingerprint,
    NormProfile,
    test_support::{FragmentInput, config, fingerprint, fragment, pair},
};

#[test]
fn duplicate_hashes_do_not_inflate_jaccard_score() {
    let left = [
        fingerprint(1, 0..3),
        fingerprint(1, 3..6),
        fingerprint(2, 6..9),
    ];
    let right = [fingerprint(1, 0..3), fingerprint(2, 3..6)];

    let score =
        jaccard_similarity(&left, &right).expect("score should be present for non-empty fragments");

    assert_eq!(score, SimilarityRatio::new(2, 2));
}

#[test]
fn jaccard_returns_none_for_empty_fragments() {
    let non_empty = [fingerprint(1, 0..3)];
    let empty: [Fingerprint; 0] = [];

    assert!(jaccard_similarity(&empty, &non_empty).is_none());
    assert!(jaccard_similarity(&non_empty, &empty).is_none());
}

/// Build a T2 emission run containing one accepted fragment pair.
fn make_t2_emission_run() -> Result<whitaker_sarif::Run, Run0Error> {
    let fragments = vec![
        fragment(&FragmentInput {
            id: "alpha",
            profile: NormProfile::T2,
            file_uri: "src/a.rs",
            source_text: "fn a(x: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
        fragment(&FragmentInput {
            id: "beta",
            profile: NormProfile::T2,
            file_uri: "src/b.rs",
            source_text: "fn b(y: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
    ];
    let accepted: Vec<AcceptedPair> = pair("alpha", "beta")
        .map(|candidate| AcceptedPair::new(candidate, NormProfile::T2, SimilarityRatio::new(2, 2)))
        .into_iter()
        .collect();

    emit_run0(&fragments, &accepted, &config())
}

#[test]
fn emitted_t2_result_has_correct_rule_id() {
    let run = make_t2_emission_run().expect("Run 0 emission should succeed");
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };

    assert_eq!(result.rule_id, WHK002_ID);
}

#[test]
fn emitted_t2_result_contains_required_fingerprint_keys() {
    let run = make_t2_emission_run().expect("Run 0 emission should succeed");
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };

    assert!(
        result
            .partial_fingerprints
            .contains_key(WHITAKER_FRAGMENT_KEY)
    );
    assert!(
        result.partial_fingerprints.contains_key(TOKEN_HASH_KEY),
        "the token digest must serialize under the shared key constant"
    );
}

/// Check that Run 0 omits the cosine property rather than writing a
/// placeholder: `0.0` is a meaningful score, so absence is the only honest
/// encoding for "not measured".
#[test]
fn emitted_t2_result_omits_the_cosine_property() {
    let run = make_t2_emission_run().expect("Run 0 emission should succeed");
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };
    let properties = result
        .properties
        .as_ref()
        .expect("Whitaker properties must be present");
    let whitaker = properties
        .get("whitaker")
        .expect("the whitaker property bag must be present");

    assert!(
        whitaker.get("cosine").is_none(),
        "Run 0 must not write a cosine placeholder, got {whitaker}"
    );
}

#[test]
fn emitted_t2_result_properties_match_config() {
    let run = make_t2_emission_run().expect("Run 0 emission should succeed");
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };
    let properties = result
        .properties
        .as_ref()
        .expect("Whitaker properties must be present");
    let extracted =
        WhitakerProperties::try_from(properties).expect("Whitaker properties should extract");

    assert_eq!(
        (
            extracted.profile.as_str(),
            extracted.k,
            extracted.window,
            extracted.class_size,
        ),
        ("T2", 25, 16, 2),
    );
}
