//! Scripted ports that record calls and return chosen evidence.
//!
//! The orchestration tests use these in place of Git, the filesystem, and the
//! network. Every fake records the calls it received, so a test can show both
//! what happened and, just as importantly, what did not happen.

use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
};

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    error::ClippySnapshotError,
    generation::GenerationMetadata,
    policy::{IntegrityEvidence, SelfContainedEvidence, StructureEvidence},
    ports::{BundleOperations, MetadataStore, RepositoryQueries, RootLock, SnapshotStore, Staging},
};
use crate::{
    artefact::sha256_digest::Sha256Digest,
    dependency_binaries::{DependencyArchiveDownloader, DependencyBinaryInstallError},
};

/// Append `entry` to a call log.
fn record(log: &Mutex<Vec<String>>, entry: String) {
    log.lock().expect("lock call log").push(entry);
}

/// Take a copy of a call log.
fn recorded(log: &Mutex<Vec<String>>) -> Vec<String> { log.lock().expect("lock call log").clone() }

/// Append `entry` to a fake's own log and, when it shares one, the shared log.
///
/// A shared log records calls from several ports in the order they happened,
/// which is the only way to assert that one port was used before another.
fn record_shared(
    log: &Mutex<Vec<String>>,
    shared: Option<&Arc<Mutex<Vec<String>>>>,
    entry: String,
) {
    if let Some(shared) = shared {
        record(shared, entry.clone());
    }
    record(log, entry);
}

/// Repository queries with scripted answers.
pub(super) struct FakeRepository {
    calls: Mutex<Vec<String>>,
    /// The answer to [`RepositoryQueries::structure`].
    pub(super) structure: Result<StructureEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::self_contained`].
    pub(super) self_contained: Result<SelfContainedEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::integrity`].
    pub(super) integrity: Result<IntegrityEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::toolchain_commits`].
    pub(super) commits: Result<Vec<String>, ClippySnapshotError>,
    /// Toolchain file contents, keyed by `(commit, file)`; absent means none.
    pub(super) files: Mutex<Vec<((String, String), String)>>,
}

impl FakeRepository {
    /// Script a repository with no answers beyond the given ones.
    pub(super) fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            structure: Ok(rejecting_structure()),
            self_contained: Ok(SelfContainedEvidence {
                is_shallow: "false".to_owned(),
                remotes: String::new(),
            }),
            integrity: Ok(IntegrityEvidence {
                succeeded: true,
                stderr: String::new(),
            }),
            commits: Ok(Vec::new()),
            files: Mutex::new(Vec::new()),
        }
    }

    /// Return the calls received so far, in order.
    pub(super) fn calls(&self) -> Vec<String> { recorded(&self.calls) }

    /// Serve `contents` as `file` at `commit`.
    pub(super) fn add_file(&mut self, commit: &str, file: &str, contents: &str) -> &mut Self {
        self.files
            .lock()
            .expect("lock files")
            .push(((commit.to_owned(), file.to_owned()), contents.to_owned()));
        self
    }
}

impl RepositoryQueries for FakeRepository {
    fn structure(&self, path: &Utf8Path) -> Result<StructureEvidence, ClippySnapshotError> {
        record(&self.calls, format!("structure {path}"));
        self.structure.clone()
    }

    fn self_contained(
        &self,
        path: &Utf8Path,
    ) -> Result<SelfContainedEvidence, ClippySnapshotError> {
        record(&self.calls, format!("self_contained {path}"));
        self.self_contained.clone()
    }

    fn integrity(&self, path: &Utf8Path) -> Result<IntegrityEvidence, ClippySnapshotError> {
        record(&self.calls, format!("integrity {path}"));
        self.integrity.clone()
    }

    fn toolchain_commits(
        &self,
        path: &Utf8Path,
        head: &str,
    ) -> Result<Vec<String>, ClippySnapshotError> {
        record(&self.calls, format!("toolchain_commits {path} {head}"));
        self.commits.clone()
    }

    fn file_at_commit(
        &self,
        path: &Utf8Path,
        commit: &str,
        file: &str,
    ) -> Result<Option<String>, ClippySnapshotError> {
        record(
            &self.calls,
            format!("file_at_commit {path} {commit} {file}"),
        );
        let files = self.files.lock().expect("lock files");
        Ok(files
            .iter()
            .find(|((recorded_commit, recorded_file), _)| {
                recorded_commit == commit && recorded_file == file
            })
            .map(|(_, contents)| contents.clone()))
    }
}

/// A structure observation that fails its policy check.
fn rejecting_structure() -> StructureEvidence {
    StructureEvidence {
        is_bare: "false".to_owned(),
        head: String::new(),
        refs: String::new(),
    }
}

/// Bundle operations with scripted answers.
pub(super) struct FakeBundles {
    calls: Mutex<Vec<String>>,
    /// The answer to [`BundleOperations::verify_header`].
    pub(super) verify_header: Result<(), ClippySnapshotError>,
    /// The answer to [`BundleOperations::list_heads`].
    pub(super) heads: Result<String, ClippySnapshotError>,
    /// The answer to [`BundleOperations::stage_clone`].
    pub(super) stage_clone: Result<(), ClippySnapshotError>,
}

impl FakeBundles {
    /// Script a bundle that passes every header and clone check.
    pub(super) fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            verify_header: Ok(()),
            heads: Ok(String::new()),
            stage_clone: Ok(()),
        }
    }

    /// Return the calls received so far, in order.
    pub(super) fn calls(&self) -> Vec<String> { recorded(&self.calls) }
}

impl BundleOperations for FakeBundles {
    fn verify_header(&self, bundle: &Utf8Path) -> Result<(), ClippySnapshotError> {
        record(&self.calls, format!("verify_header {bundle}"));
        self.verify_header.clone()
    }

    fn list_heads(&self, bundle: &Utf8Path) -> Result<String, ClippySnapshotError> {
        record(&self.calls, format!("list_heads {bundle}"));
        self.heads.clone()
    }

    fn stage_clone(
        &self,
        bundle: &Utf8Path,
        destination: &Utf8Path,
    ) -> Result<(), ClippySnapshotError> {
        record(&self.calls, format!("stage_clone {bundle} {destination}"));
        self.stage_clone.clone()
    }
}

/// A root lock that records nothing and never blocks.
pub(super) struct FakeLock;

impl RootLock for FakeLock {}

/// A staging directory at a fixed path.
pub(super) struct FakeStaging {
    path: Utf8PathBuf,
}

impl Staging for FakeStaging {
    fn path(&self) -> &Utf8Path { &self.path }
}

/// Snapshot-root operations with scripted answers.
pub(super) struct FakeStore {
    calls: Mutex<Vec<String>>,
    /// A log shared with other ports, recording the same entries in order.
    shared: Option<Arc<Mutex<Vec<String>>>>,
    /// Paths for which [`SnapshotStore::is_present`] answers `true`.
    present: Mutex<HashSet<String>>,
    /// Whether [`SnapshotStore::is_present_dir`] answers `true`.
    pub(super) present_dir: bool,
    /// The answer to [`SnapshotStore::digest`].
    pub(super) digest: Result<Sha256Digest, ClippySnapshotError>,
    /// The answer to [`SnapshotStore::read_to_string`].
    pub(super) text: Result<String, ClippySnapshotError>,
    /// The answer to [`SnapshotStore::publish`].
    pub(super) publish: Result<(), ClippySnapshotError>,
}

impl FakeStore {
    /// Script a store whose root is empty and whose digest matches `digest`.
    pub(super) fn new(digest: Sha256Digest) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            shared: None,
            present: Mutex::new(HashSet::new()),
            present_dir: true,
            digest: Ok(digest),
            text: Ok(String::new()),
            publish: Ok(()),
        }
    }

    /// Record calls into `log` as well, so a test can assert cross-port order.
    pub(super) fn sharing(mut self, log: &Arc<Mutex<Vec<String>>>) -> Self {
        self.shared = Some(Arc::clone(log));
        self
    }

    /// Make [`SnapshotStore::is_present`] answer `true` for `path`.
    pub(super) fn present(self, path: &Utf8Path) -> Self {
        self.present
            .lock()
            .expect("lock presence")
            .insert(path.to_string());
        self
    }

    /// Return the calls received so far, in order.
    pub(super) fn calls(&self) -> Vec<String> { recorded(&self.calls) }

    /// Return whether any call log entry starts with `prefix`.
    pub(super) fn has_call_starting_with(&self, prefix: &str) -> bool {
        self.calls().iter().any(|call| call.starts_with(prefix))
    }

    /// Record `entry` in this fake's log and in the shared log, if any.
    fn record(&self, entry: String) { record_shared(&self.calls, self.shared.as_ref(), entry); }
}

impl SnapshotStore for FakeStore {
    fn create_root(&self, root: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
        self.record(format!("create_root {root}"));
        Ok(root.to_owned())
    }

    fn absolute(&self, path: &Utf8Path) -> Result<Utf8PathBuf, ClippySnapshotError> {
        self.record(format!("absolute {path}"));
        Ok(path.to_owned())
    }

    fn is_present(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
        self.record(format!("is_present {path}"));
        Ok(self
            .present
            .lock()
            .expect("lock presence")
            .contains(path.as_str()))
    }

    fn is_present_dir(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
        self.record(format!("is_present_dir {path}"));
        Ok(self.present_dir)
    }

    fn is_present_file(&self, path: &Utf8Path) -> Result<bool, ClippySnapshotError> {
        self.record(format!("is_present_file {path}"));
        Ok(true)
    }

    fn digest(&self, path: &Utf8Path) -> Result<Sha256Digest, ClippySnapshotError> {
        self.record(format!("digest {path}"));
        self.digest.clone()
    }

    fn read_to_string(&self, path: &Utf8Path) -> Result<String, ClippySnapshotError> {
        self.record(format!("read_to_string {path}"));
        self.text.clone()
    }

    fn lock(&self, root: &Utf8Path) -> Result<Box<dyn RootLock>, ClippySnapshotError> {
        self.record(format!("lock {root}"));
        Ok(Box::new(FakeLock))
    }

    fn staging(
        &self,
        root: &Utf8Path,
        prefix: &str,
    ) -> Result<Box<dyn Staging>, ClippySnapshotError> {
        self.record(format!("staging {root} {prefix}"));
        Ok(Box::new(FakeStaging {
            path: root.join(format!("{prefix}fake")),
        }))
    }

    fn publish(&self, staged: &Utf8Path, generation: &Utf8Path) -> Result<(), ClippySnapshotError> {
        self.record(format!("publish {staged} {generation}"));
        self.publish.clone()
    }
}

/// Metadata operations with scripted answers.
pub(super) struct FakeMetadata {
    calls: Mutex<Vec<String>>,
    /// A log shared with other ports, recording the same entries in order.
    shared: Option<Arc<Mutex<Vec<String>>>>,
    /// The answer to [`MetadataStore::read`].
    pub(super) read: Result<GenerationMetadata, ClippySnapshotError>,
}

impl FakeMetadata {
    /// Script a metadata read that returns `metadata`.
    pub(super) fn new(metadata: GenerationMetadata) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            shared: None,
            read: Ok(metadata),
        }
    }

    /// Record calls into `log` as well, so a test can assert cross-port order.
    pub(super) fn sharing(mut self, log: &Arc<Mutex<Vec<String>>>) -> Self {
        self.shared = Some(Arc::clone(log));
        self
    }

    /// Return the calls received so far, in order.
    pub(super) fn calls(&self) -> Vec<String> { recorded(&self.calls) }

    /// Return whether any call log entry starts with `prefix`.
    pub(super) fn has_call_starting_with(&self, prefix: &str) -> bool {
        self.calls().iter().any(|call| call.starts_with(prefix))
    }

    /// Record `entry` in this fake's log and in the shared log, if any.
    fn record(&self, entry: String) { record_shared(&self.calls, self.shared.as_ref(), entry); }
}

impl MetadataStore for FakeMetadata {
    fn read(&self, generation: &Utf8Path) -> Result<GenerationMetadata, ClippySnapshotError> {
        self.record(format!("read {generation}"));
        self.read.clone()
    }

    fn write(
        &self,
        generation: &Utf8Path,
        _metadata: &GenerationMetadata,
    ) -> Result<(), ClippySnapshotError> {
        self.record(format!("write {generation}"));
        Ok(())
    }
}

/// A downloader that must never be asked for bytes.
pub(super) struct UnusedDownloader;

impl DependencyArchiveDownloader for UnusedDownloader {
    fn download(
        &self,
        filename: &str,
        _destination: &Path,
    ) -> Result<(), DependencyBinaryInstallError> {
        Err(DependencyBinaryInstallError::Download {
            url: filename.to_owned(),
            reason: "no download expected in this orchestration test".to_owned(),
        })
    }
}
