//! Bounded execution of Git commands for the repository adapter.

use crate::error::{InstallerError, Result};
use camino::Utf8Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use wait_timeout::ChildExt;

/// Default timeout for Git operations.
const GIT_TIMEOUT: Duration = Duration::from_secs(300);

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
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || -> std::io::Result<String> {
        stdout_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(|opt| opt.unwrap_or_default())
    });
    let stderr_thread = std::thread::spawn(move || -> std::io::Result<String> {
        stderr_pipe
            .map(std::io::read_to_string)
            .transpose()
            .map(|opt| opt.unwrap_or_default())
    });

    match child.wait_timeout(GIT_TIMEOUT)? {
        Some(status) => {
            let stdout = stdout_thread
                .join()
                .map_err(|_| InstallerError::Git {
                    operation,
                    message: "failed to read stdout".to_owned(),
                })?
                .unwrap_or_default();
            let stderr = stderr_thread
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
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
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
