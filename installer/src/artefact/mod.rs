//! Artefact naming, manifest schema, packaging, and verification policy.
//!
//! This module implements the type-safe domain model for prebuilt lint
//! library artefact distribution as specified in ADR-001, covering:
//!
//! - Roadmap 3.4.1: naming, manifest schema, and verification policy.
//! - Roadmap 3.4.2: packaging (archive creation and manifest emission).
//!
//! # Sub-modules
//!
//! - [`error`] — Semantic error types for validation failures.
//! - [`git_sha`] — Git commit SHA newtype (`GitSha`).
//! - [`manifest`] — Manifest schema types (`Manifest`, `GeneratedAt`).
//! - [`naming`] — Artefact archive naming policy (`ArtefactName`).
//! - [`packaging`] — Archive creation and manifest emission.
//! - [`packaging_error`] — Error types for packaging operations.
//! - [`schema_version`] — Manifest version newtype (`SchemaVersion`).
//! - [`sha256_digest`] — SHA-256 digest newtype (`Sha256Digest`).
//! - [`target`] — Target triple validation (`TargetTriple`).
//! - [`toolchain_channel`] — Toolchain channel newtype (`ToolchainChannel`).
//! - [`download`] — Artefact download trait and HTTP implementation.
//! - [`extraction`] — Archive extraction with path traversal protection.
//! - [`manifest_parser`] — Manifest JSON deserialization.
//! - [`verification`] — Verification policy and failure action types.

pub mod download;
pub mod error;
pub mod extraction;
pub mod git_sha;
pub mod manifest;
pub mod manifest_parser;
pub mod naming;
pub mod packaging;
pub mod packaging_error;
pub mod schema_version;
pub mod sha256_digest;
pub mod target;
pub mod toolchain_channel;
pub mod verification;
