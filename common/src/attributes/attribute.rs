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

    /// Indicates whether the attribute marks a test-like context when supplied
    /// with additional recognised paths.
    ///
    /// Test-like attributes include `test`, `tokio::test`, `async_std::test`,
    /// `rstest`, and any entries provided via the `additional` parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::{Attribute, AttributeKind, AttributePath};
    ///
    /// let attr = Attribute::new(AttributePath::from("custom::test"), AttributeKind::Outer);
    /// let additional = vec![AttributePath::from("custom::test")];
    /// assert!(attr.is_test_like_with(&additional));
    /// ```
    #[must_use]
    pub fn is_test_like_with(&self, additional: &[AttributePath]) -> bool {
        if matches_builtin_test_like_path(&self.path) {
            return true;
        }

        additional.iter().any(|path| {
            self.path
                .matches(path.segments().iter().map(String::as_str))
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

fn matches_builtin_test_like_path(path: &AttributePath) -> bool {
    TEST_LIKE_PATHS
        .iter()
        .any(|candidate| path.matches(candidate.iter().copied()))
        || is_prelude_test_attribute(path)
}

fn is_prelude_test_attribute(path: &AttributePath) -> bool {
    // `is_prelude_test_attribute` treats the third segment from
    // `AttributePath::segments()` as a wildcard because `_edition` may vary
    // across toolchains (`v1`, `rust_2021`, `rust_2024`). The matcher
    // therefore fixes only the root, `prelude`, and trailing `test`.
    let [root, prelude, _edition, test] = path.segments() else {
        return false;
    };

    matches!(root.as_str(), "core" | "std") && prelude == "prelude" && test == "test"
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::core_v1("core::prelude::v1::test", true)]
    #[case::std_rust_2021("std::prelude::rust_2021::test", true)]
    #[case::std_rust_2023("std::prelude::rust_2023::test", true)]
    #[case::std_rust_2024("std::prelude::rust_2024::test", true)]
    #[case::three_segments("core::prelude::test", false)]
    #[case::five_segments("core::prelude::v1::extra::test", false)]
    #[case::wrong_middle("core::not_prelude::v1::test", false)]
    #[case::wrong_root("alloc::prelude::v1::test", false)]
    #[case::wrong_final("core::prelude::v1::bench", false)]
    fn prelude_test_attribute_shape(#[case] path: &str, #[case] expected: bool) {
        assert_eq!(
            is_prelude_test_attribute(&AttributePath::from(path)),
            expected
        );
    }

    #[rstest]
    #[case::builtin_test("test", true)]
    #[case::builtin_tokio("tokio::test", true)]
    #[case::prelude_test("std::prelude::rust_2024::test", true)]
    #[case::short_prelude("std::prelude::test", false)]
    #[case::long_prelude("std::prelude::rust_2024::extra::test", false)]
    #[case::wrong_prelude_segment("std::not_prelude::rust_2024::test", false)]
    fn builtin_test_like_paths(#[case] path: &str, #[case] expected: bool) {
        assert_eq!(
            matches_builtin_test_like_path(&AttributePath::from(path)),
            expected
        );
    }
}
