//! Unit tests for snippet-based module doc detection.

use super::{ModuleDocDisposition, detect_module_docs_from_snippet};
use rstest::rstest;

#[rstest]
fn detects_missing_docs_when_no_content() {
    assert_eq!(
        detect_module_docs_from_snippet("\n  \n"),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_leading_inner_doc() {
    assert_eq!(
        detect_module_docs_from_snippet("//! module docs"),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn accepts_inner_doc_attribute() {
    assert_eq!(
        detect_module_docs_from_snippet("#![doc = \"text\"]"),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn rejects_doc_after_inner_attribute() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![allow(dead_code)]\n//! doc"),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[rstest]
fn outer_docs_do_not_satisfy_requirement() {
    assert_eq!(
        detect_module_docs_from_snippet("/// doc"),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn outer_doc_attribute_does_not_satisfy_requirement() {
    assert_eq!(
        detect_module_docs_from_snippet("#[doc = \"module docs\"]\npub fn demo() {}"),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn rejects_allow_undocumented_unsafe_blocks() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![allow(undocumented_unsafe_blocks)]"),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[rstest]
fn accepts_cfg_attr_doc() {
    assert_eq!(
        detect_module_docs_from_snippet("#![cfg_attr(feature = \"docs\", doc = \"text\")]"),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn handles_whitespace_in_inner_doc_attribute() {
    assert_eq!(
        detect_module_docs_from_snippet(" #! [ doc = \"\" ] "),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn rejects_similar_but_non_doc_attribute() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![documentation = \"text\"]"),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}
