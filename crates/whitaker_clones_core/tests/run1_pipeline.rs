//! End-to-end pipeline: source files to a serialized SARIF log with Run 1.
//!
//! This is the one test that runs the *real* token pass. Every other Run 1 test
//! constructs `AcceptedPair` values directly, which is what lets them express
//! "the token pass missed this pair" without reverse-engineering a Jaccard
//! score. Here the verdicts come from normalization, shingling, winnowing,
//! `MinHash` and LSH, so a regression anywhere in that chain shows up as a wrong
//! Run 1 result.

mod run1_pipeline_support;

use run1_pipeline_support::{
    CARRIED_A,
    CARRIED_B,
    Fixture,
    NEAR_MISS_A,
    UNRELATED,
    assert_single_near_miss,
    assert_unrelated_is_not_reported,
    candidates,
    configs,
    fragments_from,
    near_miss_was_accepted,
    read_fixtures_back,
    round_trip_fixtures,
    run_named_pipeline,
    run_pipeline,
};
use whitaker_clones_core::{
    AstPassConfig,
    CandidatePair,
    NormProfile,
    RefinementInput,
    TokenFragment,
    TokenPassConfig,
    accept_candidate_pairs,
    emit_run1,
    normalize,
    refine_candidate_pairs,
};
use whitaker_sarif::{AST_HASH_KEY, SarifLog, WHITAKER_FRAGMENT_KEY, WHK003_ID, merge_runs};

/// Check the whole pipeline from source files to a serialized SARIF log.
#[test]
fn pipeline_produces_run_one_with_an_added_near_miss() {
    let (run0, run1, accepted) = run_pipeline();
    let was_accepted = near_miss_was_accepted(&accepted);

    assert!(
        !was_accepted,
        "the fixtures must make the near-miss a genuine token-pass miss"
    );
    assert!(
        run0.results.is_empty(),
        "Run 0 must report nothing for these fixtures, got {} results",
        run0.results.len()
    );
    assert_single_near_miss(&run1, was_accepted);
    assert_unrelated_is_not_reported(&run1);

    let log = SarifLog {
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_owned(),
        version: "2.1.0".to_owned(),
        runs: vec![run0, run1],
    };
    let json = serde_json::to_string(&log).expect("the log must serialize");
    let parsed: SarifLog = serde_json::from_str(&json).expect("the log must parse back");

    let parsed_runs = &parsed.runs;
    assert_eq!(parsed_runs.len(), 2, "the log carries Run 0 and Run 1");
    let second = parsed_runs.get(1).expect("Run 1 is the second run");
    assert_eq!(
        second.tool.driver.name, "whitaker_clones_cli@ast",
        "the refined run names the AST producer"
    );
    assert!(
        second
            .results
            .iter()
            .any(|result| result.rule_id == WHK003_ID),
        "the parsed Run 1 still holds the added near-miss"
    );
}

/// Check that E1 holds on real data: Run 1 covers every Run 0 result.
///
/// This test needs fixtures the token pass *accepts*, because Run 0 reports only
/// accepted pairs. The shipped pipeline fixtures are a deliberate token-pass
/// miss, so using them here would leave the loop below with nothing to check and
/// the test would pass even if Run 1 dropped every token result. The carried
/// fixtures are rename-equivalent, so the token pass accepts them under both
/// profiles and the AST pass carries each one.
#[test]
fn run_one_covers_every_run_zero_result() {
    let sources = round_trip_fixtures(&[
        ("carried_a", CARRIED_A),
        ("carried_b", CARRIED_B),
        ("unrelated", UNRELATED),
    ]);
    let (token_config, ast_config) = configs();
    let (run0, run1) = run_named_pipeline(&sources, &token_config, &ast_config);

    assert!(
        !run0.results.is_empty(),
        "the fixtures must make the token pass accept at least one pair, or this test would check \
         nothing"
    );
    assert!(
        run1.results.len() >= run0.results.len(),
        "Run 1 cannot report fewer results than Run 0, got {} against {}",
        run1.results.len(),
        run0.results.len()
    );

    let mut carried = 0;
    for result in &run0.results {
        let key = result
            .partial_fingerprints
            .get(WHITAKER_FRAGMENT_KEY)
            .expect("every Run 0 result carries a fragment key");
        let counterparts = run1
            .results
            .iter()
            .filter(|refined| {
                refined
                    .partial_fingerprints
                    .get(WHITAKER_FRAGMENT_KEY)
                    .is_some_and(|other| other == key)
            })
            .collect::<Vec<_>>();
        let [counterpart] = counterparts.as_slice() else {
            panic!(
                "Run 1 must hold exactly one counterpart for {key}, got {}",
                counterparts.len()
            );
        };

        assert_eq!(
            counterpart.locations, result.locations,
            "a carried or reclassified result keeps its Run 0 primary location"
        );
        assert_eq!(
            counterpart.related_locations.len(),
            result.related_locations.len(),
            "a carried or reclassified result keeps its related locations"
        );
        // The only permitted rule transition is a token rule becoming WHK003.
        let same_rule = counterpart.rule_id == result.rule_id;
        let strengthened = counterpart.rule_id == WHK003_ID && result.rule_id != WHK003_ID;
        assert!(
            same_rule || strengthened,
            "Run 1 may keep rule {} or strengthen it to WHK003, got {}",
            result.rule_id,
            counterpart.rule_id
        );
        if same_rule {
            carried += 1;
        }
    }
    assert!(
        carried > 0,
        "at least one Run 0 result must be carried, so the carried arm is covered"
    );
}

/// Check the `astHash` fingerprint keeps the 64-hex-digit digest shape.
///
/// The snapshot redacts the digest so it survives a parser re-pin, which would
/// otherwise leave the shape unchecked.
#[test]
fn ast_hash_fingerprint_is_a_sixty_four_digit_digest() {
    let (_run0, run1, _accepted) = run_pipeline();

    let hashes = run1
        .results
        .iter()
        .filter_map(|result| result.partial_fingerprints.get(AST_HASH_KEY))
        .collect::<Vec<_>>();
    assert!(
        !hashes.is_empty(),
        "the fixture must produce at least one conclusive result"
    );
    for hash in hashes {
        assert_eq!(hash.len(), 64, "the astHash digest must be 64 hex digits");
        assert!(
            hash.chars().all(|character| character.is_ascii_hexdigit()),
            "the astHash digest must be hexadecimal, got {hash}"
        );
    }
}

/// Check that a merged log keeps Run 1's refinements when Run 1 comes first.
#[test]
fn merged_log_keeps_run_one_first() {
    let (run0, run1, _accepted) = run_pipeline();
    let merged = merge_runs(&[run1, run0]).expect("the runs must merge");

    assert_eq!(
        merged.tool.driver.name, "whitaker_clones_cli@ast",
        "the merged tool name comes from the first run"
    );
    assert!(
        merged
            .results
            .iter()
            .any(|result| result.rule_id == WHK003_ID),
        "the added near-miss must survive the merge"
    );
}

/// Pin the complete Run 1 JSON for the pipeline fixture.
///
/// The snapshot redacts the two fields that legitimately move without a
/// contract change: the producer version, and the AST digests, which change
/// whenever the parser pin moves. Everything else — rules, ordering, baseline
/// states, provenance, regions and scores — is pinned.
#[test]
fn run_one_json_snapshot() {
    let (_run0, run1, _accepted) = run_pipeline();
    let value = serde_json::to_value(&run1).expect("Run 1 must serialize");
    let mut redacted = value;
    redact(&mut redacted);
    insta::assert_json_snapshot!("run1_pipeline", redacted);
}

/// Replace version- and parser-dependent values with stable placeholders.
fn redact(value: &mut serde_json::Value) {
    redact_version(value);
    let Some(results) = value.get_mut("results").and_then(|r| r.as_array_mut()) else {
        return;
    };
    for result in results {
        redact_fingerprints(result);
    }
}

/// Replace the producer version, which moves with every release.
fn redact_version(value: &mut serde_json::Value) {
    let version_slot = value
        .get_mut("tool")
        .and_then(|tool| tool.get_mut("driver"))
        .and_then(|driver| driver.get_mut("version"));
    if let Some(version) = version_slot {
        *version = serde_json::Value::String("<version>".to_owned());
    }
}

/// Replace the digests that change when the parser pin moves.
fn redact_fingerprints(result: &mut serde_json::Value) {
    let Some(fingerprints) = result
        .get_mut("partialFingerprints")
        .and_then(|map| map.as_object_mut())
    else {
        return;
    };
    for key in ["astHash", "tokenHash", "whitakerFragment"] {
        if fingerprints.contains_key(key) {
            fingerprints.insert(
                key.to_owned(),
                serde_json::Value::String(format!("<{key}>")),
            );
        }
    }
}

/// Check the fixtures are large enough for the plan's pipeline requirement.
///
/// The `ExecPlan` asks for at least 150 tokens per fixture. Small functions make
/// the Jaccard and cosine scores unrepresentative, and they are also where the
/// trigram feature is least discriminating.
#[test]
fn fixtures_are_at_least_one_hundred_and_fifty_tokens() {
    let sources = read_fixtures_back();
    for (name, text) in &sources {
        let tokens = normalize(text, NormProfile::T1).expect("the fixture must normalize");
        assert!(
            tokens.len() >= 150,
            "fixture `{name}` has {} tokens, below the plan's 150",
            tokens.len()
        );
    }
}

/// E2: Run 1 is byte-identical under permutation and duplication of its input.
///
/// The emitter sorts and deduplicates, so a caller that feeds candidates in a
/// different order — or the same pair twice, as happens when both profiles
/// propose it — must get the same log.
#[test]
fn run_one_is_deterministic_under_input_permutation() {
    let sources = read_fixtures_back();
    let fragments = fragments_from(&sources);
    let candidates = candidates(&fragments);
    let (token_config, ast_config) = configs();

    let mut doubled = candidates.clone();
    doubled.extend(candidates.iter().cloned());
    doubled.reverse();

    let forward = emit_for(&fragments, &candidates, &token_config, &ast_config);
    let shuffled = emit_for(&fragments, &doubled, &token_config, &ast_config);

    let forward_json = serde_json::to_string(&forward).expect("Run 1 must serialize");
    let shuffled_json = serde_json::to_string(&shuffled).expect("Run 1 must serialize");
    assert_eq!(
        forward_json, shuffled_json,
        "permuting and duplicating the candidate input must not change Run 1"
    );
}

/// Refine and emit Run 1 for a given candidate set.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn emit_for(
    fragments: &[TokenFragment],
    candidates: &[CandidatePair],
    token_config: &TokenPassConfig,
    ast_config: &AstPassConfig,
) -> whitaker_sarif::Run {
    let accepted = accept_candidate_pairs(fragments, candidates, token_config)
        .expect("token-pass acceptance must succeed");
    let input = RefinementInput::new(fragments, candidates, &accepted, token_config);
    let refined = refine_candidate_pairs(&input, ast_config).expect("refinement must succeed");
    emit_run1(&input, refined.pairs(), ast_config).expect("Run 1 must emit")
}

/// Check that an empty candidate set still emits a well-formed Run 1.
#[test]
fn empty_candidates_still_emit_a_run() {
    let sources = read_fixtures_back();
    let fragments = fragments_from(&sources);
    let (token_config, ast_config) = configs();
    let input = RefinementInput::new(&fragments, &[], &[], &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, refined.pairs(), &ast_config).expect("Run 1 must emit");

    assert!(
        run1.results.is_empty(),
        "no candidates means no results, got {}",
        run1.results.len()
    );
    assert_eq!(
        run1.tool.driver.rules.len(),
        3,
        "Run 1 still declares all three rules"
    );
    assert!(
        run1.invocations
            .first()
            .is_some_and(|invocation| invocation.execution_successful),
        "Run 1 records one successful invocation"
    );
}

/// Check that identical functions in different files report a cosine of one.
#[test]
fn identical_functions_score_a_whole_cosine() {
    let left = Fixture::new("twin_left", NEAR_MISS_A);
    let right = Fixture::new("twin_right", NEAR_MISS_A);
    let fragments = vec![
        left.fragment(NormProfile::T1),
        right.fragment(NormProfile::T1),
    ];
    let candidates = candidates(&fragments);
    let (token_config, ast_config) = configs();
    let accepted = accept_candidate_pairs(&fragments, &candidates, &token_config)
        .expect("the identical pair must be accepted");
    assert!(
        !accepted.is_empty(),
        "identical functions must be accepted by the token pass"
    );
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, refined.pairs(), &ast_config).expect("Run 1 must emit");

    let [result] = run1.results.as_slice() else {
        panic!(
            "the twin pair must produce one result, got {:?}",
            run1.results
        );
    };
    let properties = result
        .properties
        .as_ref()
        .expect("every result carries Whitaker properties");
    let extracted =
        whitaker_sarif::WhitakerProperties::try_from(properties).expect("properties extract");
    assert_eq!(
        extracted.cosine.map(f64::to_bits),
        Some(1.0_f64.to_bits()),
        "identical function bodies must score a cosine of exactly one"
    );
}
