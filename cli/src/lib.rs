//! Configuration boundary for the planned Whitaker application.

mod commands;
mod environment;
mod error;
mod legacy;
mod lint_config;
mod loader;
mod settings;
mod validated;

pub use legacy::source_fallback_override;
pub use lint_config::{
    BumpyRoad, BumpyWeights, ConditionalMaxBranches, ModuleMaxLines, NoStdFs, NoUnwrapOrElsePanic,
    RstestHelper, TestAttributes,
};
pub use loader::{load_with_sources, LoadedConfiguration, Sources};
pub use settings::{Configuration, Diagnostics, Install, Lint, Output};
pub use validated::ValidatedSettings;
