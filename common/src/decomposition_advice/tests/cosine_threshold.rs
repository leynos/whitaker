use crate::decomposition_advice::vector::{
    MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED, MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
    cosine_threshold_met, test_feature_vector,
};

#[test]
fn cosine_threshold_met_accepts_exact_boundary_equality() {
    let left = test_feature_vector(
        "left",
        &[
            ("shared", 1),
            ("left-heavy", 4),
            ("left-side-a", 2),
            ("left-side-b", 2),
        ],
    );
    let right = test_feature_vector("right", &[("shared", 1)]);

    // `dot = 1`, `left_norm = 25`, and `right_norm = 1`, so
    // `25 * dot^2 == left_norm * right_norm`.
    assert!(cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    ));
}

#[test]
fn cosine_threshold_met_rejects_just_below_boundary() {
    let left = test_feature_vector(
        "left",
        &[
            ("shared", 1),
            ("left-heavy", 4),
            ("left-side-a", 2),
            ("left-side-b", 2),
        ],
    );
    let right = test_feature_vector("right", &[("shared", 1), ("right-side", 1)]);

    // `dot = 1`, `left_norm = 25`, and `right_norm = 2`, so
    // `25 * dot^2 < left_norm * right_norm`.
    assert!(!cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    ));
}

#[test]
fn cosine_threshold_met_rejects_zero_dot_product() {
    let left = test_feature_vector("left", &[("left-only", 3)]);
    let right = test_feature_vector("right", &[("right-only", 5)]);

    assert!(!cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    ));
}

#[test]
fn cosine_threshold_met_rejects_left_zero_norm() {
    let left = test_feature_vector("left", &[]);
    let right = test_feature_vector("right", &[("shared", 5)]);

    assert!(!cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    ));
}

#[test]
fn cosine_threshold_met_rejects_right_zero_norm() {
    let left = test_feature_vector("left", &[("shared", 5)]);
    let right = test_feature_vector("right", &[]);

    assert!(!cosine_threshold_met(
        &left,
        &right,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    ));
}
