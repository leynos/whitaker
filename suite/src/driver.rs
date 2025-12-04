//! Combined lint wiring for the suite cdylib.

use conditional_max_n_branches::ConditionalMaxNBranches;
use dylint_linting::dylint_library;
use function_attrs_follow_docs::FunctionAttrsFollowDocs;
use module_max_lines::ModuleMaxLines;
use module_must_have_inner_docs::ModuleMustHaveInnerDocs;
use no_expect_outside_tests::NoExpectOutsideTests;
use no_std_fs_operations::NoStdFsOperations;
use no_unwrap_or_else_panic::NoUnwrapOrElsePanic;
use rustc_lint::{Lint, LintStore, LintVec, declare_combined_late_lint_pass};
use rustc_session::Session;

dylint_library!();
rustc_lint::late_lint_methods!(
    declare_combined_late_lint_pass,
    [
        SuitePass,
        [
            FunctionAttrsFollowDocs: FunctionAttrsFollowDocs::default(),
            NoExpectOutsideTests: NoExpectOutsideTests::default(),
            ModuleMustHaveInnerDocs: ModuleMustHaveInnerDocs::default(),
            ConditionalMaxNBranches: ConditionalMaxNBranches::default(),
            ModuleMaxLines: ModuleMaxLines::default(),
            NoUnwrapOrElsePanic: NoUnwrapOrElsePanic::default(),
            NoStdFsOperations: NoStdFsOperations::default(),
        ]
    ]
);

const SUITE_LINT_DECLS: &[&Lint] = &[
    function_attrs_follow_docs::FUNCTION_ATTRS_FOLLOW_DOCS,
    no_expect_outside_tests::NO_EXPECT_OUTSIDE_TESTS,
    module_must_have_inner_docs::MODULE_MUST_HAVE_INNER_DOCS,
    conditional_max_n_branches::CONDITIONAL_MAX_N_BRANCHES,
    module_max_lines::MODULE_MAX_LINES,
    no_unwrap_or_else_panic::NO_UNWRAP_OR_ELSE_PANIC,
    no_std_fs_operations::NO_STD_FS_OPERATIONS,
];

/// Registers the suite lints into the provided lint store.
///
/// Callers should initialise configuration with
/// `dylint_linting::init_config` when integrating with the Dylint driver.
///
/// # Examples
///
/// ```
/// # use rustc_lint::LintStore;
/// # use suite::register_suite_lints;
/// let mut store = LintStore::new();
/// register_suite_lints(&mut store);
/// assert_eq!(store.get_lints().len(), 7);
/// ```
pub fn register_suite_lints(store: &mut LintStore) {
    store.register_lints(SUITE_LINT_DECLS);
    store.register_late_pass(|_| Box::new(SuitePass::new()));
}

/// Returns the lint declarations bundled into the suite.
///
/// # Examples
///
/// ```
/// # use suite::suite_lint_decls;
/// let names: Vec<_> = suite_lint_decls()
///     .iter()
///     .map(|lint| lint.name_lower())
///     .collect();
/// assert!(names.contains(&"no_unwrap_or_else_panic".to_string()));
/// ```
#[must_use]
pub fn suite_lint_decls() -> &'static [&'static Lint] {
    SUITE_LINT_DECLS
}

/// Dylint entrypoint that initialises configuration and registers lints.
#[unsafe(no_mangle)]
pub extern "C" fn register_lints(sess: &Session, store: &mut LintStore) {
    dylint_linting::init_config(sess);
    register_suite_lints(store);
}
