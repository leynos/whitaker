//! Git subprocesses for the snapshot boundary, local-only by construction.
//!
//! Installation and verification read nothing but local files, so every
//! command they run forbids every transport except `file`; a bundle clone, a
//! local clone, and repository inspection keep working while an accidental
//! network fetch fails instead of silently reaching public Clippy. Only the
//! producer's [`SnapshotGit::network`] runner may contact a remote.

use std::{process::Output, time::Duration};

use camino::Utf8Path;

use super::{
    error::ClippySnapshotError,
    policy::{IntegrityEvidence, SelfContainedEvidence, StructureEvidence},
    ports::RepositoryQueries,
    selection::TOOLCHAIN_FILES,
};
use crate::git::{GitInvocation, run_git};

#[path = "git/answers.rs"]
mod answers;

#[path = "git/bundle.rs"]
mod bundle;

#[path = "git/listing.rs"]
mod listing;

#[path = "git/scratch.rs"]
mod scratch;

use answers::{boolean_answer, git_failure, integrity_diagnostic};
use listing::{listing_lines, parse_refs};

/// Local operations are bounded so a wedged filesystem cannot hang an install.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(600);

/// The producer's upstream clone transfers the whole Clippy history.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(1800);

/// Options that forbid every transport but `file`.
const LOCAL_ONLY: [&str; 4] = [
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.file.allow=always",
];

/// Prompting would hang a non-interactive installer, so it is always off.
const ENVIRONMENT: [(&str, &str); 1] = [("GIT_TERMINAL_PROMPT", "0")];

/// Inherited variables that choose which repository Git reads or where its
/// objects come from (from `git rev-parse --local-env-vars`). Left in place,
/// `GIT_DIR` or `GIT_ALTERNATE_OBJECT_DIRECTORIES` could point verification at
/// another repository or supply objects from outside the snapshot, so every
/// snapshot command drops them. Configuration variables such as
/// `GIT_CONFIG_COUNT` stay: authenticated clones pass their header that way.
/// `GIT_NO_REPLACE_OBJECTS` stays too, since it only ever disables replacement.
pub(crate) const REPOSITORY_SELECTION_VARIABLES: [&str; 11] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_INDEX_FILE",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
    "GIT_REPLACE_REF_BASE",
    "GIT_NAMESPACE",
];

/// Which transports a runner permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Local,
    Network,
}

/// Runs Git for the snapshot module with a fixed transport policy.
///
/// Extra environment entries ride on the runner rather than on each call;
/// secrets travel that way instead of as arguments, which any user on the
/// host can read from the process table.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SnapshotGit<'env> {
    reach: Reach,
    envs: &'env [(&'env str, &'env str)],
    /// Where throwaway repositories are created; `None` is the system temp
    /// directory. Only tests set it, so the location and cleanup are
    /// observable.
    scratch_parent: Option<&'env Utf8Path>,
    /// The directory a command runs in when the call names none; `None`
    /// inherits the process directory. Only tests set it, so a command that
    /// must work outside a repository can be observed doing so.
    working_dir: Option<&'env Utf8Path>,
}

impl SnapshotGit<'static> {
    /// A runner that can read only local repositories and bundles.
    pub(crate) const fn local() -> Self {
        Self {
            reach: Reach::Local,
            envs: &[],
            scratch_parent: None,
            working_dir: None,
        }
    }

    /// A runner that may fetch from a remote; producer builds only.
    pub(crate) const fn network() -> Self {
        Self {
            reach: Reach::Network,
            envs: &[],
            scratch_parent: None,
            working_dir: None,
        }
    }
}

impl<'env> SnapshotGit<'env> {
    /// The same runner with `envs` added to every command's environment.
    pub(crate) const fn with_env(self, envs: &'env [(&'env str, &'env str)]) -> Self {
        Self {
            reach: self.reach,
            envs,
            scratch_parent: self.scratch_parent,
            working_dir: self.working_dir,
        }
    }

    /// The same runner, creating throwaway repositories under `parent`.
    ///
    /// Only tests call this, so the scratch location and its cleanup can be
    /// observed without guessing at the system temp directory.
    #[cfg(test)]
    pub(crate) const fn with_scratch_parent(self, parent: &'env Utf8Path) -> Self {
        Self {
            reach: self.reach,
            envs: self.envs,
            scratch_parent: Some(parent),
            working_dir: self.working_dir,
        }
    }

    /// The same runner, running dir-less commands in `dir`.
    ///
    /// Only tests call this, so a command that promises to work outside a
    /// repository is observed from a directory that is not one.
    #[cfg(test)]
    pub(crate) const fn with_working_directory(self, dir: &'env Utf8Path) -> Self {
        Self {
            reach: self.reach,
            envs: self.envs,
            scratch_parent: self.scratch_parent,
            working_dir: Some(dir),
        }
    }

    /// Run `args`, requiring success, and return trimmed standard output.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when Git cannot start, times out,
    /// or exits unsuccessfully.
    pub(crate) fn stdout(
        self,
        operation: &'static str,
        dir: Option<&Utf8Path>,
        args: &[&str],
    ) -> Result<String, ClippySnapshotError> {
        let output = self.output(operation, dir, args)?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(git_failure(operation, &output))
        }
    }

    /// Run `args` and return the raw output whatever the exit status.
    ///
    /// # Errors
    ///
    /// Returns [`ClippySnapshotError::Git`] when Git cannot start or times out.
    #[tracing::instrument(name = "clippy_snapshot.git", skip_all, fields(operation = operation))]
    pub(crate) fn output(
        self,
        operation: &'static str,
        dir: Option<&Utf8Path>,
        args: &[&str],
    ) -> Result<Output, ClippySnapshotError> {
        let envs: Vec<(&str, &str)> = ENVIRONMENT.iter().chain(self.envs).copied().collect();
        let owned_args = self.full_args(args);
        let full_args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
        let invocation = GitInvocation {
            args: &full_args,
            working_dir: dir,
            operation,
            timeout: self.timeout(),
            envs: &envs,
            env_remove: &REPOSITORY_SELECTION_VARIABLES,
        };
        run_git(&invocation).map_err(|error| ClippySnapshotError::Git {
            operation,
            message: error.to_string(),
        })
    }

    /// Prefix the transport policy to the caller's arguments.
    fn full_args(self, args: &[&str]) -> Vec<String> {
        let policy: &[&str] = match self.reach {
            Reach::Local => &LOCAL_ONLY,
            Reach::Network => &[],
        };
        policy
            .iter()
            .chain(args)
            .map(|arg| (*arg).to_owned())
            .collect()
    }

    /// The bound for one command under this runner's policy.
    const fn timeout(self) -> Duration {
        match self.reach {
            Reach::Local => LOCAL_TIMEOUT,
            Reach::Network => NETWORK_TIMEOUT,
        }
    }
}

impl RepositoryQueries for SnapshotGit<'_> {
    /// Observe the repository's shape, symbolic HEAD, and refs.
    fn structure(&self, path: &Utf8Path) -> Result<StructureEvidence, ClippySnapshotError> {
        let bare = self.stdout(
            "rev-parse",
            Some(path),
            &["rev-parse", "--is-bare-repository"],
        )?;
        let is_bare = boolean_answer("rev-parse", "bare repository", &bare)?;
        // A detached HEAD (exit 1 under `--quiet`) is evidence, not a failure:
        // the policy reports it. Any other failure, such as a malformed or
        // unreadable HEAD, is an infrastructure fault and stays an error, so
        // the policy cannot blame the snapshot for it.
        let symbolic = self.output(
            "symbolic-ref",
            Some(path),
            &["symbolic-ref", "--quiet", "HEAD"],
        )?;
        let head = match symbolic.status.code() {
            Some(0 | 1) => String::from_utf8_lossy(&symbolic.stdout).trim().to_owned(),
            _ => return Err(git_failure("symbolic-ref", &symbolic)),
        };
        let refs = self.stdout(
            "for-each-ref",
            Some(path),
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        )?;
        Ok(StructureEvidence {
            is_bare,
            head,
            refs: parse_refs(&refs),
        })
    }

    /// Observe whether history is shallow and which remotes exist.
    fn self_contained(
        &self,
        path: &Utf8Path,
    ) -> Result<SelfContainedEvidence, ClippySnapshotError> {
        let shallow = self.stdout(
            "rev-parse",
            Some(path),
            &["rev-parse", "--is-shallow-repository"],
        )?;
        let is_shallow = boolean_answer("rev-parse", "shallow repository", &shallow)?;
        let remotes = self.stdout("remote", Some(path), &["remote"])?;
        Ok(SelfContainedEvidence {
            is_shallow,
            remotes: listing_lines(&remotes),
        })
    }

    /// Re-hash every object and check connectivity.
    fn integrity(&self, path: &Utf8Path) -> Result<IntegrityEvidence, ClippySnapshotError> {
        let output = self.output(
            "fsck",
            Some(path),
            &[
                "fsck",
                "--full",
                "--strict",
                "--no-dangling",
                "--no-progress",
            ],
        )?;
        let succeeded = output.status.success();
        Ok(IntegrityEvidence {
            succeeded,
            diagnostic: integrity_diagnostic(succeeded, &output.stderr),
        })
    }

    /// List the first-parent commits that touched a toolchain file.
    fn toolchain_commits(
        &self,
        path: &Utf8Path,
        head: &str,
    ) -> Result<Vec<String>, ClippySnapshotError> {
        let mut args = vec!["rev-list", "--first-parent", head, "--"];
        args.extend(TOOLCHAIN_FILES);
        let listing = self.stdout("rev-list", Some(path), &args)?;
        Ok(listing.lines().map(ToOwned::to_owned).collect())
    }

    /// Read one file at one commit, keeping absence distinct from failure.
    fn file_at_commit(
        &self,
        path: &Utf8Path,
        commit: &str,
        file: &str,
    ) -> Result<Option<String>, ClippySnapshotError> {
        // The tree decides whether the path exists, so a read that fails
        // afterwards stays an error. Treating it as absence would let a
        // lower-priority toolchain file win, which rustup never would.
        let listing = self.stdout(
            "ls-tree",
            Some(path),
            &["ls-tree", "--name-only", commit, "--", file],
        )?;
        if listing.is_empty() {
            return Ok(None);
        }
        let object = format!("{commit}:{file}");
        let output = self.output("show", Some(path), &["show", &object])?;
        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
        } else {
            Err(git_failure("show", &output))
        }
    }
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
