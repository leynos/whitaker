//! Attribute metadata helpers for lint analysis.

use super::{AttributeKind, AttributePath, TEST_LIKE_PATHS};

/// Represents a Rust attribute, tracking its path and attachment style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    path: AttributePath,
    kind: AttributeKind,
    arguments: Vec<String>,
}

impl Attribute {
    /// Creates a new attribute without arguments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("test"), AttributeKind::Outer);
    /// assert!(attribute.is_outer());
    /// ```
    #[must_use]
    pub fn new(path: AttributePath, kind: AttributeKind) -> Self {
        Self {
            path,
            kind,
            arguments: Vec::new(),
        }
    }

    /// Creates an attribute with the provided argument strings.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::with_arguments(
    ///     AttributePath::from("allow"),
    ///     AttributeKind::Outer,
    ///     ["clippy::needless_bool"],
    /// );
    /// assert_eq!(attribute.arguments(), &["clippy::needless_bool"]);
    /// ```
    #[must_use]
    pub fn with_arguments<I, S>(path: AttributePath, kind: AttributeKind, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            path,
            kind,
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }

    /// Creates an attribute with borrowed string arguments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::with_str_arguments(
    ///     AttributePath::from("allow"),
    ///     AttributeKind::Outer,
    ///     &["clippy::needless_bool"],
    /// );
    /// assert_eq!(attribute.arguments(), &["clippy::needless_bool"]);
    /// ```
    #[must_use]
    pub fn with_str_arguments(path: AttributePath, kind: AttributeKind, args: &[&str]) -> Self {
        Self::with_arguments(path, kind, args.iter().copied())
    }

    /// Returns the underlying attribute path.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("doc"), AttributeKind::Outer);
    /// assert!(attribute.path().is_doc());
    /// ```
    #[must_use]
    pub fn path(&self) -> &AttributePath {
        &self.path
    }

    /// Returns the attachment kind (inner or outer).
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("doc"), AttributeKind::Inner);
    /// assert!(attribute.kind().is_inner());
    /// ```
    #[must_use]
    pub const fn kind(&self) -> AttributeKind {
        self.kind
    }

    /// Returns the attribute arguments.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::with_str_arguments(
    ///     AttributePath::from("allow"),
    ///     AttributeKind::Outer,
    ///     &["dead_code"],
    /// );
    /// assert_eq!(attribute.arguments(), &["dead_code"]);
    /// ```
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// Indicates whether the attribute is a doc comment (`#[doc = ...]`).
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("doc"), AttributeKind::Outer);
    /// assert!(attribute.is_doc());
    /// ```
    #[must_use]
    pub fn is_doc(&self) -> bool {
        self.path.is_doc()
    }

    /// Indicates whether the attribute marks a test-like context.
    ///
    /// Test-like attributes include `test`, `tokio::test`, `async_std::test`,
    /// and `rstest`.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let rstest = Attribute::new(AttributePath::from("rstest"), AttributeKind::Outer);
    /// assert!(rstest.is_test_like());
    /// ```
    #[must_use]
    pub fn is_test_like(&self) -> bool {
        self.is_test_like_with(&[])
    }

    #[must_use]
    pub fn is_test_like_with(&self, additional: &[AttributePath]) -> bool {
        if TEST_LIKE_PATHS
            .iter()
            .any(|candidate| self.path.matches(candidate.iter().copied()))
        {
            return true;
        }

        additional.iter().any(|path| {
            let segments: Vec<&str> = path.segments().iter().map(String::as_str).collect();
            self.path.matches(segments)
        })
    }

    /// Returns `true` when the attribute is an inner attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("doc"), AttributeKind::Inner);
    /// assert!(attribute.is_inner());
    /// ```
    #[must_use]
    pub const fn is_inner(&self) -> bool {
        self.kind.is_inner()
    }

    /// Returns `true` when the attribute is an outer attribute.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attribute = Attribute::new(AttributePath::from("doc"), AttributeKind::Outer);
    /// assert!(attribute.is_outer());
    /// ```
    #[must_use]
    pub const fn is_outer(&self) -> bool {
        self.kind.is_outer()
    }
}
