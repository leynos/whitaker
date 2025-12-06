//! Combined lint wiring for the suite cdylib.

use crate::lints::{SUITE_LINT_DECLS, suite_lints_with};
use dylint_linting::dylint_library;
use rustc_lint::{Lint, LintStore, LintVec, declare_combined_late_lint_pass};
use rustc_session::Session;

dylint_library!();

macro_rules! suite_pass_entry {
    ($lint_mod:ident, $crate_mod:ident, $pass_ty:ident, $lint_const:ident) => {
        $pass_ty: $crate_mod::$pass_ty::default(),
    };
}

macro_rules! build_suite_pass {
    ($(($lint_mod:ident, $crate_mod:ident, $pass_ty:ident, $lint_const:ident)),+ $(,)?) => {
        rustc_lint::late_lint_methods!(
            declare_combined_late_lint_pass,
            [
                SuitePass,
                [$( $pass_ty: $crate_mod::$pass_ty::default(), )+]
            ]
        );
    };
}

suite_lints_with!(build_suite_pass);

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
#[no_mangle]
pub unsafe extern "C" fn register_lints(sess: &Session, store: &mut LintStore) {
    dylint_linting::init_config(sess);
    register_suite_lints(store);
}
