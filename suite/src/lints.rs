//! Suite lint registry and shared metadata.

/// Which rustc lint-pass phase a lint's checks run in.
///
/// The distinction is not cosmetic. Late passes are combined into one
/// `SuitePass` by `late_lint_methods!`, so omitting a late lint from that macro
/// fails to compile. An early pass is registered by its own
/// `register_early_pass` call, and deleting that call compiles cleanly: the
/// lint stays declared, still appears in the descriptor lists, and never runs.
/// Recording the phase here lets the registration tests assert *which* pass was
/// registered instead of counting how many there are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassKind {
    /// Runs on rustc's parsed AST before macro expansion, via
    /// `register_pre_expansion_pass`.
    PreExpansion,
    /// Runs on rustc's expanded AST via `register_early_pass`.
    Early,
    /// Runs on HIR via the combined late pass.
    Late,
}

/// Minimal metadata describing an included lint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LintDescriptor {
    /// Canonical lint name used by the driver.
    pub name: &'static str,
    /// Crate that defines the lint.
    pub crate_name: &'static str,
    /// Phase whose `check_*` hooks this lint's diagnostics come from.
    pub pass: PassKind,
}

impl LintDescriptor {
    /// Reports whether the lint is experimental, so it runs only when a
    /// repository opts in. Derived from `whitaker_common::experimental` so the
    /// stage has one source of truth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use whitaker_suite::SUITE_LINTS;
    /// let experimental = SUITE_LINTS
    ///     .iter()
    ///     .filter(|lint| lint.is_experimental())
    ///     .count();
    /// assert_eq!(experimental, 4);
    /// ```
    #[must_use]
    pub fn is_experimental(&self) -> bool {
        whitaker_common::experimental::EXPERIMENTAL_LINTS.contains(&self.name)
    }
}

/// Static list of the lints exposed by the Whitaker suite.
pub const SUITE_LINTS: &[LintDescriptor] = &[
    LintDescriptor {
        name: "function_attrs_follow_docs",
        crate_name: "function_attrs_follow_docs",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "no_expect_outside_tests",
        crate_name: "no_expect_outside_tests",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "test_must_not_have_example",
        crate_name: "test_must_not_have_example",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "module_must_have_inner_docs",
        crate_name: "module_must_have_inner_docs",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "conditional_max_n_branches",
        crate_name: "conditional_max_n_branches",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "module_max_lines",
        crate_name: "module_max_lines",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "no_unwrap_or_else_panic",
        crate_name: "no_unwrap_or_else_panic",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "no_std_fs_operations",
        crate_name: "no_std_fs_operations",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "bumpy_road_function",
        crate_name: "bumpy_road_function",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "string_continuation_style",
        crate_name: "string_continuation_style",
        pass: PassKind::Early,
    },
    LintDescriptor {
        name: "macro_aware_cognitive_complexity",
        crate_name: "macro_aware_cognitive_complexity",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "rstest_helper_should_be_fixture",
        crate_name: "rstest_helper_should_be_fixture",
        pass: PassKind::Late,
    },
    LintDescriptor {
        name: "missing_docs_in_test_functions",
        crate_name: "missing_docs_in_test_functions",
        pass: PassKind::PreExpansion,
    },
];

#[cfg(feature = "dylint-driver")]
use rustc_lint::Lint;

/// Lint declarations derived from the suite membership.
#[cfg(feature = "dylint-driver")]
pub const SUITE_LINT_DECLS: &[&Lint] = &[
    function_attrs_follow_docs::FUNCTION_ATTRS_FOLLOW_DOCS,
    no_expect_outside_tests::NO_EXPECT_OUTSIDE_TESTS,
    test_must_not_have_example::TEST_MUST_NOT_HAVE_EXAMPLE,
    module_must_have_inner_docs::MODULE_MUST_HAVE_INNER_DOCS,
    conditional_max_n_branches::CONDITIONAL_MAX_N_BRANCHES,
    module_max_lines::MODULE_MAX_LINES,
    no_unwrap_or_else_panic::NO_UNWRAP_OR_ELSE_PANIC,
    no_std_fs_operations::NO_STD_FS_OPERATIONS,
    bumpy_road_function::BUMPY_ROAD_FUNCTION,
    string_continuation_style::STRING_CONTINUATION_STYLE,
    macro_aware_cognitive_complexity::MACRO_AWARE_COGNITIVE_COMPLEXITY,
    rstest_helper_should_be_fixture::RSTEST_HELPER_SHOULD_BE_FIXTURE,
    missing_docs_in_test_functions::MISSING_DOCS_IN_TEST_FUNCTIONS,
];

/// Returns an iterator over the canonical lint names in suite order.
///
/// # Examples
///
/// ```
/// # use whitaker_suite::suite_lint_names;
/// let names: Vec<_> = suite_lint_names().collect();
/// for expected in [
///     "function_attrs_follow_docs",
///     "no_expect_outside_tests",
///     "test_must_not_have_example",
///     "module_must_have_inner_docs",
///     "conditional_max_n_branches",
///     "module_max_lines",
///     "no_unwrap_or_else_panic",
///     "no_std_fs_operations",
///     "bumpy_road_function",
///     // Experimental lints are always declared; opt-in decides whether they run.
///     "string_continuation_style",
///     "rstest_helper_should_be_fixture",
///     "macro_aware_cognitive_complexity",
///     "missing_docs_in_test_functions",
/// ] {
///     assert!(names.contains(&expected));
/// }
/// ```
#[must_use = "Discarding the iterator hides suite wiring errors"]
pub fn suite_lint_names() -> impl Iterator<Item = &'static str> {
    SUITE_LINTS.iter().map(|descriptor| descriptor.name)
}

/// Returns the canonical names of the lints whose checks run in `pass`.
///
/// Registration tests use this to assert *which* pass was wired, rather than
/// how many. A count alone would still pass if the wrong pass were registered.
///
/// # Examples
///
/// ```
/// # use whitaker_suite::{PassKind, suite_lint_names_for_pass};
/// let early: Vec<_> = suite_lint_names_for_pass(PassKind::Early).collect();
/// assert_eq!(early, ["string_continuation_style"]);
///
/// let pre_expansion: Vec<_> = suite_lint_names_for_pass(PassKind::PreExpansion).collect();
/// assert_eq!(pre_expansion, ["missing_docs_in_test_functions"]);
///
/// let late: Vec<_> = suite_lint_names_for_pass(PassKind::Late).collect();
/// assert!(late.contains(&"macro_aware_cognitive_complexity"));
/// assert!(late.contains(&"rstest_helper_should_be_fixture"));
/// ```
#[must_use = "Discarding the iterator hides suite wiring errors"]
pub fn suite_lint_names_for_pass(pass: PassKind) -> impl Iterator<Item = &'static str> {
    SUITE_LINTS
        .iter()
        .filter(move |descriptor| descriptor.pass == pass)
        .map(|descriptor| descriptor.name)
}
