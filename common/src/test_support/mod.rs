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
/// The guard acquires [`env_test_guard`] only while mutating the process
/// environment during construction and drop. It deliberately does not hold the
/// mutex for the full guard lifetime, so callers can execute callbacks that
/// perform their own guarded environment setup without deadlocking. Use this
/// as the shared environment-mutation helper for tests that need temporary
/// global environment changes with `env_test_guard`-serialized setup and
/// teardown semantics.
pub struct EnvVarGuard {
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

    fn set_scoped(key: &str, value: impl AsRef<OsStr>) -> Self {
        Self::mutate_scoped(key, EnvVarMutation::Set(value.as_ref()))
    }

    fn remove_scoped(key: &str) -> Self {
        Self::mutate_scoped(key, EnvVarMutation::Remove)
    }

    fn mutate_scoped(key: &str, mutation: EnvVarMutation<'_>) -> Self {
        let _env_guard = env_test_guard();
        let previous = std::env::var_os(key);
        // SAFETY: `env_test_guard` serializes this environment mutation.
        unsafe {
            match mutation {
                EnvVarMutation::Set(value) => std::env::set_var(key, value),
                EnvVarMutation::Remove => std::env::remove_var(key),
            }
        }
        Self {
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
        let _env_guard = env_test_guard();
        match &self.previous {
            Some(previous) => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
                unsafe {
                    std::env::set_var(&self.key, previous);
                }
            }
            None => {
                // SAFETY: `env_test_guard` serializes this environment mutation.
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
/// locale-sensitive tests cannot leak global state between cases.
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
mod tests {
    //! Regression tests for process-wide environment synchronization.

    use std::{ffi::OsStr, sync::mpsc, thread, time::Duration};

    use super::{EnvVarGuard, env_test_guard, with_env_var, with_env_var_removed, with_locale};

    const ENVIRONMENT_KEY: &str = "WHITAKER_TEST_SUPPORT_SHARED_GUARD_TEST";

    #[test]
    fn scoped_mutation_blocks_env_var_guard_until_callback_finishes() {
        let (scoped_entered_sender, scoped_entered_receiver) = mpsc::channel();
        let (release_scoped_sender, release_scoped_receiver) = mpsc::channel();
        let scoped_thread = thread::spawn(move || {
            with_env_var(ENVIRONMENT_KEY, "scoped", || {
                scoped_entered_sender
                    .send(())
                    .expect("scoped callback entry must be reported");
                release_scoped_receiver
                    .recv()
                    .expect("scoped callback release must be received");
            });
        });
        scoped_entered_receiver
            .recv()
            .expect("scoped callback must enter before competing mutation");

        let (guard_started_sender, guard_started_receiver) = mpsc::channel();
        let (guard_created_sender, guard_created_receiver) = mpsc::channel();
        let guard_thread = thread::spawn(move || {
            guard_started_sender
                .send(())
                .expect("competing guard attempt must be reported");
            let _guard = EnvVarGuard::set(ENVIRONMENT_KEY, "guarded");
            guard_created_sender
                .send(())
                .expect("competing guard creation must be reported");
        });
        guard_started_receiver
            .recv()
            .expect("competing guard attempt must start");

        assert!(
            guard_created_receiver
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "shared guard mutation must wait for the scoped callback"
        );

        release_scoped_sender
            .send(())
            .expect("scoped callback must be released");
        scoped_thread
            .join()
            .expect("scoped callback thread must complete");
        guard_created_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("competing guard must proceed after the scoped callback");
        guard_thread
            .join()
            .expect("competing guard thread must complete");
    }

    #[test]
    fn scoped_mutation_allows_nested_shared_environment_setup() {
        with_env_var(ENVIRONMENT_KEY, "scoped", || {
            let _nested_guard = env_test_guard();
            assert_eq!(
                std::env::var(ENVIRONMENT_KEY)
                    .expect("scoped environment variable must remain available"),
                "scoped"
            );
        });
    }

    #[test]
    fn scoped_mutations_restore_values_after_parallel_guard_use() {
        let baseline = EnvVarGuard::set(ENVIRONMENT_KEY, "before");
        let (scoped_entered_sender, scoped_entered_receiver) = mpsc::channel();
        let (release_scoped_sender, release_scoped_receiver) = mpsc::channel();
        let scoped_thread = thread::spawn(move || {
            with_env_var(ENVIRONMENT_KEY, "scoped", || {
                scoped_entered_sender
                    .send(())
                    .expect("scoped callback entry must be reported");
                release_scoped_receiver
                    .recv()
                    .expect("scoped callback release must be received");
            });
        });
        scoped_entered_receiver
            .recv()
            .expect("scoped callback must enter before competing guard");

        let guard_thread = thread::spawn(|| {
            let _guard = EnvVarGuard::remove(ENVIRONMENT_KEY);
            assert!(std::env::var_os(ENVIRONMENT_KEY).is_none());
        });
        release_scoped_sender
            .send(())
            .expect("scoped callback must be released");
        scoped_thread
            .join()
            .expect("scoped callback thread must complete");
        guard_thread
            .join()
            .expect("competing guard thread must complete");

        assert_eq!(
            std::env::var_os(ENVIRONMENT_KEY).as_deref(),
            Some(OsStr::new("before"))
        );
        with_env_var_removed(ENVIRONMENT_KEY, || {
            assert!(std::env::var_os(ENVIRONMENT_KEY).is_none());
        });
        assert_eq!(
            std::env::var_os(ENVIRONMENT_KEY).as_deref(),
            Some(OsStr::new("before"))
        );
        drop(baseline);
        assert!(std::env::var_os(ENVIRONMENT_KEY).is_none());
    }

    #[test]
    fn locale_scopes_restore_set_and_removed_values() {
        let baseline = EnvVarGuard::set("DYLINT_LOCALE", "en-GB");

        with_locale(Some("cy"), || {
            assert_eq!(
                std::env::var_os("DYLINT_LOCALE").as_deref(),
                Some(OsStr::new("cy"))
            );
        });
        assert_eq!(
            std::env::var_os("DYLINT_LOCALE").as_deref(),
            Some(OsStr::new("en-GB"))
        );
        drop(baseline);

        let cleared = EnvVarGuard::remove("DYLINT_LOCALE");
        with_locale(None, || {
            assert!(std::env::var_os("DYLINT_LOCALE").is_none());
        });
        assert!(std::env::var_os("DYLINT_LOCALE").is_none());
        drop(cleared);
    }
}
