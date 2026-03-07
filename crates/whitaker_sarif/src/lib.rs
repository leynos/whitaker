//! SARIF 2.1.0 models, builders, and merge logic for Whitaker clone detection.
//!
//! This crate provides the data model and construction utilities for Static
//! Analysis Results Interchange Format (SARIF) 2.1.0 documents used by the
//! Whitaker clone detection pipeline. It includes:
//!
//! - **Model types** representing the SARIF 2.1.0 schema subset used by
//!   Whitaker (logs, runs, results, locations, regions, rules).
//! - **Fluent builders** for ergonomic construction of SARIF objects.
//! - **Rule definitions** for the three clone types (WHK001–WHK003).
//! - **Whitaker properties** extension for attaching similarity metadata.
//! - **Merge and deduplication** logic for combining detection pass outputs.
//! - **Path helpers** for the stable `target/whitaker/` file layout.

pub mod builders;
pub mod error;
pub mod merge;
pub mod model;
pub mod paths;
pub mod rules;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod whitaker_properties;

// Error types
pub use error::{Result, SarifError};

// Model types
pub use model::{
    Artifact, ArtifactLocation, Invocation, Level, Location, Message, MultiformatMessageString,
    PhysicalLocation, Region, RelatedLocation, ReportingDescriptor, Run, SarifLog, SarifResult,
    Tool, ToolComponent,
};

// Builders
pub use builders::{LocationBuilder, RegionBuilder, ResultBuilder, RunBuilder, SarifLogBuilder};

// Rules
pub use rules::{
    WHK001_ID, WHK002_ID, WHK003_ID, all_rules, whk001_rule, whk002_rule, whk003_rule,
};

// Whitaker properties extension
pub use whitaker_properties::{WhitakerProperties, WhitakerPropertiesBuilder};

// Merge logic
pub use merge::{WHITAKER_FRAGMENT_KEY, deduplicate_results, merge_runs};

// Path helpers
pub use paths::{
    AST_PASS_FILENAME, REFINED_FILENAME, TOKEN_PASS_FILENAME, WHITAKER_DIR, ast_pass_path,
    refined_path, token_pass_path, whitaker_dir,
};
