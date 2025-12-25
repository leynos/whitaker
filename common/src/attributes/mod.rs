//! Helpers for reasoning about Rust attributes.

/// Placeholder path for parsed attributes that don't expose their path.
///
/// When HIR attributes like `#[must_use]` are pre-processed by rustc, they
/// become `Parsed` variants that lack an accessible path. Calling `path()` on
/// them would panic. This constant provides a consistent placeholder value
/// for such attributes across all lint crates.
pub const PARSED_ATTRIBUTE_PLACEHOLDER: &str = "parsed";

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
