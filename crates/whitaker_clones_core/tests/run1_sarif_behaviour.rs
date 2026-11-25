//! Behaviour-driven Run 1 scenarios: step definitions and bindings.
//!
//! Keep this harness in sync with `tests/features/run1_sarif.feature`. The
//! fixtures, world state and shared helpers live in `run1_sarif_support`; this
//! file holds only the steps and their scenario bindings, so a new scenario
//! reads as its Gherkin.

mod run1_sarif_support;

use rstest_bdd_macros::{given, scenario, then, when};
use run1_sarif_support::{
    Run1World,
    accept_pair,
    input_parts,
    load_fragment,
    properties_of,
    queue_candidate,
    refine_and_emit,
    snippet,
    with_results,
    world,
};
use whitaker_clones_core::{
    AstPassConfig,
    NormProfile,
    Run1Error,
    SimilarityThreshold,
    TokenPassConfig,
    emit_run0,
};
use whitaker_sarif::{AST_HASH_KEY, BaselineState, WHK003_ID};

#[given("the Rust fragments {left} and {right}")]
fn given_fragments(world: &Run1World, left: String, right: String) -> Result<(), String> {
    load_fragment(world, &left, NormProfile::T2)?;
    load_fragment(world, &right, NormProfile::T2)?;
    queue_candidate(world, &left, &right, NormProfile::T2);
    Ok(())
}

#[given("the token pass does not accept the pair")]
fn given_not_accepted(world: &Run1World) { world.accepted.borrow_mut().clear(); }

#[given("the token pass accepts the pair as WHK001")]
fn given_accepted_t1(world: &Run1World) -> Result<(), String> {
    let (left, right) = first_candidate_names(world)?;
    // The T1 profile needs its own fragments: a fragment's identity carries
    // the normalization profile even when its physical location is shared.
    load_fragment(world, &left, NormProfile::T1)?;
    load_fragment(world, &right, NormProfile::T1)?;
    accept_pair(world, &left, &right, NormProfile::T1);
    Ok(())
}

#[given("the token pass accepts the pair as WHK002")]
fn given_accepted_t2(world: &Run1World) -> Result<(), String> {
    let (left, right) = first_candidate_names(world)?;
    accept_pair(world, &left, &right, NormProfile::T2);
    Ok(())
}

/// Return the fixture names behind the first queued candidate.
fn first_candidate_names(world: &Run1World) -> Result<(String, String), String> {
    let candidates = world.candidates.borrow();
    let Some(pair) = candidates.first() else {
        return Err("a candidate pair must be queued first".to_owned());
    };
    let strip = |id: &str| {
        id.strip_suffix("_t1")
            .or_else(|| id.strip_suffix("_t2"))
            .map(str::to_owned)
            .ok_or_else(|| format!("fragment `{id}` is not a named fixture"))
    };
    Ok((strip(pair.left().as_str())?, strip(pair.right().as_str())?))
}

#[given("the Rust fragments {left} and {right} under both profiles")]
fn given_both_profiles(world: &Run1World, left: String, right: String) -> Result<(), String> {
    for profile in [NormProfile::T1, NormProfile::T2] {
        load_fragment(world, &left, profile)?;
        load_fragment(world, &right, profile)?;
        queue_candidate(world, &left, &right, profile);
    }
    Ok(())
}

#[given("the token pass accepts only the T2 pair")]
fn given_accepts_only_t2(world: &Run1World) -> Result<(), String> {
    let candidates = world.candidates.borrow().clone();
    let Some(pair) = candidates
        .iter()
        .find(|pair| pair.left().as_str().ends_with("_t2"))
    else {
        return Err("a T2 candidate pair must be queued first".to_owned());
    };
    let strip = |id: &str| id.trim_end_matches("_t2").to_owned();
    let left = strip(pair.left().as_str());
    let right = strip(pair.right().as_str());
    accept_pair(world, &left, &right, NormProfile::T2);
    Ok(())
}

#[given("lowering {name} reports an unparsable span")]
fn given_unparsable(name: String) -> Result<(), String> {
    let text = snippet(&name)?;
    let span = whitaker_clones_core::ByteSpan::new(text, 0, u32::try_from(text.len()).unwrap_or(0))
        .map_err(|error| error.to_string())?;
    match whitaker_clones_core::lower_span(text, span) {
        Err(whitaker_clones_core::AstError::UnparsableSpan { .. }) => Ok(()),
        other => Err(format!(
            "fixture `{name}` must lower to an unparsable span, got {other:?}"
        )),
    }
}

#[given("a type3 threshold of {type3} and a type2 threshold of {type2}")]
fn given_thresholds(world: &Run1World, type3: String, type2: String) -> Result<(), String> {
    let type3_threshold = decimal_threshold(&type3)?;
    let type2_threshold = decimal_threshold(&type2)?;
    let token_config = TokenPassConfig::new("whitaker_clones_cli@token", env!("CARGO_PKG_VERSION"))
        .with_type2_threshold(type2_threshold);
    world.config.replace(
        AstPassConfig::new(env!("CARGO_PKG_VERSION")).with_type3_threshold(type3_threshold),
    );
    world.token_config.replace(token_config);
    Ok(())
}

/// Parse a two-decimal threshold such as `0.95` into an exact ratio.
///
/// The parser works on digits, so no floating-point value influences a
/// threshold.
fn decimal_threshold(text: &str) -> Result<SimilarityThreshold, String> {
    let (integer, fraction) = text
        .split_once('.')
        .ok_or_else(|| format!("threshold `{text}` must have two decimals"))?;
    let digits = format!("{integer}{fraction}");
    let numerator = digits
        .parse::<usize>()
        .map_err(|error| format!("threshold `{text}` is not numeric: {error}"))?;
    let scale = 10_usize
        .checked_pow(u32::try_from(fraction.len()).map_err(|error| error.to_string())?)
        .ok_or_else(|| format!("threshold `{text}` has too many decimals"))?;
    SimilarityThreshold::new("custom", numerator, scale).map_err(|error| error.to_string())
}

#[when("the AST pass refines the candidates")]
fn when_refined(world: &Run1World) { refine_and_emit(world); }

/// Return every result whose rule is `WHK003`.
fn type3_results(world: &Run1World) -> Vec<whitaker_sarif::SarifResult> {
    let borrowed = world.run.borrow();
    borrowed
        .as_ref()
        .map(|run| {
            run.results
                .iter()
                .filter(|result| result.rule_id == WHK003_ID)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[then("Run 1 contains a WHK003 result with baseline state {state}")]
fn then_type3_with_state(world: &Run1World, state: String) {
    let expected = match state.as_str() {
        "new" => BaselineState::New,
        "updated" => BaselineState::Updated,
        "unchanged" => BaselineState::Unchanged,
        other => panic!("unknown baseline state `{other}`"),
    };
    let results = type3_results(world);
    assert!(
        results
            .iter()
            .any(|result| result.baseline_state == Some(expected)),
        "Run 1 must contain a WHK003 result with baseline state {state}, got {results:?}"
    );
}

#[then("the result records a cosine of at least {minimum}")]
fn then_cosine_at_least(world: &Run1World, minimum: String) {
    let bound = match minimum.parse::<f64>() {
        Ok(bound) => bound,
        Err(error) => panic!("the feature's cosine bound must parse as a float: {error}"),
    };
    let results = type3_results(world);
    assert!(
        results.iter().any(|result| {
            properties_of(result)
                .cosine
                .is_some_and(|cosine| cosine >= bound)
        }),
        "a WHK003 result must record a cosine of at least {bound}, got {results:?}"
    );
}

#[then("the result carries an astHash partial fingerprint")]
fn then_has_ast_hash(world: &Run1World) {
    let results = type3_results(world);
    assert!(
        results
            .iter()
            .any(|result| result.partial_fingerprints.contains_key(AST_HASH_KEY)),
        "a WHK003 result must carry an astHash fingerprint, got {results:?}"
    );
}

#[then("Run 1 contains a WHK002 result with baseline state {state}")]
fn then_type2_with_state(world: &Run1World, state: String) {
    let expected = match state.as_str() {
        "new" => BaselineState::New,
        "updated" => BaselineState::Updated,
        "unchanged" => BaselineState::Unchanged,
        other => panic!("unknown baseline state `{other}`"),
    };
    with_results(world, |results| {
        assert!(
            results.iter().any(|result| {
                result.rule_id == "WHK002" && result.baseline_state == Some(expected)
            }),
            "Run 1 must contain a WHK002 result with baseline state {state}, got {results:?}"
        );
    });
}

#[then("the result records a cosine of {expected}")]
fn then_cosine_is(world: &Run1World, expected: String) {
    let wanted = match expected.parse::<f64>() {
        Ok(wanted) => wanted,
        Err(error) => panic!("the feature's cosine must parse as a float: {error}"),
    };
    with_results(world, |results| {
        assert!(
            results
                .iter()
                .any(|result| properties_of(result).cosine == Some(wanted)),
            "a result must record a cosine of {wanted}, got {results:?}"
        );
    });
}

#[then("the result keeps its Run 0 whitakerFragment fingerprint")]
fn then_keeps_run_zero_fingerprint(world: &Run1World) {
    let (fragments, _, accepted) = input_parts(world);
    let token_config = world.token_config.borrow().clone();
    let run0 = match emit_run0(&fragments, &accepted, &token_config) {
        Ok(run) => run,
        Err(error) => panic!("Run 0 emission must succeed for the accepted fixtures: {error}"),
    };
    let run0_keys = run0
        .results
        .iter()
        .map(|result| {
            result
                .partial_fingerprints
                .get(whitaker_sarif::WHITAKER_FRAGMENT_KEY)
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    with_results(world, |results| {
        assert!(
            results.iter().any(|result| {
                result
                    .partial_fingerprints
                    .get(whitaker_sarif::WHITAKER_FRAGMENT_KEY)
                    .is_some_and(|key| run0_keys.contains(key))
            }),
            "a Run 1 result must keep its Run 0 fingerprint, Run 0 had {run0_keys:?}"
        );
    });
}

#[then("the result records token rule {rule}")]
fn then_token_rule(world: &Run1World, rule: String) {
    let results = type3_results(world);
    assert!(
        results
            .iter()
            .any(|result| properties_of(result).token_rule_id.as_deref() == Some(rule.as_str())),
        "a WHK003 result must record token rule {rule}, got {results:?}"
    );
}

#[then("Run 1 contains exactly {count} result")]
fn then_result_count(world: &Run1World, count: usize) {
    with_results(world, |results| {
        assert_eq!(
            results.len(),
            count,
            "Run 1 must contain exactly {count} result(s), got {results:?}"
        );
    });
}

#[then("Run 1 contains no result for the pair")]
fn then_no_results(world: &Run1World) {
    with_results(world, |results| {
        assert!(
            results.is_empty(),
            "no result must be reported for an unrelated pair, got {results:?}"
        );
    });
}

#[then("the result has no cosine and no astHash")]
fn then_no_ast_data(world: &Run1World) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist, got {results:?}");
        };
        assert!(
            !result.partial_fingerprints.contains_key(AST_HASH_KEY),
            "an inconclusive result must not carry an astHash"
        );
        assert_eq!(
            properties_of(result).cosine,
            None,
            "an inconclusive result must not record a cosine"
        );
    });
}

#[then("refinement fails with a threshold order error")]
fn then_threshold_order(world: &Run1World) {
    let borrowed = world.error.borrow();
    assert!(
        matches!(borrowed.as_ref(), Some(Run1Error::ThresholdOrder { .. })),
        "refinement must fail with a threshold order error, got {borrowed:?}"
    );
}

#[scenario(path = "tests/features/run1_sarif.feature", index = 0)]
fn scenario_near_miss_added(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 1)]
fn scenario_renamed_carried(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 2)]
fn scenario_reshaped_reclassified(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 3)]
fn scenario_near_exact_reclassified(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 4)]
fn scenario_both_profiles(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 5)]
fn scenario_unrelated(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 6)]
fn scenario_unparsable(world: Run1World) { let _ = world; }

#[scenario(path = "tests/features/run1_sarif.feature", index = 7)]
fn scenario_misordered_thresholds(world: Run1World) { let _ = world; }
