//! Reproduces Dylint's choice of Clippy revision for a dated nightly.
//!
//! Dylint 6.3.0 (`dylint_internal::clippy_utils::Revs::channel_iter`) walks the
//! first-parent history back from `HEAD`. It groups consecutive commits that
//! share a `rust-toolchain` channel and yields the oldest commit of each group:
//! the commit that introduced that channel. The driver build takes the first
//! yielded commit whose channel sorts at or before the requested nightly. A
//! group is only yielded once the walk has seen an older commit with a
//! different channel, so the selected revision needs its first parent too.
//!
//! The channel only changes where the toolchain file changes, so the walk is
//! reproduced from the first-parent commits that touch `rust-toolchain` or
//! `rust-toolchain.toml` rather than by checking out every commit.

use super::error::ClippySnapshotError;
use super::git::SnapshotGit;
use camino::Utf8Path;

/// The toolchain files Dylint reads, in rustup's precedence order.
const TOOLCHAIN_FILES: [&str; 2] = ["rust-toolchain", "rust-toolchain.toml"];

/// A first-parent commit that touched the toolchain file, and its channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainChange {
    /// The commit object ID.
    pub commit: String,
    /// The channel the toolchain file names at that commit.
    pub channel: String,
}

/// Why no revision could be selected from the available history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionGap {
    /// A matching channel was found, but not the commit that introduced it.
    MissingIntroduction {
        /// The oldest available commit on the matching channel.
        oldest_commit: String,
        /// The channel it names.
        channel: String,
    },
    /// Every available channel sorts after the requested nightly.
    NoEarlierChannel {
        /// The oldest channel the history reaches.
        oldest_channel: Option<String>,
    },
}

impl std::fmt::Display for SelectionGap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingIntroduction {
                oldest_commit,
                channel,
            } => write!(
                formatter,
                "the history ends at {oldest_commit} on {channel} without the \
                 commit that introduced {channel}"
            ),
            Self::NoEarlierChannel {
                oldest_channel: Some(channel),
            } => write!(formatter, "the oldest reachable channel is {channel}"),
            Self::NoEarlierChannel {
                oldest_channel: None,
            } => formatter.write_str("no commit names a toolchain channel"),
        }
    }
}

/// Select the revision Dylint would build its driver from.
///
/// `changes` lists the first-parent commits that touched the toolchain file,
/// newest first, ending where the reachable history ends. The comparison is
/// Dylint's own: a plain string comparison of channel names.
///
/// # Errors
///
/// Returns a [`SelectionGap`] when the history cannot support a selection.
///
/// # Examples
///
/// ```
/// use whitaker_installer::clippy_snapshot::{ToolchainChange, select_revision};
///
/// let change = |commit: &str, channel: &str| ToolchainChange {
///     commit: commit.to_owned(),
///     channel: channel.to_owned(),
/// };
/// let history = [
///     change("c3", "nightly-2026-06-11"),
///     change("c2", "nightly-2026-05-28"),
///     change("c1", "nightly-2026-05-13"),
/// ];
/// let selected = select_revision(&history, "nightly-2026-05-28").expect("selected");
/// assert_eq!(selected.commit, "c2");
/// ```
pub fn select_revision<'a>(
    changes: &'a [ToolchainChange],
    toolchain: &str,
) -> Result<&'a ToolchainChange, SelectionGap> {
    for (index, change) in changes.iter().enumerate() {
        let Some(older) = changes.get(index + 1) else {
            return Err(if change.channel.as_str() <= toolchain {
                SelectionGap::MissingIntroduction {
                    oldest_commit: change.commit.clone(),
                    channel: change.channel.clone(),
                }
            } else {
                SelectionGap::NoEarlierChannel {
                    oldest_channel: Some(change.channel.clone()),
                }
            });
        };
        let introduces_channel = older.channel != change.channel;
        if introduces_channel && change.channel.as_str() <= toolchain {
            return Ok(change);
        }
    }
    Err(SelectionGap::NoEarlierChannel {
        oldest_channel: None,
    })
}

/// Read the toolchain changes on `head`'s first-parent history.
///
/// The list stops before the first commit whose toolchain file is absent or
/// names no TOML channel, such as Clippy's early plain-text `rust-toolchain`
/// files. Dylint's walk fails at such a commit rather than reading past it,
/// so history beyond it cannot support a selection.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Git`] when the history cannot be read.
pub(crate) fn toolchain_changes(
    git: SnapshotGit<'_>,
    repo: &Utf8Path,
    head: &str,
) -> Result<Vec<ToolchainChange>, ClippySnapshotError> {
    let mut args = vec!["rev-list", "--first-parent", head, "--"];
    args.extend(TOOLCHAIN_FILES);
    let commits = git.stdout("rev-list", Some(repo), &args)?;
    let mut changes = Vec::new();
    for commit in commits.lines() {
        let Some(channel) = channel_at(git, repo, commit)? else {
            break;
        };
        changes.push(ToolchainChange {
            commit: commit.to_owned(),
            channel,
        });
    }
    Ok(changes)
}

/// Read the channel the toolchain file names at `commit`, as Dylint reads it.
///
/// Rustup's precedence applies: `rust-toolchain` wins over
/// `rust-toolchain.toml` when both exist. `None` means Dylint could not read
/// a channel there.
///
/// # Errors
///
/// Returns [`ClippySnapshotError::Git`] when Git cannot run.
pub(crate) fn channel_at(
    git: SnapshotGit<'_>,
    repo: &Utf8Path,
    commit: &str,
) -> Result<Option<String>, ClippySnapshotError> {
    for file in TOOLCHAIN_FILES {
        let object = format!("{commit}:{file}");
        let output = git.output("show", Some(repo), &["show", &object])?;
        if output.status.success() {
            return Ok(parse_channel(&String::from_utf8_lossy(&output.stdout)));
        }
    }
    Ok(None)
}

/// Extract `toolchain.channel` from a toolchain file, as Dylint does.
fn parse_channel(contents: &str) -> Option<String> {
    let table = toml::from_str::<toml::Table>(contents).ok()?;
    table
        .get("toolchain")?
        .as_table()?
        .get("channel")?
        .as_str()
        .map(ToOwned::to_owned)
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
