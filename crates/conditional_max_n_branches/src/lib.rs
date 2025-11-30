//! Conditional complexity lint enforcing a maximum number of predicate branches.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn conditional_max_n_branches_disabled_stub() {}
}

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "lib_ui_tests.rs"]
mod ui;
