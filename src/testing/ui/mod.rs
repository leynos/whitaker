//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralizes input validation so lint crates can depend on a
//! small helper rather than repeat the same checks.

use std::{env, fmt, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};

mod toolchain;

use self::toolchain::{CrateName, ensure_toolchain_library};
use whitaker_common::test_support::env_test_guard;

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
    let directory: Utf8PathBuf = ui_directory.into();

    if directory.as_str().trim().is_empty() {
        return Err(HarnessError::EmptyDirectory);
    }

    if directory_is_rooted(directory.as_ref()) {
        // The helper rejects any path with a root so Unix-style absolute inputs and
        // drive-qualified paths such as `C:\ui` never escape the crate tree. On
        // Windows, `has_root` alone misses drive-relative paths (for example `C:ui`),
        // so the helper also inspects the first component for a Windows prefix to
        // ensure those prefixed-but-rootless inputs are rejected as well.
        return Err(HarnessError::AbsoluteDirectory { directory });
    }

    let crate_name_owned =
        CrateName::try_from(crate_name).map_err(|_| HarnessError::EmptyCrateName)?;
    let crate_name_str = crate_name_owned.as_str();
    let _runner_env_guard = runner_env_guard();
    ensure_toolchain_library(&crate_name_owned)?;

    match runner(crate_name_str, directory.as_ref()) {
        Ok(()) => Ok(()),
        Err(message) => Err(HarnessError::RunnerFailure {
            crate_name: crate_name_owned.into_inner(),
            directory,
            message,
        }),
    }
}

/// Serializes environment mutations required by `run_with_runner`.
///
/// `RUSTC_WRAPPER` must be cleared on every platform when set (for example to
/// `sccache`), because `dylint_testing::Test::example` scans
/// `cargo build --message-format=json` output for bare `rustc` invocations.
/// When a wrapper is active, Cargo invokes `<wrapper> rustc ...` instead, and
/// `dylint_testing` finds zero invocations, causing a panic.
///
/// On Windows, one additional environment variable needs temporary adjustment:
///
/// - `VCPKG_ROOT`: must be set to `C:\vcpkg` when that directory exists and the
///   variable is otherwise absent, so downstream `cargo` invocations resolve vcpkg.
///
/// During LLVM coverage, `cargo-llvm-cov` runs Nextest with
/// `<base>/llvm-cov-target` but exposes only `base` through
/// `CARGO_LLVM_COV_TARGET_DIR`. The nested Cargo commands issued by
/// `dylint_testing` do not receive Nextest's `--target-dir`; direct them to the
/// same coverage target so Cargo does not reuse ordinary-target artefacts built
/// with incompatible coverage metadata.
///
/// Each mutation and restoration step acquires `env_test_guard()` only for the
/// environment write itself. The guard deliberately does not hold that mutex
/// across the UI runner callback, because runner closures can perform their
/// own environment-guarded setup.
struct RunnerEnvGuard {
    #[cfg(windows)]
    vcpkg_root_was_absent: bool,
    coverage_target_previous: Option<Option<std::ffi::OsString>>,
    rustc_wrapper_previous: Option<std::ffi::OsString>,
}

impl Drop for RunnerEnvGuard {
    fn drop(&mut self) {
        let _env_guard = env_test_guard();

        // SAFETY: `env_test_guard` serializes the restoration writes below.
        #[cfg(windows)]
        {
            if self.vcpkg_root_was_absent {
                unsafe {
                    env::remove_var("VCPKG_ROOT");
                }
            }
        }
        if let Some(prev) = &self.rustc_wrapper_previous {
            unsafe {
                env::set_var("RUSTC_WRAPPER", prev);
            }
        }
        if let Some(previous) = &self.coverage_target_previous {
            match previous {
                Some(previous) => unsafe {
                    env::set_var("CARGO_TARGET_DIR", previous);
                },
                None => unsafe {
                    env::remove_var("CARGO_TARGET_DIR");
                },
            }
        }
    }
}

fn runner_env_guard() -> Option<RunnerEnvGuard> {
    #[cfg(windows)]
    let vcpkg_candidate = Utf8Path::new(r"C:\vcpkg");
    #[cfg(windows)]
    let vcpkg_applicable = vcpkg_candidate.is_dir();

    let _env_guard = env_test_guard();
    let coverage_target = coverage_target_dir();
    let has_rustc_wrapper = env::var_os("RUSTC_WRAPPER").is_some();

    #[cfg(windows)]
    if !vcpkg_applicable && !has_rustc_wrapper && coverage_target.is_none() {
        return None;
    }
    #[cfg(not(windows))]
    if !has_rustc_wrapper && coverage_target.is_none() {
        return None;
    }

    // All environment reads and writes below are serialized by `_env_guard`.
    #[cfg(windows)]
    let vcpkg_root_was_absent = if vcpkg_applicable && env::var_os("VCPKG_ROOT").is_none() {
        // SAFETY: `_env_guard` serializes concurrent environment mutations.
        unsafe {
            env::set_var("VCPKG_ROOT", vcpkg_candidate.as_std_path());
        }
        true
    } else {
        false
    };

    let rustc_wrapper_previous = env::var_os("RUSTC_WRAPPER").inspect(|_| {
        // SAFETY: `_env_guard` serializes concurrent environment mutations.
        unsafe {
            env::remove_var("RUSTC_WRAPPER");
        }
    });
    let coverage_target_previous = coverage_target.map(|target| {
        let previous = env::var_os("CARGO_TARGET_DIR");
        // SAFETY: `_env_guard` serializes this environment mutation.
        unsafe {
            env::set_var("CARGO_TARGET_DIR", target);
        }
        previous
    });

    #[cfg(windows)]
    if !vcpkg_root_was_absent
        && rustc_wrapper_previous.is_none()
        && coverage_target_previous.is_none()
    {
        // Nothing was mutated; release the guard early.
        return None;
    }

    Some(RunnerEnvGuard {
        #[cfg(windows)]
        vcpkg_root_was_absent,
        coverage_target_previous,
        rustc_wrapper_previous,
    })
}

fn coverage_target_dir() -> Option<PathBuf> {
    env::var_os("CARGO_LLVM_COV_TARGET_DIR")
        .filter(|base| !base.is_empty())
        .map(PathBuf::from)
        .map(|base| base.join("llvm-cov-target"))
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
