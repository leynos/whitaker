//! Behaviour-driven coverage for the token pass.
//!
//! Keep this harness in sync with `tests/features/token_pass.feature`. Touching
//! the Rust file forces a recompilation when only the feature text changes.

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_clones_core::{
    Fingerprint, NormProfile, ShingleSize, TokenPassError, WinnowWindow, hash_shingles, normalize,
    winnow,
};

#[derive(Debug, Default)]
struct TokenPassWorld {
    source: RefCell<String>,
    profile: RefCell<NormProfile>,
    normalized_labels: RefCell<Vec<String>>,
    normalization_error: RefCell<Option<TokenPassError>>,
    fingerprints: RefCell<Vec<Fingerprint>>,
    retained: RefCell<Vec<Fingerprint>>,
    k: RefCell<Option<ShingleSize>>,
    window: RefCell<Option<WinnowWindow>>,
    size_error: RefCell<Option<TokenPassError>>,
}

#[fixture]
fn world() -> TokenPassWorld {
    TokenPassWorld::default()
}

fn with_fingerprints(world: &TokenPassWorld, assert_fn: impl FnOnce(&[Fingerprint])) {
    let fingerprints = world.fingerprints.borrow();
    assert_fn(&fingerprints);
}

fn with_retained(world: &TokenPassWorld, assert_fn: impl FnOnce(&[Fingerprint])) {
    let retained = world.retained.borrow();
    assert_fn(&retained);
}

#[given("the source snippet {name}")]
fn given_source(world: &TokenPassWorld, name: String) {
    let source = match name.as_str() {
        "commented_function" => "fn demo(x: i32) { /* note */ x + 1 }",
        "renamed_function_a" => "fn alpha(total: i32) { total + 1 }",
        "renamed_function_b" => "fn beta(count: i32) { count + 9 }",
        "short_function" => "fn tiny() {}",
        "unterminated_string" => "let value = \"open",
        other => panic!("unknown source snippet: {other}"),
    };

    *world.source.borrow_mut() = source.to_owned();
}

#[given("the profile is {profile}")]
fn given_profile(world: &TokenPassWorld, profile: String) {
    *world.profile.borrow_mut() = match profile.as_str() {
        "T1" => NormProfile::T1,
        "T2" => NormProfile::T2,
        other => panic!("unknown profile: {other}"),
    };
}

#[given("shingle size {size}")]
fn given_shingle_size(world: &TokenPassWorld, size: usize) {
    match ShingleSize::try_from(size) {
        Ok(size) => *world.k.borrow_mut() = Some(size),
        Err(error) => *world.size_error.borrow_mut() = Some(error),
    }
}

#[given("winnow window {window}")]
fn given_window(world: &TokenPassWorld, window: usize) {
    match WinnowWindow::try_from(window) {
        Ok(window) => *world.window.borrow_mut() = Some(window),
        Err(error) => *world.size_error.borrow_mut() = Some(error),
    }
}

#[given("the known fingerprint sequence {name}")]
fn given_known_sequence(world: &TokenPassWorld, name: String) {
    let sequence = match name.as_str() {
        "rightmost_minimum" => vec![
            Fingerprint::new(9, 0..1),
            Fingerprint::new(4, 1..2),
            Fingerprint::new(4, 2..3),
            Fingerprint::new(7, 3..4),
        ],
        other => panic!("unknown fingerprint sequence: {other}"),
    };

    *world.fingerprints.borrow_mut() = sequence;
}

#[when("the source is normalized")]
fn when_source_is_normalized(world: &TokenPassWorld) {
    match normalize(&world.source.borrow(), *world.profile.borrow()) {
        Ok(tokens) => {
            *world.normalized_labels.borrow_mut() = tokens
                .into_iter()
                .map(|token| token.kind.to_string())
                .collect();
            *world.normalization_error.borrow_mut() = None;
        }
        Err(error) => {
            world.normalized_labels.borrow_mut().clear();
            *world.normalization_error.borrow_mut() = Some(error);
        }
    }
}

#[when("fingerprints are generated")]
fn when_fingerprints_are_generated(world: &TokenPassWorld) {
    let Some(k) = *world.k.borrow() else {
        panic!("shingle size must be set before generating fingerprints");
    };
    match normalize(&world.source.borrow(), *world.profile.borrow()) {
        Ok(tokens) => {
            *world.normalized_labels.borrow_mut() =
                tokens.iter().map(|token| token.kind.to_string()).collect();
            *world.fingerprints.borrow_mut() = hash_shingles(&tokens, k);
            *world.normalization_error.borrow_mut() = None;
        }
        Err(error) => *world.normalization_error.borrow_mut() = Some(error),
    }
}

#[when("fingerprints are winnowed")]
fn when_fingerprints_are_winnowed(world: &TokenPassWorld) {
    let Some(window) = *world.window.borrow() else {
        panic!("winnow window must be set before winnowing fingerprints");
    };
    *world.retained.borrow_mut() = winnow(&world.fingerprints.borrow(), window);
}

#[then("the normalized labels are {labels}")]
fn then_normalized_labels_are(world: &TokenPassWorld, labels: String) {
    let expected = labels
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(*world.normalized_labels.borrow(), expected);
}

#[then("the fingerprint count is {count}")]
fn then_fingerprint_count_is(world: &TokenPassWorld, count: usize) {
    with_fingerprints(world, |fingerprints| {
        assert_eq!(fingerprints.len(), count);
    });
}

#[then("the first fingerprint spans {start} to {end}")]
fn then_first_fingerprint_spans(world: &TokenPassWorld, start: usize, end: usize) {
    with_fingerprints(world, |fingerprints| {
        let Some(first) = fingerprints.first() else {
            panic!("fingerprints must exist before checking the first span");
        };
        assert_eq!(first.range, start..end);
    });
}

#[then("the retained hashes are {hashes}")]
fn then_retained_hashes_are(world: &TokenPassWorld, hashes: String) {
    let expected = hashes
        .split_whitespace()
        .map(|value| value.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .expect("expected hash list should be valid");

    with_retained(world, |retained| {
        assert_eq!(
            retained
                .iter()
                .map(|fingerprint| fingerprint.hash)
                .collect::<Vec<_>>(),
            expected
        );
    });
}

#[then("the error is {message}")]
fn then_error_is(world: &TokenPassWorld, message: String) {
    if let Some(error) = world.normalization_error.borrow().as_ref() {
        assert_eq!(error.to_string(), message);
        return;
    }

    if let Some(error) = world.size_error.borrow().as_ref() {
        assert_eq!(error.to_string(), message);
        return;
    }

    panic!("an error must be present before checking it");
}

#[scenario(path = "tests/features/token_pass.feature", index = 0)]
fn scenario_t1_trivia_removal(world: TokenPassWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/token_pass.feature", index = 1)]
fn scenario_t2_renamed_functions_match(world: TokenPassWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/token_pass.feature", index = 2)]
fn scenario_exact_k_fingerprint(world: TokenPassWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/token_pass.feature", index = 3)]
fn scenario_winnowing_rightmost_minimum(world: TokenPassWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/token_pass.feature", index = 4)]
fn scenario_invalid_size(world: TokenPassWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/token_pass.feature", index = 5)]
fn scenario_unterminated_literal(world: TokenPassWorld) {
    let _ = world;
}
