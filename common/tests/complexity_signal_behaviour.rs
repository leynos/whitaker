//! Behaviour-driven coverage for per-line complexity signal building and smoothing.

use common::complexity_signal::{
    LineSegment, SignalBuildError, SmoothingError, rasterise_signal, smooth_moving_average,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, RefCell};

#[derive(Debug, Default)]
struct SignalWorld {
    function_start: Cell<Option<usize>>,
    function_end: Cell<Option<usize>>,
    segments: RefCell<Vec<LineSegment>>,
    raw_signal: RefCell<Option<Vec<f64>>>,
    smoothing_window: Cell<Option<usize>>,
    built_signal: RefCell<Option<Result<Vec<f64>, SignalBuildError>>>,
    smoothed_signal: RefCell<Option<Result<Vec<f64>, SmoothingError>>>,
}

impl SignalWorld {
    fn set_function_range(&self, start: usize, end: usize) {
        self.function_start.set(Some(start));
        self.function_end.set(Some(end));
    }

    fn push_segment(&self, segment: LineSegment) {
        self.segments.borrow_mut().push(segment);
    }

    fn set_raw_signal(&self, signal: Vec<f64>) {
        self.raw_signal.replace(Some(signal));
    }

    fn set_smoothing_window(&self, window: usize) {
        self.smoothing_window.set(Some(window));
    }

    fn build_signal(&self) {
        let start = self
            .function_start
            .get()
            .unwrap_or_else(|| panic!("function range start must be configured"));
        let end = self
            .function_end
            .get()
            .unwrap_or_else(|| panic!("function range end must be configured"));
        let segments = self.segments.borrow();
        self.built_signal
            .replace(Some(rasterise_signal(start..=end, segments.as_slice())));
    }

    fn smooth(&self) {
        let window = self
            .smoothing_window
            .get()
            .unwrap_or_else(|| panic!("smoothing window must be configured"));
        let raw_signal = self
            .raw_signal
            .borrow()
            .clone()
            .unwrap_or_else(|| panic!("raw signal must be configured"));
        self.smoothed_signal
            .replace(Some(smooth_moving_average(&raw_signal, window)));
    }

    fn built_signal(&self) -> Result<Vec<f64>, SignalBuildError> {
        self.built_signal
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("built signal must be recorded"))
    }

    fn smoothed_signal(&self) -> Result<Vec<f64>, SmoothingError> {
        self.smoothed_signal
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("smoothed signal must be recorded"))
    }
}

#[fixture]
fn world() -> SignalWorld {
    SignalWorld::default()
}

/// Parses a comma-separated list of floating-point values.
///
/// The feature text uses values like `0.0, 1.0, 2.0`. Whitespace is ignored and
/// empty segments are skipped.
fn parse_f64_list(values: &str) -> Vec<f64> {
    values
        .split(',')
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            chunk
                .parse::<f64>()
                .unwrap_or_else(|error| panic!("failed to parse `{chunk}` as f64: {error}"))
        })
        .collect()
}

/// Asserts that two floating-point vectors are equal within a tiny tolerance.
///
/// This helper is intended for deterministic test values that may experience
/// insignificant rounding differences.
fn assert_vec_approx_eq(actual: &[f64], expected: &[f64]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "expected vector length {expected_len}, got {actual_len}",
        expected_len = expected.len(),
        actual_len = actual.len()
    );

    for (idx, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        let delta = (actual - expected).abs();
        assert!(
            delta <= 1e-12,
            "expected element {idx} to be {expected}, got {actual} (delta {delta})",
        );
    }
}

#[given("a function spanning lines {start} to {end}")]
fn given_function_range(world: &SignalWorld, start: usize, end: usize) {
    world.set_function_range(start, end);
}

#[given("a segment from line {start} to {end} with value {value}")]
fn given_segment(world: &SignalWorld, start: usize, end: usize, value: f64) {
    let segment = LineSegment::new(start, end, value)
        .unwrap_or_else(|error| panic!("segment inputs should be valid: {error}"));
    world.push_segment(segment);
}

#[given("the raw signal is {values}")]
fn given_raw_signal(world: &SignalWorld, values: String) {
    world.set_raw_signal(parse_f64_list(&values));
}

#[given("the smoothing window is {window}")]
fn given_window(world: &SignalWorld, window: usize) {
    world.set_smoothing_window(window);
}

#[when("I build the per-line signal")]
fn when_build(world: &SignalWorld) {
    world.build_signal();
}

#[when("I smooth the signal")]
fn when_smooth(world: &SignalWorld) {
    world.smooth();
}

#[then("the built signal equals {expected}")]
fn then_built_signal(world: &SignalWorld, expected: String) {
    let actual = world
        .built_signal()
        .unwrap_or_else(|error| panic!("signal build should succeed: {error}"));
    let expected = parse_f64_list(&expected);
    assert_vec_approx_eq(&actual, &expected);
}

#[then("signal building fails")]
fn then_build_fails(world: &SignalWorld) {
    assert!(world.built_signal().is_err());
}

#[then("the smoothed signal equals {expected}")]
fn then_smoothed_signal(world: &SignalWorld, expected: String) {
    let actual = world
        .smoothed_signal()
        .unwrap_or_else(|error| panic!("smoothing should succeed: {error}"));
    let expected = parse_f64_list(&expected);
    assert_vec_approx_eq(&actual, &expected);
}

#[then("smoothing fails")]
fn then_smoothing_fails(world: &SignalWorld) {
    assert!(world.smoothed_signal().is_err());
}

#[scenario(path = "tests/features/complexity_signal.feature", index = 0)]
fn scenario_overlapping_segments(world: SignalWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/complexity_signal.feature", index = 1)]
fn scenario_out_of_range_segments(world: SignalWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/complexity_signal.feature", index = 2)]
fn scenario_smoothing_happy_path(world: SignalWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/complexity_signal.feature", index = 3)]
fn scenario_smoothing_even_window(world: SignalWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/complexity_signal.feature", index = 4)]
fn scenario_smoothing_zero_window(world: SignalWorld) {
    let _ = world;
}
