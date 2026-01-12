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
///
/// # Examples
///
/// ```ignore
/// use no_unwrap_or_else_panic::policy::{should_flag, LintPolicy};
/// use no_unwrap_or_else_panic::context::ContextSummary;
///
/// let policy = LintPolicy::new(false);
/// let summary = ContextSummary { is_test: false, in_main: false };
/// assert!(should_flag(&policy, &summary, true, false));
/// ```
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

    if is_doctest {
        return false;
    }

    if summary.in_main && policy.allow_in_main {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(is_test: bool, in_main: bool) -> ContextSummary {
        ContextSummary { is_test, in_main }
    }

    #[test]
    fn skips_when_closure_is_safe() {
        assert!(!should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            false,
            false
        ));
    }

    #[test]
    fn flags_panicking_closure_in_production() {
        assert!(should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            true,
            false
        ));
    }

    #[test]
    fn flags_in_tests() {
        assert!(should_flag(
            &LintPolicy::new(false),
            &summary(true, false),
            true,
            false
        ));
    }

    #[test]
    fn skips_in_doctests() {
        assert!(!should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            true,
            true
        ));
    }

    #[test]
    fn respects_allow_in_main() {
        let policy = LintPolicy::new(true);
        assert!(!should_flag(&policy, &summary(false, true), true, false));
    }

    #[test]
    fn flags_main_when_not_allowed() {
        let policy = LintPolicy::new(false);
        assert!(should_flag(&policy, &summary(false, true), true, false));
    }
}
