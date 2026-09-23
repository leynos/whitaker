//! Shared test helpers for Whitaker crates.
//!
//! The helpers in this module are intended for use from unit and integration
//! tests so repeated boilerplate (such as locale overrides) can live in one
//! place with the necessary safety documentation.
//!
//! ## Available helpers
//!
//! - [`fixtures`]: Copies UI fixtures (source files, `.stderr` expectations and support
//!   directories) into isolated workspaces for dylint UI harnesses.
//! - [`decomposition`]: Reusable decomposition-advice fixtures for unit and behaviour tests.
//! - [`env_test_guard`]: Serializes tests that temporarily mutate process-wide environment
//!   variables.
//! - [`ui`]: Discovers fixtures, prepares isolated workspaces, and runs dylint UI tests with
//!   consistent panic handling.
//! - [`EnvVarGuard`]: Sets or removes one environment variable and restores
//!   its prior state on drop.
//! - [`with_locale`], [`with_env_var`], and [`with_env_var_removed`]: Scope temporary environment
//!   mutations (such as `DYLINT_LOCALE` overrides) to a callback so tests cannot leak global state
//!   between cases.

pub mod decomposition;
pub mod fixtures;
pub mod ui;

use std::{
    ffi::{OsStr, OsString},
    sync::OnceLock,
};

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};

pub use fixtures::{copy_directory, copy_fixture};
pub use ui::{
    FixtureEnvironment, discover_fixtures, prepare_fixture, read_directory_config,
    read_fixture_config, resolve_fixture_config, run_fixtures_with, run_test_runner,
};

/// Held while serializing process-wide environment mutations in tests.
///
/// This alias keeps callers coupled to the shared test-support contract rather
/// than its lock implementation.
pub type EnvTestGuard = ReentrantMutexGuard<'static, ()>;

/// Serializes tests that mutate process-wide environment variables.
///
/// Use this guard around environment mutations when the test would otherwise
/// race with other cases changing the same global process state.
///
/// The lock is reentrant so a callback that already holds the shared
/// serialization token can safely call another shared environment helper.
/// This permits scoped overrides to wrap UI runners, which guard their own
/// setup and restoration. The guarded value is `()`, so no mutable state can
/// be accessed through the guard.
pub fn env_test_guard() -> EnvTestGuard {
    static LOCK: OnceLock<ReentrantMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| ReentrantMutex::new(())).lock()
}

/// Guard that sets one environment variable and restores its prior state.
///
/// The guard retains [`env_test_guard`] from construction until after it
/// restores the prior value in [`Drop`]. This prevents another thread from
/// observing or restoring an interleaved environment value. The shared lock is
/// reentrant, so callers can still nest helpers that acquire it on the same
/// thread.
pub struct EnvVarGuard {
    _env_guard: EnvTestGuard,
    key: String,
    previous: Option<OsString>,
}

enum EnvVarMutation<'a> {
    Set(&'a OsStr),
    Remove,
}

impl EnvVarGuard {
    /// Sets `key` to `value`, returning a guard that restores the previous
    /// value or removes the variable when dropped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_common::test_support::EnvVarGuard;
    ///
    /// let _guard = EnvVarGuard::set("WHITAKER_TEST_ENV_VAR", "enabled");
    /// assert_eq!(
    ///     std::env::var("WHITAKER_TEST_ENV_VAR").expect("test env var should be set"),
    ///     "enabled",
    /// );
    /// ```
    #[must_use]
    pub fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        Self::set_scoped(key, value)
    }

    /// Sets a callback-supplied key while retaining the shared guard.
    fn set_scoped(key: &str, value: impl AsRef<OsStr>) -> Self {
        Self::mutate_scoped(key, EnvVarMutation::Set(value.as_ref()))
    }

    /// Removes a callback-supplied key while retaining the shared guard.
    fn remove_scoped(key: &str) -> Self {
        Self::mutate_scoped(key, EnvVarMutation::Remove)
    }

    /// Applies `mutation` while retaining the shared guard until restoration.
    ///
    /// Keeping this guard in the returned value serializes construction, use,
    /// and [`Drop`] as one process-environment transaction.
    fn mutate_scoped(key: &str, mutation: EnvVarMutation<'_>) -> Self {
        let env_guard = env_test_guard();
        let previous = std::env::var_os(key);
        // SAFETY: `env_test_guard` serializes this environment mutation.
        unsafe {
            match mutation {
                EnvVarMutation::Set(value) => std::env::set_var(key, value),
                EnvVarMutation::Remove => std::env::remove_var(key),
            }
        }
        Self {
            _env_guard: env_guard,
            key: key.to_owned(),
            previous,
        }
    }

    /// Removes `key`, returning a guard that restores the previous value when
    /// dropped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_common::test_support::EnvVarGuard;
    ///
    /// let _guard = EnvVarGuard::remove("WHITAKER_REMOVED_TEST_ENV_VAR");
    /// assert!(std::env::var_os("WHITAKER_REMOVED_TEST_ENV_VAR").is_none());
    /// ```
    #[must_use]
    pub fn remove(key: &'static str) -> Self {
        Self::remove_scoped(key)
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => {
                // SAFETY: the retained `env_test_guard` serializes this restoration.
                unsafe {
                    std::env::set_var(&self.key, previous);
                }
            }
            None => {
                // SAFETY: the retained `env_test_guard` serializes this restoration.
                unsafe {
                    std::env::remove_var(&self.key);
                }
            }
        }
    }
}

/// Runs `callback` with one environment variable temporarily set.
///
/// The mutation is scoped to the callback and the prior value is restored
/// afterwards, even on panic. The callback holds [`env_test_guard`], so its
/// mutation and restoration cannot interleave with [`EnvVarGuard`] or manually
/// guarded environment mutations.
///
/// # Examples
///
/// ```rust
/// use whitaker_common::test_support::with_env_var;
///
/// with_env_var("WHITAKER_TEST_ENV_VAR", "enabled", || {
///     assert_eq!(
///         std::env::var("WHITAKER_TEST_ENV_VAR").expect("test env var should be set"),
///         "enabled",
///     );
/// });
/// ```
pub fn with_env_var<R>(key: &str, value: impl AsRef<OsStr>, callback: impl FnOnce() -> R) -> R {
    let _env_guard = env_test_guard();
    let _variable_guard = EnvVarGuard::set_scoped(key, value);
    callback()
}

/// Runs `callback` with one environment variable temporarily removed.
///
/// The prior value (if any) is restored after the callback completes or
/// panics. The callback holds [`env_test_guard`] so its mutation and
/// restoration cannot interleave with other shared environment helpers.
///
/// # Examples
///
/// ```rust
/// use whitaker_common::test_support::with_env_var_removed;
///
/// with_env_var_removed("WHITAKER_REMOVED_TEST_ENV_VAR", || {
///     assert!(std::env::var_os("WHITAKER_REMOVED_TEST_ENV_VAR").is_none());
/// });
/// ```
pub fn with_env_var_removed<R>(key: &str, callback: impl FnOnce() -> R) -> R {
    let _env_guard = env_test_guard();
    let _variable_guard = EnvVarGuard::remove_scoped(key);
    callback()
}

/// Runs `callback` with `DYLINT_LOCALE` overridden.
///
/// `Some(locale)` sets the variable for the duration of the callback;
/// `None` removes it. Any prior value is restored afterwards, so
/// locale-sensitive tests cannot leak global state between cases. The callback
/// remains within the shared reentrant environment-mutation protocol.
///
/// # Examples
///
/// ```rust
/// use whitaker_common::test_support::with_locale;
///
/// with_locale(Some("cy"), || {
///     assert_eq!(
///         std::env::var("DYLINT_LOCALE").expect("locale should be set"),
///         "cy",
///     );
/// });
/// ```
pub fn with_locale<R>(locale: Option<&str>, callback: impl FnOnce() -> R) -> R {
    match locale {
        Some(locale_value) => with_env_var("DYLINT_LOCALE", locale_value, callback),
        None => with_env_var_removed("DYLINT_LOCALE", callback),
    }
}

#[cfg(test)]
mod env_tests;
