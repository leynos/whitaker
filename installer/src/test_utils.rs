//! Shared test utilities for the installer crate.

use crate::deps::CommandExecutor;
use crate::error::Result;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::process::{ExitStatus, Output};

/// Creates an `ExitStatus` from an exit code (Unix implementation).
#[cfg(unix)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(code << 8)
}

/// Creates an `ExitStatus` from an exit code (Windows implementation).
#[cfg(windows)]
pub fn exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(code as u32)
}

/// Creates a successful command `Output` with empty stdout and stderr.
pub fn success_output() -> Output {
    Output {
        status: exit_status(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

/// Creates a failed command `Output` with the given stderr message.
pub fn failure_output(stderr: &str) -> Output {
    Output {
        status: exit_status(1),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// Represents an expected command invocation for testing.
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
#[derive(Debug)]
pub struct StubExecutor {
    expected: RefCell<VecDeque<ExpectedCall>>,
}

impl StubExecutor {
    /// Creates a new `StubExecutor` with the given expected calls.
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
    pub fn assert_finished(&self) {
        assert!(
            self.expected.borrow().is_empty(),
            "expected no further command invocations"
        );
    }
}

impl CommandExecutor for StubExecutor {
    fn run(&self, cmd: &str, args: &[&str]) -> Result<Output> {
        let mut expected = self.expected.borrow_mut();
        let call = expected.pop_front().expect("unexpected command invocation");

        assert_eq!(call.cmd, cmd);
        assert_eq!(call.args.as_slice(), args);

        call.result
    }
}
