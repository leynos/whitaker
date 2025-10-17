//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralizes input validation so lint crates can depend on a
//! small helper rather than repeat the same checks.

use std::{env, fmt, fs, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, MetadataCommand};

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
    /// The compiled lint library was not present in the expected location.
    LibraryMissing { path: String },
    /// Copying the compiled library to the toolchain-qualified name failed.
    LibraryCopyFailed {
        source: String,
        target: String,
        message: String,
    },
    /// Building the lint library failed before the UI runner executed.
    LibraryBuildFailed { crate_name: String, message: String },
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

    ensure_toolchain_library(trimmed)?;

    match runner(trimmed, directory.as_ref()) {
        Ok(()) => Ok(()),
        Err(message) => Err(HarnessError::RunnerFailure {
            crate_name: trimmed.to_string(),
            directory,
            message,
        }),
    }
}

fn ensure_toolchain_library(crate_name: &str) -> Result<(), HarnessError> {
    let metadata = fetch_metadata(crate_name)?;

    if !workspace_has_package(&metadata, crate_name) {
        // The harness is being exercised with a synthetic crate name. In that case the caller
        // controls the build and we should not attempt to prepare artifacts.
        return Ok(());
    }

    let profile_dir = metadata.target_directory.join("debug").into_std_path_buf();
    let crate_basename = crate_name.replace('-', "_");

    let base_name = format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        crate_basename,
        env::consts::DLL_SUFFIX
    );
    let mut source = profile_dir.join(&base_name);

    if !source.exists() {
        build_library(crate_name, &metadata)?;
        source = profile_dir.join(&base_name);
    }

    if !source.exists() {
        return Err(HarnessError::LibraryMissing {
            path: source.to_string_lossy().into_owned(),
        });
    }

    let toolchain = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(str::to_string))
        .unwrap_or_else(|| "unknown-toolchain".to_string());
    let target_name = format!(
        "{}{}@{}{}",
        env::consts::DLL_PREFIX,
        crate_basename,
        toolchain,
        env::consts::DLL_SUFFIX
    );
    let target = profile_dir.join(&target_name);

    // Always refresh the toolchain-qualified artefact so UI tests exercise the latest build.
    fs::copy(&source, &target).map_err(|error| HarnessError::LibraryCopyFailed {
        source: source.to_string_lossy().into_owned(),
        target: target.to_string_lossy().into_owned(),
        message: error.to_string(),
    })?;

    Ok(())
}

fn build_library(crate_name: &str, metadata: &Metadata) -> Result<(), HarnessError> {
    if !workspace_has_package(metadata, crate_name) {
        return Ok(());
    }

    let output = Command::new("cargo")
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .arg("--package")
        .arg(crate_name)
        .current_dir(metadata.workspace_root.as_std_path())
        .output()
        .map_err(|error| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_string(),
            message: error.to_string(),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_string(),
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn fetch_metadata(crate_name: &str) -> Result<Metadata, HarnessError> {
    MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|error| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_string(),
            message: error.to_string(),
        })
}

fn workspace_has_package(metadata: &Metadata, crate_name: &str) -> bool {
    metadata
        .packages
        .iter()
        .any(|package| package.name == crate_name)
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
                    Ok(message) => (*message).to_string(),
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
mod tests {
    use super::{HarnessError, run_with_runner};
    use camino::{Utf8Path, Utf8PathBuf};
    use rstest::rstest;

    #[rstest]
    #[case(
        "  ",
        "ui",
        HarnessError::EmptyCrateName,
        "crate name validation should fail"
    )]
    #[case(
        "lint",
        "   ",
        HarnessError::EmptyDirectory,
        "empty directories should be rejected"
    )]
    fn rejects_invalid_inputs(
        #[case] crate_name: &str,
        #[case] directory: &str,
        #[case] expected: HarnessError,
        #[case] panic_message: &str,
    ) {
        let Err(error) = run_with_runner(crate_name, directory, |_, _| Ok(())) else {
            panic!("{panic_message}");
        };

        assert_eq!(error, expected);
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
