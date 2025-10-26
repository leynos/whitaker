//! Helpers for reasoning about Rust attributes.

pub(super) const TEST_LIKE_PATHS: &[&[&str]] = &[
    &["test"],
    &["tokio", "test"],
    &["async_std", "test"],
    &["rstest"],
    &["rstest", "case"],
];

mod attribute;
mod helpers;
mod kind;
mod path;

pub use attribute::Attribute;
pub use helpers::{
    has_test_like_attribute, has_test_like_attribute_with, outer_attributes, split_doc_attributes,
};
pub use kind::AttributeKind;
pub use path::AttributePath;

#[cfg(test)]
mod tests;
