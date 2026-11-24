//! Behaviour-driven coverage for AST-pass Run 1 refinement.
//!
//! Keep this harness in sync with `tests/features/run1_sarif.feature`.
//! The snippets are real Rust so the scenarios exercise the real lowering
//! adapter; only the token-pass verdicts are constructed directly, which is
//! what lets one scenario express "the token pass missed this pair" without
//! reverse-engineering a Jaccard score.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{
    AcceptedPair,
    AstPassConfig,
    CandidatePair,
    FragmentId,
    FragmentLocation,
    NormProfile,
    RefinedPair,
    RefinementInput,
    Run1Error,
    SimilarityRatio,
    SimilarityThreshold,
    TokenFragment,
    TokenPassConfig,
    emit_run0,
    emit_run1,
    refine_candidate_pairs,
};
use whitaker_sarif::{AST_HASH_KEY, BaselineState, Run, WHK003_ID, WhitakerProperties};
use whitaker_test_macros::allow_fixture_expansion_lints;

/// A function with a loop and two bindings.
const NEAR_MISS_LEFT: &str = concat!(
    "fn parse(input: &str) -> usize {\n",
    "    let mut total = 0;\n",
    "    for part in input.split(',') {\n",
    "        total += part.len();\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// The same function with one extra statement: a near miss.
const NEAR_MISS_RIGHT: &str = concat!(
    "fn parse(input: &str) -> usize {\n",
    "    let mut total = 0;\n",
    "    for part in input.split(',') {\n",
    "        total += part.len();\n",
    "    }\n",
    "    total += 1;\n",
    "    total\n",
    "}\n",
);

/// A function whose identifiers differ from [`RENAMED_RIGHT`] only.
const RENAMED_LEFT: &str = concat!(
    "fn alpha(value: i32) -> i32 {\n",
    "    let doubled = value * 2;\n",
    "    let offset = doubled + 1;\n",
    "    check(offset);\n",
    "    offset\n",
    "}\n",
);

/// A rename-equivalent twin of [`RENAMED_LEFT`].
const RENAMED_RIGHT: &str = concat!(
    "fn beta(item: i32) -> i32 {\n",
    "    let twice = item * 2;\n",
    "    let shifted = twice + 1;\n",
    "    check(shifted);\n",
    "    shifted\n",
    "}\n",
);

/// A loop over numbers.
const UNRELATED_LEFT: &str = concat!(
    "fn alpha(value: i32) -> i32 {\n",
    "    let mut total = 0;\n",
    "    while total < value {\n",
    "        total += 3;\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// A string builder with a different shape.
const UNRELATED_RIGHT: &str = concat!(
    "fn beta(text: &str) -> String {\n",
    "    let mut output = String::new();\n",
    "    for part in text.split('-') {\n",
    "        output.push_str(part);\n",
    "    }\n",
    "    output\n",
    "}\n",
);

/// Rust that `ra_ap_syntax` lowers to an `ERROR` node.
///
/// The crate's lowering tests pin this fixture as unparsable, so the scenario
/// exercises a real parser error rather than a synthetic one.
const BROKEN_RIGHT: &str = "@error@";

/// World state for the Run 1 scenarios.
#[derive(Debug)]
struct Run1World {
    fragments: RefCell<Vec<TokenFragment>>,
    candidates: RefCell<Vec<CandidatePair>>,
    accepted: RefCell<Vec<AcceptedPair>>,
    config: RefCell<AstPassConfig>,
    token_config: RefCell<TokenPassConfig>,
    refined: RefCell<Option<Vec<RefinedPair>>>,
    run: RefCell<Option<Run>>,
    error: RefCell<Option<Run1Error>>,
}

impl Default for Run1World {
    fn default() -> Self {
        Self {
            fragments: RefCell::new(Vec::new()),
            candidates: RefCell::new(Vec::new()),
            accepted: RefCell::new(Vec::new()),
            config: RefCell::new(AstPassConfig::new(env!("CARGO_PKG_VERSION"))),
            token_config: RefCell::new(TokenPassConfig::new(
                "whitaker_clones_cli@token",
                env!("CARGO_PKG_VERSION"),
            )),
            refined: RefCell::new(None),
            run: RefCell::new(None),
            error: RefCell::new(None),
        }
    }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> Run1World { Run1World::default() }

/// Return a snippet by fixture name.
fn snippet(name: &str) -> Result<&'static str, String> {
    // The three near-miss scenario families share one snippet pair; keeping
    // them as separate names documents which scenario each one feeds.
    match name {
        "near_miss_left" | "reshaped_left" | "near_exact_left" => Ok(NEAR_MISS_LEFT),
        "near_miss_right" | "reshaped_right" | "near_exact_right" => Ok(NEAR_MISS_RIGHT),
        "renamed_left" => Ok(RENAMED_LEFT),
        "renamed_right" => Ok(RENAMED_RIGHT),
        "unrelated_left" => Ok(UNRELATED_LEFT),
        "unrelated_right" => Ok(UNRELATED_RIGHT),
        "broken_right" => Ok(BROKEN_RIGHT),
        other => Err(format!("unknown fragment fixture `{other}`")),
    }
}

/// Load one fragment under the requested profile, replacing any earlier copy.
fn load_fragment(world: &Run1World, name: &str, profile: NormProfile) -> Result<(), String> {
    let text = snippet(name)?;
    let id = format!("{name}_{}", profile_label(profile));
    let location = FragmentLocation::new("src/fixture.rs", 0..text.len());
    let fragment = TokenFragment::new(FragmentId::from(id.as_str()), profile, location, text)
        .with_retained_fingerprints(vec![whitaker_clones_core::Fingerprint::new(11, 0..4)]);
    let mut fragments = world.fragments.borrow_mut();
    fragments.retain(|existing| existing.id().as_str() != id);
    fragments.push(fragment);
    Ok(())
}

/// Return the profile label used in fixture identifiers.
const fn profile_label(profile: NormProfile) -> &'static str {
    match profile {
        NormProfile::T1 => "t1",
        NormProfile::T2 => "t2",
    }
}

/// Resolve a fixture name and profile to its fragment identifier.
fn fragment_id(name: &str, profile: NormProfile) -> FragmentId {
    FragmentId::from(format!("{name}_{}", profile_label(profile)))
}

/// Queue a candidate pair for two fragments under the same profile.
fn queue_candidate(world: &Run1World, left: &str, right: &str, profile: NormProfile) {
    let left_id = fragment_id(left, profile);
    let right_id = fragment_id(right, profile);
    if let Some(pair) = CandidatePair::new(left_id, right_id) {
        let mut candidates = world.candidates.borrow_mut();
        if !candidates.contains(&pair) {
            candidates.push(pair);
        }
    }
}

/// Record that the token pass accepted a pair under a profile.
fn accept_pair(world: &Run1World, left: &str, right: &str, profile: NormProfile) {
    let left_id = fragment_id(left, profile);
    let right_id = fragment_id(right, profile);
    let Some(pair) = CandidatePair::new(left_id, right_id) else {
        return;
    };
    world.accepted.borrow_mut().push(AcceptedPair::new(
        pair,
        profile,
        SimilarityRatio::new(18, 20),
    ));
}

/// Return the fragments, candidates and accepted pairs for refinement.
fn input_parts(world: &Run1World) -> (Vec<TokenFragment>, Vec<CandidatePair>, Vec<AcceptedPair>) {
    (
        world.fragments.borrow().clone(),
        world.candidates.borrow().clone(),
        world.accepted.borrow().clone(),
    )
}

/// Run the AST pass and emit Run 1, recording any error.
fn refine_and_emit(world: &Run1World) {
    let (fragments, candidates, accepted) = input_parts(world);
    let config = world.config.borrow().clone();
    let token_config = world.token_config.borrow().clone();
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    match refine_candidate_pairs(&input, &config) {
        Ok(refined) => match emit_run1(&input, &refined, &config) {
            Ok(run) => {
                *world.refined.borrow_mut() = Some(refined);
                *world.run.borrow_mut() = Some(run);
                *world.error.borrow_mut() = None;
            }
            Err(error) => {
                *world.run.borrow_mut() = None;
                *world.error.borrow_mut() = Some(error);
            }
        },
        Err(error) => {
            *world.refined.borrow_mut() = None;
            *world.run.borrow_mut() = None;
            *world.error.borrow_mut() = Some(error);
        }
    }
}

/// Run an assertion over the emitted results, failing when no run exists.
fn with_results(world: &Run1World, assert_fn: impl FnOnce(&[whitaker_sarif::SarifResult])) {
    let borrowed = world.run.borrow();
    let Some(run) = borrowed.as_ref() else {
        panic!("Run 1 must be emitted before checking results");
    };
    assert_fn(&run.results);
}

/// Extract the Whitaker properties of one result.
fn properties_of(result: &whitaker_sarif::SarifResult) -> WhitakerProperties {
    let Some(properties) = result.properties.as_ref() else {
        panic!("every Run 1 result carries Whitaker properties");
    };
    match WhitakerProperties::try_from(properties) {
        Ok(extracted) => extracted,
        Err(error) => panic!("Whitaker properties must extract: {error}"),
    }
}

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
