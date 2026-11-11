//! Unit tests for the HIR attribute classifier.
//!
//! The classifier works on records extracted from HIR, so these cases build
//! synthetic records at distinct byte positions. That isolates the ordering
//! and selection rules from the compiler-backed UI fixtures.

use rstest::rstest;
use rustc_span::{BytePos, DUMMY_SP, Span};

use super::{
    ModuleDocDisposition,
    hir_attrs::{ModuleAttr, classify},
};

/// Builds a span at a distinct byte offset so record order is unambiguous.
///
/// `Span::new` swaps its bounds when `lo > hi`, so the high bound is set first;
/// setting the low bound on a dummy span would otherwise collapse back to zero.
fn span_at(lo: u32) -> Span { DUMMY_SP.with_hi(BytePos(lo + 1)).with_lo(BytePos(lo)) }

/// Builds a documentation record at the given byte offset.
fn doc_at(lo: u32) -> ModuleAttr {
    ModuleAttr {
        is_doc: true,
        span: span_at(lo),
    }
}

/// Builds a non-documentation record at the given byte offset.
fn other_at(lo: u32) -> ModuleAttr {
    ModuleAttr {
        is_doc: false,
        span: span_at(lo),
    }
}

#[rstest]
#[case::no_body_with_doc(vec![doc_at(10)], false, ModuleDocDisposition::Unknown)]
#[case::no_body_without_records(Vec::new(), false, ModuleDocDisposition::Unknown)]
#[case::empty_body(Vec::new(), true, ModuleDocDisposition::MissingDocs)]
#[case::doc_only(vec![doc_at(10)], true, ModuleDocDisposition::HasLeadingDoc)]
#[case::non_doc_only(vec![other_at(10)], true, ModuleDocDisposition::MissingDocs)]
#[case::doc_before_non_doc(
    vec![doc_at(10), other_at(20)],
    true,
    ModuleDocDisposition::HasLeadingDoc
)]
#[case::non_doc_before_doc(
    vec![other_at(10), doc_at(20)],
    true,
    ModuleDocDisposition::FirstInnerIsNotDoc(span_at(10))
)]
#[case::unsorted_input_with_earliest_doc(
    vec![other_at(30), other_at(20), doc_at(10)],
    true,
    ModuleDocDisposition::HasLeadingDoc
)]
#[case::unsorted_input_with_earliest_non_doc(
    vec![doc_at(40), other_at(30), other_at(15)],
    true,
    ModuleDocDisposition::FirstInnerIsNotDoc(span_at(15))
)]
fn classifies_records(
    #[case] attrs: Vec<ModuleAttr>,
    #[case] has_body: bool,
    #[case] expected: ModuleDocDisposition,
) {
    assert_eq!(classify(&attrs, has_body), expected);
}
