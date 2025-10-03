//! Shared helpers for working with `::`-delimited paths.

use std::fmt;

/// Represents a syntactic path composed of `::`-separated segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimplePath {
    segments: Vec<String>,
}

impl SimplePath {
    /// Builds a path from iterator segments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::path::SimplePath;
    ///
    /// let path = SimplePath::new(["tokio", "test"]);
    /// assert_eq!(path.segments(), &["tokio", "test"]);
    /// ```
    #[must_use]
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    /// Parses a Rust path from its textual representation.
    ///
    /// Empty segments produced by leading, trailing, or repeated separators are
    /// discarded.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::path::SimplePath;
    ///
    /// let parsed = SimplePath::from("tokio::test");
    /// assert_eq!(parsed.segments(), &["tokio", "test"]);
    /// let compact = SimplePath::parse("::test::");
    /// assert_eq!(compact.segments(), &["test"]);
    /// ```
    #[must_use]
    pub fn parse(path: &str) -> Self {
        Self::new(path.split("::").filter(|segment| !segment.is_empty()))
    }

    /// Returns the path segments as a slice.
    #[must_use]
    #[rustfmt::skip]
    pub fn segments(&self) -> &[String] { &self.segments }

    /// Returns the final path segment when present.
    #[must_use]
    #[rustfmt::skip]
    pub fn last(&self) -> Option<&str> { self.segments.last().map(String::as_str) }

    /// Returns `true` when this path matches the provided sequence exactly.
    #[must_use]
    pub fn matches<I, S>(&self, candidate: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let ours = self.segments.iter().map(String::as_str);
        let theirs = candidate.into_iter().map(|segment| segment.as_ref());
        ours.eq(theirs)
    }

    /// Returns `true` when the path denotes a doc comment (`doc`).
    #[must_use]
    #[rustfmt::skip]
    pub fn is_doc(&self) -> bool { self.matches(["doc"]) }
}

impl From<&str> for SimplePath {
    fn from(path: &str) -> Self {
        Self::parse(path)
    }
}

impl From<String> for SimplePath {
    fn from(path: String) -> Self {
        Self::parse(&path)
    }
}

impl fmt::Display for SimplePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("::"))
    }
}

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
