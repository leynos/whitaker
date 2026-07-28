//! Dylint crate implementing the `no_std_fs_operations` lint, which is only
//! available when compiled with the `dylint-driver` feature enabled.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(all(feature = "dylint-driver", test))]
mod behaviour;
#[cfg(feature = "dylint-driver")]
mod config;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(feature = "dylint-driver")]
mod exclusion;
#[cfg(all(feature = "dylint-driver", test))]
mod exclusion_behaviour;
#[cfg(all(feature = "dylint-driver", test))]
mod tests;
#[cfg(feature = "dylint-driver")]
mod usage;
#[cfg(feature = "dylint-driver")]
pub use config::NoStdFsConfig;
#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    //! Placeholder compiled when the `dylint-driver` feature is off, so the
    //! crate still builds without the `rustc_private` toolchain internals.

    #[expect(
        dead_code,
        reason = "Exposed only when built without the `dylint-driver` feature"
    )]
    pub fn no_std_fs_operations_disabled_stub() {}
}
