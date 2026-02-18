//! Manifest schema types for prebuilt artefact metadata.
//!
//! Defines the JSON manifest structure specified in ADR-001. Each artefact
//! archive ships a `manifest.json` capturing provenance, content listing,
//! and the archive checksum.

use super::git_sha::GitSha;
use super::schema_version::SchemaVersion;
use super::sha256_digest::Sha256Digest;
use super::target::TargetTriple;
use super::toolchain_channel::ToolchainChannel;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Provenance fields that identify an artefact build.
///
/// Groups the identity components (git SHA, schema version, toolchain, and
/// target) so that the [`Manifest`] constructor stays within Clippy's
/// parameter limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestProvenance {
    /// The git commit SHA the artefact was built from.
    pub git_sha: GitSha,
    /// The schema version of this manifest.
    pub schema_version: SchemaVersion,
    /// The Rust toolchain channel used for the build.
    pub toolchain: ToolchainChannel,
    /// The target triple the artefact was compiled for.
    pub target: TargetTriple,
}

/// Build output fields that describe the artefact contents.
///
/// Groups the output metadata (timestamp, file list, and checksum) so that
/// the [`Manifest`] constructor stays within Clippy's parameter limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestContent {
    /// ISO 8601 timestamp recording when the artefact was built.
    pub generated_at: GeneratedAt,
    /// List of files contained in the archive.
    pub files: Vec<String>,
    /// SHA-256 digest of the archive.
    pub sha256: Sha256Digest,
}

/// The manifest shipped inside each prebuilt artefact archive.
///
/// The schema mirrors the JSON example in ADR-001:
///
/// ```json
/// {
///   "git_sha": "abc1234",
///   "schema_version": 1,
///   "toolchain": "nightly-2025-09-18",
///   "target": "x86_64-unknown-linux-gnu",
///   "generated_at": "2026-02-03T00:00:00Z",
///   "files": ["libwhitaker_lints@nightly-2025-09-18-x86_64-unknown-linux-gnu.so"],
///   "sha256": "..."
/// }
/// ```
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::manifest::{
///     GeneratedAt, Manifest, ManifestContent, ManifestProvenance,
/// };
/// use whitaker_installer::artefact::git_sha::GitSha;
/// use whitaker_installer::artefact::schema_version::SchemaVersion;
/// use whitaker_installer::artefact::sha256_digest::Sha256Digest;
/// use whitaker_installer::artefact::target::TargetTriple;
/// use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;
///
/// let provenance = ManifestProvenance {
///     git_sha: GitSha::try_from("abc1234").expect("valid git SHA"),
///     schema_version: SchemaVersion::current(),
///     toolchain: ToolchainChannel::try_from("nightly-2025-09-18")
///         .expect("valid toolchain channel"),
///     target: TargetTriple::try_from("x86_64-unknown-linux-gnu")
///         .expect("valid target triple"),
/// };
/// let content = ManifestContent {
///     generated_at: GeneratedAt::new("2026-02-03T00:00:00Z"),
///     files: vec!["libwhitaker_lints.so".to_owned()],
///     sha256: Sha256Digest::try_from("a".repeat(64).as_str())
///         .expect("valid SHA-256 digest"),
/// };
/// let manifest = Manifest::new(provenance, content);
/// assert_eq!(manifest.git_sha().as_str(), "abc1234");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(flatten)]
    provenance: ManifestProvenance,
    #[serde(flatten)]
    content: ManifestContent,
}

/// An ISO 8601 timestamp string recording when the artefact was built.
///
/// This is stored as an opaque string; parsing and validation of the
/// timestamp format is deferred to later tasks that consume manifests
/// from external sources.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GeneratedAt(String);

impl GeneratedAt {
    /// Create a new timestamp wrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::manifest::GeneratedAt;
    ///
    /// let ts = GeneratedAt::new("2026-02-03T00:00:00Z");
    /// assert_eq!(ts.as_str(), "2026-02-03T00:00:00Z");
    /// ```
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the timestamp as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::artefact::manifest::GeneratedAt;
    ///
    /// let ts = GeneratedAt::new("2026-02-03T00:00:00Z");
    /// assert_eq!(ts.as_str(), "2026-02-03T00:00:00Z");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GeneratedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Helper macro for manifest doc examples — constructs a sample
/// [`Manifest`] bound to `manifest`.
///
/// Not public; exists only to keep per-function doc examples concise.
#[doc(hidden)]
#[macro_export]
macro_rules! _manifest_doc_setup {
    ($manifest:ident) => {
        use whitaker_installer::artefact::git_sha::GitSha;
        use whitaker_installer::artefact::manifest::{
            GeneratedAt, Manifest, ManifestContent, ManifestProvenance,
        };
        use whitaker_installer::artefact::schema_version::SchemaVersion;
        use whitaker_installer::artefact::sha256_digest::Sha256Digest;
        use whitaker_installer::artefact::target::TargetTriple;
        use whitaker_installer::artefact::toolchain_channel::ToolchainChannel;

        let provenance = ManifestProvenance {
            git_sha: GitSha::try_from("abc1234").expect("valid git SHA"),
            schema_version: SchemaVersion::current(),
            toolchain: ToolchainChannel::try_from("nightly-2025-09-18")
                .expect("valid toolchain channel"),
            target: TargetTriple::try_from("x86_64-unknown-linux-gnu")
                .expect("valid target triple"),
        };
        let content = ManifestContent {
            generated_at: GeneratedAt::new("2026-02-03T00:00:00Z"),
            files: vec!["libwhitaker_lints.so".to_owned()],
            sha256: Sha256Digest::try_from("a".repeat(64).as_str()).expect("valid SHA-256 digest"),
        };
        let $manifest = Manifest::new(provenance, content);
    };
}

impl Manifest {
    /// Construct a manifest from provenance and content groups.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.git_sha().as_str(), "abc1234");
    /// ```
    #[must_use]
    pub fn new(provenance: ManifestProvenance, content: ManifestContent) -> Self {
        Self {
            provenance,
            content,
        }
    }

    /// Return the git SHA.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.git_sha().as_str(), "abc1234");
    /// ```
    #[must_use]
    pub fn git_sha(&self) -> &GitSha {
        &self.provenance.git_sha
    }

    /// Return the schema version.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(u32::from(manifest.schema_version()), 1);
    /// ```
    #[must_use]
    pub fn schema_version(&self) -> SchemaVersion {
        self.provenance.schema_version
    }

    /// Return the toolchain channel.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.toolchain().as_str(), "nightly-2025-09-18");
    /// ```
    #[must_use]
    pub fn toolchain(&self) -> &ToolchainChannel {
        &self.provenance.toolchain
    }

    /// Return the target triple.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.target().as_str(), "x86_64-unknown-linux-gnu");
    /// ```
    #[must_use]
    pub fn target(&self) -> &TargetTriple {
        &self.provenance.target
    }

    /// Return the build timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.generated_at().as_str(), "2026-02-03T00:00:00Z");
    /// ```
    #[must_use]
    pub fn generated_at(&self) -> &GeneratedAt {
        &self.content.generated_at
    }

    /// Return the list of files in the archive.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.files(), &["libwhitaker_lints.so"]);
    /// ```
    #[must_use]
    pub fn files(&self) -> &[String] {
        &self.content.files
    }

    /// Return the SHA-256 digest of the archive.
    ///
    /// # Examples
    ///
    /// ```
    /// whitaker_installer::_manifest_doc_setup!(manifest);
    /// assert_eq!(manifest.sha256().as_str().len(), 64);
    /// ```
    #[must_use]
    pub fn sha256(&self) -> &Sha256Digest {
        &self.content.sha256
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
