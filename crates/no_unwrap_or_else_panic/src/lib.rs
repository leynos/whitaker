//! Lint crate discouraging `unwrap_or_else(|| panic!(..))` in production code.
//!
//! The lint inspects `unwrap_or_else` invocations on `Option` and `Result`
//! receivers, flagging closures that panic directly or indirectly through
//! `unwrap` or `expect`. Doctest contexts are exempt, and teams may optionally
//! allow panicking fallbacks inside `main` via configuration.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
pub(crate) const LINT_NAME: &str = "no_unwrap_or_else_panic";

#[cfg(feature = "dylint-driver")]
mod context;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod panic_detector;
#[cfg(feature = "dylint-driver")]
mod policy;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub used when the driver feature is disabled")]
    pub fn no_unwrap_or_else_panic_disabled_stub() {}
}

#[cfg(all(test, feature = "dylint-driver"))]
mod tests;

#[cfg(all(test, feature = "dylint-driver"))]
#[path = "lib_ui_tests.rs"]
mod ui;
