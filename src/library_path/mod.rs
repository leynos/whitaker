//! What a Dylint library path list contains, as pure policy.
//!
//! Two places need this answer, and they must agree: the readiness check,
//! which decides whether a user-managed path may reach Dylint at all, and the
//! constructor of the prepared value that Dylint later runs with. Keeping the
//! rules here means a prepared value cannot name a path the readiness check
//! would have refused.
//!
//! The rules are Dylint 6.3.0's, because Dylint searches the list afterwards.
//! It requires every entry to be an absolute path to an existing directory, it
//! discovers libraries by file name alone, and it refuses to choose between
//! two candidates for `whitaker_suite`. Each of those refusals exits `1`, the
//! status Dylint also uses for a lint denial, so a list that breaks one of
//! them is a provisioning defect rather than something to pass on.
//!
//! This module performs no I/O. The adapter's listing seam
//! ([`crate::adapters::library_path`]) reads the filesystem and reports what it
//! found in [`EntryReport`]s; [`classify`] rules on those reports. Keeping the
//! two apart means a test can state a listing without a temporary directory,
//! and the same rules judge every caller's evidence.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use whitaker_installer::builder::{library_extension, library_prefix};
/// The crate name Dylint selects when `check` asks for the Whitaker suite.
///
/// Re-exported rather than restated so the argument vector and the listing
/// rules name one crate: the installer's resolution list is canonical, and a
/// second copy could drift from the libraries it actually stages. The command
/// layer reaches it here rather than importing the installer itself.
pub(crate) use whitaker_installer::resolution::SUITE_CRATE;

/// An entry of a library path list that is not an absolute path.
pub(crate) const NOT_ABSOLUTE: &str = "is not an absolute path";

/// An entry of a library path list that does not name a directory.
pub(crate) const NOT_A_DIRECTORY: &str = "is not a directory";

/// A suite-named entry that Dylint could not load as a library.
pub(crate) const NOT_A_LIBRARY: &str = "is not a library file";

/// A library path list that is not valid Unicode.
///
/// Dylint reads `DYLINT_LIBRARY_PATH` through `std::env::var`, so a value it
/// cannot decode is ignored in full rather than read as bytes. A directory
/// that exists but carries a non-Unicode name would be approved here and then
/// silently dropped by Dylint, whose "no library" failure uses the status a
/// lint denial uses. The list is therefore refused before Dylint runs.
pub(crate) const NOT_UNICODE: &str = "is not valid Unicode, which Dylint cannot read";

/// What a Dylint library path list holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SuitePresence {
    /// No directory on the list holds a Whitaker suite library.
    Absent,
    /// Exactly one Whitaker suite library is reachable.
    ExactlyOne,
    /// More than one is reachable, so Dylint would refuse to choose.
    Ambiguous,
    /// The list is not one Dylint can search, so it is refused before Dylint
    /// runs.
    ///
    /// The reason is a predicate over the entry rather than a rendered
    /// message, so the same wording reaches every caller and nothing about a
    /// user's path leaks into a bounded label.
    Unusable {
        /// The entry that cannot be searched.
        path: String,
        /// What is wrong with the entry.
        reason: &'static str,
    },
}

/// What the adapter found for one path-list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryListing {
    /// The adapter did not inspect the entry: it is not absolute, and the
    /// pure rules reject it before any filesystem question is asked.
    Uninspected,
    /// The entry names nothing on the host.
    Missing,
    /// The entry names something that is not a directory.
    NotADirectory,
    /// The list is not valid Unicode, so Dylint would ignore all of it.
    NotUnicode,
    /// The entry names a directory holding these entries.
    Directory(Vec<ListedFile>),
}

/// One directory entry, as the listing reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListedFile {
    /// The entry's full path.
    pub path: PathBuf,
    /// Whether a suite-named entry resolves to a regular file (symlinks
    /// followed).
    ///
    /// Only suite-named entries are asked for their type, so this is `false`
    /// for every other name. The rules read it only after filtering by name.
    pub is_regular_file: bool,
}

/// One path-list entry with the listing the adapter produced for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryReport {
    /// The entry exactly as it appeared on the list.
    pub path: PathBuf,
    /// What the adapter found there.
    pub listing: EntryListing,
}

/// A path-list entry, or a file inside one, that the adapter could not
/// inspect.
///
/// Kept as its own type rather than folded into [`SuitePresence`]: an entry
/// that cannot be read is a host fault, and reporting it as "the path holds
/// nothing" would send the user looking for the wrong problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectFault {
    /// The path that could not be inspected.
    path: String,
    /// Why it could not be inspected.
    reason: String,
}

impl InspectFault {
    /// Build the fault a failed filesystem call reported for `path`.
    pub(crate) fn io(path: &Path, error: &std::io::Error) -> Self {
        Self {
            path: path.display().to_string(),
            reason: error.to_string(),
        }
    }

    /// The path that could not be inspected.
    pub(crate) fn path(&self) -> &str { &self.path }

    /// Why it could not be inspected.
    pub(crate) fn reason(&self) -> &str { &self.reason }
}

/// Judge a library path list by what the adapter found in it.
///
/// Entries are judged in list order. An entry the adapter did not inspect is
/// refused as not absolute, because Dylint rejects the whole list rather than
/// skipping an entry it cannot search; an entry that is missing or is not a
/// directory is refused next. Discovered library paths are deduplicated by
/// path, because Dylint stores them in a set: a directory listed twice still
/// holds one library. A path observed more than once is loadable only if every
/// observation called it a regular file, so a directory that became a suite
/// between two listings cannot slip past as a library.
///
/// Whether the single candidate is loadable is the listing's answer, not
/// re-derived here.
#[must_use]
pub(crate) fn classify(reports: &[EntryReport]) -> SuitePresence {
    let mut discovered: Vec<&ListedFile> = Vec::new();
    for report in reports {
        match &report.listing {
            EntryListing::Uninspected => return refuse(&report.path, NOT_ABSOLUTE),
            EntryListing::NotUnicode => return refuse(&report.path, NOT_UNICODE),
            EntryListing::Missing | EntryListing::NotADirectory => {
                return refuse(&report.path, NOT_A_DIRECTORY);
            }
            EntryListing::Directory(files) => discovered.extend(
                files
                    .iter()
                    .filter(|file| file.path.file_name().is_some_and(has_suite_name)),
            ),
        }
    }
    discovered.sort_by(|left, right| left.path.cmp(&right.path));
    let candidates = deduplicate(discovered);
    match candidates.as_slice() {
        [] => SuitePresence::Absent,
        [only] if only.loadable => SuitePresence::ExactlyOne,
        [only] => refuse(only.path, NOT_A_LIBRARY),
        [_, _, ..] => SuitePresence::Ambiguous,
    }
}

/// One suite candidate, combined from every observation of its path.
struct Candidate<'file> {
    /// The candidate's full path.
    path: &'file Path,
    /// Whether every observation called it a regular file.
    loadable: bool,
}

/// Combine repeated observations of a path into one candidate.
///
/// The input must already be sorted by path, which is what makes one pass
/// enough. An observation that found something other than a regular file wins:
/// the path is not a library Dylint can load, however it was described the
/// other times it was seen.
fn deduplicate(discovered: Vec<&ListedFile>) -> Vec<Candidate<'_>> {
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    for file in discovered {
        match candidates.last_mut() {
            Some(last) if last.path == file.path => last.loadable &= file.is_regular_file,
            _ => candidates.push(Candidate {
                path: &file.path,
                loadable: file.is_regular_file,
            }),
        }
    }
    candidates
}

/// Report whether a file name is one Dylint would discover as the suite.
///
/// This mirrors Dylint's own discovery rather than the installer's parser.
/// Dylint strips the platform's library suffix and prefix and splits the rest
/// at the first `@`; it does not reject an empty toolchain, so
/// `libwhitaker_suite@.so` is a second library as far as its selection is
/// concerned. The installer's parser does reject that form, because a staged
/// library always carries a toolchain, so it cannot be used here: counting
/// only the well-formed name would report one suite where Dylint finds two and
/// refuse to choose.
pub(crate) fn has_suite_name(name: &OsStr) -> bool {
    let Some(text) = name.to_str() else {
        return false;
    };
    let Some(stem) = text
        .strip_suffix(library_extension())
        .and_then(|rest| rest.strip_prefix(library_prefix()))
    else {
        return false;
    };
    let Some((krate, _toolchain)) = stem.split_once('@') else {
        return false;
    };
    krate == SUITE_CRATE
}

/// Report that a path-list entry is not one Dylint can search.
fn refuse(path: &Path, reason: &'static str) -> SuitePresence {
    SuitePresence::Unusable {
        path: path.display().to_string(),
        reason,
    }
}

#[cfg(test)]
mod tests;
