//! Shared test utilities for the installer crate.

#[cfg(any(test, feature = "test-support"))]
use crate::deps::CommandExecutor;
#[cfg(any(test, feature = "test-support"))]
use crate::error::InstallerError;
use crate::error::Result;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::process::{ExitStatus, Output};

/// Creates an `ExitStatus` from an exit code (Unix implementation).
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::exit_status;
///
/// let success = exit_status(0);
/// assert!(success.success());
///
/// let failure = exit_status(1);
/// assert!(!failure.success());
/// ```
#[cfg(unix)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

/// Creates an `ExitStatus` from an exit code (Windows implementation).
///
/// # Examples
///
/// ```ignore
/// use whitaker_installer::test_utils::exit_status;
///
/// let success = exit_status(0);
/// assert!(success.success());
///
/// let failure = exit_status(1);
/// assert!(!failure.success());
/// ```
#[cfg(windows)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code as u32)
}

/// Creates a successful command `Output` with empty stdout and stderr.
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::success_output;
///
/// let output = success_output();
/// assert!(output.status.success());
/// assert!(output.stdout.is_empty());
/// assert!(output.stderr.is_empty());
/// ```
pub fn success_output() -> Output {
    Output {
        status: exit_status(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

/// Creates a failed command `Output` with the given stderr message.
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::failure_output;
///
/// let output = failure_output("command failed");
/// assert!(!output.status.success());
/// assert!(output.stdout.is_empty());
/// assert_eq!(output.stderr, b"command failed");
/// ```
pub fn failure_output(stderr: impl AsRef<str>) -> Output {
    let stderr = stderr.as_ref();
    Output {
        status: exit_status(1),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// Compute the SHA-256 hex digest of a byte slice for test fixtures.
#[cfg(any(test, feature = "test-support"))]
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(data))
}

/// Build prebuilt manifest JSON for tests with configurable fields.
#[cfg(any(test, feature = "test-support"))]
pub fn prebuilt_manifest_json(
    toolchain: impl AsRef<str>,
    target: impl AsRef<str>,
    sha256: impl AsRef<str>,
) -> String {
    let toolchain = toolchain.as_ref();
    let target = target.as_ref();
    let sha256 = sha256.as_ref();
    format!(
        concat!(
            r#"{{"git_sha":"abc1234","schema_version":1,"#,
            r#""toolchain":"{toolchain}","#,
            r#""target":"{target}","#,
            r#""generated_at":"2026-02-03T00:00:00Z","#,
            r#""files":["libwhitaker_suite.so"],"#,
            r#""sha256":"{sha256}"}}"#,
        ),
        toolchain = toolchain,
        target = target,
        sha256 = sha256,
    )
}

/// Represents an expected command invocation for testing.
///
/// # Examples
///
/// ```
/// use whitaker_installer::test_utils::{ExpectedCall, success_output};
///
/// let call = ExpectedCall {
///     cmd: "cargo",
///     args: vec!["build", "--release"],
///     result: Ok(success_output()),
/// };
///
/// assert_eq!(call.cmd, "cargo");
/// assert_eq!(call.args, vec!["build", "--release"]);
/// assert!(call.result.is_ok());
/// ```
#[derive(Debug)]
pub struct ExpectedCall {
    /// The command to execute (e.g., "cargo").
    pub cmd: &'static str,
    /// The arguments to pass to the command.
    pub args: Vec<&'static str>,
    /// The result to return when this command is invoked.
    pub result: Result<Output>,
}

/// A stub implementation of `CommandExecutor` for testing.
///
/// Records expected command invocations and returns predefined results,
/// allowing tests to verify command execution without side effects.
///
/// # Examples
///
/// ```
/// use whitaker_installer::deps::CommandExecutor;
/// use whitaker_installer::test_utils::{ExpectedCall, StubExecutor, success_output};
///
/// let executor = StubExecutor::new(vec![
///     ExpectedCall {
///         cmd: "cargo",
///         args: vec!["--version"],
///         result: Ok(success_output()),
///     },
/// ]);
///
/// let output = executor.run("cargo", &["--version"]).expect("command failed");
/// assert!(output.status.success());
///
/// executor.assert_finished();
/// ```
#[derive(Debug)]
pub struct StubExecutor {
    expected: RefCell<VecDeque<ExpectedCall>>,
}

impl StubExecutor {
    /// Creates a new `StubExecutor` with the given expected calls.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::test_utils::{ExpectedCall, StubExecutor, success_output};
    ///
    /// let executor = StubExecutor::new(vec![
    ///     ExpectedCall {
    ///         cmd: "cargo",
    ///         args: vec!["build"],
    ///         result: Ok(success_output()),
    ///     },
    /// ]);
    /// ```
    pub fn new(expected: Vec<ExpectedCall>) -> Self {
        Self {
            expected: RefCell::new(expected.into()),
        }
    }

    /// Asserts that all expected command invocations have been consumed.
    ///
    /// # Panics
    ///
    /// Panics if there are remaining expected calls that were not invoked.
    ///
    /// # Examples
    ///
    /// ```
    /// use whitaker_installer::deps::CommandExecutor;
    /// use whitaker_installer::test_utils::{ExpectedCall, StubExecutor, success_output};
    ///
    /// let executor = StubExecutor::new(vec![
    ///     ExpectedCall {
    ///         cmd: "cargo",
    ///         args: vec!["test"],
    ///         result: Ok(success_output()),
    ///     },
    /// ]);
    ///
    /// // Execute the expected command
    /// let _ = executor.run("cargo", &["test"]);
    ///
    /// // Verify all expected calls were consumed
    /// executor.assert_finished();
    /// ```
    pub fn assert_finished(&self) {
        assert!(
            self.expected.borrow().is_empty(),
            "expected no further command invocations"
        );
    }
}

#[cfg(any(test, feature = "test-support"))]
impl CommandExecutor for StubExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let mut expected = self.expected.borrow_mut();
        let call = expected
            .pop_front()
            .ok_or_else(|| InstallerError::StubMismatch {
                message: format!("unexpected command invocation: {cmd} {}", args.join(" ")),
            })?;

        if call.cmd != cmd {
            return Err(InstallerError::StubMismatch {
                message: format!("command mismatch: expected {}, got {cmd}", call.cmd),
            });
        }

        if call.args.as_slice() != args {
            return Err(InstallerError::StubMismatch {
                message: format!(
                    "argument mismatch for {cmd}: expected {:?}, got {args:?}",
                    call.args
                ),
            });
        }

        call.result
    }
}
