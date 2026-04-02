//! Shared test utilities for the installer crate.

#[cfg(any(test, feature = "test-support"))]
use crate::deps::CommandExecutor;
use crate::dirs::BaseDirs;
#[cfg(any(test, feature = "test-support"))]
use crate::error::InstallerError;
use crate::error::Result;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
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

/// Minimal directory stub for tests that only care about the binary path.
#[derive(Debug, Clone, Default)]
pub struct StubDirs {
    /// Directory returned by [`BaseDirs::bin_dir`].
    pub bin_dir: Option<PathBuf>,
}

impl BaseDirs for StubDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }

    fn bin_dir(&self) -> Option<PathBuf> {
        self.bin_dir.clone()
    }

    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        None
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

/// Test helpers for dependency binary installation behaviour tests.
#[cfg(any(test, feature = "test-support"))]
pub mod dependency_binary_helpers {
    use super::{ExpectedCall, failure_output, success_output};

    /// Configuration for generating expected calls in dependency binary tests.
    pub struct ExpectedCallConfig<'a> {
        /// Whether cargo-binstall is available.
        pub binstall_available: bool,
        /// Whether to verify repository installation.
        pub verify_repository_install: bool,
        /// Whether repository verification should fail.
        pub verification_fails: bool,
        /// Error message for cargo binstall failure (None if succeeds).
        pub cargo_binstall_failure: Option<&'a str>,
        /// Error message for cargo install failure (None if succeeds).
        pub cargo_install_failure: Option<&'a str>,
    }

    /// Creates an expected call for checking cargo-binstall availability.
    pub fn binstall_version_check(binstall_available: bool) -> ExpectedCall {
        ExpectedCall {
            cmd: "cargo",
            args: vec!["binstall", "--version"],
            result: if binstall_available {
                Ok(success_output())
            } else {
                Ok(failure_output("missing binstall"))
            },
        }
    }

    /// Creates an expected call for verifying repository installation.
    pub fn repository_verification_call(tool: &str, verification_fails: bool) -> ExpectedCall {
        let result = if verification_fails {
            Ok(failure_output("still missing"))
        } else {
            Ok(success_output())
        };
        match tool {
            "cargo-dylint" => ExpectedCall {
                cmd: "cargo",
                args: vec!["dylint", "--version"],
                result,
            },
            "dylint-link" => ExpectedCall {
                cmd: "dylint-link",
                args: vec!["--version"],
                result,
            },
            other => panic!("unexpected tool: {other}"),
        }
    }

    /// Returns the expected verification call for a given tool.
    fn tool_verification_check(tool: &str) -> ExpectedCall {
        match tool {
            "cargo-dylint" => cargo_dylint_check(),
            "dylint-link" => dylint_link_check(),
            other => panic!("unexpected tool: {other}"),
        }
    }

    /// Builds the sequence of calls that follow the primary install attempt.
    #[allow(clippy::too_many_arguments)]
    fn post_primary_calls(
        tool: &str,
        tool_static: &'static str,
        primary_succeeded: bool,
        use_binstall: bool,
        cargo_install_failure: Option<&str>,
    ) -> Vec<ExpectedCall> {
        if primary_succeeded {
            return vec![tool_verification_check(tool)];
        }
        if !use_binstall {
            return vec![];
        }
        // binstall failed: check if we should sequence a cargo-install attempt
        if cargo_install_failure.is_none() {
            // cargo install succeeds after binstall fails
            let cargo_call = ExpectedCall {
                cmd: "cargo",
                args: vec!["install", tool_static],
                result: Ok(success_output()),
            };
            return vec![cargo_call, tool_verification_check(tool)];
        }
        // binstall failed and cargo install also fails
        let cargo_call = ExpectedCall {
            cmd: "cargo",
            args: vec!["install", tool_static],
            result: Ok(failure_output(cargo_install_failure.unwrap())),
        };
        vec![cargo_call]
    }

    /// Creates expected calls for cargo fallback installation (binstall or install).
    pub fn cargo_fallback_calls(
        tool: &str,
        binstall_available: bool,
        cargo_binstall_failure: Option<&str>,
        cargo_install_failure: Option<&str>,
    ) -> Vec<ExpectedCall> {
        // Intentional leak in tests to extend lifetime for static string usage;
        // acceptable here as it will not be freed.
        let tool_static: &'static str = Box::leak(tool.to_owned().into_boxed_str());

        let (use_binstall, failure_message) = if binstall_available {
            (true, cargo_binstall_failure)
        } else {
            (false, cargo_install_failure)
        };

        let install_call = ExpectedCall {
            cmd: "cargo",
            args: if use_binstall {
                vec!["binstall", "-y", tool_static]
            } else {
                vec!["install", tool_static]
            },
            result: Ok(match failure_message {
                Some(message) => failure_output(message),
                None => success_output(),
            }),
        };

        let mut calls = vec![install_call];
        calls.extend(post_primary_calls(
            tool,
            tool_static,
            failure_message.is_none(),
            use_binstall,
            cargo_install_failure,
        ));
        calls
    }

    /// Builds the complete list of expected calls for a dependency binary test scenario.
    pub fn expected_calls(tool: &str, config: ExpectedCallConfig<'_>) -> Vec<ExpectedCall> {
        let mut calls = vec![binstall_version_check(config.binstall_available)];

        if config.verify_repository_install {
            calls.push(repository_verification_call(
                tool,
                config.verification_fails,
            ));
            if !config.verification_fails {
                return calls;
            }
        }

        calls.extend(cargo_fallback_calls(
            tool,
            config.binstall_available,
            config.cargo_binstall_failure,
            config.cargo_install_failure,
        ));
        calls
    }

    /// Creates an expected call for verifying cargo-dylint installation.
    pub fn cargo_dylint_check() -> ExpectedCall {
        ExpectedCall {
            cmd: "cargo",
            args: vec!["dylint", "--version"],
            result: Ok(success_output()),
        }
    }

    /// Creates an expected call for verifying dylint-link installation.
    pub fn dylint_link_check() -> ExpectedCall {
        ExpectedCall {
            cmd: "dylint-link",
            args: vec!["--version"],
            result: Ok(success_output()),
        }
    }
}
