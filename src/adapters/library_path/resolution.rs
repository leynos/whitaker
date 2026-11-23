//! Turning candidate directories into the Dylint library path to export.
//!
//! Each candidate is classified once, through the same listing seam the
//! readiness check uses, and the survivors are joined for Dylint to select a
//! toolchain from. A stale candidate is dropped rather than failing the whole
//! resolution; only a candidate that exists but cannot be used is reported.

use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    LibraryPath,
    listing::{EntryKind, classify_entry},
};
use crate::ports::CliError;

/// How one candidate directory fared during resolution.
enum CandidateFate {
    /// The path names a directory that can be represented in a path list.
    Usable,
    /// The path no longer names a directory, so an earlier install has gone.
    Absent,
    /// The path exists but cannot be used, with the reason it cannot.
    Unusable(String),
}

/// Classify one candidate directory without discarding the reason it failed.
///
/// The metadata question is answered by the same seam the readiness check
/// uses, so a path cannot be "missing" here and "unreadable" there.
/// `is_dir()` collapses both into `false`, which hides a permission or I/O
/// failure behind the normal "nothing installed" case; the seam keeps them
/// apart.
fn classify_candidate(candidate: &Utf8Path) -> CandidateFate {
    if !candidate.is_absolute() {
        return CandidateFate::Unusable("the path is not absolute".to_owned());
    }
    match classify_entry(candidate.as_std_path()) {
        Ok(EntryKind::Directory) => match std::env::join_paths([candidate.as_std_path()]) {
            Ok(_) => CandidateFate::Usable,
            Err(error) => CandidateFate::Unusable(format!(
                "the path cannot be represented in a path list: {error}"
            )),
        },
        Ok(EntryKind::NotADirectory | EntryKind::Missing) => CandidateFate::Absent,
        Err(error) => CandidateFate::Unusable(error.reason().to_owned()),
    }
}

/// Join every usable candidate directory so Dylint can select by toolchain.
///
/// # Errors
///
/// Returns [`CliError::LibraryPathUnusable`] when no candidate survives and at
/// least one exists but cannot be used, and [`CliError::LibraryPath`] when the
/// surviving candidates cannot be joined. Each survivor is individually
/// representable, so the latter is a defensive report rather than an expected
/// outcome.
pub(super) fn join_existing_directories(
    candidates: Vec<Utf8PathBuf>,
) -> Result<LibraryPath, CliError> {
    let mut set = CandidateSet::default();
    for candidate in candidates {
        set.add(candidate);
    }
    set.into_library_path()
}

/// The candidates a resolution kept, and why it dropped the rest.
#[derive(Default)]
struct CandidateSet {
    usable: Vec<PathBuf>,
    absent: usize,
    unusable: Vec<String>,
}

impl CandidateSet {
    /// Classify one candidate and file it under the fate it received.
    fn add(&mut self, candidate: Utf8PathBuf) {
        match classify_candidate(&candidate) {
            CandidateFate::Usable => self.usable.push(candidate.into_std_path_buf()),
            CandidateFate::Absent => self.absent += 1,
            CandidateFate::Unusable(reason) => self.record_unusable(&candidate, &reason),
        }
    }

    /// Report and record a candidate that exists but cannot be used.
    fn record_unusable(&mut self, candidate: &Utf8Path, reason: &str) {
        tracing::warn!(
            path = %candidate,
            %reason,
            "dropping an unusable Dylint library path candidate"
        );
        self.unusable.push(format!("{candidate}: {reason}"));
    }

    /// Resolve the candidates seen into the path list to export.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::LibraryPathUnusable`] when nothing survives and a
    /// candidate exists but cannot be used, and [`CliError::LibraryPath`] when
    /// the surviving candidates cannot be joined.
    fn into_library_path(self) -> Result<LibraryPath, CliError> {
        super::super::telemetry::record_dropped_candidates(self.absent, self.unusable.len());
        if self.usable.is_empty() {
            return self.no_survivor();
        }
        let candidate_count = self.usable.len();
        std::env::join_paths(self.usable)
            .map(|path| LibraryPath::Resolved {
                path,
                candidates: candidate_count,
            })
            .map_err(|source| CliError::LibraryPath { source })
    }

    /// Report a resolution that kept no usable candidate.
    fn no_survivor(self) -> Result<LibraryPath, CliError> {
        if self.unusable.is_empty() {
            return Ok(LibraryPath::NoCandidate);
        }
        Err(CliError::LibraryPathUnusable {
            message: self.unusable.join("; "),
        })
    }
}
