//! Dylint crate implementing the `test_must_not_have_example` lint.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(all(feature = "dylint-driver", test))]
mod behaviour;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod heuristics;
#[cfg(all(feature = "dylint-driver", test))]
#[path = "lib_ui_tests.rs"]
mod ui;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    //! Stub exports used when the lint driver feature is disabled.
    //!
    //! These no-op symbols keep the crate linkable in environments that do not
    //! compile the `rustc_private` driver implementation.

    /// No-op placeholder exposed when `dylint-driver` is disabled.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// test_must_not_have_example_disabled_stub();
    /// // Outcome: this call has no side effects and returns unit.
    /// ```
    #[expect(
        dead_code,
        reason = "Exposed only when built without the `dylint-driver` feature"
    )]
    pub fn test_must_not_have_example_disabled_stub() {}
}
