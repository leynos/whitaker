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
// Re-export only the documented lint surface. `impl_late_lint!` also expands
// to the Dylint ABI entry point and lint-pass glue, which have no source
// location that could carry documentation; keeping them out of the public
// path satisfies `missing_docs` without suppressing it. The `no_mangle`
// symbol is still exported from the cdylib for standalone Dylint loading.
#[cfg(feature = "dylint-driver")]
pub use driver::{NO_STD_FS_OPERATIONS, NoStdFsOperations};
