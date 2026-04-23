//! Fragment identifier newtype for clone-detector candidate generation.

use std::fmt;

/// Opaque fragment identifier used by candidate generation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FragmentId(String);

impl FragmentId {
    /// Creates a new fragment identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::FragmentId;
    ///
    /// let id = FragmentId::new("src/lib.rs:10..20");
    /// assert_eq!(id.as_str(), "src/lib.rs:10..20");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the fragment identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Consumes the identifier and returns the owned string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::FragmentId;
    ///
    /// let id = FragmentId::from("fragment-a");
    /// assert_eq!(id.into_inner(), "fragment-a".to_owned());
    /// ```
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&str> for FragmentId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for FragmentId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for FragmentId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for FragmentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
