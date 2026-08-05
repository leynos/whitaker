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
