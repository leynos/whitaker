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
