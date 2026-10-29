//! Git subprocesses for the snapshot boundary, local-only by construction.
//!
//! Installation and verification read nothing but local files, so every
//! command they run forbids every transport except `file`; a bundle clone, a
//! local clone, and repository inspection keep working while an accidental
//! network fetch fails instead of silently reaching public Clippy. Only the
//! producer's [`SnapshotGit::network`] runner may contact a remote.

use super::error::ClippySnapshotError;
use crate::git::{GitInvocation, run_git};
use camino::Utf8Path;
use std::process::Output;
use std::time::Duration;

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
        let full_args = self.full_args(args);
        let full_args: Vec<&str> = full_args.iter().map(String::as_str).collect();
        let invocation = GitInvocation {
            args: &full_args,
            working_dir: dir,
            operation,
            timeout: self.timeout(),
            envs: &envs,
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

/// Describe a failed Git command from its standard error.
pub(crate) fn git_failure(operation: &'static str, output: &Output) -> ClippySnapshotError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = match stderr.trim() {
        "" => format!("exited with {}", output.status),
        trimmed => trimmed.to_owned(),
    };
    ClippySnapshotError::Git { operation, message }
}
