//! Tests for complexity-signal segmentation, construction, and smoothing.

use rstest::rstest;

use super::*;

#[rstest]
fn rasterize_signal_accumulates_overlapping_segments() {
    let segments = vec![
        LineSegment::new(10, 12, 1.0).expect("segment should be valid"),
        LineSegment::new(12, 14, 2.0).expect("segment should be valid"),
    ];

    let signal = rasterize_signal(10..=14, &segments).expect("signal should build");
    assert_eq!(signal, vec![1.0, 1.0, 3.0, 2.0, 2.0]);
}

#[rstest]
fn rasterize_signal_rejects_segments_outside_function_range() {
    let segments = vec![LineSegment::new(9, 10, 1.0).expect("segment should be valid")];

    let err = rasterize_signal(11..=14, &segments).expect_err("segment should be rejected");
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
