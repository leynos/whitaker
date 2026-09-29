//! Behaviour-driven coverage for token-pass Run 0 SARIF emission.
//!
//! Keep this harness in sync with `tests/features/run0_sarif.feature`.

use std::{cell::RefCell, collections::BTreeMap};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{
    CandidatePair, FragmentId, Run0Error, TokenFragment, TokenPassConfig, accept_candidate_pairs,
    emit_run0,
};
use whitaker_sarif::{Run, SarifResult, WhitakerProperties};
use whitaker_test_macros::allow_fixture_expansion_lints;

#[derive(Debug)]
struct Run0World {
    fragments: RefCell<BTreeMap<String, TokenFragment>>,
    candidates: RefCell<Vec<CandidatePair>>,
    run: RefCell<Option<Run>>,
    error: RefCell<Option<Run0Error>>,
    config: TokenPassConfig,
}

impl Default for Run0World {
    fn default() -> Self {
        Self {
            fragments: RefCell::new(BTreeMap::new()),
            candidates: RefCell::new(Vec::new()),
            run: RefCell::new(None),
            error: RefCell::new(None),
            config: TokenPassConfig::new("whitaker_clones_cli@token", env!("CARGO_PKG_VERSION")),
        }
    }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> Run0World {
    Run0World::default()
}

fn with_results(world: &Run0World, assert_fn: impl FnOnce(&[SarifResult])) {
    let borrowed_run = world.run.borrow();
    let Some(emitted_run) = borrowed_run.as_ref() else {
        panic!("run must be emitted before checking results");
    };
    assert_fn(&emitted_run.results);
}

fn with_whitaker_properties(world: &Run0World, assert_fn: impl FnOnce(&WhitakerProperties)) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist before checking Whitaker properties");
        };
        let Some(properties) = result.properties.as_ref() else {
            panic!("Whitaker properties must be present");
        };
        let extracted = match WhitakerProperties::try_from(properties) {
            Ok(extracted) => extracted,
            Err(error) => panic!("unexpected property extraction error: {error}"),
        };
        assert_fn(&extracted);
    });
}

fn fragment_fixture(name: &str) -> Result<TokenFragment, String> {
    match name {
        "alpha_t1" => Ok(TokenFragment::new(
            FragmentId::from("alpha_t1"),
            whitaker_clones_core::NormProfile::T1,
            "src/a.rs",
            "fn a() {}\n",
        )
        .with_retained_fingerprints(vec![whitaker_clones_core::Fingerprint::new(11, 0..8)])),
        "beta_t1" => Ok(TokenFragment::new(
            FragmentId::from("beta_t1"),
            whitaker_clones_core::NormProfile::T1,
            "src/b.rs",
            "fn b() {}\n",
        )
        .with_retained_fingerprints(vec![whitaker_clones_core::Fingerprint::new(11, 0..8)])),
        "alpha_t2" => Ok(TokenFragment::new(
            FragmentId::from("alpha_t2"),
            whitaker_clones_core::NormProfile::T2,
            "src/a.rs",
            "fn a(x: i32) {}\n",
        )
        .with_retained_fingerprints(vec![
            whitaker_clones_core::Fingerprint::new(1, 0..15),
            whitaker_clones_core::Fingerprint::new(2, 0..15),
        ])),
        "beta_t2" => Ok(TokenFragment::new(
            FragmentId::from("beta_t2"),
            whitaker_clones_core::NormProfile::T2,
            "src/b.rs",
            "fn b(y: i32) {}\n",
        )
        .with_retained_fingerprints(vec![
            whitaker_clones_core::Fingerprint::new(1, 0..15),
            whitaker_clones_core::Fingerprint::new(2, 0..15),
        ])),
        "beta_t2_partial" => Ok(TokenFragment::new(
            FragmentId::from("beta_t2_partial"),
            whitaker_clones_core::NormProfile::T2,
            "src/b.rs",
            "fn b(y: i32) {}\n",
        )
        .with_retained_fingerprints(vec![
            whitaker_clones_core::Fingerprint::new(1, 0..15),
            whitaker_clones_core::Fingerprint::new(3, 0..15),
        ])),
        "alpha_empty" => Ok(TokenFragment::new(
            FragmentId::from("alpha_empty"),
            whitaker_clones_core::NormProfile::T1,
            "src/a.rs",
            "fn a() {}\n",
        )),
        "alpha_multiline" => Ok(TokenFragment::new(
            FragmentId::from("alpha_multiline"),
            whitaker_clones_core::NormProfile::T1,
            "src/a.rs",
            "fn alpha() {\n    value();\n}\n",
        )
        .with_retained_fingerprints(vec![whitaker_clones_core::Fingerprint::new(21, 13..27)])),
        "beta_multiline" => Ok(TokenFragment::new(
            FragmentId::from("beta_multiline"),
            whitaker_clones_core::NormProfile::T1,
            "src/b.rs",
            "fn beta() {\n    value();\n}\n",
        )
        .with_retained_fingerprints(vec![whitaker_clones_core::Fingerprint::new(21, 12..26)])),
        other => Err(format!("unknown fragment fixture `{other}`")),
    }
}

#[given("token fragment {name} is loaded")]
fn given_token_fragment(world: &Run0World, name: String) -> Result<(), String> {
    let fragment = fragment_fixture(&name)?;
    world.fragments.borrow_mut().insert(name, fragment);
    Ok(())
}

#[given("candidate pair {left} and {right} is queued")]
fn given_candidate_pair(world: &Run0World, left: String, right: String) -> Result<(), String> {
    let pair = CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
        .ok_or_else(|| "candidate pairs must use distinct fragment IDs".to_owned())?;
    world.candidates.borrow_mut().push(pair);
    Ok(())
}

#[when("Run 0 is emitted")]
fn when_run_zero_is_emitted(world: &Run0World) {
    let fragments = world
        .fragments
        .borrow()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let accepted =
        match accept_candidate_pairs(&fragments, &world.candidates.borrow(), &world.config) {
            Ok(accepted) => accepted,
            Err(error) => {
                *world.error.borrow_mut() = Some(error);
                *world.run.borrow_mut() = None;
                return;
            }
        };

    match emit_run0(&fragments, &accepted, &world.config) {
        Ok(run) => {
            *world.run.borrow_mut() = Some(run);
            *world.error.borrow_mut() = None;
        }
        Err(error) => {
            *world.run.borrow_mut() = None;
            *world.error.borrow_mut() = Some(error);
        }
    }
}

#[then("exactly {count} result is emitted")]
fn then_exactly_one_result_is_emitted(world: &Run0World, count: usize) {
    with_results(world, |results| {
        assert_eq!(
            results.len(),
            count,
            "emitted result count must match the scenario expectation"
        );
    });
}

#[then("the emitted rule is {rule_id}")]
fn then_emitted_rule_is(world: &Run0World, rule_id: String) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist before checking the rule");
        };
        assert_eq!(
            result.rule_id, rule_id,
            "emitted rule identifier must match the scenario expectation"
        );
    });
}

#[then("the result has {primary_count} primary location and {related_count} related location")]
fn then_result_has_locations(world: &Run0World, primary_count: usize, related_count: usize) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist before checking locations");
        };
        assert_eq!(
            result.locations.len(),
            primary_count,
            "primary location count must match the scenario expectation"
        );
        assert_eq!(
            result.related_locations.len(),
            related_count,
            "related location count must match the scenario expectation"
        );
    });
}

#[then("the Whitaker profile is {profile}")]
fn then_whitaker_profile_is(world: &Run0World, profile: String) {
    with_whitaker_properties(world, |props| {
        assert_eq!(
            props.profile, profile,
            "Whitaker profile must match the scenario expectation"
        );
    });
}

#[then("the Whitaker k is {k}")]
fn then_whitaker_k_is(world: &Run0World, k: usize) {
    with_whitaker_properties(world, |props| {
        assert_eq!(props.k, k, "Whitaker k must match the scenario expectation");
    });
}

#[then("the Whitaker window is {window}")]
fn then_whitaker_window_is(world: &Run0World, window: usize) {
    with_whitaker_properties(world, |props| {
        assert_eq!(
            props.window, window,
            "Whitaker window must match the scenario expectation"
        );
    });
}

#[then("no results are emitted")]
fn then_no_results_are_emitted(world: &Run0World) {
    with_results(world, |results| {
        assert!(
            results.is_empty(),
            "no results must be emitted for this scenario"
        );
    });
}

#[then("the emission error is {message}")]
fn then_emission_error_is(world: &Run0World, message: String) -> Result<(), String> {
    world.error.borrow().as_ref().map_or_else(
        || Err("an emission error must be present".to_owned()),
        |error| {
            let actual = error.to_string();
            if actual == message {
                Ok(())
            } else {
                Err(format!(
                    "expected emission error `{message}`, but the run reported `{actual}`"
                ))
            }
        },
    )
}

#[then("the primary region is {region}")]
fn then_primary_region_is(world: &Run0World, region: String) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist before checking the primary region");
        };
        let Some(location) = result.locations.first() else {
            panic!("a primary location must be present");
        };
        let Some(region_value) = location.physical_location.region.as_ref() else {
            panic!("a primary region must be present");
        };
        let actual = format!(
            "{}:{}-{}:{}",
            region_value.start_line,
            region_value.start_column.unwrap_or(1),
            region_value.end_line.unwrap_or(region_value.start_line),
            region_value
                .end_column
                .unwrap_or_else(|| region_value.start_column.unwrap_or(1))
        );
        assert_eq!(
            actual, region,
            "primary region must match the scenario expectation"
        );
    });
}

#[then("the primary file is {file_uri}")]
fn then_primary_file_is(world: &Run0World, file_uri: String) {
    with_results(world, |results| {
        let [result] = results else {
            panic!("exactly one result must exist before checking the primary file");
        };
        let Some(location) = result.locations.first() else {
            panic!("a primary location must be present");
        };
        assert_eq!(
            location.physical_location.artefact_location.uri, file_uri,
            "primary file URI must match the scenario expectation"
        );
    });
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 0)]
fn scenario_type1_pair(world: Run0World) {
    let _ = world;
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 1)]
fn scenario_type2_pair(world: Run0World) {
    let _ = world;
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 2)]
fn scenario_below_threshold(world: Run0World) {
    let _ = world;
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 3)]
fn scenario_empty_fingerprints(world: Run0World) {
    let _ = world;
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 4)]
fn scenario_multiline_region(world: Run0World) {
    let _ = world;
}

#[scenario(path = "tests/features/run0_sarif.feature", index = 5)]
fn scenario_reversed_pair(world: Run0World) {
    let _ = world;
}
