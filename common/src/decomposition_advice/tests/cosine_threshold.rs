use crate::decomposition_advice::vector::{
    MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED, MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
    cosine_threshold_met, test_feature_vector,
};

fn check_cosine_threshold(left_weights: &[(&str, u64)], right_weights: &[(&str, u64)]) -> bool {
    let left = test_feature_vector("left", left_weights);
    let right = test_feature_vector("right", right_weights);
    cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    )
}

#[test]
fn cosine_threshold_met_accepts_exact_boundary_equality() {
    // `dot = 1`, `left_norm = 25`, and `right_norm = 1`, so
    // `25 * dot^2 == left_norm * right_norm`.
    assert!(check_cosine_threshold(
        &[
            ("shared", 1),
            ("left-heavy", 4),
            ("left-side-a", 2),
            ("left-side-b", 2)
        ],
        &[("shared", 1)],
    ));
}

#[test]
fn cosine_threshold_met_rejects_just_below_boundary() {
    // `dot = 1`, `left_norm = 25`, and `right_norm = 2`, so
    // `25 * dot^2 < left_norm * right_norm`.
    assert!(!check_cosine_threshold(
        &[
            ("shared", 1),
            ("left-heavy", 4),
            ("left-side-a", 2),
            ("left-side-b", 2)
        ],
        &[("shared", 1), ("right-side", 1)],
    ));
}

#[test]
fn cosine_threshold_met_rejects_zero_dot_product() {
    assert!(!check_cosine_threshold(
        &[("left-only", 3)],
        &[("right-only", 5)],
    ));
}

#[test]
fn cosine_threshold_met_rejects_left_zero_norm() {
    assert!(!check_cosine_threshold(&[], &[("shared", 5)]));
}

#[test]
fn cosine_threshold_met_rejects_right_zero_norm() {
    assert!(!check_cosine_threshold(&[("shared", 5)], &[]));
}
