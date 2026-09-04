//! Dylint lint that flags modules exceeding the configured line budget.
//!
//! The lint drives contributors toward smaller, reviewable modules; the
//! `dylint-driver` feature gates the rustc-facing implementation so the crate
//! also builds as an ordinary library.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn module_max_lines_disabled_stub() {}
}
