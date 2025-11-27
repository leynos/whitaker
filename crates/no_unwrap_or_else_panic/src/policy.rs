//! Pure lint policy evaluation logic shared by driver and behaviour tests.

use crate::context::ContextSummary;

/// Configuration flags controlling when the lint should emit diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LintPolicy {
    allow_in_main: bool,
}

impl LintPolicy {
    /// Create a policy with the given `allow_in_main` flag.
    #[must_use]
    pub(crate) const fn new(allow_in_main: bool) -> Self {
        Self { allow_in_main }
    }
}

/// Decide whether the lint should emit based on context and closure behaviour.
#[must_use]
pub(crate) fn should_flag(
    policy: &LintPolicy,
    summary: &ContextSummary,
    closure_panics: bool,
    is_doctest: bool,
) -> bool {
    if !closure_panics {
        return false;
    }

    if is_doctest || summary.is_test {
        return false;
    }

    if summary.in_main && policy.allow_in_main {
        return false;
    }

    true
}
