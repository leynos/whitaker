//! Tests for attribute helpers.

use super::*;
use rstest::rstest;

#[rstest]
#[case::empty(Vec::<String>::new())]
#[case::single(vec!["dead_code".to_string()])]
#[case::complex(vec!["cfg(feature = \"test\")".to_string(), "path(\"std::io\")".to_string()])]
fn attribute_with_arguments_preserves_inputs(#[case] arguments: Vec<String>) {
    let attribute = Attribute::with_arguments(
        AttributePath::from("allow"),
        AttributeKind::Outer,
        arguments.clone(),
    );

    assert_eq!(attribute.arguments(), arguments);
}

#[rstest]
fn attribute_with_str_arguments_converts() {
    let attribute = Attribute::with_str_arguments(
        AttributePath::from("allow"),
        AttributeKind::Outer,
        &["dead_code", "unused_variables"],
    );

    assert_eq!(attribute.arguments(), &["dead_code", "unused_variables"]);
}

#[rstest]
fn attribute_with_str_arguments_handles_empty() {
    let attribute =
        Attribute::with_str_arguments(AttributePath::from("allow"), AttributeKind::Outer, &[]);

    assert!(attribute.arguments().is_empty());
}

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
#[case::rstest_qualified("rstest::rstest", true)]
#[case::case_imported("case", true)]
#[case::case_qualified("rstest::case", true)]
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
