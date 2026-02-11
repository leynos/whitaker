//! Artefact naming, manifest schema, and verification policy.
//!
//! This module implements roadmap item 3.4.1: the type-safe domain model
//! for prebuilt lint library artefact distribution as specified in ADR-001.
//!
//! # Sub-modules
//!
//! - [`error`] — Semantic error types for validation failures.
//! - [`git_sha`] — Git commit SHA newtype (`GitSha`).
//! - [`manifest`] — Manifest schema types (`Manifest`, `GeneratedAt`).
//! - [`naming`] — Artefact archive naming policy (`ArtefactName`).
//! - [`schema_version`] — Manifest version newtype (`SchemaVersion`).
//! - [`sha256_digest`] — SHA-256 digest newtype (`Sha256Digest`).
//! - [`target`] — Target triple validation (`TargetTriple`).
//! - [`toolchain_channel`] — Toolchain channel newtype (`ToolchainChannel`).
//! - [`verification`] — Verification policy and failure action types.

pub mod error;
pub mod git_sha;
pub mod manifest;
pub mod naming;
pub mod schema_version;
pub mod sha256_digest;
pub mod target;
pub mod toolchain_channel;
pub mod verification;
