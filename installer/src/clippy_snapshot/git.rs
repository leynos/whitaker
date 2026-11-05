//! Git subprocesses for the snapshot boundary, local-only by construction.
//!
//! Installation and verification read nothing but local files, so every
//! command they run forbids every transport except `file`; a bundle clone, a
//! local clone, and repository inspection keep working while an accidental
//! network fetch fails instead of silently reaching public Clippy. Only the
//! producer's [`SnapshotGit::network`] runner may contact a remote.

use std::{process::Output, time::Duration};

use camino::{Utf8Path, Utf8PathBuf};

use super::{
    error::ClippySnapshotError,
    policy::{IntegrityEvidence, SelfContainedEvidence, StructureEvidence},
    ports::{BundleOperations, RepositoryQueries},
    selection::TOOLCHAIN_FILES,
};
use crate::git::{GitInvocation, run_git};

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
}

impl SnapshotGit<'static> {
    /// A runner that can read only local repositories and bundles.
    pub(crate) const fn local() -> Self {
        Self {
            reach: Reach::Local,
            envs: &[],
        }
    }

    /// A runner that may fetch from a remote; producer builds only.
    pub(crate) const fn network() -> Self {
        Self {
            reach: Reach::Network,
            envs: &[],
        }
    }
}

impl<'env> SnapshotGit<'env> {
    /// The same runner with `envs` added to every command's environment.
    pub(crate) const fn with_env(self, envs: &'env [(&'env str, &'env str)]) -> Self {
        Self {
            reach: self.reach,
            envs,
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
    fn structure(&self, path: &Utf8Path) -> Result<StructureEvidence, ClippySnapshotError> {
        let is_bare = self.stdout(
            "rev-parse",
            Some(path),
            &["rev-parse", "--is-bare-repository"],
        )?;
        // A detached HEAD is evidence, not a failure: the policy reports it.
        let head = self.output("symbolic-ref", Some(path), &["symbolic-ref", "HEAD"])?;
        let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
        let refs = self.stdout(
            "for-each-ref",
            Some(path),
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        )?;
        Ok(StructureEvidence {
            is_bare,
            head,
            refs,
        })
    }

    fn self_contained(
        &self,
        path: &Utf8Path,
    ) -> Result<SelfContainedEvidence, ClippySnapshotError> {
        let is_shallow = self.stdout(
            "rev-parse",
            Some(path),
            &["rev-parse", "--is-shallow-repository"],
        )?;
        let remotes = self.stdout("remote", Some(path), &["remote"])?;
        Ok(SelfContainedEvidence {
            is_shallow,
            remotes,
        })
    }

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
        Ok(IntegrityEvidence {
            succeeded: output.status.success(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

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

impl BundleOperations for SnapshotGit<'_> {
    fn verify_header(&self, bundle: &Utf8Path) -> Result<(), ClippySnapshotError> {
        let (_scratch, scratch) = self.scratch_repository(bundle)?;
        let verify = self.output(
            "bundle verify",
            Some(&scratch),
            &["bundle", "verify", bundle.as_str()],
        )?;
        if verify.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&verify.stderr);
        Err(ClippySnapshotError::invalid(
            bundle,
            format!("git bundle verify failed: {}", stderr.trim()),
        ))
    }

    fn list_heads(&self, bundle: &Utf8Path) -> Result<String, ClippySnapshotError> {
        let (_scratch, scratch) = self.scratch_repository(bundle)?;
        self.stdout(
            "bundle list-heads",
            Some(&scratch),
            &["bundle", "list-heads", bundle.as_str()],
        )
    }

    fn stage_clone(
        &self,
        bundle: &Utf8Path,
        destination: &Utf8Path,
    ) -> Result<(), ClippySnapshotError> {
        // `git bundle verify` reads only the header, so a truncated or
        // damaged pack first shows up here. Only a clone that ran and failed
        // says the bundle is invalid; a Git that could not start or timed out
        // is an infrastructure failure and keeps its own error.
        let clone = self.output(
            "clone",
            None,
            &[
                "clone",
                "--quiet",
                "--bare",
                bundle.as_str(),
                destination.as_str(),
            ],
        )?;
        if !clone.status.success() {
            let stderr = String::from_utf8_lossy(&clone.stderr);
            return Err(ClippySnapshotError::invalid(
                bundle,
                format!("the bundle could not be cloned: {}", stderr.trim()),
            ));
        }
        self.stdout(
            "remote remove",
            Some(destination),
            &["remote", "remove", "origin"],
        )?;
        Ok(())
    }
}

impl SnapshotGit<'_> {
    /// Create an empty bare repository for bundle header commands.
    ///
    /// `git bundle verify` insists on a repository even for a bundle without
    /// prerequisites. The returned `TempDir` must outlive the commands run in
    /// the returned path.
    fn scratch_repository(
        self,
        bundle: &Utf8Path,
    ) -> Result<(tempfile::TempDir, Utf8PathBuf), ClippySnapshotError> {
        let scratch =
            tempfile::tempdir().map_err(|error| ClippySnapshotError::io(bundle, &error))?;
        let path = Utf8Path::from_path(scratch.path())
            .ok_or_else(|| ClippySnapshotError::Io {
                path: scratch.path().display().to_string(),
                reason: "the path is not valid UTF-8".to_owned(),
            })?
            .to_owned();
        self.stdout("init", None, &["init", "--quiet", "--bare", path.as_str()])?;
        Ok((scratch, path))
    }
}

/// Describe a failed Git command from its standard error.
pub(crate) fn git_failure(operation: &'static str, output: &Output) -> ClippySnapshotError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = match stderr.trim() {
        "" => format!("exited with {}", output.status),
        trimmed => trimmed.to_owned(),
    };
    ClippySnapshotError::Git { operation, message }
}
