//! Feature step and scenario bindings for complexity-signal behaviour tests.

use super::{LineSegment, SignalWorld, assert_vec_approx_eq, parse_f64_list, world};
use rstest_bdd_macros::{given, scenario, then, when};

/// Set the function line range from the feature step.
#[given("a function spanning lines {start} to {end}")]
fn given_function_range(world: &SignalWorld, start: usize, end: usize) {
    world.set_function_range(start, end);
}

/// Add a validated segment from the feature step.
///
/// # Errors
///
/// Return the segment validation error when its line range or value is invalid.
#[given("a segment from line {start} to {end} with value {value}")]
fn given_segment(world: &SignalWorld, start: usize, end: usize, value: f64) -> Result<(), String> {
    let segment = LineSegment::new(start, end, value)
        .map_err(|error| format!("segment inputs should be valid: {error}"))?;
    world.push_segment(segment);
    Ok(())
}

/// Parse and store the raw signal provided by the feature step.
///
/// # Errors
///
/// Return a parsing error when a non-empty comma-separated value is not an `f64`.
#[given("the raw signal is {values}")]
fn given_raw_signal(world: &SignalWorld, values: String) -> Result<(), String> {
    world.set_raw_signal(parse_f64_list(&values)?);
    Ok(())
}

/// Store the smoothing window provided by the feature step.
#[given("the smoothing window is {window}")]
fn given_window(world: &SignalWorld, window: usize) {
    world.set_smoothing_window(window);
}

/// Build and record the per-line signal for the scenario.
#[when("I build the per-line signal")]
fn when_build(world: &SignalWorld) {
    world.build_signal();
}

/// Smooth and record the configured signal for the scenario.
#[when("I smooth the signal")]
fn when_smooth(world: &SignalWorld) {
    world.smooth();
}

/// Compare the built signal with the feature's expected values.
///
/// # Errors
///
/// Return an error if building failed or an expected value cannot be parsed.
#[then("the built signal equals {expected}")]
fn then_built_signal(world: &SignalWorld, expected: String) -> Result<(), String> {
    let actual = world
        .built_signal()
        .map_err(|error| format!("signal build should succeed: {error}"))?;
    let expected_values = parse_f64_list(&expected)?;
    assert_vec_approx_eq(&actual, &expected_values);
    Ok(())
}

/// Assert that signal construction returned an error.
#[then("signal building fails")]
fn then_build_fails(world: &SignalWorld) {
    assert!(
        world.built_signal().is_err(),
        "expected signal building to fail"
    );
}

/// Compare the smoothed signal with the feature's expected values.
///
/// # Errors
///
/// Return an error if smoothing failed or an expected value cannot be parsed.
#[then("the smoothed signal equals {expected}")]
fn then_smoothed_signal(world: &SignalWorld, expected: String) -> Result<(), String> {
    let actual = world
        .smoothed_signal()
        .map_err(|error| format!("smoothing should succeed: {error}"))?;
    let expected_values = parse_f64_list(&expected)?;
    assert_vec_approx_eq(&actual, &expected_values);
    Ok(())
}

/// Assert that smoothing returned an error.
#[then("smoothing fails")]
fn then_smoothing_fails(world: &SignalWorld) {
    assert!(
        world.smoothed_signal().is_err(),
        "expected smoothing to fail"
    );
}

/// Exercise overlapping signal segments through the feature scenario.
#[scenario(path = "tests/features/complexity_signal.feature", index = 0)]
fn scenario_overlapping_segments(world: SignalWorld) {
    let _ = world;
}

/// Exercise out-of-range signal segments through the feature scenario.
#[scenario(path = "tests/features/complexity_signal.feature", index = 1)]
fn scenario_out_of_range_segments(world: SignalWorld) {
    let _ = world;
}

/// Exercise successful moving-average smoothing through the feature scenario.
#[scenario(path = "tests/features/complexity_signal.feature", index = 2)]
fn scenario_smoothing_happy_path(world: SignalWorld) {
    let _ = world;
}

/// Exercise rejection of an even smoothing window through the feature scenario.
#[scenario(path = "tests/features/complexity_signal.feature", index = 3)]
fn scenario_smoothing_even_window(world: SignalWorld) {
    let _ = world;
}

/// Exercise rejection of a zero smoothing window through the feature scenario.
#[scenario(path = "tests/features/complexity_signal.feature", index = 4)]
fn scenario_smoothing_zero_window(world: SignalWorld) {
    let _ = world;
}
