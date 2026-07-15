//! Experimental lint crate for `rstest` helper fixture extraction.
//!
//! This crate declares the `rstest_helper_should_be_fixture` Dylint lint and
//! wires its configuration defaults. The first implementation intentionally
//! stops before helper-call collection and diagnostic emission; later roadmap
//! items add the analysis that decides when a repeated helper call should
//! become an `rstest` fixture.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod collector;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod visitor;

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    //! Disabled-driver stub module.
    //!
    //! This module keeps non-driver builds valid when the `dylint-driver`
    //! feature is off. It exposes
    //! `rstest_helper_should_be_fixture_disabled_stub` as the inert public
    //! symbol for that build mode.

    #[expect(
        dead_code,
        reason = "stub used when the driver feature is disabled; tracked in https://github.com/leynos/whitaker/issues/233"
    )]
    pub fn rstest_helper_should_be_fixture_disabled_stub() {}
}
