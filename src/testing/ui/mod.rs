//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralizes input validation so lint crates can depend on a
//! small helper rather than repeat the same checks.

use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};

mod toolchain;

use self::toolchain::{CrateName, ensure_toolchain_library};

/// Errors produced when preparing or executing Dylint UI tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarnessError {
    /// The harness requires the lint crate name to be populated.
    EmptyCrateName,
    /// The harness requires the UI test directory to be provided.
    EmptyDirectory,
    /// UI tests must live within the crate so the path may not be absolute.
    AbsoluteDirectory {
        /// Directory provided by the caller.
        directory: Utf8PathBuf,
    },
    /// The underlying runner reported a failure (for example, a diff mismatch).
    RunnerFailure {
        /// Lint crate whose tests failed.
        crate_name: String,
        /// Directory containing the failing UI tests.
        directory: Utf8PathBuf,
        /// Failure reported by the runner.
        message: String,
    },
    /// The compiled lint library was not present in the expected location.
    LibraryMissing {
        /// Path that should have contained the compiled library.
        path: String,
    },
    /// Copying the compiled library to the toolchain-qualified name failed.
    LibraryCopyFailed {
        /// Location of the compiled library artefact.
        source: String,
        /// Target path for the toolchain-qualified copy.
        target: String,
        /// Error produced while copying the artefact.
        message: String,
    },
    /// Building the lint library failed before the UI runner executed.
    LibraryBuildFailed {
        /// Lint crate whose build failed.
        crate_name: String,
        /// Error emitted by the build command.
        message: String,
    },
    /// Retrieving Cargo workspace metadata failed.
    MetadataFailed {
        /// Error emitted while loading metadata.
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
            Self::LibraryMissing { path } => {
                write!(formatter, "lint library missing: {path}")
            }
            Self::LibraryCopyFailed {
                source,
                target,
                message,
            } => {
                write!(
                    formatter,
                    "failed to prepare lint library {target} from {source}: {message}",
                )
            }
            Self::LibraryBuildFailed {
                crate_name,
                message,
            } => {
                write!(
                    formatter,
                    "failed to build lint library for {crate_name}: {message}",
                )
            }
            Self::MetadataFailed { message } => {
                write!(formatter, "failed to retrieve Cargo metadata: {message}")
            }
        }
    }
}

impl std::error::Error for HarnessError {}

/// Run UI tests for an explicit crate name.
///
/// Run UI tests using a custom runner.
///
/// The caller supplies the runner so tests can replace the default implementation
/// with a stub while verifying how the harness validates and prepares inputs.
///
/// # Errors
///
/// Returns [`HarnessError`] when either input validation fails or the provided
/// runner reports a failure.
///
/// # Examples
///
/// ```no_run
/// use camino::Utf8Path;
/// use whitaker::testing::ui::run_with_runner;
///
/// fn main() -> Result<(), whitaker::testing::ui::HarnessError> {
///     run_with_runner("my_lint", "ui", |crate_name, dir: &Utf8Path| {
///         ::dylint_testing::ui_test(crate_name, dir);
///         Ok(())
///     })
/// }
/// ```
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
    let directory_ref: &Utf8Path = directory.as_ref();

    if directory_ref.as_str().trim().is_empty() {
        return Err(HarnessError::EmptyDirectory);
    }

    if directory_is_rooted(directory_ref) {
        // The helper rejects any path with a root so Unix-style absolute inputs and
        // drive-qualified paths such as `C:\ui` never escape the crate tree. On
        // Windows, `has_root` alone misses drive-relative paths (for example `C:ui`),
        // so the helper also inspects the first component for a Windows prefix to
        // ensure those prefixed-but-rootless inputs are rejected as well.
        return Err(HarnessError::AbsoluteDirectory { directory });
    }

    let crate_name_owned =
        CrateName::try_from(trimmed).map_err(|_| HarnessError::EmptyCrateName)?;
    ensure_toolchain_library(&crate_name_owned)?;

    match runner(trimmed, directory_ref) {
        Ok(()) => Ok(()),
        Err(message) => Err(HarnessError::RunnerFailure {
            crate_name: trimmed.to_owned(),
            directory,
            message,
        }),
    }
}

fn directory_is_rooted(path: &Utf8Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::Component;

        path.has_root()
            || matches!(
                path.as_std_path().components().next(),
                Some(Component::Prefix(_))
            )
    }

    #[cfg(not(windows))]
    {
        path.has_root()
    }
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
        $crate::testing::ui::run_with_runner(crate_name, $directory, |crate_name, directory| {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                ::dylint_testing::ui_test(crate_name, directory);
            }))
            .map_err(|payload| match payload.downcast::<String>() {
                Ok(message) => *message,
                Err(payload) => match payload.downcast::<&'static str>() {
                    Ok(message) => (*message).to_owned(),
                    Err(_) => String::from("dylint UI tests panicked without a message"),
                },
            })
        })
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
mod tests;
