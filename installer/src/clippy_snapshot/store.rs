//! Filesystem adapter for snapshot roots, staging, locks and metadata.
//!
//! Every filesystem probe, digest, lock, temporary directory, rename and JSON
//! write in the snapshot boundary lives here. The orchestration in
//! [`super::verify`], [`super::install`] and [`super::provision`] sees only
//! [`super::ports::SnapshotStore`] and [`super::ports::MetadataStore`], so a
//! test can replace this adapter without touching a real directory.

use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use fs2::FileExt;

use super::{
    error::ClippySnapshotError,
    generation::{GenerationMetadata, METADATA_FILE, METADATA_SCHEMA},
    ports::{MetadataStore, RootLock, SnapshotStore, Staging},
    probe::{is_present, is_present_dir, is_present_file},
};
use crate::artefact::{packaging::compute_sha256, sha256_digest::Sha256Digest};

/// Serializes installers sharing one snapshot root.
const ROOT_LOCK_FILE: &str = ".whitaker-clippy-snapshot.lock";

/// The production filesystem adapter.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProductionStore;

impl SnapshotStore for ProductionStore {
    fn create_root(&self, root: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
        std::fs::create_dir_all(root).map_err(|error| ClippySnapshotError::io(root, &error))?;
        absolute(root)
    }

    fn absolute(&self, path: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
        absolute(path)
    }

    fn is_present(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> { is_present(path) }

    fn is_present_dir(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
        is_present_dir(path)
    }

    fn is_present_file(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
        is_present_file(path)
    }

    fn digest(&self, path: &Utf8Path) -> Result<Sha256Digest, ClippySnapshotError> {
        compute_sha256(path.as_std_path()).map_err(|error| ClippySnapshotError::Io {
            path: path.to_string(),
            reason: error.to_string(),
        })
    }

    fn lock(&self, root: &Utf8Path) -> Result<Box<dyn RootLock>, ClippySnapshotError> {
        let path = root.join(ROOT_LOCK_FILE);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|error| ClippySnapshotError::io(&path, &error))?;
        file.lock_exclusive()
            .map_err(|error| ClippySnapshotError::io(&path, &error))?;
        Ok(Box::new(RootLockFile { _file: file }))
    }

    fn staging(
        &self,
        root: &Utf8Path,
        prefix: &str,
    ) -> Result<Box<dyn Staging>, ClippySnapshotError> {
        let directory = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(root)
            .map_err(|error| ClippySnapshotError::io(root, &error))?;
        let path = utf8(directory.path())?.to_owned();
        Ok(Box::new(StagingDirectory {
            _directory: directory,
            path,
        }))
    }

    fn publish(&self, staged: &Utf8Path, generation: &Utf8Path) -> Result<(), ClippySnapshotError> {
        std::fs::rename(staged, generation)
            .map_err(|error| ClippySnapshotError::io(generation, &error))
    }
}

impl MetadataStore for ProductionStore {
    fn read(&self, generation: &Utf8Path) -> Result<GenerationMetadata, ClippySnapshotError> {
        read_metadata(generation)
    }

    fn write(
        &self,
        generation: &Utf8Path,
        metadata: &GenerationMetadata,
    ) -> Result<(), ClippySnapshotError> {
        write_metadata(generation, metadata)
    }
}

/// An exclusive root lock released when the file handle drops.
struct RootLockFile {
    _file: std::fs::File,
}

impl RootLock for RootLockFile {}

/// A staging directory removed when the value drops.
struct StagingDirectory {
    _directory: tempfile::TempDir,
    path: Utf8PathBuf,
}

impl Staging for StagingDirectory {
    fn path(&self) -> &Utf8Path { &self.path }
}

/// Read and schema-check the metadata of the generation at `generation`.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::NotInstalled`] when the generation is
/// absent, [`ClippySnapshotError::Io`] when it cannot be inspected, and
/// [`ClippySnapshotError::Invalid`] when its metadata is missing, unreadable,
/// or from an unknown schema.
pub(crate) fn read_metadata(
    generation: &Utf8Path,
) -> Result<GenerationMetadata, ClippySnapshotError> {
    if !is_present_dir(generation)? {
        return Err(ClippySnapshotError::NotInstalled {
            path: generation.to_string(),
        });
    }
    let path = generation.join(METADATA_FILE);
    let contents = std::fs::read_to_string(&path).map_err(|error| {
        ClippySnapshotError::invalid(
            generation,
            format!("{METADATA_FILE} is unreadable: {error}"),
        )
    })?;
    let metadata: GenerationMetadata = serde_json::from_str(&contents).map_err(|error| {
        ClippySnapshotError::invalid(generation, format!("{METADATA_FILE} is malformed: {error}"))
    })?;
    if metadata.schema_version != METADATA_SCHEMA {
        return Err(ClippySnapshotError::invalid(
            generation,
            format!(
                "{METADATA_FILE} has schema {}, expected {METADATA_SCHEMA}",
                metadata.schema_version
            ),
        ));
    }
    Ok(metadata)
}

/// Persist `metadata` inside `generation` atomically.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Io`] when the file cannot be written.
pub(crate) fn write_metadata(
    generation: &Utf8Path,
    metadata: &GenerationMetadata,
) -> Result<(), ClippySnapshotError> {
    let io_error = |error: &std::io::Error| ClippySnapshotError::io(generation, error);
    let json = serde_json::to_vec_pretty(metadata).map_err(|error| ClippySnapshotError::Io {
        path: generation.to_string(),
        reason: error.to_string(),
    })?;
    let mut file = tempfile::NamedTempFile::new_in(generation).map_err(|e| io_error(&e))?;
    file.write_all(&json).map_err(|e| io_error(&e))?;
    file.as_file().sync_all().map_err(|e| io_error(&e))?;
    file.persist(generation.join(METADATA_FILE))
        .map_err(|error| io_error(&error.error))?;
    Ok(())
}

/// Resolve `path` against the current directory, as `DYLINT_CLIPPY_REPO` needs.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Io`] when the path cannot be resolved or is
/// not valid UTF-8.
pub(crate) fn absolute(path: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
    let resolved =
        std::path::absolute(path).map_err(|error| ClippySnapshotError::io(path, &error))?;
    Ok(utf8(&resolved)?.to_owned())
}

/// Require a UTF-8 path, which the environment variables Dylint reads need.
fn utf8(path: &std::path::Path) -> Result<&Utf8Path, ClippySnapshotError> {
    Utf8Path::from_path(path).ok_or_else(|| ClippySnapshotError::Io {
        path: path.display().to_string(),
        reason: "the path is not valid UTF-8".to_owned(),
    })
}
