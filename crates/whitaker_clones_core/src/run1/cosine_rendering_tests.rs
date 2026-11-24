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

/// R3: every representable micros value renders with six decimals inside the
/// unit interval.
#[test]
fn every_micros_value_renders_exactly() {
    for micros in 0..=1_000_000_u32 {
        let score = CosineMicros::from_micros(micros).expect("micros within range");
        let rendered = score.as_decimal_string();
        let (integer, fraction) = rendered
            .split_once('.')
            .expect("every printed score has a decimal point");
        assert_eq!(
            integer,
            if micros == 1_000_000 { "1" } else { "0" },
            "score {micros} rendered with an unexpected integer part: {rendered}"
        );
        assert_eq!(
            fraction.len(),
            6,
            "score {micros} must render exactly six decimals: {rendered}"
        );
        let parsed = rendered
            .parse::<f64>()
            .expect("every printed score parses as a float");
        assert!(
            (0.0..=1.0).contains(&parsed),
            "score {micros} rendered as {rendered} is outside [0, 1]"
        );
    }
}
