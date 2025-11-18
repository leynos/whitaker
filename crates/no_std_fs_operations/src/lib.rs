#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(all(feature = "dylint-driver", test))]
mod behaviour;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(all(feature = "dylint-driver", test))]
mod tests;
#[cfg(feature = "dylint-driver")]
mod usage;
#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[allow(dead_code)]
    pub fn no_std_fs_operations_disabled_stub() {}
}
