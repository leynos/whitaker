//! Pure acceptance rules for Clippy snapshot evidence.
//!
//! Adapters observe; this module decides. Every function here takes values
//! (strings, booleans, parsed history) and returns a rejection message or a
//! domain rejection, so the rules can be tested without Git, a filesystem, or
//! a subprocess. The effectful call sites live in [`super::verify`],
//! [`super::install`], and [`super::selection`].

use std::fmt;

use super::{
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
    pub(crate) const fn new(commit: &'a str) -> Self { Self(commit) }

    /// Return the object name as a string slice.
    pub(crate) const fn as_str(&self) -> &'a str { self.0 }
}

impl fmt::Display for PinnedCommit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.0) }
}

/// One ref a repository lists, as its name and the object it points at.
///
/// The adapter splits the listing Git printed; policy compares these values,
/// so the one-line output format never reaches a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefEntry {
    /// The full ref name, such as `refs/heads/main`.
    pub(crate) name: String,
    /// The object name the ref points at.
    pub(crate) object: String,
}

/// One head a bundle carries, as the object and the ref it names.
///
/// `git bundle list-heads` prints the object first, so the adapter reverses
/// the fields it parsed; policy only sees the two values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleHead {
    /// The object name the head points at.
    pub(crate) object: String,
    /// The ref name, such as `refs/heads/main` or `HEAD`.
    pub(crate) name: String,
}

impl fmt::Display for BundleHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.object, self.name)
    }
}

/// What the repository-shape queries reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructureEvidence {
    /// Whether `git rev-parse --is-bare-repository` reported a bare repository.
    pub(crate) is_bare: bool,
    /// Standard output of `git symbolic-ref HEAD`, trimmed.
    pub(crate) head: String,
    /// The refs `git for-each-ref` listed, already split into name and object.
    pub(crate) refs: Vec<RefEntry>,
}

/// Accept a repository's shape and refs, or say what is wrong.
pub(crate) fn accept_structure(
    evidence: &StructureEvidence,
    commit: PinnedCommit<'_>,
) -> Result<(), String> {
    if !evidence.is_bare {
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
fn accept_refs(refs: &[RefEntry], commit: PinnedCommit<'_>) -> Result<(), String> {
    let main = main_ref();
    let extra: Vec<&str> = refs
        .iter()
        .map(|entry| entry.name.as_str())
        .filter(|name| *name != main)
        .collect();
    if !extra.is_empty() {
        return Err(format!(
            "it carries refs other than {main}: {}",
            extra.join(", ")
        ));
    }
    match refs.first() {
        Some(entry) if entry.object == commit.as_str() => Ok(()),
        Some(entry) => Err(format!(
            "{main} is {}, expected the pinned commit {commit}",
            entry.object
        )),
        None => Err(format!("it has no {main}")),
    }
}

/// What the self-containment queries reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfContainedEvidence {
    /// Whether `git rev-parse --is-shallow-repository` reported shallow history.
    pub(crate) is_shallow: bool,
    /// The remotes `git remote` listed, one entry per line.
    pub(crate) remotes: Vec<String>,
}

/// Require complete local history with no outside object sources or remotes.
///
/// `has_alternates` is a filesystem probe rather than a Git query, so the
/// caller that owns the store supplies it alongside the Git evidence.
pub(crate) fn accept_self_contained(
    evidence: &SelfContainedEvidence,
    has_alternates: bool,
) -> Result<(), String> {
    if evidence.is_shallow {
        return Err("its history is shallow".to_owned());
    }
    if has_alternates {
        return Err("it borrows objects through objects/info/alternates".to_owned());
    }
    if !evidence.remotes.is_empty() {
        return Err(format!(
            "it has remotes ({}), so it could fetch",
            evidence.remotes.join(", ")
        ));
    }
    Ok(())
}

/// What the integrity check reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegrityEvidence {
    /// Whether the check exited successfully.
    pub(crate) succeeded: bool,
    /// The diagnostic the adapter recorded, already phrased for the user.
    ///
    /// The adapter owns the command-specific wording, so an acceptance rule
    /// never names another program's flags.
    pub(crate) diagnostic: String,
}

/// Require a clean object database.
pub(crate) fn accept_integrity(evidence: &IntegrityEvidence) -> Result<(), String> {
    if evidence.succeeded {
        return Ok(());
    }
    Err(evidence.diagnostic.clone())
}

/// Require the bundle to carry only `main` and `HEAD` at the pinned commit.
///
/// Unexpected heads are reported before a missing `main` head, and duplicate
/// heads are not rejected: the bundle's own header is the only source here.
pub(crate) fn accept_bundle_heads(
    heads: &[BundleHead],
    commit: PinnedCommit<'_>,
) -> Result<(), String> {
    let main = main_ref();
    let is_expected = |head: &BundleHead| {
        head.object == commit.as_str() && (head.name == main || head.name == "HEAD")
    };
    let unexpected: Vec<String> = heads
        .iter()
        .filter(|head| !is_expected(head))
        .map(ToString::to_string)
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "the bundle must carry only {main} and HEAD at {commit}, but lists: {}",
            unexpected.join("; ")
        ));
    }
    if !heads
        .iter()
        .any(|head| head.object == commit.as_str() && head.name == main)
    {
        return Err(format!(
            "the bundle does not carry {main} at the pinned commit {commit}"
        ));
    }
    Ok(())
}

/// Return whether the published and computed digests agree.
pub(crate) fn digests_match(expected: &str, actual: &str) -> bool { expected == actual }

/// Require the generation metadata to record the pinned commit.
///
/// The warm path trusts this agreement instead of re-hashing the object
/// database, so the decision belongs here with every other acceptance rule
/// rather than beside the metadata read.
pub(crate) fn accept_recorded_commit(recorded: &str, expected: &str) -> Result<(), String> {
    if recorded == expected {
        return Ok(());
    }
    Err(format!(
        "its metadata records commit {recorded}, expected {expected}"
    ))
}

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
/// names no channel. An absent file falls through to the next one. A read that
/// fails stops the walk, because the file's existence is unknown: letting a
/// later file win could pick a channel rustup would never have chosen.
///
/// The error type is the caller's, so policy never names an infrastructure
/// failure: it only decides that a failed read outranks every later file.
pub(crate) fn first_channel<E>(
    results: impl IntoIterator<Item = Result<ChannelRead, E>>,
) -> Result<Option<String>, E> {
    for result in results {
        match result? {
            ChannelRead::Present(channel) => return Ok(channel),
            ChannelRead::Absent => {}
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
