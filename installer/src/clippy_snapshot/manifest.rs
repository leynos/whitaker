//! The committed identity of the Clippy history Whitaker publishes.
//!
//! `installer/clippy-snapshot.toml` is compiled into the installer, so the
//! commit object ID it names is the trust anchor for every snapshot the
//! installer accepts. A downloaded bundle is only as trustworthy as its match
//! against that ID: Git object IDs hash their content and ancestry, so a
//! repository whose `main` is this commit and which passes `git fsck --full`
//! holds exactly the pinned history, whatever bytes the bundle used.

use super::error::ClippySnapshotError;
use crate::git::CommitSha;
use serde::Deserialize;
use std::sync::OnceLock;

/// The only manifest schema this installer understands.
pub const SCHEMA_VERSION: u32 = 1;

/// The branch every published snapshot exposes, and the only ref it carries.
pub const SNAPSHOT_BRANCH: &str = "main";

/// Asset name prefix shared by the bundle and its sidecar.
const ASSET_PREFIX: &str = "whitaker-clippy-snapshot-";

/// The raw TOML shape, validated into [`ClippySnapshotManifest`].
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: u32,
    upstream: String,
    branch: String,
    commit: String,
    toolchain: String,
    selected_revision: String,
    dylint_version: String,
}

/// A validated description of one published Clippy snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClippySnapshotManifest {
    /// The upstream repository the history was taken from.
    pub upstream: String,
    /// The pinned tip of [`SNAPSHOT_BRANCH`].
    pub commit: CommitSha,
    /// The dated nightly Dylint builds its driver for.
    pub toolchain: String,
    /// The revision Dylint selects for `toolchain` from `commit`'s history.
    pub selected_revision: CommitSha,
    /// The Dylint release whose selection algorithm `selected_revision` records.
    pub dylint_version: String,
}

impl ClippySnapshotManifest {
    /// Name the release asset that carries this snapshot's bundle.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::clippy_snapshot::embedded_manifest;
    ///
    /// let manifest = embedded_manifest()?;
    /// let asset = manifest.bundle_asset_name();
    /// assert!(asset.starts_with("whitaker-clippy-snapshot-"));
    /// assert!(asset.ends_with(".bundle"));
    /// # Ok::<(), whitaker_installer::clippy_snapshot::ClippySnapshotError>(())
    /// ```
    #[must_use]
    pub fn bundle_asset_name(&self) -> String {
        format!("{ASSET_PREFIX}{}.bundle", self.commit)
    }

    /// Name the `.sha256` sidecar published beside the bundle.
    #[must_use]
    pub fn sidecar_asset_name(&self) -> String {
        format!("{}.sha256", self.bundle_asset_name())
    }
}

/// Return the manifest text compiled into the installer.
#[must_use]
pub fn manifest_contents() -> &'static str {
    include_str!("../../clippy-snapshot.toml")
}

/// Parse and validate a snapshot manifest.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Manifest`] when the TOML is malformed, the
/// schema version is unsupported, the branch is not [`SNAPSHOT_BRANCH`], or a
/// commit is not a full lowercase object ID.
///
/// # Examples
///
/// ```
/// use whitaker_installer::clippy_snapshot::parse_manifest;
///
/// let manifest = parse_manifest(concat!(
///     "schema_version = 1\n",
///     "upstream = \"https://github.com/rust-lang/rust-clippy\"\n",
///     "branch = \"main\"\n",
///     "commit = \"e4608a25c2e20116fbc1e97a51a73f094a1db7a1\"\n",
///     "toolchain = \"nightly-2026-05-28\"\n",
///     "selected_revision = \"9fca3bc9fc2bc83c60bde26d18ed68f11564b228\"\n",
///     "dylint_version = \"6.3.0\"\n",
/// ))?;
/// assert_eq!(manifest.toolchain, "nightly-2026-05-28");
/// # Ok::<(), whitaker_installer::clippy_snapshot::ClippySnapshotError>(())
/// ```
pub fn parse_manifest(contents: &str) -> Result<ClippySnapshotManifest, ClippySnapshotError> {
    let raw: RawManifest = toml::from_str(contents).map_err(|error| manifest_error(&error))?;
    if raw.schema_version != SCHEMA_VERSION {
        return Err(manifest_error(&format!(
            "schema_version {} is not supported; expected {SCHEMA_VERSION}",
            raw.schema_version
        )));
    }
    if raw.branch != SNAPSHOT_BRANCH {
        return Err(manifest_error(&format!(
            "branch `{}` is not supported; snapshots expose only `{SNAPSHOT_BRANCH}`",
            raw.branch
        )));
    }
    Ok(ClippySnapshotManifest {
        upstream: raw.upstream,
        commit: commit_field("commit", &raw.commit)?,
        toolchain: nightly_field(&raw.toolchain)?,
        selected_revision: commit_field("selected_revision", &raw.selected_revision)?,
        dylint_version: raw.dylint_version,
    })
}

/// Return the manifest compiled into this installer, parsed once.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Manifest`] if the embedded manifest is
/// invalid, which a unit test rules out for every build.
pub fn embedded_manifest() -> Result<&'static ClippySnapshotManifest, ClippySnapshotError> {
    static MANIFEST: OnceLock<Result<ClippySnapshotManifest, ClippySnapshotError>> =
        OnceLock::new();
    MANIFEST
        .get_or_init(|| parse_manifest(manifest_contents()))
        .as_ref()
        .map_err(Clone::clone)
}

/// Validate a full commit object ID field.
fn commit_field(field: &str, value: &str) -> Result<CommitSha, ClippySnapshotError> {
    CommitSha::try_from(value).map_err(|error| manifest_error(&format!("{field}: {error}")))
}

/// Validate that the toolchain is a dated nightly, which Dylint's walk needs.
fn nightly_field(value: &str) -> Result<String, ClippySnapshotError> {
    let is_dated_nightly = value
        .strip_prefix("nightly-")
        .is_some_and(|date| date.len() == 10 && date.split('-').count() == 3);
    if is_dated_nightly {
        Ok(value.to_owned())
    } else {
        Err(manifest_error(&format!(
            "toolchain `{value}` is not a dated nightly such as `nightly-2026-05-28`"
        )))
    }
}

/// Wrap a parse or validation failure.
fn manifest_error(reason: &dyn std::fmt::Display) -> ClippySnapshotError {
    ClippySnapshotError::Manifest {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
