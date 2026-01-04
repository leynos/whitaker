//! Whitaker installer library.
//!
//! This crate provides the core functionality for building, linking, and staging
//! Dylint lint libraries. It is used by the `whitaker-installer` CLI binary and
//! can be consumed programmatically for testing or custom installation workflows.
//!
//! # Modules
//!
//! - [`builder`] - Cargo build orchestration for lint crates
//! - [`error`] - Semantic error types with recovery hints
//! - [`output`] - Shell snippet generation for environment configuration
//! - [`stager`] - File staging with platform-specific naming conventions
//! - [`toolchain`] - Rust toolchain detection and validation

pub mod builder;
pub mod error;
pub mod output;
pub mod stager;
pub mod toolchain;
