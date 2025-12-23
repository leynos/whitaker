//! Experimental lint crate detecting "Bumpy Road" complexity clusters.
//!
//! The bumpy road detector models complexity as a per-line signal, smooths it,
//! then flags functions exhibiting two or more separated peaks. Consumers can
//! run the lint directly by loading this crate as a Dylint library or opt into
//! it via the aggregated suite's experimental feature.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

pub mod analysis;

#[cfg(feature = "dylint-driver")]
mod driver;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[expect(dead_code, reason = "stub when dylint-driver is disabled")]
    pub fn bumpy_road_function_disabled_stub() {}
}
