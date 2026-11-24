//! Decimal-rendering and constructor tests for the cosine score.
//!
//! Split from the sibling test module to stay inside the 400-line module
//! budget; the fixtures are duplicated deliberately so each file can be
//! read on its own.

use super::{CosineMicros, CosineScore};
use crate::KindHistogram;

/// R3: the public constructor rejects values a cosine cannot take.
#[test]
fn out_of_range_micros_are_rejected() {
    assert!(
        CosineMicros::from_micros(1_000_001).is_none(),
        "a cosine above one must be rejected"
    );
    assert!(
        CosineMicros::from_micros(u32::MAX).is_none(),
        "the largest micros value must be rejected"
    );
}

/// R2: two empty histograms have no defined cosine.
#[test]
fn empty_histograms_have_no_cosine() {
    let left = KindHistogram::default();
    let right = KindHistogram::default();

    assert!(
        CosineScore::between(&left, &right).is_none(),
        "two empty histograms have no cosine"
    );
}
