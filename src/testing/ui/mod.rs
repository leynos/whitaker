//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralizes input validation so lint crates can depend on a
//! small helper rather than repeat the same checks.

use std::{env, fmt, fs, io::Cursor, path::PathBuf, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Message, Metadata, MetadataCommand};

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
            crate_name: trimmed.to_owned(),
            directory,
            message,
        }),
    }
}

fn ensure_toolchain_library(crate_name: &str) -> Result<(), HarnessError> {
    let metadata = fetch_metadata()?;

    if !workspace_has_package(&metadata, crate_name) {
        // The harness is being exercised with a synthetic crate name. In that case the caller
        // controls the build and we should not attempt to prepare artefacts.
        return Ok(());
    }

    let source = build_and_locate_cdylib(crate_name, &metadata)?;
    let parent = source
        .parent()
        .ok_or_else(|| HarnessError::LibraryMissing {
            path: source.display().to_string(),
        })?;

    let toolchain = env::var("RUSTUP_TOOLCHAIN")
        .ok()
        .or_else(|| option_env!("RUSTUP_TOOLCHAIN").map(String::from))
        .unwrap_or_else(|| "unknown-toolchain".to_owned());
    let file_name = source
        .file_name()
        .ok_or_else(|| HarnessError::LibraryMissing {
            path: source.display().to_string(),
        })?
        .to_string_lossy()
        .into_owned();
    let suffix = env::consts::DLL_SUFFIX;
    let target_name = file_name.as_str().strip_suffix(suffix).map_or_else(
        || format!("{file_name}@{toolchain}"),
        |stripped| format!("{stripped}@{toolchain}{suffix}"),
    );
    let target = parent.join(&target_name);

    // Always refresh the toolchain-qualified artefact so UI tests exercise the latest build.
    fs::copy(&source, &target).map_err(|error| HarnessError::LibraryCopyFailed {
        source: source.display().to_string(),
        target: target.display().to_string(),
        message: error.to_string(),
    })?;

    Ok(())
}

fn build_and_locate_cdylib(crate_name: &str, metadata: &Metadata) -> Result<PathBuf, HarnessError> {
    let output = execute_build_command(crate_name, metadata)?;
    let package_id = find_package_id(crate_name, metadata)?;
    find_cdylib_in_artifacts(&output.stdout, &package_id, crate_name)
}

fn execute_build_command(
    crate_name: &str,
    metadata: &Metadata,
) -> Result<std::process::Output, HarnessError> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--lib")
        .arg("--quiet")
        .arg("--message-format=json")
        .arg("--package")
        .arg(crate_name)
        .current_dir(metadata.workspace_root.as_std_path());

    let output = command
        .output()
        .map_err(|error| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_owned(),
            message: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_owned(),
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(output)
}

fn find_package_id(
    crate_name: &str,
    metadata: &Metadata,
) -> Result<cargo_metadata::PackageId, HarnessError> {
    metadata
        .packages
        .iter()
        .find(|package| {
            package.name == crate_name
                && metadata
                    .workspace_members
                    .iter()
                    .any(|member| member == &package.id)
        })
        .map(|package| package.id.clone())
        .ok_or_else(|| HarnessError::LibraryBuildFailed {
            crate_name: crate_name.to_owned(),
            message: format!(
                "package metadata missing for {crate_name}; unable to locate cdylib artefact"
            ),
        })
}

fn find_cdylib_in_artifacts(
    stdout: &[u8],
    package_id: &cargo_metadata::PackageId,
    crate_name: &str,
) -> Result<PathBuf, HarnessError> {
    for message in Message::parse_stream(Cursor::new(stdout)) {
        let Ok(Message::CompilerArtifact(artifact)) = message else {
            // Ignore unrelated output and parse errors; the build succeeded so any
            // remaining noise should not block locating the compiled artefact.
            continue;
        };

        if let Some(path) = extract_cdylib_path(&artifact, package_id) {
            return Ok(path);
        }
    }

    Err(HarnessError::LibraryMissing {
        path: format!("cdylib for {crate_name} not reported by cargo"),
    })
}

fn extract_cdylib_path(
    artifact: &cargo_metadata::Artifact,
    package_id: &cargo_metadata::PackageId,
) -> Option<PathBuf> {
    if artifact.package_id != *package_id {
        return None;
    }

    if !artifact.target.is_cdylib() {
        return None;
    }

    artifact
        .filenames
        .iter()
        .find(|candidate| candidate.as_str().ends_with(env::consts::DLL_SUFFIX))
        .map(|path| path.clone().into_std_path_buf())
}

fn fetch_metadata() -> Result<Metadata, HarnessError> {
    MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|error| HarnessError::MetadataFailed {
            message: error.to_string(),
        })
}

fn workspace_has_package(metadata: &Metadata, crate_name: &str) -> bool {
    metadata.packages.iter().any(|package| {
        package.name == crate_name
            && metadata
                .workspace_members
                .iter()
                .any(|member| member == &package.id)
    })
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
