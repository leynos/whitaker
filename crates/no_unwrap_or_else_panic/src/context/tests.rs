//! Unit tests for parsed attribute handling.
//!
//! Verifies that `convert_attribute` and `is_cfg_test_attribute` handle
//! parsed attributes (e.g., `#[must_use]`) without panicking.

#[cfg(feature = "dylint-driver")]
use super::{convert_attribute, is_cfg_test_attribute};
#[cfg(feature = "dylint-driver")]
use common::{AttributeKind, PARSED_ATTRIBUTE_PLACEHOLDER};
#[cfg(feature = "dylint-driver")]
use rustc_hir as hir;
#[cfg(feature = "dylint-driver")]
use rustc_hir::attrs::AttributeKind as HirAttributeKind;
#[cfg(feature = "dylint-driver")]
use rustc_span::DUMMY_SP;

/// Verify that `convert_attribute` handles parsed attributes without panicking.
///
/// Parsed attributes (e.g., `#[must_use]`) are pre-processed by rustc and don't
/// have an accessible path. Calling `path()` on them would panic.
#[cfg(feature = "dylint-driver")]
#[test]
fn convert_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    let attribute = convert_attribute(&parsed_attr);

    // Should return a placeholder "parsed" path instead of panicking.
    assert_eq!(
        attribute.path().segments(),
        &[PARSED_ATTRIBUTE_PLACEHOLDER.to_string()]
    );
    assert_eq!(attribute.kind(), AttributeKind::Outer);
}

/// Verify that `is_cfg_test_attribute` handles parsed attributes without panicking.
#[cfg(feature = "dylint-driver")]
#[test]
fn is_cfg_test_attribute_handles_parsed_must_use() {
    let parsed_attr = hir::Attribute::Parsed(HirAttributeKind::MustUse {
        span: DUMMY_SP,
        reason: None,
    });

    // Should return false (not a cfg(test) attribute) without panicking.
    assert!(!is_cfg_test_attribute(&parsed_attr));
}
