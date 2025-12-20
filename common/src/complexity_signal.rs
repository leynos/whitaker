//! Per-line complexity signal helpers used by higher-level lints.
//!
//! The Bumpy Road detector described in `docs/whitaker-dylint-suite-design.md`
//! models complexity as a per-line signal, then applies smoothing to highlight
//! sustained "bumps" rather than short spikes. This module provides the
//! low-level building blocks: rasterising weighted line segments into a
//! per-line vector and applying a centred moving-average smoothing window.

use std::ops::RangeInclusive;

use thiserror::Error;

/// Describes a constant contribution spanning an inclusive set of one-based
/// line numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSegment {
    start_line: usize,
    end_line: usize,
    value: f64,
}

impl LineSegment {
    /// Creates a new line segment.
    ///
    /// Line numbers are one-based and ranges are inclusive.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::complexity_signal::LineSegment;
    ///
    /// let segment = LineSegment::new(3, 5, 1.25).expect("segment should be valid");
    /// assert_eq!(segment.start_line(), 3);
    /// assert_eq!(segment.end_line(), 5);
    /// ```
    #[must_use = "Inspect the segment creation result to handle invalid ranges"]
    pub fn new(start_line: usize, end_line: usize, value: f64) -> Result<Self, SegmentError> {
        if start_line == 0 || end_line == 0 {
            return Err(SegmentError::LineNumberMustBeOneBased {
                start_line,
                end_line,
            });
        }

        if start_line > end_line {
            return Err(SegmentError::StartAfterEnd {
                start_line,
                end_line,
            });
        }

        Ok(Self {
            start_line,
            end_line,
            value,
        })
    }

    /// Returns the first line covered by the segment (inclusive).
    #[must_use]
    pub const fn start_line(self) -> usize {
        self.start_line
    }

    /// Returns the last line covered by the segment (inclusive).
    #[must_use]
    pub const fn end_line(self) -> usize {
        self.end_line
    }

    /// Returns the per-line contribution.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }
}

/// Errors emitted when constructing a [`LineSegment`].
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SegmentError {
    /// Line numbers must be one-based (line 0 is invalid).
    #[error("line numbers must be one-based (got start_line={start_line}, end_line={end_line})")]
    LineNumberMustBeOneBased { start_line: usize, end_line: usize },

    /// The segment start must not occur after its end.
    #[error(
        "segment start must not occur after end (start_line={start_line}, end_line={end_line})"
    )]
    StartAfterEnd { start_line: usize, end_line: usize },
}

/// Errors emitted when building a per-line signal.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SignalBuildError {
    /// The function span must be expressed with one-based line numbers.
    #[error(
        "function line range must be one-based (got start_line={start_line}, end_line={end_line})"
    )]
    FunctionLineRangeMustBeOneBased { start_line: usize, end_line: usize },

    /// The function start must not occur after its end.
    #[error(
        "function start must not occur after end (start_line={start_line}, end_line={end_line})"
    )]
    FunctionStartAfterEnd { start_line: usize, end_line: usize },

    /// A segment does not intersect the function's line range.
    #[error(
        "segment lies outside function range (segment={segment_start}..={segment_end}, function={function_start}..={function_end})"
    )]
    SegmentOutsideFunctionRange {
        segment_start: usize,
        segment_end: usize,
        function_start: usize,
        function_end: usize,
    },
}

fn validate_function_range(
    function_start: usize,
    function_end: usize,
) -> Result<(), SignalBuildError> {
    if function_start == 0 || function_end == 0 {
        return Err(SignalBuildError::FunctionLineRangeMustBeOneBased {
            start_line: function_start,
            end_line: function_end,
        });
    }

    if function_start > function_end {
        return Err(SignalBuildError::FunctionStartAfterEnd {
            start_line: function_start,
            end_line: function_end,
        });
    }

    Ok(())
}

fn validate_segment_in_range(
    segment: &LineSegment,
    function_start: usize,
    function_end: usize,
) -> Result<(), SignalBuildError> {
    if segment.end_line() < function_start || segment.start_line() > function_end {
        return Err(SignalBuildError::SegmentOutsideFunctionRange {
            segment_start: segment.start_line(),
            segment_end: segment.end_line(),
            function_start,
            function_end,
        });
    }

    Ok(())
}

fn apply_segment_to_diff(segment: &LineSegment, diff: &mut [f64], function_start: usize) {
    let segment_start = segment.start_line().saturating_sub(function_start);
    let segment_end = segment.end_line().saturating_sub(function_start);

    if let Some(cell) = diff.get_mut(segment_start) {
        *cell += segment.value();
    }

    if let Some(cell) = diff.get_mut(segment_end + 1) {
        *cell -= segment.value();
    }
}

fn accumulate_signal_from_diff(diff: &[f64], len: usize) -> Vec<f64> {
    let mut signal = Vec::with_capacity(len);
    let mut running = 0.0_f64;
    for delta in diff.iter().take(len) {
        running += delta;
        signal.push(running);
    }
    signal
}

/// Rasterises weighted [`LineSegment`] values into a per-line signal.
///
/// The returned vector has length `function_end - function_start + 1`, where the
/// value at index `0` corresponds to `function_start`.
///
/// # Errors
///
/// - Returns [`SignalBuildError::FunctionLineRangeMustBeOneBased`] when the
///   provided range includes line `0`.
/// - Returns [`SignalBuildError::FunctionStartAfterEnd`] when the range is
///   inverted.
/// - Returns [`SignalBuildError::SegmentOutsideFunctionRange`] when any segment
///   does not overlap the function range.
///
/// # Examples
///
/// ```
/// use common::complexity_signal::{LineSegment, rasterise_signal};
///
/// let segments = [
///     LineSegment::new(10, 12, 1.0).expect("segment should be valid"),
///     LineSegment::new(12, 14, 2.0).expect("segment should be valid"),
/// ];
///
/// let signal = rasterise_signal(10..=14, &segments).expect("signal should build");
/// assert_eq!(signal, vec![1.0, 1.0, 3.0, 2.0, 2.0]);
/// ```
#[must_use = "Inspect the signal build result to handle invalid ranges"]
pub fn rasterise_signal(
    function_lines: RangeInclusive<usize>,
    segments: &[LineSegment],
) -> Result<Vec<f64>, SignalBuildError> {
    let function_start = *function_lines.start();
    let function_end = *function_lines.end();

    validate_function_range(function_start, function_end)?;

    let len = function_end - function_start + 1;
    let mut diff = vec![0.0_f64; len + 1];

    for segment in segments {
        validate_segment_in_range(segment, function_start, function_end)?;
        apply_segment_to_diff(segment, diff.as_mut_slice(), function_start);
    }

    Ok(accumulate_signal_from_diff(diff.as_slice(), len))
}

/// Errors emitted when smoothing a signal.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum SmoothingError {
    /// The moving average window must be positive.
    #[error("smoothing window must be positive (got {window})")]
    WindowMustBePositive { window: usize },

    /// The moving average window must be odd so the average is centred.
    #[error("smoothing window must be odd (got {window})")]
    WindowMustBeOdd { window: usize },
}

/// Applies a centred moving-average smoothing window.
///
/// The smoothing window is centred on each element. Near the start/end of the
/// signal the window contracts to include only the available samples. This
/// avoids padding or introducing edge-specific artefacts.
///
/// # Errors
///
/// Returns [`SmoothingError`] when the window size is invalid.
///
/// # Examples
///
/// ```
/// use common::complexity_signal::smooth_moving_average;
///
/// let signal = vec![0.0, 0.0, 3.0, 0.0, 0.0];
/// let smoothed = smooth_moving_average(&signal, 3).expect("window should be valid");
/// assert_eq!(smoothed, vec![0.0, 1.0, 1.0, 1.0, 0.0]);
/// ```
#[must_use = "Inspect the smoothing result to handle invalid window sizes"]
pub fn smooth_moving_average(signal: &[f64], window: usize) -> Result<Vec<f64>, SmoothingError> {
    if window == 0 {
        return Err(SmoothingError::WindowMustBePositive { window });
    }

    fn is_even(value: usize) -> bool {
        (value & 1) == 0
    }

    if is_even(window) {
        return Err(SmoothingError::WindowMustBeOdd { window });
    }

    if signal.is_empty() {
        return Ok(Vec::new());
    }

    let half_window = window / 2;
    let mut prefix = Vec::with_capacity(signal.len() + 1);
    prefix.push(0.0_f64);
    for &value in signal {
        let next = prefix[prefix.len() - 1] + value;
        prefix.push(next);
    }

    let last_index = signal.len() - 1;
    let mut smoothed = Vec::with_capacity(signal.len());
    for idx in 0..signal.len() {
        let start = idx.saturating_sub(half_window);
        let end = (idx + half_window).min(last_index);
        let sum = prefix[end + 1] - prefix[start];
        let count = (end - start + 1) as f64;
        smoothed.push(sum / count);
    }

    Ok(smoothed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn rasterise_signal_accumulates_overlapping_segments() {
        let segments = vec![
            LineSegment::new(10, 12, 1.0).expect("segment should be valid"),
            LineSegment::new(12, 14, 2.0).expect("segment should be valid"),
        ];

        let signal = rasterise_signal(10..=14, &segments).expect("signal should build");
        assert_eq!(signal, vec![1.0, 1.0, 3.0, 2.0, 2.0]);
    }

    #[rstest]
    fn rasterise_signal_rejects_segments_outside_function_range() {
        let segments = vec![LineSegment::new(9, 10, 1.0).expect("segment should be valid")];

        let err = rasterise_signal(11..=14, &segments).expect_err("segment should be rejected");
        assert!(matches!(
            err,
            SignalBuildError::SegmentOutsideFunctionRange { .. }
        ));
    }

    #[rstest]
    fn moving_average_smoothing_uses_central_window() {
        let signal = vec![0.0, 0.0, 3.0, 0.0, 0.0];
        let smoothed = smooth_moving_average(&signal, 3).expect("window should be valid");
        assert_eq!(smoothed, vec![0.0, 1.0, 1.0, 1.0, 0.0]);
    }

    #[rstest]
    fn moving_average_window_must_be_positive() {
        let err = smooth_moving_average(&[1.0, 2.0], 0).expect_err("window should be rejected");
        assert_eq!(err, SmoothingError::WindowMustBePositive { window: 0 });
    }

    #[rstest]
    fn moving_average_window_must_be_odd() {
        let err = smooth_moving_average(&[1.0, 2.0], 2).expect_err("window should be rejected");
        assert_eq!(err, SmoothingError::WindowMustBeOdd { window: 2 });
    }

    #[rstest]
    fn segment_validation_rejects_start_after_end() {
        let err = LineSegment::new(6, 4, 1.0).expect_err("segment should be invalid");
        assert_eq!(
            err,
            SegmentError::StartAfterEnd {
                start_line: 6,
                end_line: 4,
            }
        );
    }
}
