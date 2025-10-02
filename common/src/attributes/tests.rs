//! Tests for attribute helpers.

use super::*;
use rstest::rstest;

#[rstest]
#[case::outer(AttributeKind::Outer, true)]
#[case::inner(AttributeKind::Inner, false)]
fn attribute_kind_is_outer(#[case] kind: AttributeKind, #[case] expected: bool) {
    assert_eq!(kind.is_outer(), expected);
}

#[rstest]
#[case::doc(AttributePath::from("doc"), true)]
#[case::allow(AttributePath::from("allow"), false)]
fn path_is_doc(#[case] path: AttributePath, #[case] expected: bool) {
    assert_eq!(path.is_doc(), expected);
}

#[rstest]
#[case::test("test", true)]
#[case::tokio_test("tokio::test", true)]
#[case::async_std("async_std::test", true)]
#[case::rstest("rstest", true)]
#[case::other("allow", false)]
fn attribute_is_test_like(#[case] path: &str, #[case] expected: bool) {
    let attribute = Attribute::new(AttributePath::from(path), AttributeKind::Outer);
    assert_eq!(attribute.is_test_like(), expected);
}

#[rstest]
fn split_doc_groups() {
    let doc = Attribute::new(AttributePath::from("doc"), AttributeKind::Outer);
    let allow = Attribute::new(AttributePath::from("allow"), AttributeKind::Outer);
    let attributes = vec![doc.clone(), allow.clone()];
    let (docs, rest) = split_doc_attributes(&attributes);

    assert_eq!(docs, vec![&doc]);
    assert_eq!(rest, vec![&allow]);
}

#[rstest]
fn finds_outer_attributes() {
    let inner = Attribute::new(AttributePath::from("doc"), AttributeKind::Inner);
    let outer = Attribute::new(AttributePath::from("test"), AttributeKind::Outer);
    let attributes = vec![inner, outer.clone()];
    let outer_only = outer_attributes(&attributes);

    assert_eq!(outer_only, vec![&outer]);
}
