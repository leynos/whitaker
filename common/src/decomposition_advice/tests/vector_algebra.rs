//! Validates the runtime vector algebra used by decomposition advice.

use crate::decomposition_advice::vector::{dot_product, test_feature_vector};
use rstest::rstest;

#[rstest]
#[case::left_smaller(
    test_feature_vector("left", &[("field:grammar", 6), ("keyword:parse", 2)]),
    test_feature_vector(
        "right",
        &[
            ("field:grammar", 6),
            ("keyword:parse", 2),
            ("domain:serde::json", 5),
        ],
    )
)]
#[case::right_smaller(
    test_feature_vector(
        "left",
        &[
            ("field:grammar", 6),
            ("keyword:parse", 2),
            ("domain:serde::json", 5),
        ],
    ),
    test_feature_vector("right", &[("field:grammar", 6), ("keyword:parse", 2)])
)]
fn dot_product_is_commutative(
    #[case] left: crate::decomposition_advice::vector::MethodFeatureVector,
    #[case] right: crate::decomposition_advice::vector::MethodFeatureVector,
) {
    assert_eq!(
        dot_product(left.weights(), right.weights()),
        dot_product(right.weights(), left.weights())
    );
}

#[test]
fn norm_squared_is_zero_for_empty_vector() {
    let vector = test_feature_vector("empty", &[]);

    assert_eq!(vector.norm_squared(), 0);
}

#[test]
fn norm_squared_is_positive_for_non_empty_vector() {
    let vector = test_feature_vector(
        "parse_tokens",
        &[("field:grammar", 6), ("keyword:parse", 2)],
    );

    assert!(vector.norm_squared() > 0);
    assert_eq!(vector.norm_squared(), 40);
}

#[test]
fn dot_product_is_zero_when_vectors_share_no_positive_feature() {
    let left = test_feature_vector("left", &[("field:grammar", 6), ("keyword:shared", 0)]);
    let right = test_feature_vector("right", &[("domain:std::fs", 5), ("keyword:shared", 7)]);

    assert_eq!(dot_product(left.weights(), right.weights()), 0);
}
