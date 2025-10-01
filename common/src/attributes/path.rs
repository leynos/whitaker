use std::fmt;

/// A structured representation of an attribute path such as `tokio::test`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributePath {
    segments: Vec<String>,
}

impl AttributePath {
    /// Builds a path from iterator segments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// let path = AttributePath::new(["tokio", "test"]);
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

    /// Parses a Rust attribute path from its textual representation.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// let parsed = AttributePath::from("tokio::test");
    /// assert_eq!(parsed.segments(), &["tokio", "test"]);
    /// ```
    #[must_use]
    pub fn from(path: &str) -> Self {
        Self::new(path.split("::"))
    }

    /// Returns the path segments as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// let path = AttributePath::from("doc");
    /// assert_eq!(path.segments(), &["doc"]);
    /// ```
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns the final path segment when present.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// let path = AttributePath::from("rstest::case");
    /// assert_eq!(path.last(), Some("case"));
    /// ```
    #[must_use]
    pub fn last(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Returns `true` when this path matches the provided sequence exactly.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// let path = AttributePath::from("tokio::test");
    /// assert!(path.matches(["tokio", "test"]));
    /// assert!(!path.matches(["test"]));
    /// ```
    #[must_use]
    pub fn matches<'a, I>(&self, candidate: I) -> bool
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.segments
            .iter()
            .map(String::as_str)
            .eq(candidate.into_iter())
    }

    /// Returns `true` when the path denotes a doc comment (`doc`).
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    ///
    /// assert!(AttributePath::from("doc").is_doc());
    /// assert!(!AttributePath::from("allow").is_doc());
    /// ```
    #[must_use]
    pub fn is_doc(&self) -> bool {
        self.matches(["doc"])
    }
}

impl fmt::Display for AttributePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.segments.join("::"))
    }
}
