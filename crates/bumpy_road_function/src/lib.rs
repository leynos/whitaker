//! Lint crate detecting "Bumpy Road" complexity clusters.
//!
//! The bumpy road detector models complexity as a per-line signal, smooths it,
//! then flags functions exhibiting two or more separated peaks. Consumers can
//! run the lint directly by loading this crate as a Dylint library or use it
//! via the aggregated suite, where it is included by default.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

pub mod analysis;

#[cfg(feature = "dylint-driver")]
mod driver;

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{BUMPY_ROAD_FUNCTION, BumpyRoadFunction};

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn bumpy_road_function_disabled_stub() {}
}
