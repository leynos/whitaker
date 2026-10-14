//! Behaviour-driven coverage for per-line complexity signal building and smoothing.

use std::cell::{Cell, RefCell};

use proptest::prelude::*;
use rstest::fixture;
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
    /// Set the inclusive line range covered by the function under test.
    fn set_function_range(&self, start: usize, end: usize) {
        self.function_start.set(Some(start));
        self.function_end.set(Some(end));
    }

    /// Add a line segment to the scenario state.
    fn push_segment(&self, segment: LineSegment) {
        self.segments.borrow_mut().push(segment);
    }

    /// Store the raw signal used by the smoothing step.
    fn set_raw_signal(&self, signal: Vec<f64>) {
        self.raw_signal.replace(Some(signal));
    }

    /// Store the moving-average window used by the smoothing step.
    fn set_smoothing_window(&self, window: usize) {
        self.smoothing_window.set(Some(window));
    }

    /// Rasterize the configured line range and segments into the saved result.
    ///
    /// Panics if the scenario has not configured both range endpoints.
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

    /// Smooth the configured raw signal and save the result.
    ///
    /// Panics if the scenario has not configured a window and raw signal.
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

    /// Clone the result recorded by the signal-building step.
    ///
    /// Panics if the signal-building step has not run.
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

    /// Clone the result recorded by the smoothing step.
    ///
    /// Panics if the smoothing step has not run.
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

/// Create fresh state for one complexity-signal scenario.
#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> SignalWorld {
    SignalWorld::default()
}

/// Parse a comma-separated list of floating-point values.
///
/// The feature text uses values like `0.0, 1.0, 2.0`. Whitespace is ignored and
/// empty segments are skipped.
///
/// # Errors
///
/// Return a description of the first segment that is not a valid `f64`.
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

/// Map a float to a monotonically ordered integer for ULP comparison.
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

/// Return the distance between two floats in units of least precision, or
/// `None` when either value is NaN.
const fn ulp_distance(left: f64, right: f64) -> Option<u64> {
    if left.is_nan() || right.is_nan() {
        return None;
    }
    Some(monotonic_bits(left).abs_diff(monotonic_bits(right)))
}

/// Reconstruct a float from the ordering representation used for ULP tests.
const fn f64_from_monotonic_bits(bits: u64) -> f64 {
    let raw_bits = if (bits & (1 << 63)) == 0 {
        !bits
    } else {
        bits & !(1 << 63)
    };
    f64::from_bits(raw_bits)
}

/// Assert that two floating-point vectors are equal within a tiny tolerance.
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

/// Rejects NaN inputs regardless of their payload bits.
#[test]
fn ulp_distance_rejects_nan_inputs() {
    let left_nan = f64::from_bits(0x7ff0_0000_0000_0001);
    let right_nan = f64::from_bits(0xfff8_0000_0000_0042);

    assert_eq!(ulp_distance(left_nan, f64::MAX), None);
    assert_eq!(ulp_distance(f64::MAX, left_nan), None);
    assert_eq!(ulp_distance(right_nan, f64::MAX), None);
}

/// Rejects a NaN actual value instead of treating it as a close match.
#[test]
#[should_panic(expected = "ULP distance None")]
fn approx_vector_rejects_nan_actual_value() {
    assert_vec_approx_eq(&[f64::NAN], &[0.0]);
}

/// Rejects a NaN expected value instead of treating it as a close match.
#[test]
#[should_panic(expected = "ULP distance None")]
fn approx_vector_rejects_nan_expected_value() {
    assert_vec_approx_eq(&[0.0], &[f64::NAN]);
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

    /// Preserves the undefined-distance contract for arbitrary NaN payloads.
    #[test]
    fn ulp_distance_is_undefined_when_either_value_is_nan(
        nan_payload in any::<u64>(),
        finite_bits in any::<u64>(),
    ) {
        let nan = f64::from_bits(0x7ff0_0000_0000_0001 | (nan_payload & 0x000f_ffff_ffff_ffff));
        let finite = f64::from_bits(finite_bits);
        prop_assume!(finite.is_finite());

        prop_assert_eq!(ulp_distance(nan, finite), None);
        prop_assert_eq!(ulp_distance(finite, nan), None);
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

#[path = "complexity_signal_behaviour/steps.rs"]
mod steps;
