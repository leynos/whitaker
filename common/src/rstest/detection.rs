//! Pure detection helpers for strict `rstest` tests and fixtures.

use crate::attributes::{Attribute, AttributePath};

const RSTEST_TEST_PATHS: &[&[&str]] = &[&["rstest"], &["rstest", "rstest"]];
const RSTEST_FIXTURE_PATHS: &[&[&str]] = &[&["fixture"], &["rstest", "fixture"]];
const DEFAULT_PROVIDER_ATTRIBUTE_PATHS: &[&[&str]] = &[
    &["case"],
    &["rstest", "case"],
    &["values"],
    &["rstest", "values"],
    &["files"],
    &["rstest", "files"],
    &["future"],
    &["rstest", "future"],
    &["context"],
    &["rstest", "context"],
];

/// Optional macro-expansion metadata used as a conservative fallback when
/// direct attributes are not available.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpansionTrace {
    frames: Vec<AttributePath>,
}

impl ExpansionTrace {
    /// Builds an expansion trace from attribute-like path frames.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    /// use common::rstest::ExpansionTrace;
    ///
    /// let trace = ExpansionTrace::new([AttributePath::from("rstest")]);
    /// assert_eq!(trace.frames(), &[AttributePath::from("rstest")]);
    /// ```
    #[must_use]
    pub fn new<I>(frames: I) -> Self
    where
        I: IntoIterator<Item = AttributePath>,
    {
        Self {
            frames: frames.into_iter().collect(),
        }
    }

    /// Returns the stored expansion frames.
    #[must_use]
    pub fn frames(&self) -> &[AttributePath] {
        &self.frames
    }
}

/// Runtime options for strict `rstest` detection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RstestDetectionOptions {
    provider_param_attributes: Vec<AttributePath>,
    use_expansion_trace_fallback: bool,
}

impl RstestDetectionOptions {
    /// Builds detection options from explicit values.
    ///
    /// # Examples
    ///
    /// ```
    /// use common::attributes::AttributePath;
    /// use common::rstest::RstestDetectionOptions;
    ///
    /// let options = RstestDetectionOptions::new(
    ///     vec![AttributePath::from("case"), AttributePath::from("rstest::case")],
    ///     true,
    /// );
    /// assert!(options.use_expansion_trace_fallback());
    /// ```
    #[must_use]
    pub fn new(
        provider_param_attributes: Vec<AttributePath>,
        use_expansion_trace_fallback: bool,
    ) -> Self {
        Self {
            provider_param_attributes,
            use_expansion_trace_fallback,
        }
    }

    /// Returns the configured provider-parameter attribute paths.
    #[must_use]
    pub fn provider_param_attributes(&self) -> &[AttributePath] {
        &self.provider_param_attributes
    }

    /// Returns whether expansion-trace fallback is enabled.
    #[must_use]
    pub const fn use_expansion_trace_fallback(&self) -> bool {
        self.use_expansion_trace_fallback
    }
}

impl Default for RstestDetectionOptions {
    fn default() -> Self {
        Self::new(
            DEFAULT_PROVIDER_ATTRIBUTE_PATHS
                .iter()
                .map(|path| AttributePath::new(path.iter().copied()))
                .collect(),
            false,
        )
    }
}

/// Returns `true` when the attributes mark a function as a strict `rstest`
/// test.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::rstest::is_rstest_test;
///
/// let attrs = vec![Attribute::new(AttributePath::from("rstest"), AttributeKind::Outer)];
/// assert!(is_rstest_test(&attrs));
/// ```
#[must_use]
pub fn is_rstest_test(attrs: &[Attribute]) -> bool {
    has_matching_attribute(attrs, RSTEST_TEST_PATHS)
}

/// Returns `true` when a function is a strict `rstest` test, optionally
/// consulting expansion-trace metadata.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::rstest::{ExpansionTrace, RstestDetectionOptions, is_rstest_test_with};
///
/// let attrs = vec![Attribute::new(AttributePath::from("allow"), AttributeKind::Outer)];
/// let trace = ExpansionTrace::new([AttributePath::from("rstest")]);
/// let options = RstestDetectionOptions::new(Vec::new(), true);
/// assert!(is_rstest_test_with(&attrs, Some(&trace), &options));
/// ```
#[must_use]
pub fn is_rstest_test_with(
    attrs: &[Attribute],
    trace: Option<&ExpansionTrace>,
    options: &RstestDetectionOptions,
) -> bool {
    matches_direct_or_trace(attrs, trace, options, RSTEST_TEST_PATHS)
}

/// Returns `true` when the attributes mark a function as a strict `rstest`
/// fixture.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::rstest::is_rstest_fixture;
///
/// let attrs = vec![Attribute::new(AttributePath::from("fixture"), AttributeKind::Outer)];
/// assert!(is_rstest_fixture(&attrs));
/// ```
#[must_use]
pub fn is_rstest_fixture(attrs: &[Attribute]) -> bool {
    has_matching_attribute(attrs, RSTEST_FIXTURE_PATHS)
}

/// Returns `true` when a function is a strict `rstest` fixture, optionally
/// consulting expansion-trace metadata.
///
/// # Examples
///
/// ```
/// use common::attributes::{Attribute, AttributeKind, AttributePath};
/// use common::rstest::{ExpansionTrace, RstestDetectionOptions, is_rstest_fixture_with};
///
/// let attrs = vec![Attribute::new(AttributePath::from("allow"), AttributeKind::Outer)];
/// let trace = ExpansionTrace::new([AttributePath::from("rstest::fixture")]);
/// let options = RstestDetectionOptions::new(Vec::new(), true);
/// assert!(is_rstest_fixture_with(&attrs, Some(&trace), &options));
/// ```
#[must_use]
pub fn is_rstest_fixture_with(
    attrs: &[Attribute],
    trace: Option<&ExpansionTrace>,
    options: &RstestDetectionOptions,
) -> bool {
    matches_direct_or_trace(attrs, trace, options, RSTEST_FIXTURE_PATHS)
}

fn matches_direct_or_trace(
    attrs: &[Attribute],
    trace: Option<&ExpansionTrace>,
    options: &RstestDetectionOptions,
    candidates: &[&[&str]],
) -> bool {
    has_matching_attribute(attrs, candidates)
        || (options.use_expansion_trace_fallback()
            && trace.is_some_and(|trace| has_matching_trace(trace, candidates)))
}

fn has_matching_attribute(attrs: &[Attribute], candidates: &[&[&str]]) -> bool {
    attrs
        .iter()
        .any(|attribute| path_matches_candidates(attribute.path(), candidates))
}

fn has_matching_trace(trace: &ExpansionTrace, candidates: &[&[&str]]) -> bool {
    trace
        .frames()
        .iter()
        .any(|frame| path_matches_candidates(frame, candidates))
}

fn path_matches_candidates(path: &AttributePath, candidates: &[&[&str]]) -> bool {
    candidates
        .iter()
        .any(|candidate| path.matches(candidate.iter().copied()))
}
