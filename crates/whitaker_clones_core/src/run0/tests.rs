//! Unit coverage for token-pass acceptance and Run 0 emission.

use whitaker_sarif::{Region, WHITAKER_FRAGMENT_KEY, WHK001_ID, WHK002_ID, WhitakerProperties};

use crate::{CandidatePair, Fingerprint, FragmentId, NormProfile};

use super::{
    AcceptedPair, Run0Error, SimilarityThreshold, TokenFragment, TokenPassConfig,
    accept_candidate_pairs, emit_run0,
    score::{SimilarityRatio, jaccard_similarity},
    span::region_for_range,
};

fn fingerprint(hash: u64, range: std::ops::Range<usize>) -> Fingerprint {
    Fingerprint::new(hash, range)
}

struct FragmentInput<'a> {
    id: &'a str,
    profile: NormProfile,
    file_uri: &'a str,
    source_text: &'a str,
    hashes: &'a [(u64, std::ops::Range<usize>)],
}

fn fragment(input: FragmentInput<'_>) -> TokenFragment {
    TokenFragment::new(
        FragmentId::from(input.id),
        input.profile,
        input.file_uri,
        input.source_text,
    )
    .with_retained_fingerprints(
        input
            .hashes
            .iter()
            .map(|(hash, range)| fingerprint(*hash, range.clone()))
            .collect(),
    )
}

fn pair(left: &str, right: &str) -> CandidatePair {
    CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .unwrap_or_else(|| panic!("pair `{left}` and `{right}` must be distinct"))
}

fn config() -> TokenPassConfig {
    TokenPassConfig::new("whitaker_clones_cli@token", "0.2.1")
}

fn build_pair_and_accept(
    left: FragmentInput<'_>,
    right: FragmentInput<'_>,
    cfg: &TokenPassConfig,
) -> Result<Vec<AcceptedPair>, Run0Error> {
    let fragments = vec![fragment(left), fragment(right)];
    accept_candidate_pairs(&fragments, &[pair("alpha", "beta")], cfg)
}

fn assert_single_accepted(
    accepted: &[AcceptedPair],
    expected_profile: NormProfile,
    expected_score: SimilarityRatio,
) {
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].profile(), expected_profile);
    assert_eq!(accepted[0].score(), expected_score);
}

fn assert_region(id: &str, source: &str, range: std::ops::Range<usize>, expected: Region) {
    let region = region_for_range(id, source, range)
        .unwrap_or_else(|error| panic!("unexpected region error: {error}"));
    assert_eq!(region, expected);
}

#[test]
fn boundary_threshold_accepts_type1_pair() {
    let accepted = build_pair_and_accept(
        FragmentInput {
            id: "alpha",
            profile: NormProfile::T1,
            file_uri: "src/a.rs",
            source_text: "fn a() {}\n",
            hashes: &[(1, 0..8), (2, 0..8)],
        },
        FragmentInput {
            id: "beta",
            profile: NormProfile::T1,
            file_uri: "src/b.rs",
            source_text: "fn b() {}\n",
            hashes: &[(1, 0..8), (2, 0..8)],
        },
        &config(),
    )
    .unwrap_or_else(|error| panic!("unexpected acceptance error: {error}"));

    assert_single_accepted(&accepted, NormProfile::T1, SimilarityRatio::new(2, 2));
}

#[test]
fn boundary_threshold_accepts_type2_pair() {
    let config = config().with_type2_threshold(
        SimilarityThreshold::new("type2", 1, 3)
            .unwrap_or_else(|error| panic!("unexpected threshold error: {error}")),
    );
    let accepted = build_pair_and_accept(
        FragmentInput {
            id: "alpha",
            profile: NormProfile::T2,
            file_uri: "src/a.rs",
            source_text: "fn a(x: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        },
        FragmentInput {
            id: "beta",
            profile: NormProfile::T2,
            file_uri: "src/b.rs",
            source_text: "fn b(y: i32) {}\n",
            hashes: &[(1, 0..15), (3, 0..15)],
        },
        &config,
    )
    .unwrap_or_else(|error| panic!("unexpected acceptance error: {error}"));

    assert_single_accepted(&accepted, NormProfile::T2, SimilarityRatio::new(1, 3));
}

#[test]
fn just_below_threshold_is_rejected() {
    let accepted = build_pair_and_accept(
        FragmentInput {
            id: "alpha",
            profile: NormProfile::T2,
            file_uri: "src/a.rs",
            source_text: "fn a(x: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        },
        FragmentInput {
            id: "beta",
            profile: NormProfile::T2,
            file_uri: "src/b.rs",
            source_text: "fn b(y: i32) {}\n",
            hashes: &[(1, 0..15), (3, 0..15)],
        },
        &config(),
    )
    .unwrap_or_else(|error| panic!("unexpected acceptance error: {error}"));

    assert!(accepted.is_empty());
}

#[test]
fn duplicate_hashes_do_not_inflate_jaccard_score() {
    let left = [
        fingerprint(1, 0..3),
        fingerprint(1, 3..6),
        fingerprint(2, 6..9),
    ];
    let right = [fingerprint(1, 0..3), fingerprint(2, 3..6)];

    let score = jaccard_similarity(&left, &right)
        .unwrap_or_else(|| panic!("score should be present for non-empty fragments"));

    assert_eq!(score, SimilarityRatio::new(2, 2));
}

#[test]
fn single_line_region_uses_one_based_columns() {
    assert_region(
        "alpha",
        "fn a() {}\n",
        0..8,
        Region {
            start_line: 1,
            start_column: Some(1),
            end_line: Some(1),
            end_column: Some(8),
            byte_offset: Some(0),
            byte_length: Some(8),
        },
    );
}

#[test]
fn multi_line_region_tracks_trailing_newline() {
    assert_region(
        "alpha",
        "fn alpha() {\n    value();\n}\n",
        13..27,
        Region {
            start_line: 2,
            start_column: Some(1),
            end_line: Some(3),
            end_column: Some(1),
            byte_offset: Some(13),
            byte_length: Some(14),
        },
    );
}

#[test]
fn emit_run0_uses_primary_and_related_locations() {
    let fragments = vec![
        fragment(FragmentInput {
            id: "alpha",
            profile: NormProfile::T1,
            file_uri: "src/a.rs",
            source_text: "fn a() {}\n",
            hashes: &[(11, 0..8)],
        }),
        fragment(FragmentInput {
            id: "beta",
            profile: NormProfile::T1,
            file_uri: "src/b.rs",
            source_text: "fn b() {}\n",
            hashes: &[(11, 0..8)],
        }),
    ];
    let accepted = vec![AcceptedPair::new(
        pair("alpha", "beta"),
        NormProfile::T1,
        SimilarityRatio::new(1, 1),
    )];

    let run = emit_run0(&fragments, &accepted, &config())
        .unwrap_or_else(|error| panic!("unexpected emit error: {error}"));

    let [result] = run.results.as_slice() else {
        panic!("expected exactly one result");
    };
    assert_eq!(result.rule_id, WHK001_ID);
    assert_eq!(result.locations.len(), 1);
    assert_eq!(result.related_locations.len(), 1);
    assert_eq!(
        result.locations[0].physical_location.artifact_location.uri,
        "src/a.rs"
    );
    assert_eq!(
        result.related_locations[0]
            .physical_location
            .artifact_location
            .uri,
        "src/b.rs"
    );
}

#[test]
fn emit_run0_sorts_and_deduplicates_results() {
    let fragments = vec![
        fragment(FragmentInput {
            id: "alpha",
            profile: NormProfile::T1,
            file_uri: "src/a.rs",
            source_text: "fn a() {}\n",
            hashes: &[(11, 0..8)],
        }),
        fragment(FragmentInput {
            id: "beta",
            profile: NormProfile::T1,
            file_uri: "src/b.rs",
            source_text: "fn b() {}\n",
            hashes: &[(11, 0..8)],
        }),
        fragment(FragmentInput {
            id: "gamma",
            profile: NormProfile::T2,
            file_uri: "src/c.rs",
            source_text: "fn c(x: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
        fragment(FragmentInput {
            id: "delta",
            profile: NormProfile::T2,
            file_uri: "src/d.rs",
            source_text: "fn d(y: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
    ];
    let accepted = vec![
        AcceptedPair::new(
            pair("gamma", "delta"),
            NormProfile::T2,
            SimilarityRatio::new(2, 2),
        ),
        AcceptedPair::new(
            pair("beta", "alpha"),
            NormProfile::T1,
            SimilarityRatio::new(1, 1),
        ),
        AcceptedPair::new(
            pair("alpha", "beta"),
            NormProfile::T1,
            SimilarityRatio::new(1, 1),
        ),
    ];

    let run = emit_run0(&fragments, &accepted, &config())
        .unwrap_or_else(|error| panic!("unexpected emit error: {error}"));

    assert_eq!(run.results.len(), 2);
    assert_eq!(run.results[0].rule_id, WHK001_ID);
    assert_eq!(run.results[1].rule_id, WHK002_ID);
}

#[test]
fn invalid_range_produces_typed_error() {
    let error = region_for_range("alpha", "fn a() {}\n", 9..12)
        .err()
        .unwrap_or_else(|| panic!("invalid range must error"));

    match error {
        Run0Error::InvalidFingerprintRange {
            fragment_id,
            start,
            end,
            source_len,
        } => {
            assert_eq!(fragment_id, "alpha");
            assert_eq!(start, 9);
            assert_eq!(end, 12);
            assert_eq!(source_len, 10);
        }
        other => panic!("unexpected error: {other}"),
    }
}

fn make_t2_emission_run() -> whitaker_sarif::Run {
    let fragments = vec![
        fragment(FragmentInput {
            id: "alpha",
            profile: NormProfile::T2,
            file_uri: "src/a.rs",
            source_text: "fn a(x: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
        fragment(FragmentInput {
            id: "beta",
            profile: NormProfile::T2,
            file_uri: "src/b.rs",
            source_text: "fn b(y: i32) {}\n",
            hashes: &[(1, 0..15), (2, 0..15)],
        }),
    ];
    let accepted = vec![AcceptedPair::new(
        pair("alpha", "beta"),
        NormProfile::T2,
        SimilarityRatio::new(2, 2),
    )];

    emit_run0(&fragments, &accepted, &config())
        .unwrap_or_else(|error| panic!("unexpected emit error: {error}"))
}

#[test]
fn emitted_t2_result_has_correct_rule_id() {
    let run = make_t2_emission_run();
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };

    assert_eq!(result.rule_id, WHK002_ID);
}

#[test]
fn emitted_t2_result_contains_required_fingerprint_keys() {
    let run = make_t2_emission_run();
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };

    assert!(
        result
            .partial_fingerprints
            .contains_key(WHITAKER_FRAGMENT_KEY)
    );
    assert!(result.partial_fingerprints.contains_key("tokenHash"));
}

#[test]
fn emitted_t2_result_properties_match_config() {
    let run = make_t2_emission_run();
    let [result] = run.results.as_slice() else {
        panic!("expected one result");
    };
    let properties = result
        .properties
        .as_ref()
        .unwrap_or_else(|| panic!("Whitaker properties must be present"));
    let extracted = WhitakerProperties::try_from(properties)
        .unwrap_or_else(|error| panic!("unexpected property extraction error: {error}"));

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
