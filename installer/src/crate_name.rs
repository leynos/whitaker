//! Semantic wrapper for lint crate names.
//!
//! This module provides the [`CrateName`] newtype for type-safe handling of
//! crate names throughout the installer.

use std::fmt;

/// A semantic crate name for lint libraries.
///
/// This newtype wrapper provides type safety for crate names, ensuring they are
/// passed explicitly rather than as raw strings. Validation is performed by
/// [`super::builder::validate_crate_names`] and related helpers, not by this
/// type itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrateName(String);

impl CrateName {
    /// Create a new crate name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the crate name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CrateName {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for CrateName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn new_creates_valid_instance() {
        let name = CrateName::new("test_crate");
        assert_eq!(name.as_str(), "test_crate");
    }

    #[test]
    fn as_str_returns_inner_value() {
        let name = CrateName::from("module_max_lines");
        assert_eq!(name.as_str(), "module_max_lines");
    }

    #[test]
    fn into_inner_consumes_and_returns_string() {
        let name = CrateName::from("test");
        let inner = name.into_inner();
        assert_eq!(inner, "test");
    }

    #[test]
    fn from_str_creates_crate_name() {
        let name: CrateName = "from_str_test".into();
        assert_eq!(name.as_str(), "from_str_test");
    }

    #[test]
    fn from_string_creates_crate_name() {
        let owned = String::from("from_string_test");
        let name: CrateName = owned.into();
        assert_eq!(name.as_str(), "from_string_test");
    }

    #[test]
    fn display_formats_as_inner_string() {
        let name = CrateName::from("display_test");
        assert_eq!(format!("{name}"), "display_test");
    }

    #[test]
    fn as_ref_returns_str_slice() {
        let name = CrateName::from("as_ref_test");
        let s: &str = name.as_ref();
        assert_eq!(s, "as_ref_test");
    }

    #[test]
    fn eq_compares_inner_values() {
        let a = CrateName::from("same");
        let b = CrateName::from("same");
        let c = CrateName::from("different");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn hash_allows_use_in_collections() {
        let mut set = HashSet::new();
        set.insert(CrateName::from("a"));
        set.insert(CrateName::from("b"));
        set.insert(CrateName::from("a")); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&CrateName::from("a")));
        assert!(set.contains(&CrateName::from("b")));
    }

    #[test]
    fn clone_creates_independent_copy() {
        let original = CrateName::from("original");
        let cloned = original.clone();

        assert_eq!(original, cloned);
        // They should be equal but independent (clone doesn't affect original)
        drop(cloned);
        assert_eq!(original.as_str(), "original");
    }
}
