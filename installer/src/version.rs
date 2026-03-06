//! Semantic wrapper for crate version strings.
//!
//! This module provides the [`Version`] newtype for type-safe handling of
//! crate version strings throughout the installer.

use std::fmt;

/// A crate version string (e.g. `"0.2.1"`).
///
/// This newtype wrapper provides type safety for version strings, ensuring they
/// are passed explicitly rather than as raw strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(String);

impl Version {
    /// Create a new [`Version`] from any string-like value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the underlying version string.
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

impl AsRef<str> for Version {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for Version {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_valid_instance() {
        let v = Version::new("0.2.1");
        assert_eq!(v.as_str(), "0.2.1");
    }

    #[test]
    fn as_str_returns_inner_value() {
        let v = Version::from("1.0.0");
        assert_eq!(v.as_str(), "1.0.0");
    }

    #[test]
    fn into_inner_consumes_and_returns_string() {
        let v = Version::from("2.3.4");
        let inner = v.into_inner();
        assert_eq!(inner, "2.3.4");
    }

    #[test]
    fn from_str_creates_version() {
        let v: Version = "3.0.0-beta.1".into();
        assert_eq!(v.as_str(), "3.0.0-beta.1");
    }

    #[test]
    fn from_string_creates_version() {
        let owned = String::from("4.5.6");
        let v: Version = owned.into();
        assert_eq!(v.as_str(), "4.5.6");
    }

    #[test]
    fn display_formats_as_inner_string() {
        let v = Version::from("0.1.0");
        assert_eq!(format!("{v}"), "0.1.0");
    }

    #[test]
    fn as_ref_returns_str_slice() {
        let v = Version::from("1.2.3");
        let s: &str = v.as_ref();
        assert_eq!(s, "1.2.3");
    }

    #[test]
    fn eq_compares_inner_values() {
        let a = Version::from("1.0.0");
        let b = Version::from("1.0.0");
        let c = Version::from("2.0.0");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn clone_creates_independent_copy() {
        let original = Version::from("1.0.0");
        let cloned = original.clone();

        assert_eq!(original, cloned);
        drop(cloned);
        assert_eq!(original.as_str(), "1.0.0");
    }
}
