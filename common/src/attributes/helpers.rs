//! Helpers for working with attribute collections.

use super::{Attribute, AttributePath};

/// Splits a slice of attributes into doc and non-doc groups.
///
/// # Examples
///
/// ```
/// use common::attributes::{split_doc_attributes, Attribute, AttributeKind, AttributePath};
///
/// let doc = Attribute::new(AttributePath::from("doc"), AttributeKind::Outer);
/// let allow = Attribute::new(AttributePath::from("allow"), AttributeKind::Outer);
/// let attributes = vec![doc.clone(), allow.clone()];
/// let (docs, rest) = split_doc_attributes(&attributes);
/// assert_eq!(docs.len(), 1);
/// assert_eq!(rest.len(), 1);
/// ```
#[must_use]
pub fn split_doc_attributes<'a>(
    attrs: &'a [Attribute],
) -> (Vec<&'a Attribute>, Vec<&'a Attribute>) {
    attrs.iter().partition(|attr| attr.is_doc())
}

/// Returns the subset of attributes that apply as outer attributes.
///
/// # Examples
///
/// ```
/// use common::attributes::{outer_attributes, Attribute, AttributeKind, AttributePath};
///
/// let inner = Attribute::new(AttributePath::from("doc"), AttributeKind::Inner);
/// let outer = Attribute::new(AttributePath::from("test"), AttributeKind::Outer);
/// let attributes = vec![inner, outer.clone()];
/// let outer_only = outer_attributes(&attributes);
/// assert_eq!(outer_only, vec![&outer]);
/// ```
#[must_use]
pub fn outer_attributes<'a>(attrs: &'a [Attribute]) -> Vec<&'a Attribute> {
    attrs.iter().filter(|attr| attr.is_outer()).collect()
}

/// Returns `true` when any attribute marks the item as test-like.
///
/// # Examples
///
/// ```
/// use common::attributes::{has_test_like_attribute, Attribute, AttributeKind, AttributePath};
///
/// let attr = Attribute::new(AttributePath::from("tokio::test"), AttributeKind::Outer);
/// assert!(has_test_like_attribute(&[attr]));
/// ```
#[must_use]
pub fn has_test_like_attribute(attrs: &[Attribute]) -> bool {
    has_test_like_attribute_with(attrs, &[])
}

/// Returns `true` when any attribute marks the item as test-like, accounting
/// for custom attribute paths supplied at runtime.
///
/// # Examples
///
/// ```
/// use common::attributes::{has_test_like_attribute_with, Attribute, AttributeKind, AttributePath};
///
/// let attr = Attribute::new(AttributePath::from("custom::test"), AttributeKind::Outer);
/// let additional = vec![AttributePath::from("custom::test")];
/// assert!(has_test_like_attribute_with(&[attr], &additional));
/// ```
#[must_use]
pub fn has_test_like_attribute_with(attrs: &[Attribute], additional: &[AttributePath]) -> bool {
    attrs
        .iter()
        .any(|attribute| attribute.is_test_like_with(additional))
}
