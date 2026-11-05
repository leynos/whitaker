//! Pure acceptance rules for Clippy snapshot evidence.
//!
//! Adapters observe; this module decides. Every function here takes values
//! (strings, booleans, parsed history) and returns a rejection message or a
//! domain rejection, so the rules can be tested without Git, a filesystem, or
//! a subprocess. The effectful call sites live in [`super::verify`],
//! [`super::install`], and [`super::selection`].

use std::fmt;

use super::{
    error::ClippySnapshotError,
    manifest::SNAPSHOT_BRANCH,
    selection::{SelectionGap, ToolchainChange, select_revision},
};

/// The only ref a snapshot may carry.
pub(crate) fn main_ref() -> String { format!("refs/heads/{SNAPSHOT_BRANCH}") }

/// The commit a snapshot is pinned to, as policy compares it.
///
/// Rulings compare observed object names against one expected commit. Naming
/// that expectation keeps the pinned value distinct from the arbitrary object
/// names a listing may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PinnedCommit<'a>(&'a str);

impl<'a> PinnedCommit<'a> {
    /// Borrow `commit` as the expected object name.
    pub(crate) fn new(commit: &'a str) -> Self { Self(commit) }

    /// Return the object name as a string slice.
    pub(crate) fn as_str(&self) -> &'a str { self.0 }
}

impl fmt::Display for PinnedCommit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.0) }
}

/// What the repository-shape queries reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructureEvidence {
    /// Standard output of `git rev-parse --is-bare-repository`.
    pub(crate) is_bare: String,
    /// Standard output of `git symbolic-ref HEAD`, trimmed.
    pub(crate) head: String,
    /// Standard output of `git for-each-ref --format=%(refname) %(objectname)`.
    pub(crate) refs: String,
}

/// Accept a repository's shape and refs, or say what is wrong.
pub(crate) fn accept_structure(
    evidence: &StructureEvidence,
    commit: PinnedCommit<'_>,
) -> Result<(), String> {
    if evidence.is_bare != "true" {
        return Err("it is not a bare repository".to_owned());
    }
    let main = main_ref();
    if evidence.head != main {
        return Err(format!(
            "HEAD is `{}`, not a symbolic ref to {main}",
            evidence.head
        ));
    }
    accept_refs(&evidence.refs, commit)
}

/// Require exactly one ref, `main`, at the pinned commit.
fn accept_refs(listing: &str, commit: PinnedCommit<'_>) -> Result<(), String> {
    let refs: Vec<(&str, &str)> = listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .collect();
    let main = main_ref();
    let extra: Vec<&str> = refs
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| *name != main)
        .collect();
    if !extra.is_empty() {
        return Err(format!(
            "it carries refs other than {main}: {}",
            extra.join(", ")
        ));
    }
    match refs.first() {
        Some((_, object)) if *object == commit.as_str() => Ok(()),
        Some((_, object)) => Err(format!(
            "{main} is {object}, expected the pinned commit {commit}"
        )),
        None => Err(format!("it has no {main}")),
    }
}

/// What the self-containment queries reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfContainedEvidence {
    /// Standard output of `git rev-parse --is-shallow-repository`.
    pub(crate) is_shallow: String,
    /// Standard output of `git remote`.
    pub(crate) remotes: String,
}

/// Require complete local history with no outside object sources or remotes.
///
/// `has_alternates` is a filesystem probe rather than a Git query, so the
/// caller that owns the store supplies it alongside the Git evidence.
pub(crate) fn accept_self_contained(
    evidence: &SelfContainedEvidence,
    has_alternates: bool,
) -> Result<(), String> {
    if evidence.is_shallow != "false" {
        return Err("its history is shallow".to_owned());
    }
    if has_alternates {
        return Err("it borrows objects through objects/info/alternates".to_owned());
    }
    if !evidence.remotes.is_empty() {
        return Err(format!(
            "it has remotes ({}), so it could fetch",
            evidence.remotes.replace('\n', ", ")
        ));
    }
    Ok(())
}

/// What `git fsck` reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegrityEvidence {
    /// Whether `git fsck` exited successfully.
    pub(crate) succeeded: bool,
    /// Standard error, kept raw so the rejection can trim it.
    pub(crate) stderr: String,
}

/// Require a clean object database.
pub(crate) fn accept_integrity(evidence: &IntegrityEvidence) -> Result<(), String> {
    if evidence.succeeded {
        return Ok(());
    }
    Err(format!(
        "git fsck --full failed: {}",
        evidence.stderr.trim()
    ))
}

/// Require the bundle to carry only `main` and `HEAD` at the pinned commit.
///
/// Unexpected lines are reported before a missing `main` line, and duplicate
/// lines are not rejected: the bundle's own header is the only source here.
pub(crate) fn accept_bundle_heads(heads: &str, commit: PinnedCommit<'_>) -> Result<(), String> {
    let main = main_ref();
    let unexpected: Vec<&str> = heads
        .lines()
        .filter(|line| *line != format!("{commit} {main}") && *line != format!("{commit} HEAD"))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "the bundle must carry only {main} and HEAD at {commit}, but lists: {}",
            unexpected.join("; ")
        ));
    }
    if !heads.lines().any(|line| line == format!("{commit} {main}")) {
        return Err(format!(
            "the bundle does not carry {main} at the pinned commit {commit}"
        ));
    }
    Ok(())
}

/// Return whether the published and computed digests agree.
pub(crate) fn digests_match(expected: &str, actual: &str) -> bool { expected == actual }

/// Why a selection check failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelectionRejection {
    /// The history cannot support a selection.
    Insufficient(SelectionGap),
    /// Dylint would select a different revision.
    Revision {
        /// The revision Dylint selects.
        selected: String,
        /// The revision the manifest records.
        expected: String,
    },
    /// The selected revision names a different channel.
    Channel {
        /// The selected revision.
        selected: String,
        /// The channel it names, if any.
        channel: Option<String>,
        /// The pinned nightly.
        toolchain: String,
    },
}

/// What the manifest expects Dylint's selection to produce.
///
/// The three facts travel together: a selection is judged against the pinned
/// nightly, the recorded revision, and the channel that revision names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionExpectation<'a> {
    /// The pinned nightly toolchain.
    pub(crate) toolchain: &'a str,
    /// The revision the manifest records.
    pub(crate) revision: &'a str,
    /// The channel the selected revision names, if one was read.
    pub(crate) channel: Option<&'a str>,
}

/// Accept Dylint's selection and the channel the selected revision names.
pub(crate) fn accept_selection(
    changes: &[ToolchainChange],
    expected: &SelectionExpectation<'_>,
) -> Result<(), SelectionRejection> {
    let selected =
        select_revision(changes, expected.toolchain).map_err(SelectionRejection::Insufficient)?;
    if selected.commit != expected.revision {
        return Err(SelectionRejection::Revision {
            selected: selected.commit.clone(),
            expected: expected.revision.to_owned(),
        });
    }
    if expected.channel != Some(expected.toolchain) {
        return Err(SelectionRejection::Channel {
            selected: expected.revision.to_owned(),
            channel: expected.channel.map(ToOwned::to_owned),
            toolchain: expected.toolchain.to_owned(),
        });
    }
    Ok(())
}

/// One toolchain file's contribution to channel precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChannelRead {
    /// The file exists and names this channel, if any.
    Present(Option<String>),
    /// The file is absent at this commit.
    Absent,
}

/// Choose the first present toolchain file's channel.
///
/// Rustup's precedence applies: the first file that exists wins, even when it
/// names no channel. An absent file falls through to the next one; when every
/// read fails, the last failure is returned so an infrastructure fault never
/// reads as "no channel".
pub(crate) fn first_channel(
    results: impl IntoIterator<Item = Result<ChannelRead, ClippySnapshotError>>,
) -> Result<Option<String>, ClippySnapshotError> {
    let mut last_error = None;
    for result in results {
        match result {
            Ok(ChannelRead::Present(channel)) => return Ok(channel),
            Ok(ChannelRead::Absent) => continue,
            Err(error) => last_error = Some(error),
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
