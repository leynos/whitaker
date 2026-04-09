//! Behaviour-driven coverage for MinHash and LSH candidate generation.
//!
//! Keep this harness in sync with `tests/features/min_hash_lsh.feature`.

use std::{cell::RefCell, collections::BTreeMap};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{
    CandidatePair, Fingerprint, FragmentId, IndexError, LshConfig, LshIndex, MinHasher,
};

#[derive(Debug, Default)]
struct MinHashLshWorld {
    config: RefCell<Option<LshConfig>>,
    config_error: RefCell<Option<IndexError>>,
    fragments: RefCell<BTreeMap<String, Vec<Fingerprint>>>,
    candidates: RefCell<Vec<CandidatePair>>,
    candidate_error: RefCell<Option<IndexError>>,
}

#[fixture]
fn world() -> MinHashLshWorld {
    MinHashLshWorld::default()
}

fn with_candidates(world: &MinHashLshWorld, assert_fn: impl FnOnce(&[CandidatePair])) {
    let candidates = world.candidates.borrow();
    assert_fn(&candidates);
}

fn parse_hashes(hashes: &str) -> Result<Vec<Fingerprint>, String> {
    hashes
        .split_whitespace()
        .enumerate()
        .map(|(index, value)| {
            value
                .parse::<u64>()
                .map(|hash| Fingerprint::new(hash, index..index.saturating_add(1)))
                .map_err(|error| format!("invalid fingerprint hash `{value}`: {error}"))
        })
        .collect()
}

fn expected_error(name: &str) -> Result<IndexError, String> {
    if let Some(arguments) = name
        .strip_prefix("InvalidBandRowProduct(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let Some((bands, rows)) = arguments.split_once(',') else {
            return Err(format!(
                "invalid InvalidBandRowProduct arguments `{arguments}`"
            ));
        };
        let bands = bands
            .parse::<usize>()
            .map_err(|error| format!("invalid bands value `{bands}`: {error}"))?;
        let rows = rows
            .parse::<usize>()
            .map_err(|error| format!("invalid rows value `{rows}`: {error}"))?;
        return Ok(IndexError::invalid_band_row_product(bands, rows));
    }

    match name {
        "ZeroBands" => Ok(IndexError::ZeroBands),
        "ZeroRows" => Ok(IndexError::ZeroRows),
        "EmptyFingerprintSet" => Ok(IndexError::EmptyFingerprintSet),
        other => Err(format!("unknown error name `{other}`")),
    }
}

#[given("LSH bands {bands} and rows {rows}")]
fn given_lsh_config(world: &MinHashLshWorld, bands: usize, rows: usize) {
    match LshConfig::new(bands, rows) {
        Ok(config) => {
            *world.config.borrow_mut() = Some(config);
            *world.config_error.borrow_mut() = None;
        }
        Err(error) => {
            *world.config.borrow_mut() = None;
            *world.config_error.borrow_mut() = Some(error);
        }
    }
}

#[given("fragment {id} retains hashes {hashes}")]
fn given_fragment_hashes(
    world: &MinHashLshWorld,
    id: String,
    hashes: String,
) -> Result<(), String> {
    let parsed = parse_hashes(&hashes)?;
    world.fragments.borrow_mut().insert(id, parsed);
    Ok(())
}

#[given("fragment {id} retains no hashes")]
fn given_fragment_without_hashes(world: &MinHashLshWorld, id: String) {
    world.fragments.borrow_mut().insert(id, Vec::new());
}

#[when("candidate pairs are generated")]
fn when_candidate_pairs_are_generated(world: &MinHashLshWorld) {
    let Some(config) = *world.config.borrow() else {
        *world.candidate_error.borrow_mut() = world.config_error.borrow().clone();
        world.candidates.borrow_mut().clear();
        return;
    };

    let hasher = MinHasher::new();
    let mut index = LshIndex::new(config);
    for (id, fingerprints) in world.fragments.borrow().iter() {
        match hasher.sketch(fingerprints) {
            Ok(signature) => index.insert(&FragmentId::from(id.clone()), &signature),
            Err(error) => {
                *world.candidate_error.borrow_mut() = Some(error);
                world.candidates.borrow_mut().clear();
                return;
            }
        }
    }

    *world.candidates.borrow_mut() = index.candidate_pairs();
    *world.candidate_error.borrow_mut() = None;
}

#[then("candidate pair count is {count}")]
fn then_candidate_pair_count_is(world: &MinHashLshWorld, count: usize) {
    with_candidates(world, |candidates| {
        assert_eq!(candidates.len(), count);
    });
}

#[then("the only candidate pair is {left} and {right}")]
fn then_only_candidate_pair_is(world: &MinHashLshWorld, left: String, right: String) {
    with_candidates(world, |candidates| {
        let [candidate] = candidates else {
            panic!(
                "exactly one candidate pair must be present, found {}",
                candidates.len()
            );
        };
        let expected = CandidatePair::new(FragmentId::from(left), FragmentId::from(right))
            .expect("distinct fragment IDs should form a canonical pair");
        assert_eq!(candidate, &expected);
    });
}

#[then("no candidate pairs are returned")]
fn then_no_candidate_pairs_are_returned(world: &MinHashLshWorld) {
    with_candidates(world, |candidates| assert!(candidates.is_empty()));
}

#[then("the candidate generation error is {name}")]
fn then_candidate_generation_error_is(world: &MinHashLshWorld, name: String) -> Result<(), String> {
    let expected = expected_error(&name)?;
    match world.candidate_error.borrow().clone() {
        Some(actual) => {
            assert_eq!(actual, expected);
            Ok(())
        }
        None => Err("candidate generation error must be present".to_owned()),
    }
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 0)]
fn scenario_identical_fragments(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 1)]
fn scenario_distinct_fragments(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 2)]
fn scenario_multiple_collisions_one_pair(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 3)]
fn scenario_invalid_lsh_settings(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 4)]
fn scenario_empty_fingerprints(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 5)]
fn scenario_zero_rows(world: MinHashLshWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/min_hash_lsh.feature", index = 6)]
fn scenario_invalid_non_zero_product(world: MinHashLshWorld) {
    let _ = world;
}
