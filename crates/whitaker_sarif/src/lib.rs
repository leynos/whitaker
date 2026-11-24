//! SARIF 2.1.0 models, builders, and merge logic for Whitaker clone detection.
//!
//! This crate provides the data model and construction utilities for Static
//! Analysis Results Interchange Format (SARIF) 2.1.0 documents used by the
//! Whitaker clone detection pipeline. It includes:
//!
//! - **Model types** representing the SARIF 2.1.0 schema subset used by Whitaker (logs, runs,
//!   results, locations, regions, rules).
//! - **Fluent builders** for ergonomic construction of SARIF objects.
//! - **Rule definitions** for the three clone types (WHK001–WHK003).
//! - **Whitaker properties** extension for attaching similarity metadata.
//! - **Merge and deduplication** logic for combining detection pass outputs.
//! - **Path helpers** for the stable `target/whitaker/` file layout.

pub mod baseline_state;
pub mod builders;
pub mod error;
pub mod merge;
pub mod model;
pub mod paths;
pub mod refinement_kind;
pub mod rules;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tools;
pub mod whitaker_properties;

// Baseline state vocabulary
pub use baseline_state::BaselineState;
// Error types
// Builders
pub use builders::{LocationBuilder, RegionBuilder, ResultBuilder, RunBuilder, SarifLogBuilder};
pub use error::{Result, SarifError};
// Merge logic
pub use merge::{
    AST_HASH_KEY,
    TOKEN_HASH_KEY,
    WHITAKER_FRAGMENT_KEY,
    deduplicate_results,
    merge_runs,
};
// Model types
pub use model::{
    Artefact,
    ArtefactLocation,
    Invocation,
    Level,
    Location,
    Message,
    MultiformatMessageString,
    PhysicalLocation,
    Region,
    RelatedLocation,
    ReportingDescriptor,
    Run,
    SarifLog,
    SarifResult,
    Tool,
    ToolComponent,
};
// Path helpers
pub use paths::{
    AST_PASS_FILENAME,
    REFINED_FILENAME,
    TOKEN_PASS_FILENAME,
    WHITAKER_DIR,
    ast_pass_path,
    refined_path,
    token_pass_path,
    whitaker_dir,
};
// Refinement provenance
pub use refinement_kind::RefinementKind;
// Rules
pub use rules::{
    WHK001_ID,
    WHK002_ID,
    WHK003_ID,
    all_rules,
    whk001_rule,
    whk002_rule,
    whk003_rule,
};
// Producer names
pub use tools::{AST_PASS_TOOL_NAME, TOKEN_PASS_TOOL_NAME};
// Whitaker properties extension
pub use whitaker_properties::{WhitakerProperties, WhitakerPropertiesBuilder};
