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

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{TEST_MUST_NOT_HAVE_EXAMPLE, TestMustNotHaveExample};

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
