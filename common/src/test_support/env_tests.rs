//! Regression tests for scoped environment mutation semantics.
//!
//! These cases run on the calling thread and cover how each helper sets,
//! removes, and restores a variable, including when the callback panics.
//! Cross-thread serialization lives in [`super::env_concurrency_tests`].

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
};

use super::{
    env_test_guard,
    with_env_var,
    with_env_var_removed,
    with_env_vars,
    with_env_vars_selected,
    with_locale,
};

const ABSENT_SET_KEY: &str = "WHITAKER_TEST_SUPPORT_ABSENT_SET";
const REMOVED_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVED";
const SET_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_SET_PANIC";
const REMOVE_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVE_PANIC";
const SCOPED_KEY: &str = "WHITAKER_TEST_SUPPORT_SCOPED";
const VARS_FIRST_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_FIRST";
const VARS_SECOND_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_SECOND";
const VARS_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_PANIC";

/// Check that a scoped set exposes its value and restores an absent variable.
#[test]
fn with_env_var_exposes_and_restores_an_absent_value() {
    with_env_var_removed(ABSENT_SET_KEY, || {
        with_env_var(ABSENT_SET_KEY, "temporary", || {
            assert_eq!(
                std::env::var_os(ABSENT_SET_KEY).as_deref(),
                Some(OsStr::new("temporary"))
            );
        });

        assert!(std::env::var_os(ABSENT_SET_KEY).is_none());
    });
}

/// Check that a scoped removal restores the exact previous value.
#[test]
fn with_env_var_removed_restores_the_previous_value() {
    with_env_var(REMOVED_KEY, "original value", || {
        with_env_var_removed(REMOVED_KEY, || {
            assert!(std::env::var_os(REMOVED_KEY).is_none());
        });

        assert_eq!(
            std::env::var_os(REMOVED_KEY).as_deref(),
            Some(OsStr::new("original value"))
        );
    });
}

/// Check that scoped set restoration still runs when the callback panics.
#[test]
fn with_env_var_restores_an_absent_value_after_a_panic() {
    with_env_var_removed(SET_PANIC_KEY, || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_env_var(SET_PANIC_KEY, "temporary", || {
                panic!("intentional test panic")
            });
        }));

        assert!(result.is_err());
        assert!(std::env::var_os(SET_PANIC_KEY).is_none());
    });
}

/// Check that scoped removal restoration still runs when the callback panics.
#[test]
fn with_env_var_removed_restores_a_value_after_a_panic() {
    with_env_var(REMOVE_PANIC_KEY, "original value", || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_env_var_removed(REMOVE_PANIC_KEY, || panic!("intentional test panic"));
        }));

        assert!(result.is_err());
        assert_eq!(
            std::env::var_os(REMOVE_PANIC_KEY).as_deref(),
            Some(OsStr::new("original value"))
        );
    });
}

/// Check that one scoped call sets every variable and restores absent values.
#[test]
fn with_env_vars_sets_every_variable_and_restores_absent_values() {
    with_env_var_removed(VARS_FIRST_KEY, || {
        with_env_var_removed(VARS_SECOND_KEY, || {
            with_env_vars(
                [(VARS_FIRST_KEY, "first"), (VARS_SECOND_KEY, "second")],
                || {
                    assert_eq!(
                        std::env::var_os(VARS_FIRST_KEY).as_deref(),
                        Some(OsStr::new("first"))
                    );
                    assert_eq!(
                        std::env::var_os(VARS_SECOND_KEY).as_deref(),
                        Some(OsStr::new("second"))
                    );
                },
            );

            assert!(std::env::var_os(VARS_FIRST_KEY).is_none());
            assert!(std::env::var_os(VARS_SECOND_KEY).is_none());
        });
    });
}

/// Check that one scoped call restores every pre-existing value.
#[test]
fn with_env_vars_restores_existing_values() {
    with_env_vars(
        [
            (VARS_FIRST_KEY, "original first"),
            (VARS_SECOND_KEY, "original second"),
        ],
        || {
            with_env_vars(
                [
                    (VARS_FIRST_KEY, "temporary first"),
                    (VARS_SECOND_KEY, "temporary second"),
                ],
                || {
                    assert_eq!(
                        std::env::var_os(VARS_FIRST_KEY).as_deref(),
                        Some(OsStr::new("temporary first"))
                    );
                },
            );

            assert_eq!(
                std::env::var_os(VARS_FIRST_KEY).as_deref(),
                Some(OsStr::new("original first"))
            );
            assert_eq!(
                std::env::var_os(VARS_SECOND_KEY).as_deref(),
                Some(OsStr::new("original second"))
            );
        },
    );
}

/// Check that a multi-variable scope restores every value after a panic.
#[test]
fn with_env_vars_restores_every_value_after_a_panic() {
    with_env_var_removed(VARS_PANIC_KEY, || {
        with_env_var_removed(VARS_SECOND_KEY, || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_env_vars(
                    [
                        (VARS_PANIC_KEY, "temporary"),
                        (VARS_SECOND_KEY, "temporary"),
                    ],
                    || panic!("intentional test panic"),
                );
            }));

            assert!(result.is_err());
            assert!(std::env::var_os(VARS_PANIC_KEY).is_none());
            assert!(std::env::var_os(VARS_SECOND_KEY).is_none());
        });
    });
}

/// Check that a repeated key keeps the value applied last.
///
/// `temp_env` applies the settings in order and records the original value
/// once, so a repeated key is well defined rather than a hazard callers must
/// avoid.
#[test]
fn with_env_vars_applies_the_last_value_for_a_repeated_key() {
    with_env_var_removed(VARS_FIRST_KEY, || {
        with_env_vars(
            [
                (VARS_FIRST_KEY, "first"),
                (VARS_SECOND_KEY, "second"),
                (VARS_FIRST_KEY, "last"),
            ],
            || {
                assert_eq!(
                    std::env::var_os(VARS_FIRST_KEY).as_deref(),
                    Some(OsStr::new("last"))
                );
                assert_eq!(
                    std::env::var_os(VARS_SECOND_KEY).as_deref(),
                    Some(OsStr::new("second"))
                );
            },
        );

        assert!(std::env::var_os(VARS_FIRST_KEY).is_none());
    });
}

/// Check that an empty selection still runs the callback without locking.
///
/// The test asserts only the callback's result: reading a shared key here
/// would race the tests that mutate it, because the empty selection is
/// exactly the case that takes no lock.
#[test]
fn with_env_vars_without_variables_runs_the_callback() {
    let callback_result = with_env_vars(Vec::new(), || 42_u8);

    assert_eq!(callback_result, 42);
}

/// Check that the selecting helper applies and restores its selection.
#[test]
fn with_env_vars_selected_applies_and_restores_the_selection() {
    with_env_var_removed(VARS_FIRST_KEY, || {
        with_env_vars_selected(
            || vec![(VARS_FIRST_KEY, "selected")],
            || {
                assert_eq!(
                    std::env::var_os(VARS_FIRST_KEY).as_deref(),
                    Some(OsStr::new("selected"))
                );
            },
        );

        assert!(std::env::var_os(VARS_FIRST_KEY).is_none());
    });
}

/// Check that nested scoped overrides restore the outer value.
#[test]
fn scoped_mutations_allow_nested_environment_setup() {
    with_env_var(SCOPED_KEY, "outer", || {
        with_env_var(SCOPED_KEY, "inner", || {
            assert_eq!(
                std::env::var(SCOPED_KEY).expect("inner scoped value should be available"),
                "inner"
            );
        });

        assert_eq!(
            std::env::var(SCOPED_KEY).expect("outer scoped value should be restored"),
            "outer"
        );
        let _guard = env_test_guard();
    });
}

/// Check that locale overrides restore both set and absent values.
#[test]
fn locale_scopes_restore_set_and_removed_values() {
    with_env_var("DYLINT_LOCALE", "en-GB", || {
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

        with_locale(None, || {
            assert!(std::env::var_os("DYLINT_LOCALE").is_none());
        });
        assert_eq!(
            std::env::var_os("DYLINT_LOCALE").as_deref(),
            Some(OsStr::new("en-GB"))
        );
    });
}
