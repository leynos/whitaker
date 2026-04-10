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
/// panic message interpolates a runtime value (i.e. constructs runtime-formatted
/// arguments via `fmt::Arguments::new_v1` or `fmt::Arguments::new_v1_formatted`).
/// This allows test failures to include the actual error payload for diagnostics
/// whilst keeping production code strict.
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
/// let info = PanicInfo {
///     panics: true,
///     has_plain_panic: true,
///     has_interpolated_panic: false,
/// };
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

    if summary.is_test && panic_info.has_interpolated_panic && !panic_info.has_plain_panic {
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
    use rstest::rstest;

    #[derive(Clone, Copy, Debug)]
    struct PolicyCase {
        policy: LintPolicy,
        context: ContextSummary,
        panic_info: PanicInfo,
        is_doctest: bool,
        should_flag: bool,
    }

    const SAFE: PanicInfo = PanicInfo {
        panics: false,
        has_plain_panic: false,
        has_interpolated_panic: false,
    };

    const DEFAULT_POLICY: LintPolicy = LintPolicy::new(false);

    #[rstest]
    #[case::safe_closure(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: false, in_main: false },
        panic_info: SAFE,
        is_doctest: false,
        should_flag: false,
    })]
    #[case::panicking_in_production(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: false, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: false },
        is_doctest: false,
        should_flag: true,
    })]
    #[case::plain_panic_in_tests(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: true, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: false },
        is_doctest: false,
        should_flag: true,
    })]
    #[case::interpolated_panic_in_tests(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: true, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: false, has_interpolated_panic: true },
        is_doctest: false,
        should_flag: false,
    })]
    #[case::interpolated_panic_in_production(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: false, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: false, has_interpolated_panic: true },
        is_doctest: false,
        should_flag: true,
    })]
    #[case::skips_in_doctests(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: false, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: false },
        is_doctest: true,
        should_flag: false,
    })]
    #[case::respects_allow_in_main(PolicyCase {
        policy: LintPolicy::new(true),
        context: ContextSummary { is_test: false, in_main: true },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: false },
        is_doctest: false,
        should_flag: false,
    })]
    #[case::flags_main_when_not_allowed(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: false, in_main: true },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: false },
        is_doctest: false,
        should_flag: true,
    })]
    #[case::mixed_plain_and_interpolated_in_tests(PolicyCase {
        policy: DEFAULT_POLICY,
        context: ContextSummary { is_test: true, in_main: false },
        panic_info: PanicInfo { panics: true, has_plain_panic: true, has_interpolated_panic: true },
        is_doctest: false,
        should_flag: true,
    })]
    fn policy_evaluation(#[case] case: PolicyCase) {
        assert_eq!(
            should_flag(
                &case.policy,
                &case.context,
                &case.panic_info,
                case.is_doctest
            ),
            case.should_flag
        );
    }
}
