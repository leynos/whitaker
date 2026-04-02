//! Installation helpers for repository-hosted dependency binaries.
//!
//! These helpers install `cargo-dylint` and `dylint-link` from Whitaker release
//! assets before the installer falls back to `cargo binstall` or `cargo
//! install`.

mod downloader;
mod extractor;
mod installer;
mod metadata;

#[cfg(test)]
mod tests;

pub use downloader::DependencyArchiveDownloader;
pub use extractor::DependencyArchiveExtractor;
#[cfg(test)]
pub use installer::MockDependencyBinaryInstaller;
pub use installer::{
    DependencyBinaryInstallError, DependencyBinaryInstaller, RepositoryDependencyBinaryInstaller,
};
pub use metadata::{archive_filename, binary_filename, host_target, provenance_filename};
