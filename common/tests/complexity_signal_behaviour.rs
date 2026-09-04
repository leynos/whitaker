//! Behaviour-driven coverage for per-line complexity signal building and smoothing.

use std::cell::{Cell, RefCell};

use proptest::prelude::*;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker_common::complexity_signal::{
    LineSegment, SignalBuildError, SmoothingError, rasterize_signal, smooth_moving_average,
};
use whitaker_test_macros::allow_fixture_expansion_lints;

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

    #[expect(
        clippy::expect_used,
        reason = "fixture configuration is required for this behaviour test"
    )]
    fn build_signal(&self) {
        let start = self
            .function_start
            .get()
            .expect("function range start must be configured");
        let end = self
            .function_end
            .get()
            .expect("function range end must be configured");
        let segments = self.segments.borrow();
        self.built_signal
            .replace(Some(rasterize_signal(start..=end, segments.as_slice())));
    }

    #[expect(
        clippy::expect_used,
        reason = "fixture configuration is required for this behaviour test"
    )]
    fn smooth(&self) {
        let window = self
            .smoothing_window
            .get()
            .expect("smoothing window must be configured");
        let raw_signal = self
            .raw_signal
            .borrow()
            .clone()
            .expect("raw signal must be configured");
        self.smoothed_signal
            .replace(Some(smooth_moving_average(&raw_signal, window)));
    }

    #[expect(
        clippy::expect_used,
        reason = "fixture configuration is required for this behaviour test"
    )]
    fn built_signal(&self) -> Result<Vec<f64>, SignalBuildError> {
        self.built_signal
            .borrow()
            .as_ref()
            .cloned()
            .expect("built signal must be recorded")
    }

    #[expect(
        clippy::expect_used,
        reason = "fixture configuration is required for this behaviour test"
    )]
    fn smoothed_signal(&self) -> Result<Vec<f64>, SmoothingError> {
        self.smoothed_signal
            .borrow()
            .as_ref()
            .cloned()
            .expect("smoothed signal must be recorded")
    }
}

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> SignalWorld {
    SignalWorld::default()
}

/// Parses a comma-separated list of floating-point values.
///
/// The feature text uses values like `0.0, 1.0, 2.0`. Whitespace is ignored and
/// empty segments are skipped.
///
/// # Errors
///
/// Returns a description of the first segment that is not a valid `f64`.
fn parse_f64_list(values: &str) -> Result<Vec<f64>, String> {
    values
        .split(',')
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            chunk
                .parse::<f64>()
                .map_err(|error| format!("failed to parse `{chunk}` as f64: {error}"))
        })
        .collect()
}

/// Maximum permitted distance between two floats in units of least precision.
const MAX_ULP_DISTANCE: u64 = 4;

/// Maps a float to a monotonically ordered integer for ULP comparison.
///
/// Negative values have their bits inverted and non-negative values have the
/// sign bit set, so the resulting integers order the same way as the floats.
const fn monotonic_bits(value: f64) -> u64 {
    let bits = value.to_bits();
    if (bits & (1 << 63)) != 0 {
        !bits
    } else {
        bits | (1 << 63)
    }
}

/// Returns the distance between two floats in units of least precision, or
/// `None` when either value is NaN.
const fn ulp_distance(left: f64, right: f64) -> Option<u64> {
    if left.is_nan() || right.is_nan() {
        return None;
    }
    Some(monotonic_bits(left).abs_diff(monotonic_bits(right)))
}

/// Reconstructs a float from the ordering representation used for ULP tests.
const fn f64_from_monotonic_bits(bits: u64) -> f64 {
    let raw_bits = if (bits & (1 << 63)) == 0 {
        !bits
    } else {
        bits & !(1 << 63)
    };
    f64::from_bits(raw_bits)
}

/// Asserts that two floating-point vectors are equal within a tiny tolerance.
///
/// This helper is intended for deterministic test values that may experience
/// insignificant rounding differences. Comparison uses units of least
/// precision (ULPs), which avoids floating-point arithmetic in the test.
fn assert_vec_approx_eq(actual: &[f64], expected: &[f64]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "expected vector length {expected_len}, got {actual_len}",
        expected_len = expected.len(),
        actual_len = actual.len()
    );

    for (idx, (actual_value, expected_value)) in actual.iter().zip(expected.iter()).enumerate() {
        if actual_value.is_infinite() || expected_value.is_infinite() {
            assert_eq!(
                actual_value, expected_value,
                "expected element {idx} to be {expected_value}, got {actual_value}"
            );
            continue;
        }
        let distance = ulp_distance(*actual_value, *expected_value);
        assert!(
            distance.is_some_and(|ulps| ulps <= MAX_ULP_DISTANCE),
            "expected element {idx} to be {expected_value}, got {actual_value} (ULP distance \
             {distance:?})",
        );
    }
}

#[test]
#[should_panic(expected = "expected element 0 to be inf, got")]
fn approx_vector_rejects_finite_value_for_infinite_expectation() {
    assert_vec_approx_eq(&[f64::MAX], &[f64::INFINITY]);
}

#[test]
#[should_panic(expected = "expected element 0 to be")]
fn approx_vector_rejects_infinite_value_for_finite_expectation() {
    assert_vec_approx_eq(&[f64::INFINITY], &[f64::MAX]);
}

proptest! {
    #[test]
    fn monotonic_bits_preserve_finite_numeric_order(
        left_bits in any::<u64>(),
        right_bits in any::<u64>(),
    ) {
        let left = f64::from_bits(left_bits);
        let right = f64::from_bits(right_bits);
        prop_assume!(left.is_finite() && right.is_finite() && left != right);

        prop_assert_eq!(left < right, monotonic_bits(left) < monotonic_bits(right));
        prop_assert_eq!(left > right, monotonic_bits(left) > monotonic_bits(right));
    }

    #[test]
    fn ulp_distance_is_identity_and_symmetric_for_finite_values(
        left_bits in any::<u64>(),
        right_bits in any::<u64>(),
    ) {
        let left = f64::from_bits(left_bits);
        let right = f64::from_bits(right_bits);
        prop_assume!(left.is_finite() && right.is_finite());

        prop_assert_eq!(ulp_distance(left, left), Some(0));
        prop_assert_eq!(ulp_distance(left, right), ulp_distance(right, left));
    }

    #[test]
    fn adjacent_finite_representations_are_one_ulp_apart(bits in 0_u64..u64::MAX) {
        let left = f64::from_bits(bits);
        let right = f64::from_bits(bits + 1);
        prop_assume!(left.is_finite() && right.is_finite());

        prop_assert_eq!(ulp_distance(left, right), Some(1));
    }

    #[test]
    fn vector_approximation_accepts_finite_values_within_the_ulp_tolerance(
        ordered_bits in any::<u64>(),
        distance in 0_u64..=MAX_ULP_DISTANCE,
    ) {
        prop_assume!(ordered_bits <= u64::MAX - distance);
        let expected_bits = ordered_bits + distance;
        let actual = f64_from_monotonic_bits(ordered_bits);
        let expected = f64_from_monotonic_bits(expected_bits);
        prop_assume!(actual.is_finite() && expected.is_finite());

        assert_vec_approx_eq(&[actual], &[expected]);
    }
}

#[given("a function spanning lines {start} to {end}")]
fn given_function_range(world: &SignalWorld, start: usize, end: usize) {
    world.set_function_range(start, end);
}

#[given("a segment from line {start} to {end} with value {value}")]
fn given_segment(world: &SignalWorld, start: usize, end: usize, value: f64) -> Result<(), String> {
    let segment = LineSegment::new(start, end, value)
        .map_err(|error| format!("segment inputs should be valid: {error}"))?;
    world.push_segment(segment);
    Ok(())
}

#[given("the raw signal is {values}")]
fn given_raw_signal(world: &SignalWorld, values: String) -> Result<(), String> {
    world.set_raw_signal(parse_f64_list(&values)?);
    Ok(())
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
fn then_built_signal(world: &SignalWorld, expected: String) -> Result<(), String> {
    let actual = world
        .built_signal()
        .map_err(|error| format!("signal build should succeed: {error}"))?;
    let expected_values = parse_f64_list(&expected)?;
    assert_vec_approx_eq(&actual, &expected_values);
    Ok(())
}

#[then("signal building fails")]
fn then_build_fails(world: &SignalWorld) {
    assert!(
        world.built_signal().is_err(),
        "expected signal building to fail"
    );
}

#[then("the smoothed signal equals {expected}")]
fn then_smoothed_signal(world: &SignalWorld, expected: String) -> Result<(), String> {
    let actual = world
        .smoothed_signal()
        .map_err(|error| format!("smoothing should succeed: {error}"))?;
    let expected_values = parse_f64_list(&expected)?;
    assert_vec_approx_eq(&actual, &expected_values);
    Ok(())
}

#[then("smoothing fails")]
fn then_smoothing_fails(world: &SignalWorld) {
    assert!(
        world.smoothed_signal().is_err(),
        "expected smoothing to fail"
    );
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
