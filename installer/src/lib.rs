//! Whitaker installer library.
//!
//! This crate provides the core functionality for building, linking, and staging
//! Dylint lint libraries. It is used by the `whitaker-installer` CLI binary and
//! can be consumed programmatically for testing or custom installation workflows.
//!
//! # Modules
//!
//! - [`artefact`] - Artefact naming, manifest schema, and verification policy
//! - [`builder`] - Cargo build orchestration for lint crates
//! - [`cli`] - Command-line argument definitions
//! - [`crate_name`] - Semantic wrapper for lint crate names
//! - [`deps`] - Dylint tool dependency management
//! - [`dirs`] - Directory resolution abstraction for platform-specific paths
//! - [`error`] - Semantic error types with recovery hints
//! - [`git`] - Repository cloning and updating
//! - [`list`] - List command implementation
//! - [`list_output`] - Output formatting for lint listing
//! - [`output`] - Shell snippet generation for environment configuration
//! - [`pipeline`] - Build and staging pipeline orchestration
//! - [`resolution`] - Crate resolution and validation
//! - [`scanner`] - Lint scanner for discovering installed libraries
//! - [`stager`] - File staging with platform-specific naming conventions
//! - [`toolchain`] - Rust toolchain detection and validation
//! - [`workspace`] - Workspace detection and path resolution
//! - [`prebuilt`] - Prebuilt artefact download and verification orchestrator
//! - [`wrapper`] - Wrapper script generation

pub mod artefact;
pub mod builder;
pub mod cli;
pub mod crate_name;
pub mod deps;
pub mod dirs;
pub mod error;
pub mod git;
pub mod list;
pub mod list_output;
pub mod output;
pub mod pipeline;
pub mod prebuilt;
pub mod resolution;
pub mod scanner;
pub mod stager;
pub mod toolchain;
pub mod workspace;
pub mod wrapper;

/// Test utilities for stubbing command execution in integration tests.
///
/// This module is public to allow the binary crate's tests to import it,
/// but hidden from generated documentation as it is not part of the public API.
#[doc(hidden)]
pub mod test_utils;
