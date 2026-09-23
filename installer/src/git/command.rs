//! Bounded execution of Git commands for the repository adapter.

use crate::error::{InstallerError, Result};
use camino::Utf8Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use wait_timeout::ChildExt;

/// Default timeout for Git operations.
const GIT_TIMEOUT: Duration = Duration::from_secs(300);

type OutputReaderThread = std::thread::JoinHandle<std::io::Result<String>>;

struct OutputReaders {
    stdout: OutputReaderThread,
    stderr: OutputReaderThread,
}

/// Run one Git operation and retain its output for the repository adapter.
pub(super) fn run_git_with_timeout(
    args: &[&str],
    working_dir: Option<&Utf8Path>,
    operation: &'static str,
) -> Result<Output> {
    let started = Instant::now();
    debug!(operation, outcome = "started", "running Git operation");
    let mut cmd = Command::new("git");
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir.as_std_path());
    }

    let mut child = cmd.spawn()?;
    let readers = spawn_output_readers(&mut child);

    match child.wait_timeout(GIT_TIMEOUT)? {
        Some(status) => collect_completed_output(status, readers, operation, started),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = readers.stdout.join();
            let _ = readers.stderr.join();
            warn!(
                operation,
                outcome = "timeout",
                elapsed_ms = started.elapsed().as_millis() as u64,
                "Git operation timed out"
            );
            Err(InstallerError::Git {
                operation,
                message: format!(
                    "operation timed out after {} seconds",
                    GIT_TIMEOUT.as_secs()
                ),
            })
        }
    }
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
            .map(|opt| opt.unwrap_or_default())
    });
    let stderr = std::thread::spawn(move || -> std::io::Result<String> {
        stderr_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(|opt| opt.unwrap_or_default())
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
        elapsed_ms = started.elapsed().as_millis() as u64,
        "Git operation completed"
    );
    Ok(Output {
        status,
        stdout: stdout.into_bytes(),
        stderr: stderr.into_bytes(),
    })
}
