//! Suite lint registry and shared metadata.

/// Minimal metadata describing an included lint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LintDescriptor {
    /// Canonical lint name used by the driver.
    pub name: &'static str,
    /// Crate that defines the lint.
    pub crate_name: &'static str,
}

/// Static list of the lints exposed by the Whitaker suite.
pub const SUITE_LINTS: &[LintDescriptor] = &[
    LintDescriptor {
        name: "function_attrs_follow_docs",
        crate_name: "function_attrs_follow_docs",
    },
    LintDescriptor {
        name: "no_expect_outside_tests",
        crate_name: "no_expect_outside_tests",
    },
    LintDescriptor {
        name: "module_must_have_inner_docs",
        crate_name: "module_must_have_inner_docs",
    },
    LintDescriptor {
        name: "conditional_max_n_branches",
        crate_name: "conditional_max_n_branches",
    },
    LintDescriptor {
        name: "module_max_lines",
        crate_name: "module_max_lines",
    },
    LintDescriptor {
        name: "no_unwrap_or_else_panic",
        crate_name: "no_unwrap_or_else_panic",
    },
    LintDescriptor {
        name: "no_std_fs_operations",
        crate_name: "no_std_fs_operations",
    },
];

#[cfg(feature = "dylint-driver")]
use rustc_lint::Lint;

/// Lint declarations derived from the suite membership.
#[cfg(feature = "dylint-driver")]
pub const SUITE_LINT_DECLS: &[&Lint] = &[
    function_attrs_follow_docs::FUNCTION_ATTRS_FOLLOW_DOCS,
    no_expect_outside_tests::NO_EXPECT_OUTSIDE_TESTS,
    module_must_have_inner_docs::MODULE_MUST_HAVE_INNER_DOCS,
    conditional_max_n_branches::CONDITIONAL_MAX_N_BRANCHES,
    module_max_lines::MODULE_MAX_LINES,
    no_unwrap_or_else_panic::NO_UNWRAP_OR_ELSE_PANIC,
    no_std_fs_operations::NO_STD_FS_OPERATIONS,
];

#[must_use = "Discarding the iterator hides suite wiring errors"]
pub fn suite_lint_names() -> impl Iterator<Item = &'static str> {
    SUITE_LINTS.iter().map(|descriptor| descriptor.name)
}
