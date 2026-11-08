//! Scripted repository queries, including the ones that fail.
//!
//! The fake keeps successful reads and scripted failures apart so a test can
//! show that a failing read is reported rather than mistaken for an absent
//! file. It lives beside the other fakes rather than in `fakes.rs` only to
//! keep each file within the module size limit.

use std::sync::Mutex;

use camino::Utf8Path;

use super::{
    super::{
        error::ClippySnapshotError,
        policy::{IntegrityEvidence, SelfContainedEvidence, StructureEvidence},
        ports::RepositoryQueries,
    },
    record,
    recorded,
};

/// Repository queries with scripted answers.
pub(in crate::clippy_snapshot) struct FakeRepository {
    calls: Mutex<Vec<String>>,
    /// The answer to [`RepositoryQueries::structure`].
    pub(in crate::clippy_snapshot) structure: Result<StructureEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::self_contained`].
    pub(in crate::clippy_snapshot) self_contained:
        Result<SelfContainedEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::integrity`].
    pub(in crate::clippy_snapshot) integrity: Result<IntegrityEvidence, ClippySnapshotError>,
    /// The answer to [`RepositoryQueries::toolchain_commits`].
    pub(in crate::clippy_snapshot) commits: Result<Vec<String>, ClippySnapshotError>,
    /// Toolchain file contents, keyed by `(commit, file)`; absent means none.
    pub(in crate::clippy_snapshot) files: Mutex<Vec<((String, String), String)>>,
    /// Scripted read failures, keyed by `(commit, file)`; absent means none.
    failures: Mutex<Vec<((String, String), ClippySnapshotError)>>,
}

impl FakeRepository {
    /// Script a repository with no answers beyond the given ones.
    pub(in crate::clippy_snapshot) fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            structure: Ok(rejecting_structure()),
            self_contained: Ok(SelfContainedEvidence {
                is_shallow: "false".to_owned(),
                remotes: Vec::new(),
            }),
            integrity: Ok(IntegrityEvidence {
                succeeded: true,
                stderr: String::new(),
            }),
            commits: Ok(Vec::new()),
            files: Mutex::new(Vec::new()),
            failures: Mutex::new(Vec::new()),
        }
    }

    /// Return the calls received so far, in order.
    pub(in crate::clippy_snapshot) fn calls(&self) -> Vec<String> { recorded(&self.calls) }

    /// Serve `contents` as `file` at `commit`.
    pub(in crate::clippy_snapshot) fn add_file(
        &mut self,
        commit: &str,
        file: &str,
        contents: &str,
    ) -> &mut Self {
        record_keyed(&self.files, commit, file, contents.to_owned());
        self
    }

    /// Fail every read of `file` at `commit` with `error`.
    ///
    /// A scripted failure takes precedence over [`Self::add_file`], so a test
    /// can show that a failing read is reported rather than absorbed as
    /// "no such file".
    pub(in crate::clippy_snapshot) fn fail_file(
        &mut self,
        commit: &str,
        file: &str,
        error: ClippySnapshotError,
    ) -> &mut Self {
        record_keyed(&self.failures, commit, file, error);
        self
    }
}

/// Record `value` under the `(commit, file)` key it answers for.
fn record_keyed<T>(
    entries: &Mutex<Vec<((String, String), T)>>,
    commit: &str,
    file: &str,
    value: T,
) {
    entries
        .lock()
        .expect("lock keyed entries")
        .push(((commit.to_owned(), file.to_owned()), value));
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
        if let Some(error) = self
            .failures
            .lock()
            .expect("lock failures")
            .iter()
            .find(|((recorded_commit, recorded_file), _)| {
                recorded_commit == commit && recorded_file == file
            })
            .map(|(_, error)| error.clone())
        {
            return Err(error);
        }
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
        refs: Vec::new(),
    }
}
