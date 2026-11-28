//! Table tests for the documentation attribute classification.
//!
//! The pure kernel is exercised here over every shape in the decision record.
//! The same shapes appear as real rustc attributes in the UI fixture
//! `documented_forms.rs`, because only a real compilation proves that the
//! driver maps rustc's attribute representation onto these shapes faithfully.

use rstest::rstest;

use super::{DocAttributeShape, Documentation, documentation_status};

/// Every shape's contribution to the documentation gate.
#[rstest]
#[case(&[], Documentation::Undocumented)]
#[case(&[DocAttributeShape::Other], Documentation::Undocumented)]
#[case(&[DocAttributeShape::DocList], Documentation::Undocumented)]
#[case(
    &[DocAttributeShape::Other, DocAttributeShape::DocList],
    Documentation::Undocumented
)]
#[case(&[DocAttributeShape::DocComment], Documentation::Documented)]
#[case(&[DocAttributeShape::DocNameValue], Documentation::Documented)]
#[case(&[DocAttributeShape::CfgAttrDoc], Documentation::Documented)]
#[case(
    &[DocAttributeShape::Other, DocAttributeShape::DocComment],
    Documentation::Documented
)]
#[case(
    &[DocAttributeShape::DocList, DocAttributeShape::DocNameValue],
    Documentation::Documented
)]
fn documentation_status_table(
    #[case] shapes: &[DocAttributeShape],
    #[case] expected: Documentation,
) {
    assert_eq!(
        documentation_status(shapes.iter().copied()),
        expected,
        "shapes {shapes:?} must classify as {expected:?}",
    );
}

/// List-form `doc` attributes do not count on their own.
///
/// This is the seeded fault the UI fixture `doc_hidden_only.rs` also guards: a
/// mapper that treats every `doc` attribute as documentation would mark
/// `#[doc(hidden)]` as documented.
#[test]
fn a_list_form_doc_attribute_is_not_documentation() {
    assert_eq!(
        documentation_status([DocAttributeShape::DocList]),
        Documentation::Undocumented,
        "`#[doc(hidden)]` must not satisfy the documentation gate"
    );
    assert!(
        !DocAttributeShape::DocList.counts_as_documentation(),
        "list-form `doc` must report that it is not documentation"
    );
}

/// A name-value `doc` attribute counts even when its value is a macro call.
///
/// `#[doc = include_str!("...")]` is the seeded fault's witness: a mapper that
/// resolves the attribute's string value would see an unexpanded macro call and
/// wrongly report the function as undocumented.
#[test]
fn a_name_value_doc_attribute_counts_regardless_of_its_value_form() {
    assert_eq!(
        documentation_status([DocAttributeShape::DocNameValue]),
        Documentation::Documented,
        "`#[doc = include_str!(...)]` must satisfy the documentation gate"
    );
}
