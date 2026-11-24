//! Run 1 emission tests: wire shape, determinism, identity and merge order.

use whitaker_sarif::{
    AST_HASH_KEY,
    BaselineState,
    RefinementKind,
    SarifResult,
    WHITAKER_FRAGMENT_KEY,
    WHK001_ID,
    WHK002_ID,
    WHK003_ID,
    WhitakerProperties,
};

use super::emit_run1;
use crate::{
    AcceptedPair,
    AstEvidence,
    AstHash,
    AstPassConfig,
    CandidatePair,
    CloneRule,
    CosineMicros,
    FragmentId,
    NormProfile,
    RefinedPair,
    RefinementInput,
    SimilarityRatio,
    TokenFragment,
    TokenPassConfig,
    test_support::{FragmentSpec, fixture_fragment},
};

/// Token-pass configuration used by the emission tests.
pub(super) fn token_config() -> TokenPassConfig {
    TokenPassConfig::new("whitaker_clones_cli@token", "0.3.0")
}

/// AST-pass configuration used by the emission tests.
pub(super) fn ast_config() -> AstPassConfig { AstPassConfig::new("0.3.0") }

/// Build a fragment over the shared fixture source.
pub(super) fn fragment(
    id: &str,
    profile: NormProfile,
    span: std::ops::Range<usize>,
    hashes: &[u64],
) -> TokenFragment {
    fixture_fragment(&FragmentSpec {
        id,
        profile,
        file_uri: "src/fixture.rs",
        span,
        hashes,
    })
}

/// Build a canonical candidate pair.
pub(super) fn pair(left: &str, right: &str) -> CandidatePair {
    CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .expect("the fixtures use distinct identifiers")
}

/// Build an accepted pair.
pub(super) fn accepted(left: &str, right: &str, profile: NormProfile) -> AcceptedPair {
    AcceptedPair::new(pair(left, right), profile, SimilarityRatio::new(18, 20))
}

/// Build conclusive evidence with distinguishable hashes.
pub(super) fn evidence(equal: bool) -> AstEvidence {
    let left = AstHash::from_hex("0123456789abcdef");
    let right = if equal {
        left
    } else {
        AstHash::from_hex("fedcba9876543210")
    };
    let micros = if equal { 1_000_000 } else { 900_000 };
    AstEvidence::new(
        left,
        right,
        CosineMicros::from_micros(micros).expect("a valid score"),
    )
}

/// Build one refined pair of each provenance kind.
pub(super) fn all_kinds() -> Vec<RefinedPair> {
    vec![
        RefinedPair::Carried {
            token: accepted("carried_left", "carried_right", NormProfile::T2),
            ast: Some(evidence(true)),
        },
        RefinedPair::Reclassified {
            token: accepted("reshaped_left", "reshaped_right", NormProfile::T2),
            ast: evidence(false),
        },
        RefinedPair::Added {
            pair: pair("added_left", "added_right"),
            rule: CloneRule::Type3,
            jaccard: SimilarityRatio::new(3, 4),
            ast: evidence(false),
        },
        RefinedPair::Carried {
            token: accepted("unparsable_left", "unparsable_right", NormProfile::T1),
            ast: None,
        },
    ]
}

/// Build fragments covering every identifier used by [`all_kinds`].
pub(super) fn all_fragments() -> Vec<TokenFragment> {
    let names = [
        ("carried_left", NormProfile::T2, 0..10),
        ("carried_right", NormProfile::T2, 10..20),
        ("reshaped_left", NormProfile::T2, 20..30),
        ("reshaped_right", NormProfile::T2, 30..40),
        ("added_left", NormProfile::T2, 40..50),
        ("added_right", NormProfile::T2, 50..60),
        ("unparsable_left", NormProfile::T1, 60..70),
        ("unparsable_right", NormProfile::T1, 70..80),
    ];
    names
        .iter()
        .map(|(id, profile, span)| fragment(id, *profile, span.clone(), &[7_u64, 8]))
        .collect()
}

/// Emit Run 1 over the standard fixture.
pub(super) fn emit_all(refined: &[RefinedPair]) -> Result<whitaker_sarif::Run, super::Run1Error> {
    let fragments = all_fragments();
    let token = token_config();
    let input = RefinementInput::new(&fragments, &[], &[], &token);
    emit_run1(&input, refined, &ast_config())
}

/// Check the run's producer, invocation, rules and result ordering.
#[test]
fn run_has_the_documented_wire_shape() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");

    assert_eq!(
        run.tool.driver.name, "whitaker_clones_cli@ast",
        "Run 1 must name the AST producer"
    );
    assert_eq!(
        run.invocations.len(),
        1,
        "Run 1 must record exactly one invocation"
    );
    assert!(
        run.invocations
            .first()
            .is_some_and(|invocation| invocation.execution_successful),
        "the invocation must record success"
    );
    let rules = run
        .tool
        .driver
        .rules
        .iter()
        .map(|rule| rule.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        rules,
        vec![WHK001_ID, WHK002_ID, WHK003_ID],
        "Run 1 must carry all three clone rules"
    );

    let keys = run.results.iter().map(sort_key).collect::<Vec<_>>();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "results must be ordered by rule and fingerprint"
    );
}

/// Return a result's ordering tuple.
fn sort_key(result: &SarifResult) -> (String, String, String) {
    (
        result.rule_id.clone(),
        result
            .partial_fingerprints
            .get(WHITAKER_FRAGMENT_KEY)
            .cloned()
            .unwrap_or_default(),
        result.message.text.clone(),
    )
}

/// Check that every result declares its baseline state and provenance.
#[test]
fn every_result_declares_baseline_state_and_provenance() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");

    for result in &run.results {
        assert!(
            result.baseline_state.is_some(),
            "every Run 1 result must declare a baseline state: {result:?}"
        );
        let properties = result
            .properties
            .as_ref()
            .expect("every Run 1 result must carry Whitaker properties");
        let extracted = WhitakerProperties::try_from(properties).expect("properties extract");
        assert!(
            extracted.refinement.is_some(),
            "every Run 1 result must record its refinement provenance"
        );
    }
}

/// Check that carried results keep their Run 0 fingerprints and locations.
#[test]
fn carried_results_keep_their_run_zero_identity() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let carried = run
        .results
        .iter()
        .filter(|result| result.rule_id == WHK002_ID)
        .collect::<Vec<_>>();

    assert_eq!(carried.len(), 1, "exactly one carried WHK002 result");
    let result = carried.first().expect("the carried result exists");
    assert_eq!(
        result.baseline_state,
        Some(BaselineState::Unchanged),
        "a carried result is unchanged"
    );
    let properties =
        WhitakerProperties::try_from(result.properties.as_ref().expect("properties present"))
            .expect("properties extract");
    assert_eq!(
        properties.refinement,
        Some(RefinementKind::Carried),
        "the provenance must say carried"
    );
    assert_eq!(
        properties.cosine.map(f64::to_bits),
        Some(1.0_f64.to_bits()),
        "conclusive evidence records its cosine"
    );
    assert!(
        result.partial_fingerprints.contains_key(AST_HASH_KEY),
        "conclusive evidence records an astHash fingerprint"
    );
}

/// Check that a reclassified result reports `WHK003` and its token rule.
#[test]
fn reclassified_results_record_their_token_rule() {
    let run = emit_all(&all_kinds()).expect("Run 1 emission must succeed");
    let reclassified = run
        .results
        .iter()
        .filter(|result| result.rule_id == WHK003_ID)
        .collect::<Vec<_>>();

    assert_eq!(
        reclassified.len(),
        2,
        "one reclassified and one added result"
    );
    let updated = reclassified
        .iter()
        .find(|result| result.baseline_state == Some(BaselineState::Updated))
        .expect("the reclassified result exists");
    let properties =
        WhitakerProperties::try_from(updated.properties.as_ref().expect("properties present"))
            .expect("properties extract");
    assert_eq!(
        properties.refinement,
        Some(RefinementKind::Reclassified),
        "the provenance must say reclassified"
    );
    assert_eq!(
        properties.token_rule_id.as_deref(),
        Some(WHK002_ID),
        "the token rule must be recorded as provenance"
    );
    assert!(
        updated.message.text.contains("sim = 0.900000"),
        "a near-miss message reports the cosine, got {}",
        updated.message.text
    );
}
