//! Repository-owned metadata and installation helpers for dependency binaries.
//!
//! Whitaker requires `cargo-dylint` and `dylint-link` at known versions. This
//! module exposes the committed manifest, archive naming rules, and the
//! repository-download installation path used before Cargo-based fallback.

mod install;
mod manifest;

pub use install::{
    DependencyArchiveDownloader, DependencyArchiveExtractor, DependencyBinaryInstallError,
    DependencyBinaryInstaller, RepositoryDependencyBinaryInstaller, archive_filename,
    binary_filename, host_target, provenance_filename,
};
pub use manifest::{
    DependencyBinary, find_dependency_binary, manifest_contents, parse_manifest,
    required_dependency_binaries,
};
