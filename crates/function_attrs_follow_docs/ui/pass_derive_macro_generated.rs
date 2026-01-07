//! Regression test for compiler-generated attributes from derive macros.
//!
//! Derive macros can generate compiler-internal attributes (like inline hints)
//! that don't have source spans. Accessing such spans would cause a panic.
//! This test ensures the lint gracefully handles these cases.

#![warn(function_attrs_follow_docs)]

/// A struct with derive macros that may generate compiler-internal attributes.
#[expect(dead_code, reason = "UI test fixture is not executed")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct DerivedStruct {
    /// A field with documentation.
    pub value: u32,
}

/// An enum with derive macros.
#[expect(dead_code, reason = "UI test fixture is not executed")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedEnum {
    /// First variant.
    First,
    /// Second variant.
    Second,
}

#[expect(dead_code, reason = "UI test fixture is not executed")]
fn main() {}
