//! Dylint lint that flags modules exceeding the configured line budget.
//!
//! The lint drives contributors toward smaller, reviewable modules; the
//! `dylint-driver` feature gates the rustc-facing implementation so the crate
//! also builds as an ordinary library.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod driver;

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{MODULE_MAX_LINES, ModuleMaxLines};

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn module_max_lines_disabled_stub() {}
}
