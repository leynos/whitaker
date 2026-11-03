//! Bounded execution of Git commands for the repository adapter.

use std::{
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use camino::Utf8Path;
use tracing::{debug, warn};
use wait_timeout::ChildExt;

use crate::error::{InstallerError, Result};

/// Default timeout for Git operations.
const GIT_TIMEOUT: Duration = Duration::from_secs(300);

type OutputReaderThread = std::thread::JoinHandle<std::io::Result<String>>;

struct OutputReaders {
    stdout: OutputReaderThread,
    stderr: OutputReaderThread,
}

/// One bounded Git invocation.
///
/// The repository adapter uses the default timeout and the ambient
/// environment; the Clippy snapshot boundary adds transport restrictions,
/// `GIT_TERMINAL_PROMPT=0`, and its own bounds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GitInvocation<'a> {
    /// Arguments after `git`.
    pub(crate) args: &'a [&'a str],
    /// The directory to run in, if not the current one.
    pub(crate) working_dir: Option<&'a Utf8Path>,
    /// The operation name used in logs and errors.
    pub(crate) operation: &'static str,
    /// How long the command may run before it is killed.
    pub(crate) timeout: Duration,
    /// Extra environment entries for the child.
    pub(crate) envs: &'a [(&'a str, &'a str)],
    /// Inherited variables to remove from the child's environment first.
    pub(crate) env_remove: &'a [&'a str],
}

/// Run one Git operation and retain its output for the repository adapter.
pub(super) fn run_git_with_timeout(
    args: &[&str],
    working_dir: Option<&Utf8Path>,
    operation: &'static str,
) -> Result<Output> {
    run_git(&GitInvocation {
        args,
        working_dir,
        operation,
        timeout: GIT_TIMEOUT,
        envs: &[],
        env_remove: &[],
    })
}

/// Run one bounded Git invocation and retain its output.
///
/// # Errors
///
/// Returns `InstallerError::Io` when Git cannot be started and
/// `InstallerError::Git` when it times out or its output cannot be read.
pub(crate) fn run_git(invocation: &GitInvocation<'_>) -> Result<Output> {
    let GitInvocation {
        args,
        working_dir,
        operation,
        timeout,
        envs,
        env_remove,
    } = *invocation;
    let started = Instant::now();
    debug!(operation, outcome = "started", "running Git operation");
    let mut cmd = Command::new("git");
    for name in env_remove {
        cmd.env_remove(name);
    }
    cmd.args(args)
        .envs(envs.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir.as_std_path());
    }

    let mut child = cmd.spawn()?;
    let readers = spawn_output_readers(&mut child);

    if let Some(status) = child.wait_timeout(timeout)? {
        return collect_completed_output(status, readers, operation, started);
    }

    reap_timed_out_child(&mut child, readers);
    log_git_timeout(operation, elapsed_millis(started));
    Err(timed_out_error(operation, timeout))
}

/// Build the timeout error reported when a Git process is abandoned.
fn timed_out_error(operation: &'static str, timeout: Duration) -> InstallerError {
    InstallerError::Git {
        operation,
        message: format!("operation timed out after {} seconds", timeout.as_secs()),
    }
}

/// Stop a Git process that outlived the timeout and release its readers.
///
/// The process is already being abandoned, so every step here is best-effort:
/// a failure to kill or reap it is reported for diagnosis but cannot change
/// the timeout outcome the caller has already decided.
fn reap_timed_out_child(child: &mut std::process::Child, readers: OutputReaders) {
    stop_timed_out_child(child);
    join_output_reader(readers.stdout, "stdout");
    join_output_reader(readers.stderr, "stderr");
}

/// Kill and reap an abandoned Git process, reporting each best-effort failure.
fn stop_timed_out_child(child: &mut std::process::Child) {
    if let Err(error) = child.kill() {
        log_kill_failure(&error);
    }
    if let Err(error) = child.wait() {
        log_reap_failure(&error);
    }
}

/// Report a failed attempt to kill a timed-out Git process.
fn log_kill_failure(error: &std::io::Error) {
    debug!(%error, "failed to kill a timed-out Git process");
}

/// Report a failed attempt to reap a timed-out Git process.
fn log_reap_failure(error: &std::io::Error) {
    debug!(%error, "failed to reap a timed-out Git process");
}

/// Reclaim an output reader, reporting a panic in the reader thread.
fn join_output_reader(reader: OutputReaderThread, stream: &'static str) {
    if reader.join().is_err() {
        debug!(stream, "output reader panicked after a Git timeout");
    }
}

/// Report a Git operation that exceeded its timeout budget.
fn log_git_timeout(operation: &'static str, elapsed_ms: u64) {
    warn!(
        operation,
        outcome = "timeout",
        elapsed_ms,
        "Git operation timed out"
    );
}

/// Milliseconds elapsed since `started`, saturating at the `u64` range.
///
/// A duration that cannot fit in a `u64` of milliseconds represents centuries
/// of execution; reporting the maximum is more useful than wrapping.
fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Start concurrent readers for a child's captured output streams.
///
/// An absent pipe still produces an empty string, matching a captured empty
/// stream.
fn spawn_output_readers(child: &mut std::process::Child) -> OutputReaders {
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout = std::thread::spawn(move || -> std::io::Result<String> {
        stdout_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(Option::unwrap_or_default)
    });
    let stderr = std::thread::spawn(move || -> std::io::Result<String> {
        stderr_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(Option::unwrap_or_default)
    });

    OutputReaders { stdout, stderr }
}

/// Collect output after Git exits, preserving reader order and diagnostics.
fn collect_completed_output(
    status: std::process::ExitStatus,
    readers: OutputReaders,
    operation: &'static str,
    started: Instant,
) -> Result<Output> {
    let stdout = readers
        .stdout
        .join()
        .map_err(|_| InstallerError::Git {
            operation,
            message: "failed to read stdout".to_owned(),
        })?
        .unwrap_or_default();
    let stderr = readers
        .stderr
        .join()
        .map_err(|_| InstallerError::Git {
            operation,
            message: "failed to read stderr".to_owned(),
        })?
        .unwrap_or_default();
    let outcome = if status.success() {
        "success"
    } else {
        "failure"
    };
    debug!(
        operation,
        outcome,
        elapsed_ms = elapsed_millis(started),
        "Git operation completed"
    );
    Ok(Output {
        status,
        stdout: stdout.into_bytes(),
        stderr: stderr.into_bytes(),
    })
}
