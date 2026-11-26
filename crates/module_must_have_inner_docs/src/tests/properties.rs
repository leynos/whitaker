//! Property tests for the classifier and the containment rule.
//!
//! The named `rstest` cases in `classifier.rs` state the contract for chosen
//! examples. These properties cover the parts that examples cannot: that the
//! classifier depends only on the earliest record by source position, so any
//! ordering of the same records agrees; and that containment is the inclusive
//! interval test its callers assume.

use proptest::prelude::*;

use super::{
    ModuleDocDisposition,
    hir_attrs::{ModuleAttr, classify, span_inside},
    test_support::span_at,
};

/// Generates a record list whose byte offsets are distinct and unordered.
fn records() -> impl Strategy<Value = Vec<ModuleAttr>> {
    // Distinct offsets keep `min_by_key` unambiguous, so the property states
    // the contract rather than an arbitrary tie-break.
    prop::collection::vec(any::<bool>(), 0..8).prop_map(|flags| {
        flags
            .into_iter()
            .enumerate()
            .map(|(index, is_doc)| {
                // The generator caps the list at eight entries, so the index
                // always fits; use `u32::try_from` rather than a cast that
                // could truncate on a wider platform.
                let lo = u32::try_from(index).unwrap_or(u32::MAX) * 10;
                ModuleAttr {
                    is_doc,
                    span: span_at(lo),
                }
            })
            .collect()
    })
}

/// Sort records by the span position the classifier orders on.
fn sorted_by_lo(attrs: &[ModuleAttr]) -> Vec<ModuleAttr> {
    let mut sorted = attrs.to_vec();
    sorted.sort_by_key(|attr| attr.span.lo());
    sorted
}

proptest! {
    /// The classifier's verdict does not depend on the order records arrive in.
    #[test]
    fn classification_ignores_record_order(attrs in records()) {
        let expected = classify(&sorted_by_lo(&attrs), true);
        for rotation in 0..attrs.len() {
            let mut shuffled = attrs.clone();
            shuffled.rotate_left(rotation);
            prop_assert_eq!(
                classify(&shuffled, true),
                expected,
                "rotating the records by {} changed the verdict",
                rotation
            );
        }
    }

    /// A body with no inspectable span is always `Unknown`, whatever the records.
    #[test]
    fn absent_body_is_always_unknown(attrs in records()) {
        prop_assert_eq!(classify(&attrs, false), ModuleDocDisposition::Unknown);
    }

    /// A record list with no doc yields `MissingDocs`; otherwise the earliest
    /// record decides between the remaining two dispositions.
    #[test]
    fn earliest_record_decides(attrs in records()) {
        let disposition = classify(&attrs, true);
        let has_doc = attrs.iter().any(|attr| attr.is_doc);
        if has_doc {
            let earliest = attrs
                .iter()
                .min_by_key(|attr| attr.span.lo())
                .expect("a non-empty doc-bearing list has an earliest record");
            let expected = if earliest.is_doc {
                ModuleDocDisposition::HasLeadingDoc
            } else {
                ModuleDocDisposition::FirstInnerIsNotDoc(earliest.span)
            };
            prop_assert_eq!(disposition, expected);
        } else {
            prop_assert_eq!(disposition, ModuleDocDisposition::MissingDocs);
        }
    }

    /// Containment is the inclusive interval test, boundaries included.
    #[test]
    fn containment_is_inclusive(
        outer_lo in 0u32..100,
        outer_len in 0u32..100,
        inner_lo in 0u32..200,
        inner_len in 0u32..100,
    ) {
        let outer = span_inside_span(outer_lo, outer_lo + outer_len);
        let inner = span_inside_span(inner_lo, inner_lo + inner_len);
        let expected = outer_lo <= inner_lo && inner_lo + inner_len <= outer_lo + outer_len;
        prop_assert_eq!(
            span_inside(outer, inner),
            expected,
            "outer {}..{} inner {}..{}",
            outer_lo,
            outer_lo + outer_len,
            inner_lo,
            inner_lo + inner_len
        );
    }

    /// A span equal to the outer span is contained; one byte wider is not.
    #[test]
    fn containment_boundaries_are_inclusive(lo in 0u32..100, len in 1u32..50) {
        let outer = span_inside_span(lo, lo + len);
        let same = span_inside_span(lo, lo + len);
        prop_assert!(span_inside(outer, same), "a span must contain itself");
        let wider = span_inside_span(lo, lo + len + 1);
        prop_assert!(!span_inside(outer, wider), "an overflowing span must not fit");
    }
}

/// Builds a span covering `lo..hi` for the containment properties.
///
/// `span_at` builds a one-byte span, which cannot express a range, so the
/// bounds are set directly with the high bound first for the reason recorded
/// in `test_support`.
fn span_inside_span(lo: u32, hi: u32) -> rustc_span::Span {
    use rustc_span::{BytePos, DUMMY_SP};

    DUMMY_SP.with_hi(BytePos(hi)).with_lo(BytePos(lo))
}
