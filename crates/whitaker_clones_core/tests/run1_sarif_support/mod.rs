//! Source fixtures, world state and shared helpers for the Run 1 scenarios.
//!
//! The snippets are real Rust so the scenarios exercise the real lowering
//! adapter; only the token-pass verdicts are constructed directly, which is what
//! lets one scenario express "the token pass missed this pair" without
//! reverse-engineering a Jaccard score. Split from the behaviour harness to keep
//! both files inside the module budget.

use std::cell::RefCell;

use rstest::fixture;
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
    TokenFragment,
    TokenPassConfig,
    emit_run1,
    refine_candidate_pairs,
};
use whitaker_sarif::{Run, WhitakerProperties};
use whitaker_test_macros::allow_fixture_expansion_lints;

/// A function with a loop and two bindings.
pub(super) const NEAR_MISS_LEFT: &str = concat!(
    "fn parse(input: &str) -> usize {\n",
    "    let mut total = 0;\n",
    "    for part in input.split(',') {\n",
    "        total += part.len();\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// The same function with one extra statement: a near miss.
pub(super) const NEAR_MISS_RIGHT: &str = concat!(
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
pub(super) const RENAMED_LEFT: &str = concat!(
    "fn alpha(value: i32) -> i32 {\n",
    "    let doubled = value * 2;\n",
    "    let offset = doubled + 1;\n",
    "    check(offset);\n",
    "    offset\n",
    "}\n",
);

/// A rename-equivalent twin of [`RENAMED_LEFT`].
pub(super) const RENAMED_RIGHT: &str = concat!(
    "fn beta(item: i32) -> i32 {\n",
    "    let twice = item * 2;\n",
    "    let shifted = twice + 1;\n",
    "    check(shifted);\n",
    "    shifted\n",
    "}\n",
);

/// A loop over numbers.
pub(super) const UNRELATED_LEFT: &str = concat!(
    "fn alpha(value: i32) -> i32 {\n",
    "    let mut total = 0;\n",
    "    while total < value {\n",
    "        total += 3;\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// A string builder with a different shape.
pub(super) const UNRELATED_RIGHT: &str = concat!(
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
pub(super) const BROKEN_RIGHT: &str = "@error@";

/// World state for the Run 1 scenarios.
#[derive(Debug)]
pub(super) struct Run1World {
    pub(super) fragments: RefCell<Vec<TokenFragment>>,
    pub(super) candidates: RefCell<Vec<CandidatePair>>,
    pub(super) accepted: RefCell<Vec<AcceptedPair>>,
    pub(super) config: RefCell<AstPassConfig>,
    pub(super) token_config: RefCell<TokenPassConfig>,
    pub(super) refined: RefCell<Option<Vec<RefinedPair>>>,
    pub(super) run: RefCell<Option<Run>>,
    pub(super) error: RefCell<Option<Run1Error>>,
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
pub(super) fn world() -> Run1World { Run1World::default() }

/// Return a snippet by fixture name.
pub(super) fn snippet(name: &str) -> Result<&'static str, String> {
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
pub(super) fn load_fragment(
    world: &Run1World,
    name: &str,
    profile: NormProfile,
) -> Result<(), String> {
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
pub(super) const fn profile_label(profile: NormProfile) -> &'static str {
    match profile {
        NormProfile::T1 => "t1",
        NormProfile::T2 => "t2",
    }
}

/// Resolve a fixture name and profile to its fragment identifier.
pub(super) fn fragment_id(name: &str, profile: NormProfile) -> FragmentId {
    FragmentId::from(format!("{name}_{}", profile_label(profile)))
}

/// Queue a candidate pair for two fragments under the same profile.
pub(super) fn queue_candidate(world: &Run1World, left: &str, right: &str, profile: NormProfile) {
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
pub(super) fn accept_pair(world: &Run1World, left: &str, right: &str, profile: NormProfile) {
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
pub(super) fn input_parts(
    world: &Run1World,
) -> (Vec<TokenFragment>, Vec<CandidatePair>, Vec<AcceptedPair>) {
    (
        world.fragments.borrow().clone(),
        world.candidates.borrow().clone(),
        world.accepted.borrow().clone(),
    )
}

/// Run the AST pass and emit Run 1, recording any error.
pub(super) fn refine_and_emit(world: &Run1World) {
    let (fragments, candidates, accepted) = input_parts(world);
    let config = world.config.borrow().clone();
    let token_config = world.token_config.borrow().clone();
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    match refine_candidate_pairs(&input, &config) {
        Ok(refined) => match emit_run1(&input, refined.pairs(), &config) {
            Ok(run) => {
                *world.refined.borrow_mut() = Some(refined.into_pairs());
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
pub(super) fn with_results(
    world: &Run1World,
    assert_fn: impl FnOnce(&[whitaker_sarif::SarifResult]),
) {
    let borrowed = world.run.borrow();
    let Some(run) = borrowed.as_ref() else {
        panic!("Run 1 must be emitted before checking results");
    };
    assert_fn(&run.results);
}

/// Extract the Whitaker properties of one result.
pub(super) fn properties_of(result: &whitaker_sarif::SarifResult) -> WhitakerProperties {
    let Some(properties) = result.properties.as_ref() else {
        panic!("every Run 1 result carries Whitaker properties");
    };
    match WhitakerProperties::try_from(properties) {
        Ok(extracted) => extracted,
        Err(error) => panic!("Whitaker properties must extract: {error}"),
    }
}
