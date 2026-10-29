//! UI test harness helpers shared by Whitaker lint crates.
//!
//! Dylint UI tests follow a consistent shape across all lint crates: a single
//! `ui` test invokes `dylint_testing::ui_test` with the crate name and the
//! directory containing `.rs` source files plus their expected diagnostics.
//! This module centralizes input validation so lint crates can depend on a
//! small helper rather than repeat the same checks.

use std::env;
use std::fmt;

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

/// Runs a UI test and reports runner failures using the standard panic format.
///
/// Use this at a UI-test entry point that only needs the usual failure
/// behaviour. Call [`run_with_runner`] instead when the caller must handle a
/// [`HarnessError`] itself.
///
/// # Panics
///
/// Panics when [`run_with_runner`] returns a harness or runner error. The
/// panic preserves crate and UI-directory context while using
/// [`HarnessError`]'s display implementation for the precise error details.
///
/// # Examples
///
/// ```no_run
/// use whitaker::testing::ui::run_ui_test;
///
/// run_ui_test("example_lint", "ui", |_, _| Ok(()));
/// ```
pub fn run_ui_test(
    crate_name: &str,
    ui_directory: impl Into<Utf8PathBuf>,
    runner: impl Fn(&str, &Utf8Path) -> Result<(), String>,
) {
    let directory = ui_directory.into();
    run_with_runner(crate_name, directory.clone(), runner).unwrap_or_else(|error| {
        panic!("UI tests should execute without diffs for {crate_name} in {directory}: {error}")
    });
}

/// Serializes environment mutations required by `run_with_runner`.
///
/// Every platform needs one variable, and Windows needs a second:
///
/// - `DYLINT_LINK_ENABLE_COPY_LIBRARY`: set to `1` when absent. Since Dylint
///   6.3.0, `dylint_testing` copies `lib<crate>.so` over the
///   toolchain-qualified library unless this flag says the copy already
///   exists. The harness has just built that copy with the `dylint-driver`
///   feature, while `dylint_testing` first rebuilds the plain library with
///   default features, which for every Whitaker lint crate is an empty
///   `cdylib`. Without the flag the UI test loads that stub and fails with
///   ``could not find `dylint_version` ``.
/// - `VCPKG_ROOT` (Windows only): set to `C:\vcpkg` when that directory exists
///   and the variable is otherwise absent, so downstream `cargo` invocations
///   resolve vcpkg.
///
/// Each mutation and restoration step acquires `env_test_guard()` only for the
/// environment write itself. The guard deliberately does not hold that mutex
/// across the UI runner callback, because runner closures can perform their
/// own environment-guarded setup.
///
/// Runners in one test binary can overlap under plain `cargo test`, so the
/// variables are shared: the first active runner sets whichever are absent and
/// the last one to finish removes them. A per-runner record would let one
/// runner remove the flag while another still relies on it.
struct RunnerEnvGuard {
    _shared: (),
}

/// Every active runner's view of the variables they set while absent.
struct SharedRunnerEnv {
    active: usize,
    set_while_absent: Vec<&'static str>,
}

/// The process-wide runner environment record, written under `env_test_guard`.
static SHARED_RUNNER_ENV: std::sync::Mutex<SharedRunnerEnv> =
    std::sync::Mutex::new(SharedRunnerEnv {
        active: 0,
        set_while_absent: Vec::new(),
    });

/// Lock the shared record, recovering it if a panicking test poisoned it.
fn shared_runner_env() -> std::sync::MutexGuard<'static, SharedRunnerEnv> {
    SHARED_RUNNER_ENV
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Drop for RunnerEnvGuard {
    /// Restore temporary environment changes when the last runner finishes.
    fn drop(&mut self) {
        let _env_guard = env_test_guard();
        let mut shared = shared_runner_env();
        shared.active = shared.active.saturating_sub(1);
        if shared.active > 0 {
            return;
        }
        for key in std::mem::take(&mut shared.set_while_absent) {
            // SAFETY: `env_test_guard` serializes the restoration write.
            unsafe {
                env::remove_var(key);
            }
        }
    }
}

/// Tells `dylint_testing` that the toolchain-qualified library is prepared.
const COPY_LIBRARY_FLAG: (&str, &str) = ("DYLINT_LINK_ENABLE_COPY_LIBRARY", "1");

/// List the variables the UI runner needs on this platform.
fn runner_env_settings() -> Vec<(&'static str, &'static str)> {
    #[cfg(windows)]
    {
        let mut settings = vec![COPY_LIBRARY_FLAG];
        if Utf8Path::new(r"C:\vcpkg").is_dir() {
            settings.push(("VCPKG_ROOT", r"C:\vcpkg"));
        }
        settings
    }

    #[cfg(not(windows))]
    {
        vec![COPY_LIBRARY_FLAG]
    }
}

/// Prepare the environment required by the UI runner.
fn runner_env_guard() -> RunnerEnvGuard {
    let _env_guard = env_test_guard();
    let mut shared = shared_runner_env();
    if shared.active == 0 {
        shared.set_while_absent = runner_env_settings()
            .into_iter()
            .filter(|(key, _)| env::var_os(key).is_none())
            .map(|(key, value)| {
                // SAFETY: `_env_guard` serializes concurrent environment mutations.
                unsafe {
                    env::set_var(key, value);
                }
                key
            })
            .collect();
    }
    shared.active += 1;
    RunnerEnvGuard { _shared: () }
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
