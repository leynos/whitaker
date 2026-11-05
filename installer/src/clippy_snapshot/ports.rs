//! Driven ports for the Clippy snapshot boundary.
//!
//! Verification and installation orchestration depend only on these traits.
//! [`super::git::SnapshotGit`] implements the repository and bundle ports;
//! [`super::store::ProductionStore`] implements the snapshot-root and metadata
//! ports. Tests supply fakes that record calls and return scripted evidence,
//! so orchestration can be exercised without Git or a filesystem.
//!
//! The ports return close-to-raw observations. Acceptance rules live in
//! [`super::policy`], and infrastructure failures are translated to
//! [`ClippySnapshotError`] at the adapter boundary.

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    error::ClippySnapshotError,
    generation::GenerationMetadata,
    git::SnapshotGit,
    policy::{IntegrityEvidence, SelfContainedEvidence, StructureEvidence},
    store::ProductionStore,
};
use crate::{
    artefact::sha256_digest::Sha256Digest,
    dependency_binaries::{DependencyArchiveDownloader, RepositoryArchiveDownloader},
};

/// Read-only queries about a snapshot repository.
pub(crate) trait RepositoryQueries {
    /// Observe the repository shape and refs.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when a query cannot run.
    fn structure(&self, path: &Utf8Path) -> Result<StructureEvidence, ClippySnapshotError>;

    /// Observe shallow history and remotes.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when a query cannot run.
    fn self_contained(&self, path: &Utf8Path)
    -> Result<SelfContainedEvidence, ClippySnapshotError>;

    /// Run `git fsck --full` and report its result.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when Git cannot run.
    fn integrity(&self, path: &Utf8Path) -> Result<IntegrityEvidence, ClippySnapshotError>;

    /// List the first-parent commits touching a toolchain file, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when the history cannot be read.
    fn toolchain_commits(
        &self,
        path: &Utf8Path,
        head: &str,
    ) -> Result<Vec<String>, ClippySnapshotError>;

    /// Read `file` at `commit`, or `None` when it is absent there.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when Git cannot run.
    fn file_at_commit(
        &self,
        path: &Utf8Path,
        commit: &str,
        file: &str,
    ) -> Result<Option<String>, ClippySnapshotError>;
}

/// Bundle header checks and clone staging.
pub(crate) trait BundleOperations {
    /// Run `git bundle verify` in a scratch repository.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Invalid`] when the header is rejected
    /// and [`ClippySnapshotError::Git`] when Git cannot run.
    fn verify_header(&self, bundle: &Utf8Path) -> Result<(), ClippySnapshotError>;

    /// List the bundle's heads.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when Git cannot run.
    fn list_heads(&self, bundle: &Utf8Path) -> Result<String, ClippySnapshotError>;

    /// Clone the bundle as a bare repository and remove `origin`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Invalid`] when the clone fails and
    /// [`ClippySnapshotError::Git`] when Git cannot run.
    fn stage_clone(
        &self,
        bundle: &Utf8Path,
        destination: &Utf8Path,
    ) -> Result<(), ClippySnapshotError>;
}

/// The snapshot root: creation, probing, digests, locks and publication.
pub(crate) trait SnapshotStore {
    /// Create `root` and return its absolute path.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the directory cannot be made.
    fn create_root(&self, root: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError>;

    /// Resolve `path` against the current directory.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the path cannot be resolved.
    fn absolute(&self, path: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError>;

    /// Return whether anything exists at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
    fn is_present(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError>;

    /// Return whether `path` is an existing directory.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
    fn is_present_dir(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError>;

    /// Return whether `path` is an existing regular file.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when `path` cannot be inspected.
    fn is_present_file(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError>;

    /// Compute the SHA-256 of `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the file cannot be read.
    fn digest(&self, path: &Utf8Path) -> Result<Sha256Digest, ClippySnapshotError>;

    /// Read `path` as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the file cannot be read.
    fn read_to_string(&self, path: &Utf8Path) -> Result<String, ClippySnapshotError>;

    /// Hold an exclusive lock on `root` until the guard drops.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the lock cannot be taken.
    fn lock(&self, root: &Utf8Path) -> Result<Box<dyn RootLock>, ClippySnapshotError>;

    /// Create a scoped staging directory under `root`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the directory cannot be made.
    fn staging(
        &self,
        root: &Utf8Path,
        prefix: &str,
    ) -> Result<Box<dyn Staging>, ClippySnapshotError>;

    /// Rename `staged` to `generation` atomically.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the rename fails.
    fn publish(&self, staged: &Utf8Path, generation: &Utf8Path) -> Result<(), ClippySnapshotError>;
}

/// An exclusive root lock held for as long as the value lives.
pub(crate) trait RootLock {}

/// A staging directory removed when the value drops.
pub(crate) trait Staging {
    /// The staging directory's path.
    fn path(&self) -> &Utf8Path;
}

/// Read and write generation metadata.
pub(crate) trait MetadataStore {
    /// Read the metadata of the generation at `generation`.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::NotInstalled`] when the generation is
    /// absent and [`ClippySnapshotError::Invalid`] when its metadata is
    /// missing, unreadable, or from an unknown schema.
    fn read(&self, generation: &Utf8Path) -> Result<GenerationMetadata, ClippySnapshotError>;

    /// Persist `metadata` inside `generation` atomically.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Io`] when the file cannot be written.
    fn write(
        &self,
        generation: &Utf8Path,
        metadata: &GenerationMetadata,
    ) -> Result<(), ClippySnapshotError>;
}

/// The production Git adapter, shared by the repository and bundle ports.
static GIT: SnapshotGit<'static> = SnapshotGit::local();

/// The production filesystem adapter, shared by the store and metadata ports.
static STORE: ProductionStore = ProductionStore;

/// The collaborators one snapshot operation needs.
pub(crate) struct SnapshotSupport<'a> {
    /// Repository queries.
    pub(crate) repository: &'a dyn RepositoryQueries,
    /// Bundle header checks and clone staging.
    pub(crate) bundles: &'a dyn BundleOperations,
    /// Snapshot-root operations.
    pub(crate) store: &'a dyn SnapshotStore,
    /// Generation metadata.
    pub(crate) metadata: &'a dyn MetadataStore,
    /// Release-asset downloads.
    pub(crate) downloader: &'a dyn DependencyArchiveDownloader,
}

impl SnapshotSupport<'static> {
    /// Compose the production adapters.
    pub(crate) fn production() -> Self {
        Self {
            repository: &GIT,
            bundles: &GIT,
            store: &STORE,
            metadata: &STORE,
            downloader: &RepositoryArchiveDownloader,
        }
    }
}
