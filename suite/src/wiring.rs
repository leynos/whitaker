//! Descriptors for the lints bundled by the suite.

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

/// Returns the names of the lints bundled into the suite.
///
/// # Examples
///
/// ```
/// use suite::suite_lint_names;
///
/// let names: Vec<_> = suite_lint_names().collect();
/// assert!(names.contains(&"module_max_lines"));
/// assert_eq!(names.len(), 7);
/// ```
#[must_use = "Discarding the iterator hides suite wiring errors"]
pub fn suite_lint_names() -> impl Iterator<Item = &'static str> {
    SUITE_LINTS.iter().map(|descriptor| descriptor.name)
}
