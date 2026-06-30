//! Shared test helpers for Whitaker crates.
//!
//! The helpers in this module are intended for use from unit and integration
//! tests so repeated boilerplate (such as locale overrides) can live in one
//! place with the necessary safety documentation.
//!
//! ## Available helpers
//!
//! - [`fixtures`]: Copies UI fixtures (source files, `.stderr` expectations and
//!   support directories) into isolated workspaces for dylint UI harnesses.
//! - [`decomposition`]: Reusable decomposition-advice fixtures for unit and
//!   behaviour tests.
//! - [`env_test_guard`]: Serializes tests that temporarily mutate process-wide
//!   environment variables.
//! - [`ui`]: Discovers fixtures, prepares isolated workspaces, and runs dylint
//!   UI tests with consistent panic handling.
//! - [`LocaleOverride`]: Temporarily mutates `DYLINT_LOCALE` so locale-sensitive
//!   tests can execute without leaking global state between cases.

pub mod decomposition;
pub mod fixtures;
pub mod ui;

pub use fixtures::{copy_directory, copy_fixture};
pub use ui::{
    FixtureEnvironment, discover_fixtures, prepare_fixture, read_directory_config,
    read_fixture_config, resolve_fixture_config, run_fixtures_with, run_test_runner,
};

use std::ffi::{OsStr, OsString};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Serializes tests that mutate process-wide environment variables.
///
/// Use this guard around helpers such as `temp_env::with_var` or
/// `temp_env::with_vars_unset` when the test would otherwise race with other
/// cases changing the same global process state.
pub fn env_test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| panic!("expected environment test lock: {error}"))
}

/// Guard that sets one environment variable and restores its prior state.
///
/// The guard acquires [`env_test_guard`] only while mutating the process
/// environment during construction and drop. It deliberately does not hold the
/// mutex for the full guard lifetime, so callers can execute callbacks that
/// perform their own guarded environment setup without deadlocking. Use this
/// as the shared environment-mutation helper for tests that need temporary
/// global environment changes with `env_test_guard`-serialised setup and
/// teardown semantics.
pub struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    /// Sets `key` to `value`, returning a guard that restores the previous
    /// value or removes the variable when dropped.
    #[must_use]
    pub fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let _env_guard = env_test_guard();
        let previous = std::env::var_os(key);
        // SAFETY: `env_test_guard` serialises this environment mutation.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        let _env_guard = env_test_guard();
        match &self.previous {
            Some(previous) => {
                // SAFETY: `env_test_guard` serialises this environment mutation.
                unsafe {
                    std::env::set_var(self.key, previous);
                }
            }
            None => {
                // SAFETY: `env_test_guard` serialises this environment mutation.
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }
}
/// Guard that overrides `DYLINT_LOCALE` for the lifetime of the instance.
///
/// The guard captures any existing value and restores it when dropped. The
/// mutation itself must be executed under a serialized test harness (for
/// example via the `serial_test::serial` attribute) to ensure the unsafe
/// environment access remains race-free.
///
/// # Examples
///
/// ```ignore
/// use whitaker_common::test_support::LocaleOverride;
/// use serial_test::serial;
///
/// #[test]
/// #[serial]
/// fn ui_runs_in_welsh_locale() {
///     let _guard = LocaleOverride::set("cy");
///     // Execute lint UI harness here.
/// }
/// ```
pub struct LocaleOverride {
    previous: Option<OsString>,
}

impl LocaleOverride {
    /// Sets `DYLINT_LOCALE` to `locale`, returning a guard that will restore
    /// the prior value (if any) when dropped.
    pub fn set(locale: &str) -> Self {
        let previous = std::env::var_os("DYLINT_LOCALE");
        // SAFETY: Callers must serialize the surrounding test using a
        // synchronization primitive such as the `serial_test::serial`
        // attribute. The guard is thread-local and dropped before another
        // serialized test begins, so no two threads mutate the environment
        // concurrently.
        unsafe {
            std::env::set_var("DYLINT_LOCALE", locale);
        }
        Self { previous }
    }

    /// Removes `DYLINT_LOCALE`, returning a guard that reinstates the prior
    /// value (if any) when dropped.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use whitaker_common::test_support::LocaleOverride;
    /// use serial_test::serial;
    /// use std::ffi::OsString;
    ///
    /// #[test]
    /// #[serial]
    /// fn clears_then_restores_locale() {
    ///     unsafe {
    ///         std::env::set_var("DYLINT_LOCALE", "cy");
    ///     }
    ///     {
    ///         let _guard = LocaleOverride::clear();
    ///         assert!(std::env::var_os("DYLINT_LOCALE").is_none());
    ///     }
    ///     assert_eq!(
    ///         std::env::var_os("DYLINT_LOCALE"),
    ///         Some(OsString::from("cy"))
    ///     );
    /// }
    /// ```
    pub fn clear() -> Self {
        let previous = std::env::var_os("DYLINT_LOCALE");
        // SAFETY: Callers must serialize the surrounding test using a
        // synchronization primitive such as the `serial_test::serial`
        // attribute. Clearing the environment therefore cannot race with other
        // threads.
        unsafe {
            std::env::remove_var("DYLINT_LOCALE");
        }
        Self { previous }
    }
}

impl Drop for LocaleOverride {
    fn drop(&mut self) {
        if let Some(value) = &self.previous {
            // SAFETY: By construction the guard only lives within a serialized
            // test, so restoring the prior value cannot race with another
            // thread mutating the environment.
            unsafe {
                std::env::set_var("DYLINT_LOCALE", value);
            }
        } else {
            // SAFETY: Serialized execution also guarantees removal has no
            // concurrent callers.
            unsafe {
                std::env::remove_var("DYLINT_LOCALE");
            }
        }
    }
}
