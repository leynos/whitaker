//! Conditional complexity lint enforcing a maximum number of predicate branches.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod driver;

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{CONDITIONAL_MAX_N_BRANCHES, ConditionalMaxNBranches};

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn conditional_max_n_branches_disabled_stub() {}
}

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "lib_ui_tests.rs"]
mod ui;
