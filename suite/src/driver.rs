//! Combined lint wiring for the suite cdylib.

use crate::lints::SUITE_LINT_DECLS;
use dylint_linting::dylint_library;
use rustc_lint::{Lint, LintStore, LintVec, declare_combined_late_lint_pass};
use rustc_session::Session;

// Import constituent lint pass types required by `late_lint_methods!`.
use conditional_max_n_branches::ConditionalMaxNBranches;
use function_attrs_follow_docs::FunctionAttrsFollowDocs;
use module_max_lines::ModuleMaxLines;
use module_must_have_inner_docs::ModuleMustHaveInnerDocs;
use no_expect_outside_tests::NoExpectOutsideTests;
use no_std_fs_operations::NoStdFsOperations;
use no_unwrap_or_else_panic::NoUnwrapOrElsePanic;

dylint_library!();

rustc_lint::late_lint_methods!(
    declare_combined_late_lint_pass,
    [SuitePass, [
        FunctionAttrsFollowDocs: function_attrs_follow_docs::FunctionAttrsFollowDocs::default(),
        NoExpectOutsideTests: no_expect_outside_tests::NoExpectOutsideTests::default(),
        ModuleMustHaveInnerDocs: module_must_have_inner_docs::ModuleMustHaveInnerDocs::default(),
        ConditionalMaxNBranches: conditional_max_n_branches::ConditionalMaxNBranches::default(),
        ModuleMaxLines: module_max_lines::ModuleMaxLines::default(),
        NoUnwrapOrElsePanic: no_unwrap_or_else_panic::NoUnwrapOrElsePanic::default(),
        NoStdFsOperations: no_std_fs_operations::NoStdFsOperations::default(),
    ]]
);

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
///
/// Safety: callers must pass non-null, correctly initialised `Session` and
/// `LintStore` references from the host compiler context that remain valid on
/// this thread for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn register_lints(sess: &Session, store: &mut LintStore) {
    dylint_linting::init_config(sess);
    register_suite_lints(store);
}
