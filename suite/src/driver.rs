//! Combined lint wiring for the suite cdylib.

// Import constituent lint pass types required by `late_lint_methods!`.
use bumpy_road_function::BumpyRoadFunction;
use conditional_max_n_branches::ConditionalMaxNBranches;
use dylint_linting::dylint_library;
use function_attrs_follow_docs::FunctionAttrsFollowDocs;
use macro_aware_cognitive_complexity::MacroAwareCognitiveComplexity;
use module_max_lines::ModuleMaxLines;
use module_must_have_inner_docs::ModuleMustHaveInnerDocs;
use no_expect_outside_tests::NoExpectOutsideTests;
use no_std_fs_operations::NoStdFsOperations;
use no_unwrap_or_else_panic::NoUnwrapOrElsePanic;
use rstest_helper_should_be_fixture::RstestHelperShouldBeFixture;
use rustc_lint::{Lint, LintStore, LintVec, declare_combined_late_lint_pass};
use rustc_session::Session;
use string_continuation_style::StringContinuationStyle;
use test_must_not_have_example::TestMustNotHaveExample;
use whitaker_common::experimental::ExperimentalSelection;

use crate::{
    lints::{PassKind, SUITE_LINT_DECLS},
    opt_in::{OPT_IN_TABLE, OptInConfig, baseline_for, load},
};

dylint_library!();

// Only the always-on lints join the combined pass. `late_lint_methods!`
// expands its entry list verbatim, so an optional entry would need one call
// site per combination of experimental lints. Experimental late lints are
// registered as their own passes in `register_suite_lints` instead, which
// also lets a repository's `dylint.toml` decide at run time whether each one
// exists at all.
rustc_lint::late_lint_methods!(
    declare_combined_late_lint_pass,
    [SuitePass, [
        FunctionAttrsFollowDocs: function_attrs_follow_docs::FunctionAttrsFollowDocs::default(),
        NoExpectOutsideTests: no_expect_outside_tests::NoExpectOutsideTests::default(),
        TestMustNotHaveExample: test_must_not_have_example::TestMustNotHaveExample::default(),
        ModuleMustHaveInnerDocs: module_must_have_inner_docs::ModuleMustHaveInnerDocs::default(),
        ConditionalMaxNBranches: conditional_max_n_branches::ConditionalMaxNBranches::default(),
        ModuleMaxLines: module_max_lines::ModuleMaxLines::default(),
        NoUnwrapOrElsePanic: no_unwrap_or_else_panic::NoUnwrapOrElsePanic::default(),
        NoStdFsOperations: no_std_fs_operations::NoStdFsOperations::default(),
        BumpyRoadFunction: bumpy_road_function::BumpyRoadFunction::default(),
    ]]
);

/// Registers the suite lints into the provided lint store.
///
/// Every lint is *declared*, experimental or not, so a leftover
/// `#[allow(..)]` or `#[expect(..)]` naming an experimental lint never trips
/// `unknown_lints`. Only the experimental lints in `selection` get a pass, so
/// an unselected one cannot run or crash the compiler.
///
/// Callers should initialize configuration with
/// `dylint_linting::init_config` when integrating with the Dylint driver.
///
/// The string continuation early pass is registered *separately* from the
/// combined late pass. An early lint cannot join `late_lint_methods!`: that
/// macro declares `LateLintPass` methods. The consequence is that dropping the
/// `register_early_pass` line leaves the lint declared, passing the
/// descriptor-alignment tests, and never firing. The registration tests
/// assert which pass was wired for exactly that reason.
///
/// # Examples
///
/// ```ignore
/// # use rustc_lint::LintStore;
/// # use whitaker_common::experimental::ExperimentalSelection;
/// # use whitaker_suite::register_suite_lints;
/// let mut store = LintStore::new();
/// register_suite_lints(&mut store, &ExperimentalSelection::none());
/// assert_eq!(store.get_lints().len(), 12);
/// assert_eq!(store.late_passes.len(), 1);
/// ```
pub fn register_suite_lints(store: &mut LintStore, selection: &ExperimentalSelection) {
    store.register_lints(SUITE_LINT_DECLS);
    store.register_late_pass(|_| Box::new(SuitePass::new()));
    EXPERIMENTAL_PASSES
        .iter()
        .filter(|pass| selection.contains(pass.name))
        .for_each(|pass| (pass.register)(store));
}

/// One experimental lint's pass wiring: the name that opts it in, the phase
/// it runs in, and how to register its pass.
///
/// Keeping the three in one entry means the name a repository lists and the
/// pass it gets cannot drift apart, and the registration tests can register
/// each entry on its own and check what it adds.
pub struct ExperimentalPass {
    /// Canonical lint name, as listed under `[whitaker] experimental`.
    pub name: &'static str,
    /// Phase the registered pass runs in.
    pub kind: PassKind,
    /// Registers the pass into a lint store.
    pub register: fn(&mut LintStore),
}

/// Every experimental lint that has a pass of its own.
///
/// An entry's `name` must be in `whitaker_common::experimental::EXPERIMENTAL_LINTS`
/// and its `kind` must match the suite descriptor; a test asserts both.
pub const EXPERIMENTAL_PASSES: &[ExperimentalPass] = &[
    ExperimentalPass {
        name: "string_continuation_style",
        kind: PassKind::Early,
        register: register_string_continuation_style,
    },
    ExperimentalPass {
        name: "macro_aware_cognitive_complexity",
        kind: PassKind::Late,
        register: register_macro_aware_cognitive_complexity,
    },
    ExperimentalPass {
        name: "rstest_helper_should_be_fixture",
        kind: PassKind::Late,
        register: register_rstest_helper_should_be_fixture,
    },
];

/// Registers the string continuation early pass.
fn register_string_continuation_style(store: &mut LintStore) {
    store.register_early_pass(|| Box::new(StringContinuationStyle::default()));
}

/// Registers the macro-aware cognitive complexity late pass.
fn register_macro_aware_cognitive_complexity(store: &mut LintStore) {
    store.register_late_pass(|_| Box::new(MacroAwareCognitiveComplexity::default()));
}

/// Registers the rstest helper late pass.
fn register_rstest_helper_should_be_fixture(store: &mut LintStore) {
    store.register_late_pass(|_| Box::new(RstestHelperShouldBeFixture::default()));
}

/// Returns the lint declarations bundled into the suite.
///
/// # Examples
///
/// ```ignore
/// # use whitaker_suite::suite_lint_decls;
/// let names: Vec<_> = suite_lint_decls()
///     .iter()
///     .map(|lint| lint.name_lower())
///     .collect();
/// assert!(names.contains(&"no_unwrap_or_else_panic".to_string()));
/// ```
#[must_use]
pub const fn suite_lint_decls() -> &'static [&'static Lint] { SUITE_LINT_DECLS }

/// Reads the repository's opt-in, ending the compilation on a bad list.
///
/// A fatal error rather than a warning: a repository that lists an unknown
/// lint believes it is covered, so continuing silently would hide the gap.
fn load_selection(sess: &Session) -> ExperimentalSelection {
    let table = dylint_linting::config::<OptInConfig>(OPT_IN_TABLE);
    let baseline = baseline_for(cfg!(feature = "experimental-enable-all"));
    match load(table, &baseline) {
        Ok(loaded) => {
            for name in &loaded.redundant {
                sess.dcx().note(format!(
                    "`{name}` is enabled by default now; remove it from `[{OPT_IN_TABLE}] \
                     experimental` in dylint.toml"
                ));
            }
            loaded.selection
        }
        Err(error) => sess.dcx().fatal(error.to_string()),
    }
}

/// Initializes Dylint configuration then registers the suite lints.
fn register_suite_entry(sess: &Session, store: &mut LintStore) {
    dylint_linting::init_config(sess);
    register_suite_lints(store, &load_selection(sess));
}

whitaker_common::declare_dylint_register_entry!(register_suite_entry);
