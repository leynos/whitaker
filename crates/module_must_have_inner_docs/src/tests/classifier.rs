//! Unit tests for snippet-based module doc detection.

use super::{ModuleDocDisposition, detect_module_docs_from_snippet};
use rstest::rstest;

#[rstest]
#[case("\n  \n", ModuleDocDisposition::MissingDocs)]
#[case("//! module docs", ModuleDocDisposition::HasLeadingDoc)]
#[case("#![doc = \"text\"]", ModuleDocDisposition::HasLeadingDoc)]
#[case(
    "#![cfg_attr(feature = \"docs\", doc = \"text\")]",
    ModuleDocDisposition::HasLeadingDoc
)]
#[case(
    "#![cfg_attr(any(unix, windows), doc = \"text\")]",
    ModuleDocDisposition::HasLeadingDoc
)]
#[case(
    "#![cfg_attr(feature = \"docs\", doc = \"text\", inline)]",
    ModuleDocDisposition::HasLeadingDoc
)]
#[case(
    "#![cfg_attr(feature = \"docs\", inline, doc = \"text\")]",
    ModuleDocDisposition::HasLeadingDoc
)]
#[case("#![cfg_attr(doc, no_mangle)]", ModuleDocDisposition::MissingDocs)]
#[case(
    "#![cfg_attr(feature = \"docs\", documentation = \"text\")]",
    ModuleDocDisposition::MissingDocs
)]
#[case("/// doc", ModuleDocDisposition::MissingDocs)]
#[case(
    "#[doc = \"module docs\"]\npub fn demo() {}",
    ModuleDocDisposition::MissingDocs
)]
fn snippet_yields_expected_disposition(
    #[case] snippet: &str,
    #[case] expected: ModuleDocDisposition,
) {
    assert_eq!(detect_module_docs_from_snippet(snippet.into()), expected);
}

#[rstest]
#[case("#![DOC = \"module docs\"]")]
#[case("#![Doc = \"module docs\"]")]
#[case("#![cfg_attr(feature = \"docs\", Doc = \"module docs\")]")]
fn rejects_mixed_case_doc_identifiers(#[case] snippet: &str) {
    assert_eq!(
        detect_module_docs_from_snippet(snippet.into()),
        ModuleDocDisposition::MissingDocs
    );
}

#[rstest]
fn accepts_nested_cfg_attr_doc() {
    assert_eq!(
        detect_module_docs_from_snippet(
            "#![cfg_attr(feature = \"outer\", cfg_attr(feature = \"inner\", doc = \"Module docs\"))]"
                .into()
        ),
        ModuleDocDisposition::HasLeadingDoc
    );
}

#[rstest]
#[case("#![allow(dead_code)]\n//! doc")]
#[case("#![allow(undocumented_unsafe_blocks)]")]
#[case("#![documentation = \"text\"]")]
fn snippet_yields_first_inner_is_not_doc(#[case] snippet: &str) {
    assert!(matches!(
        detect_module_docs_from_snippet(snippet.into()),
        ModuleDocDisposition::FirstInnerIsNotDoc(_)
    ));
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
