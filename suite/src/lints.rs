//! Suite lint registry and shared metadata.

/// Minimal metadata describing an included lint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LintDescriptor {
    /// Canonical lint name used by the driver.
    pub name: &'static str,
    /// Crate that defines the lint.
    pub crate_name: &'static str,
}

/// Single source of truth for all suite lints. The callback macro receives a
/// comma-separated list of `(lint_mod, crate_mod, pass_ty, lint_const)` tuples.
macro_rules! suite_lints_with {
    ($callback:ident) => {
        $callback!(
            (
                function_attrs_follow_docs,
                function_attrs_follow_docs,
                FunctionAttrsFollowDocs,
                FUNCTION_ATTRS_FOLLOW_DOCS
            ),
            (
                no_expect_outside_tests,
                no_expect_outside_tests,
                NoExpectOutsideTests,
                NO_EXPECT_OUTSIDE_TESTS
            ),
            (
                module_must_have_inner_docs,
                module_must_have_inner_docs,
                ModuleMustHaveInnerDocs,
                MODULE_MUST_HAVE_INNER_DOCS
            ),
            (
                conditional_max_n_branches,
                conditional_max_n_branches,
                ConditionalMaxNBranches,
                CONDITIONAL_MAX_N_BRANCHES
            ),
            (
                module_max_lines,
                module_max_lines,
                ModuleMaxLines,
                MODULE_MAX_LINES
            ),
            (
                no_unwrap_or_else_panic,
                no_unwrap_or_else_panic,
                NoUnwrapOrElsePanic,
                NO_UNWRAP_OR_ELSE_PANIC
            ),
            (
                no_std_fs_operations,
                no_std_fs_operations,
                NoStdFsOperations,
                NO_STD_FS_OPERATIONS
            )
        )
    };
}

pub(crate) use suite_lints_with;

macro_rules! make_descriptor_array {
    ($(($lint_mod:ident, $crate_mod:ident, $pass_ty:ident, $lint_const:ident)),+ $(,)?) => {
        [
            $(LintDescriptor {
                name: stringify!($lint_mod),
                crate_name: stringify!($crate_mod),
            },)+
        ]
    };
}

/// Static list of the lints exposed by the Whitaker suite.
pub const SUITE_LINTS: &[LintDescriptor] = &suite_lints_with!(make_descriptor_array);

#[cfg(feature = "dylint-driver")]
use rustc_lint::Lint;

#[cfg(feature = "dylint-driver")]
macro_rules! make_decl_array {
    ($(($lint_mod:ident, $crate_mod:ident, $pass_ty:ident, $lint_const:ident)),+ $(,)?) => {
        [
            $( $crate_mod::$lint_const, )+
        ]
    };
}

/// Lint declarations derived from the suite membership.
#[cfg(feature = "dylint-driver")]
pub const SUITE_LINT_DECLS: &[&'static Lint] = &suite_lints_with!(make_decl_array);

#[must_use = "Discarding the iterator hides suite wiring errors"]
pub fn suite_lint_names() -> impl Iterator<Item = &'static str> {
    SUITE_LINTS.iter().map(|descriptor| descriptor.name)
}
