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
