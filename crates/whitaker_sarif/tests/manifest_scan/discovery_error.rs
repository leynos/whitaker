//! Why discovery could not decide whether a manifest is present.
//!
//! This type is the vocabulary discovery and the guards share, so it lives
//! apart from the code that produces it. Every variant exists for the same
//! reason: "I could not look" must never masquerade as "I looked and there was
//! nothing there". Only a confirmed absence may certify that a crate is clean,
//! and the in-flight variants are split by *which* step failed so a diagnostic
//! can say whether the file was refused, unreadable, or simply not TOML.
//!
//! The variants are grouped in the order the failure can happen: the presence
//! question ([`DiscoveryError::Unreadable`], [`DiscoveryError::NotAFile`]),
//! then the load of a file already located
//! ([`DiscoveryError::ManifestUnreadable`], [`DiscoveryError::ManifestInvalid`]),
//! and finally the disagreement between discovery's two sources
//! ([`DiscoveryError::DeclaredPathMissing`]).

use std::io;

use camino::Utf8PathBuf;

/// Why discovery could not decide whether a manifest is present.
///
/// This type exists so that "I could not look" never masquerades as "I looked
/// and there was nothing there". Only the second fact may certify absence; the
/// first must fail the guard, which is what carrying it as an error makes the
/// caller do.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DiscoveryError {
    /// The filesystem would not say whether a manifest is present.
    #[error("cannot tell whether {path} is a manifest: {source}")]
    Unreadable {
        /// The path the filesystem refused to describe.
        path: Utf8PathBuf,
        /// The failure the filesystem reported.
        source: io::Error,
    },
    /// A path exists but holds no regular file, so no manifest can be read.
    #[error("{path} exists but is not a regular file")]
    NotAFile {
        /// The path that is not a regular file.
        path: Utf8PathBuf,
    },
    /// A located manifest could not be read.
    ///
    /// This is distinct from [`DiscoveryError::Unreadable`], which reports that
    /// the filesystem would not say whether a manifest is *present*. Here the
    /// file is known to be there and its bytes could not be obtained.
    #[error("cannot read the manifest at {path}: {source}")]
    ManifestUnreadable {
        /// The manifest that could not be read.
        path: Utf8PathBuf,
        /// The failure the filesystem reported.
        source: io::Error,
    },
    /// A located manifest was read but is not valid TOML.
    #[error("the manifest at {path} is not valid TOML: {source}")]
    ManifestInvalid {
        /// The manifest that failed to parse.
        path: Utf8PathBuf,
        /// The parse failure, which names the offending line and column.
        source: toml::de::Error,
    },
    /// The root declares a path that is absent while the crate sits at its
    /// conventional sibling location.
    ///
    /// This is a disagreement between the two discovery sources, and neither
    /// answer may be acted on. Trusting the declaration reports the crate as
    /// absent, which lets the mapping-crate guard skip a manifest that is
    /// really there; falling back to the convention would silently accept a
    /// root manifest that names the wrong path. Reporting the conflict makes
    /// the stale declaration visible instead of dormant.
    #[error(
        "the root manifest declares {declared} for {crate_name}, but no manifest is there; the \
         crate is present at {sibling} instead, so the declaration is stale"
    )]
    DeclaredPathMissing {
        /// The crate whose declaration disagrees with the filesystem.
        crate_name: String,
        /// The path the root manifest declares, which holds no manifest.
        declared: Utf8PathBuf,
        /// The conventional sibling path, which does hold one.
        sibling: Utf8PathBuf,
    },
}
