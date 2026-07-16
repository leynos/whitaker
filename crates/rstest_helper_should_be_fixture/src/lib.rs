//! Dylint crate for collecting `rstest` helper-call evidence.
//!
//! The driver recognizes strict `#[rstest]` bodies and delegates HIR traversal
//! to the visitor, which lowers helper-call arguments into the collector's
//! deterministic per-callee records. Collection is diagnostic-silent: later
//! roadmap items evaluate thresholds and emit user-facing guidance.
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
