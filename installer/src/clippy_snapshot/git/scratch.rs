//! A throwaway repository for the one Git command that insists on one.
//!
//! `git bundle verify` refuses to run outside a repository, so the adapter
//! makes one for each call. `git bundle list-heads` needs no repository and is
//! left as a pure read. The port permits that single bounded write: this module
//! owns creating the repository, removing it on every exit even when the
//! command never ran, and reporting a failed creation or removal against the
//! scratch location rather than the bundle the caller owns.

use camino::{Utf8Path, Utf8PathBuf};

use super::{ClippySnapshotError, SnapshotGit};

impl SnapshotGit<'_> {
    /// Run `operation` in a fresh scratch repository, then remove it.
    ///
    /// `git bundle verify` insists on a repository, so each call needs one.
    /// `close` is used rather than the `TempDir` drop so a removal failure is
    /// reported: the port promises the scratch state never outlives the call,
    /// and a silently ignored error would break that promise.
    pub(super) fn with_scratch_repository<T>(
        self,
        operation: impl FnOnce(&Utf8Path) -> Result<T, ClippySnapshotError>,
    ) -> Result<T, ClippySnapshotError> {
        let (scratch, path) = self.scratch_repository()?;
        finish_scratch(scratch, operation(&path))
    }

    /// Create an empty bare repository for bundle header commands.
    ///
    /// The returned `TempDir` must outlive the commands run in the returned
    /// path.
    fn scratch_repository(self) -> Result<(tempfile::TempDir, Utf8PathBuf), ClippySnapshotError> {
        let scratch = self.scratch_directory()?;
        self.initialize_scratch(scratch)
    }

    /// Turn `scratch` into a repository, removing it when that fails.
    ///
    /// A path that is not UTF-8 or a refusing `git init` both end the call
    /// before any operation runs, so the directory is closed here rather than
    /// left to the `TempDir` drop, which would hide a removal failure.
    pub(super) fn initialize_scratch(
        self,
        scratch: tempfile::TempDir,
    ) -> Result<(tempfile::TempDir, Utf8PathBuf), ClippySnapshotError> {
        let initialized = Utf8Path::from_path(scratch.path())
            .ok_or_else(|| ClippySnapshotError::Io {
                path: scratch.path().display().to_string(),
                reason: "the scratch path is not valid UTF-8".to_owned(),
            })
            .map(ToOwned::to_owned)
            .and_then(|path| {
                self.stdout("init", None, &["init", "--quiet", "--bare", path.as_str()])?;
                Ok(path)
            });
        match initialized {
            Ok(path) => Ok((scratch, path)),
            Err(failure) => Err(abandon_scratch(scratch, failure)),
        }
    }

    /// Create the throwaway repository directory, in the system temp directory
    /// unless a test named another parent.
    fn scratch_directory(self) -> Result<tempfile::TempDir, ClippySnapshotError> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("whitaker-clippy-snapshot-scratch-");
        let parent = self
            .scratch_parent
            .map_or_else(std::env::temp_dir, |parent| {
                parent.as_std_path().to_path_buf()
            });
        builder
            .tempdir_in(&parent)
            .map_err(|error| creation_failure(&parent, &error))
    }
}

/// Report `outcome`, failing when the scratch repository cannot be removed.
///
/// The removal is attempted whatever the operation did, so a removal failure
/// after a successful command is still reported. When both fail the operation's
/// diagnosis stays primary, because it explains the snapshot problem, and the
/// cleanup failure is appended to it rather than replacing it.
pub(super) fn finish_scratch<T>(
    scratch: tempfile::TempDir,
    outcome: Result<T, ClippySnapshotError>,
) -> Result<T, ClippySnapshotError> {
    let location = scratch.path().to_path_buf();
    let removal = scratch
        .close()
        .map_err(|error| removal_failure(&location, &error));
    match (outcome, removal) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(failure), Ok(())) => Err(failure),
        (Err(failure), Err(cleanup)) => Err(both_failed(&location, failure, &cleanup)),
    }
}

/// Report `failure`, noting a removal that also failed.
///
/// Used when the scratch repository is abandoned before any operation runs.
fn abandon_scratch(
    scratch: tempfile::TempDir,
    failure: ClippySnapshotError,
) -> ClippySnapshotError {
    let location = scratch.path().to_path_buf();
    match scratch.close() {
        Ok(()) => failure,
        Err(error) => both_failed(&location, failure, &removal_failure(&location, &error)),
    }
}

/// Append a cleanup failure to the failure that came first.
///
/// Only a variant with free-text detail can carry the note. A bundle command
/// returns `Git` or `Invalid`, and neither may lose the cleanup failure, so any
/// other variant keeps both texts inside an `Io` error for the scratch
/// directory.
fn both_failed(
    location: &std::path::Path,
    failure: ClippySnapshotError,
    cleanup: &ClippySnapshotError,
) -> ClippySnapshotError {
    let note = format!("the scratch repository also could not be removed: {cleanup}");
    match failure {
        ClippySnapshotError::Git { operation, message } => ClippySnapshotError::Git {
            operation,
            message: format!("{message}; {note}"),
        },
        ClippySnapshotError::Invalid { path, check } => ClippySnapshotError::Invalid {
            path,
            check: format!("{check}; {note}"),
        },
        ClippySnapshotError::Io { path, reason } => ClippySnapshotError::Io {
            path,
            reason: format!("{reason}; {note}"),
        },
        other => ClippySnapshotError::Io {
            path: location.display().to_string(),
            reason: format!("{other}; {note}"),
        },
    }
}

/// Report a scratch directory that could not be created under `parent`.
fn creation_failure(parent: &std::path::Path, error: &std::io::Error) -> ClippySnapshotError {
    ClippySnapshotError::Io {
        path: parent.display().to_string(),
        reason: format!("could not create the scratch repository: {error}"),
    }
}

/// Report a scratch directory that could not be removed.
fn removal_failure(scratch: &std::path::Path, error: &std::io::Error) -> ClippySnapshotError {
    ClippySnapshotError::Io {
        path: scratch.display().to_string(),
        reason: format!("could not remove the scratch repository: {error}"),
    }
}
