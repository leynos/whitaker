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

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn function_attrs_follow_docs_disabled_stub() {}
}
