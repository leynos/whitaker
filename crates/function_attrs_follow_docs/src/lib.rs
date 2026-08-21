//! Lint crate enforcing that doc comments precede outer attributes on
//! functions.
//!
//! The `driver` module holds the lint pass and the ordering logic,
//! including the recovery of user-written spans from parsed
//! `AttributeKind` variants and the item-boundary check that tolerates
//! outer attributes sitting immediately before the item span. Unit and
//! behavioural tests live alongside the driver under `tests`. When the
//! `dylint-driver` feature is disabled, the crate retains only a tiny
//! internal stub so the package still builds cleanly in non-driver
//! configurations.

#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

// `rustc_hir` attribute structures store feature lists in `ThinVec`, which is a
// `rustc_private` crate rather than a Cargo dependency. The unit tests need to
// name the type when constructing attribute fixtures.
#[cfg(all(test, feature = "dylint-driver"))]
extern crate thin_vec;

#[cfg(feature = "dylint-driver")]
mod driver;

// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{FUNCTION_ATTRS_FOLLOW_DOCS, FunctionAttrsFollowDocs};

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn function_attrs_follow_docs_disabled_stub() {}
}
