//! Lint crate for forbidding `.expect(...)` outside of test-only code paths.
//!
//! This crate exists to catch production `Option` and `Result` expectations
//! while still allowing explicit expectations in unit tests, doctests, and
//! recognized test frameworks. When the `dylint-driver` feature is disabled,
//! the crate retains only a tiny internal stub so the package still builds
//! cleanly in non-driver configurations.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(all(feature = "dylint-driver", test))]
mod behaviour;
#[cfg(feature = "dylint-driver")]
mod context;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(all(feature = "dylint-driver", test))]
mod lib_ui_tests;
#[cfg(all(feature = "dylint-driver", test))]
mod tests;
#[cfg(all(feature = "dylint-driver", test))]
mod ui {
    whitaker_lint_core::declare_ui_tests!("ui");
}

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{NO_EXPECT_OUTSIDE_TESTS, NoExpectOutsideTests};

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(
        dead_code,
        reason = "non-driver builds keep a tiny stub so the crate still compiles cleanly"
    )]
    pub fn no_expect_outside_tests_disabled_stub() {}
}
