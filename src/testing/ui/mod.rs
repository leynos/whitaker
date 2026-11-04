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

use whitaker_common::test_support::{with_env_var, with_env_var_removed};

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
    with_runner_env(|| {
        ensure_toolchain_library(&crate_name_owned)?;

        match runner(crate_name_str, directory.as_ref()) {
            Ok(()) => Ok(()),
            Err(message) => Err(HarnessError::RunnerFailure {
                crate_name: crate_name_owned.clone().into_inner(),
                directory: directory.clone(),
                message,
            }),
        }
    })
}

/// Tells `dylint_testing` that the toolchain-qualified library is prepared.
///
/// Since Dylint 6.3.0, `dylint_testing::initialize` rebuilds the library with
/// default features and then copies the result over the toolchain-qualified
/// file. Every Whitaker lint crate keeps its lint behind the optional
/// `dylint-driver` feature, so that rebuild yields an empty `cdylib`, and the
/// UI test then fails with ``could not find `dylint_version` ``. Setting this
/// flag makes `dylint_internal::link::debug_path_with_toolchain` return the
/// path [`ensure_toolchain_library`] just wrote instead of copying over it.
/// Any value other than `0` counts as enabled.
const COPY_LIBRARY_FLAG: (&str, &str) = ("DYLINT_LINK_ENABLE_COPY_LIBRARY", "1");

/// Runs `callback` with the environment `run_with_runner` requires.
///
/// Three variables need temporary adjustment, on every platform unless noted:
///
/// - `RUSTC_WRAPPER` is removed, because `dylint_testing::Test::example` scans `cargo build
///   --message-format=json` output for bare `rustc` invocations. When a wrapper such as `sccache`
///   is active, Cargo invokes `<wrapper> rustc ...` instead, `dylint_testing` finds zero
///   invocations, and the example test panics.
/// - [`COPY_LIBRARY_FLAG`] is set when absent, so `dylint_testing` keeps the library this harness
///   built rather than replacing it with a stub. A value the caller already set is left untouched,
///   so an explicit `DYLINT_LINK_ENABLE_COPY_LIBRARY=0` still opts out.
/// - `VCPKG_ROOT` (Windows only) is set to `C:\vcpkg` when that directory exists and the variable
///   is otherwise absent, so downstream `cargo` invocations resolve vcpkg.
///
/// The mutations are scoped to the callback and restored afterwards, even on
/// panic. `temp_env`'s re-entrant global lock serializes them, so runner
/// closures that perform their own scoped environment setup nest without
/// deadlocking.
///
/// `RUSTC_WRAPPER` is removed first, outside the [`COPY_LIBRARY_FLAG`] check,
/// so that read happens while the `temp_env` lock is held. Read before the
/// lock, it could observe another scope's value: that scope sets the flag,
/// this one reads it, the other restores it, and this callback then runs
/// *without* the flag. `dylint_testing` would rebuild the library with default
/// features, copy the resulting empty `cdylib` over the prepared one, and the
/// test would fail with ``could not find `dylint_version` ``.
fn with_runner_env<R>(callback: impl FnOnce() -> R) -> R {
    with_vcpkg_root(|| with_env_var_removed("RUSTC_WRAPPER", || with_copy_library_flag(callback)))
}

/// Sets [`COPY_LIBRARY_FLAG`] when the caller has not set it already.
///
/// Call this only from inside a scoped `temp_env` mutation, as
/// [`with_runner_env`] does: the flag read must happen under the lock that
/// serializes sibling scopes.
fn with_copy_library_flag<R>(callback: impl FnOnce() -> R) -> R {
    if std::env::var_os(COPY_LIBRARY_FLAG.0).is_some() {
        callback()
    } else {
        with_env_var(COPY_LIBRARY_FLAG.0, COPY_LIBRARY_FLAG.1, callback)
    }
}

#[cfg(windows)]
fn with_vcpkg_root<R>(callback: impl FnOnce() -> R) -> R {
    let candidate = Utf8Path::new(r"C:\vcpkg");
    if candidate.is_dir() && std::env::var_os("VCPKG_ROOT").is_none() {
        with_env_var("VCPKG_ROOT", candidate.as_std_path(), callback)
    } else {
        callback()
    }
}

#[cfg(not(windows))]
fn with_vcpkg_root<R>(callback: impl FnOnce() -> R) -> R { callback() }

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
