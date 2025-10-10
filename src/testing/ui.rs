//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralises that boilerplate so each lint crate can depend on a
//! small helper rather than repeat the same function and validation logic.

use std::{
    any::Any,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use camino::{Utf8Path, Utf8PathBuf};

/// Errors produced when preparing or executing Dylint UI tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarnessError {
    /// The harness requires the lint crate name to be populated.
    EmptyCrateName,
    /// The harness requires the UI test directory to be provided.
    EmptyDirectory,
    /// UI tests must live within the crate so the path may not be absolute.
    AbsoluteDirectory { directory: Utf8PathBuf },
    /// The underlying runner reported a failure (for example, a diff mismatch).
    RunnerFailure {
        crate_name: String,
        directory: Utf8PathBuf,
        message: String,
    },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCrateName => formatter.write_str("crate name must not be empty"),
            Self::EmptyDirectory => formatter.write_str("UI test directory must not be empty"),
            Self::AbsoluteDirectory { directory } => {
                write!(formatter, "UI test directory must be relative: {directory}")
            }
            Self::RunnerFailure {
                crate_name,
                directory,
                message,
            } => {
                write!(
                    formatter,
                    "running UI tests for {crate_name} in {directory} failed: {message}",
                )
            }
        }
    }
}

impl std::error::Error for HarnessError {}

/// Run UI tests for an explicit crate name.
///
/// This is primarily useful for meta-crates that host several lint libraries in
/// a workspace yet share a single test harness.
///
/// # Examples
///
/// ```ignore
/// fn run_suite_tests() {
///     whitaker::testing::ui::run_for("suite", "tests/ui")
///         .expect("suite UI tests should succeed");
/// }
/// ```
///
/// # Errors
///
/// Returns [`HarnessError`] when the crate name or UI directory are invalid or
/// when the supplied runner reports a failure while executing Dylint UI tests.
pub fn run_for(crate_name: &str, ui_directory: impl Into<Utf8PathBuf>) -> Result<(), HarnessError> {
    run_with_runner(crate_name, ui_directory, default_runner)
}

/// Run UI tests using a custom runner.
///
/// The caller supplies the runner so tests can replace the default implementation
/// with a stub while verifying how the harness validates and prepares inputs.
///
/// # Errors
///
/// Returns [`HarnessError`] when either input validation fails or the provided
/// runner reports a failure.
pub fn run_with_runner(
    crate_name: &str,
    ui_directory: impl Into<Utf8PathBuf>,
    runner: impl Fn(&str, &Utf8Path) -> Result<(), String>,
) -> Result<(), HarnessError> {
    let trimmed = crate_name.trim();
    if trimmed.is_empty() {
        return Err(HarnessError::EmptyCrateName);
    }

    let directory: Utf8PathBuf = ui_directory.into();
    if directory.as_str().trim().is_empty() {
        return Err(HarnessError::EmptyDirectory);
    }

    if directory.is_absolute() {
        return Err(HarnessError::AbsoluteDirectory { directory });
    }

    match runner(trimmed, directory.as_ref()) {
        Ok(()) => Ok(()),
        Err(message) => Err(HarnessError::RunnerFailure {
            crate_name: trimmed.to_string(),
            directory,
            message,
        }),
    }
}

fn default_runner(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| {
        dylint_testing::ui_test(crate_name, directory);
    }))
    .map_err(panic_message)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload.downcast::<String>().map_or_else(
        |payload| {
            payload.downcast::<&'static str>().map_or_else(
                |_| "dylint UI tests panicked without a message".to_string(),
                |message| (*message).to_string(),
            )
        },
        |message| *message,
    )
}

/// Run UI tests for the crate that invokes the macro.
///
/// # Examples
///
/// ```ignore
/// whitaker::run_ui_tests!("ui").expect("UI tests should succeed");
/// ```
///
/// # Errors
///
/// Returns [`HarnessError`] when the UI directory is invalid or when the
/// underlying runner reports a failure.
#[macro_export]
macro_rules! run_ui_tests {
    ($directory:expr $(,)?) => {{
        let crate_name = env!("CARGO_PKG_NAME");
        $crate::testing::ui::run_for(crate_name, $directory)
    }};
}

/// Declare a canonical Dylint UI test for the current crate.
///
/// # Examples
///
/// ```ignore
/// whitaker::declare_ui_tests!("ui");
/// ```
#[macro_export]
macro_rules! declare_ui_tests {
    ($directory:expr $(,)?) => {
        #[test]
        fn ui() {
            $crate::run_ui_tests!($directory).expect("UI tests should execute without diffs");
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{HarnessError, run_with_runner};
    use camino::{Utf8Path, Utf8PathBuf};

    #[test]
    fn rejects_empty_crate_names() {
        let Err(error) = run_with_runner("  ", "ui", |_, _| Ok(())) else {
            panic!("crate name validation should fail");
        };

        assert_eq!(error, HarnessError::EmptyCrateName);
    }

    #[test]
    fn rejects_empty_directories() {
        let Err(error) = run_with_runner("lint", "   ", |_, _| Ok(())) else {
            panic!("empty directories should be rejected");
        };

        assert_eq!(error, HarnessError::EmptyDirectory);
    }

    #[test]
    fn rejects_absolute_directories() {
        let path = Utf8PathBuf::from("/tmp/ui");
        let Err(error) = run_with_runner("lint", path.clone(), |_, _| Ok(())) else {
            panic!("absolute directories should be rejected");
        };

        assert_eq!(error, HarnessError::AbsoluteDirectory { directory: path });
    }

    #[test]
    fn propagates_runner_failures() {
        let Err(error) = run_with_runner("lint", "ui", |crate_name, directory| {
            assert_eq!(crate_name, "lint");
            assert_eq!(directory, Utf8Path::new("ui"));
            Err("diff mismatch".to_string())
        }) else {
            panic!("runner failures should bubble up");
        };

        assert_eq!(
            error,
            HarnessError::RunnerFailure {
                crate_name: "lint".to_string(),
                directory: Utf8PathBuf::from("ui"),
                message: "diff mismatch".to_string(),
            },
        );
    }
}
