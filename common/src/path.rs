//! Shared helpers for working with `::`-delimited paths.

use std::fmt;

/// Generic helper for representing syntactic paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path<T> {
    segments: Vec<T>,
}

impl<T> Path<T> {
    /// Builds a path from iterator segments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::path::Path;
    ///
    /// let path = Path::<String>::new(["tokio", "test"]);
    /// assert_eq!(path.segments(), &["tokio", "test"]);
    /// ```
    #[must_use]
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<T>,
    {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the path segments as a slice.
    #[must_use]
    pub fn segments(&self) -> &[T] {
        &self.segments
    }

    /// Returns `true` when this path matches the provided sequence exactly.
    #[must_use]
    pub fn matches<I, U>(&self, candidate: I) -> bool
    where
        I: IntoIterator<Item = U>,
        for<'a> &'a T: PartialEq<U>,
        for<'a> U: PartialEq<&'a T>,
    {
        self.segments.iter().eq(candidate.into_iter())
    }
}

impl Path<String> {
    /// Parses a Rust path from its textual representation.
    ///
    /// Empty segments produced by leading, trailing, or repeated separators are
    /// discarded.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::path::{Path, SimplePath};
    ///
    /// let parsed = SimplePath::from("tokio::test");
    /// assert_eq!(parsed.segments(), &["tokio", "test"]);
    /// let compact = SimplePath::from("::test::");
    /// assert_eq!(compact.segments(), &["test"]);
    /// ```
    #[must_use]
    pub fn from(path: &str) -> Self {
        Self::new(path.split("::").filter(|segment| !segment.is_empty()))
    }

    /// Returns the final path segment when present.
    #[must_use]
    pub fn last(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Returns `true` when the path denotes a doc comment (`doc`).
    #[must_use]
    pub fn is_doc(&self) -> bool {
        self.matches(["doc"])
    }
}

impl fmt::Display for Path<String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("::"))
    }
}

/// Convenience alias for paths composed of text segments.
pub type SimplePath = Path<String>;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn filters_empty_segments() {
        let path = SimplePath::from("::crate::::Item::");
        assert_eq!(path.segments(), &["crate", "Item"]);
    }

    #[rstest]
    fn matches_segments() {
        let path = SimplePath::from("crate::module::Item");
        assert!(path.matches(["crate", "module", "Item"]));
        assert!(!path.matches(["crate", "module", "Other"]));
    }
}
