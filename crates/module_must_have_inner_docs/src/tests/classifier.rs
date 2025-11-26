//! Unit tests for snippet-based module doc detection.

use super::{ModuleDocDisposition, detect_module_docs_from_snippet};
use rstest::rstest;

#[rstest]
fn detects_missing_docs_when_no_content() {
    assert_eq!(
        detect_module_docs_from_snippet("\n  \n".into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn cfg_attr_with_doc_in_condition_is_not_treated_as_docs() {
    let snippet = r#"#![cfg_attr(doc, no_mangle)]"#.into();
    assert_eq!(
        detect_module_docs_from_snippet(snippet),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn cfg_attr_with_nested_cfg_expression_and_doc_attr_is_treated_as_docs() {
    let snippet = r#"#![cfg_attr(any(unix, windows), doc = "text")]"#.into();
    assert_eq!(
        detect_module_docs_from_snippet(snippet),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn cfg_attr_with_doc_and_other_attrs_doc_first_is_treated_as_docs() {
    let snippet = r#"#![cfg_attr(feature = "docs", doc = "text", inline)]"#.into();
    assert_eq!(
        detect_module_docs_from_snippet(snippet),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn cfg_attr_with_doc_and_other_attrs_doc_last_is_treated_as_docs() {
    let snippet = r#"#![cfg_attr(feature = "docs", inline, doc = "text")]"#.into();
    assert_eq!(
        detect_module_docs_from_snippet(snippet),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn cfg_attr_with_documentation_attr_is_not_treated_as_docs() {
    let snippet = r#"#![cfg_attr(feature = "docs", documentation = "text")]"#.into();
    assert_eq!(
        detect_module_docs_from_snippet(snippet),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_mixed_case_inner_doc_attribute_upper() {
    assert_eq!(
        detect_module_docs_from_snippet("#![DOC = \"module docs\"]".into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_mixed_case_inner_doc_attribute_camel() {
    assert_eq!(
        detect_module_docs_from_snippet("#![Doc = \"module docs\"]".into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_mixed_case_inner_doc_in_cfg_attr() {
    assert_eq!(
        detect_module_docs_from_snippet(
            "#![cfg_attr(feature = \"docs\", Doc = \"module docs\")]".into()
        ),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_leading_inner_doc() {
    assert_eq!(
        detect_module_docs_from_snippet("//! module docs".into()),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn accepts_inner_doc_attribute() {
    assert_eq!(
        detect_module_docs_from_snippet("#![doc = \"text\"]".into()),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn rejects_doc_after_inner_attribute() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![allow(dead_code)]\n//! doc".into()),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[rstest]
fn outer_docs_do_not_satisfy_requirement() {
    assert_eq!(
        detect_module_docs_from_snippet("/// doc".into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn outer_doc_attribute_does_not_satisfy_requirement() {
    assert_eq!(
        detect_module_docs_from_snippet("#[doc = \"module docs\"]\npub fn demo() {}".into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn rejects_allow_undocumented_unsafe_blocks() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![allow(undocumented_unsafe_blocks)]".into()),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}

#[rstest]
fn accepts_cfg_attr_doc() {
    assert_eq!(
        detect_module_docs_from_snippet("#![cfg_attr(feature = \"docs\", doc = \"text\")]".into()),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn handles_whitespace_in_inner_doc_attribute() {
    assert_eq!(
        detect_module_docs_from_snippet(" #! [ doc = \"\" ] ".into()),
        ModuleDocDisposition::HasLeadingDoc
    );

    assert_eq!(
        detect_module_docs_from_snippet("#!\n   [ doc = \"text\" ]".into()),
        ModuleDocDisposition::HasLeadingDoc
    );

    assert_eq!(
        detect_module_docs_from_snippet("#![   doc = \"text\"   ]".into()),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn handles_whitespace_in_cfg_attr_inner_doc_attribute() {
    assert_eq!(
        detect_module_docs_from_snippet(
            "#![cfg_attr(\n       feature = \"docs\",\n       doc = \"text\",\n   )]".into()
        ),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
fn rejects_similar_but_non_doc_attribute() {
    assert!(matches!(
        detect_module_docs_from_snippet("#![documentation = \"text\"]".into()),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
}
