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
//! - [`with_locale`], [`with_env_var`], and [`with_env_var_removed`]: Scope temporary environment
//!   mutations (such as `DYLINT_LOCALE` overrides) to a callback so tests cannot leak global state
//!   between cases.

pub mod decomposition;
pub mod fixtures;
pub mod ui;

use std::{
    ffi::OsStr,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};

pub use fixtures::{copy_directory, copy_fixture};
pub use ui::{
    FixtureEnvironment,
    discover_fixtures,
    prepare_fixture,
    read_directory_config,
    read_fixture_config,
    resolve_fixture_config,
    run_fixtures_with,
    run_test_runner,
};

/// Serializes tests that mutate process-wide environment variables.
///
/// Use this guard around helpers such as `temp_env::with_var` or
/// `temp_env::with_vars_unset` when the test would otherwise race with other
/// cases changing the same global process state.
///
/// The mutex guards `()`, so a poisoned lock carries no corrupted state: it
/// only records that some earlier test panicked while holding the
/// serialization token. Recovering the guard is therefore sound, and keeps one
/// failing test from cascading into every later one.
pub fn env_test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Runs `callback` with one environment variable temporarily set.
///
/// The mutation is scoped to the callback and the prior value is restored
/// afterwards, even on panic. Serialization is provided by `temp_env`'s
/// re-entrant global lock, so nested scoped mutations from the same thread
/// do not deadlock.
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
    temp_env::with_var(key, Some(value.as_ref()), callback)
}

/// Runs `callback` with one environment variable temporarily removed.
///
/// The prior value (if any) is restored after the callback completes or
/// panics.
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
    temp_env::with_var_unset(key, callback)
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
