//! Fluent builders for SARIF 2.1.0 objects.
//!
//! Each builder follows the same pattern: create with `::new(...)`, chain
//! optional setters, and finalize with `.build()`. Builders that require
//! fields return [`crate::error::Result`]; builders where all fields have
//! sensible defaults return the type directly.

pub mod location_builder;
pub mod log_builder;
pub mod result_builder;
pub mod run_builder;

pub use location_builder::{LocationBuilder, RegionBuilder};
pub use log_builder::SarifLogBuilder;
pub use result_builder::ResultBuilder;
pub use run_builder::RunBuilder;
