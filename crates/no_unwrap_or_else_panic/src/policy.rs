//! Pure lint policy evaluation logic shared by driver and behaviour tests.

use crate::context::ContextSummary;
use crate::panic_detector::PanicInfo;

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
/// In test contexts, `.unwrap_or_else(|| panic!(...))` is permitted when the
/// panic message interpolates a runtime value (i.e. uses a `_fmt` or
/// `_display` entry point). This allows test failures to include the actual
/// error payload for diagnostics whilst keeping production code strict.
///
/// # Examples
///
/// ```ignore
/// use no_unwrap_or_else_panic::policy::{should_flag, LintPolicy};
/// use no_unwrap_or_else_panic::context::ContextSummary;
/// use no_unwrap_or_else_panic::panic_detector::PanicInfo;
///
/// let policy = LintPolicy::new(false);
/// let summary = ContextSummary { is_test: false, in_main: false };
/// let info = PanicInfo { panics: true, uses_interpolation: false };
/// assert!(should_flag(&policy, &summary, &info, false));
/// ```
#[must_use]
pub(crate) fn should_flag(
    policy: &LintPolicy,
    summary: &ContextSummary,
    panic_info: &PanicInfo,
    is_doctest: bool,
) -> bool {
    if !panic_info.panics {
        return false;
    }

    if is_doctest {
        return false;
    }

    if summary.is_test && panic_info.uses_interpolation {
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

    fn panicking(uses_interpolation: bool) -> PanicInfo {
        PanicInfo {
            panics: true,
            uses_interpolation,
        }
    }

    const SAFE: PanicInfo = PanicInfo {
        panics: false,
        uses_interpolation: false,
    };

    #[test]
    fn skips_when_closure_is_safe() {
        assert!(!should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            &SAFE,
            false
        ));
    }

    #[test]
    fn flags_panicking_closure_in_production() {
        assert!(should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            &panicking(false),
            false
        ));
    }

    #[test]
    fn flags_plain_panic_in_tests() {
        assert!(should_flag(
            &LintPolicy::new(false),
            &summary(true, false),
            &panicking(false),
            false
        ));
    }

    #[test]
    fn skips_interpolated_panic_in_tests() {
        assert!(!should_flag(
            &LintPolicy::new(false),
            &summary(true, false),
            &panicking(true),
            false
        ));
    }

    #[test]
    fn flags_interpolated_panic_in_production() {
        assert!(should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            &panicking(true),
            false
        ));
    }

    #[test]
    fn skips_in_doctests() {
        assert!(!should_flag(
            &LintPolicy::new(false),
            &summary(false, false),
            &panicking(false),
            true
        ));
    }

    #[test]
    fn respects_allow_in_main() {
        let policy = LintPolicy::new(true);
        assert!(!should_flag(
            &policy,
            &summary(false, true),
            &panicking(false),
            false
        ));
    }

    #[test]
    fn flags_main_when_not_allowed() {
        let policy = LintPolicy::new(false);
        assert!(should_flag(
            &policy,
            &summary(false, true),
            &panicking(false),
            false
        ));
    }
}
