//! Regression tests for process-wide environment synchronization.

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use proptest::prelude::*;

use super::{with_env_var, with_env_var_removed, with_locale};

const ABSENT_SET_KEY: &str = "WHITAKER_TEST_SUPPORT_ABSENT_SET";
const REMOVED_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVED";
const SET_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_SET_PANIC";
const REMOVE_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVE_PANIC";
const SCOPED_KEY: &str = "WHITAKER_TEST_SUPPORT_SCOPED";
const OVERLAPPING_SCOPES_KEY: &str = "WHITAKER_TEST_SUPPORT_OVERLAPPING_SCOPES";
const PROPERTY_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY";

enum NestedMutation {
    Set,
    Remove,
}

struct ReleaseOnDrop(Option<Sender<()>>);

impl ReleaseOnDrop {
    fn release(mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

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

#[test]
fn with_env_var_removed_removes_and_restores_the_exact_value() {
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

#[test]
fn with_env_var_removed_restores_the_exact_value_after_a_panic() {
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

#[test]
fn scoped_mutation_blocks_a_competing_scoped_mutation_until_restoration() {
    assert!(std::env::var_os(OVERLAPPING_SCOPES_KEY).is_none());

    let (first_entered_sender, first_entered_receiver) = mpsc::channel();
    let (release_first_sender, release_first_receiver) = mpsc::channel();
    let first_thread = thread::spawn(move || {
        with_env_var(OVERLAPPING_SCOPES_KEY, "first", || {
            first_entered_sender
                .send(())
                .expect("first scoped mutation must be reported");
            release_first_receiver
                .recv()
                .expect("first scoped mutation must be released");
        });
    });
    let release_first = ReleaseOnDrop(Some(release_first_sender));
    first_entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("first scoped mutation must enter");

    let (second_attempted_sender, second_attempted_receiver) = mpsc::channel();
    let (second_entered_sender, second_entered_receiver) = mpsc::channel();
    let second_thread = thread::spawn(move || {
        second_attempted_sender
            .send(())
            .expect("second scoped mutation attempt must be reported");
        with_env_var(OVERLAPPING_SCOPES_KEY, "second", || {
            second_entered_sender
                .send(())
                .expect("second scoped mutation must be reported");
        });
    });
    second_attempted_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second scoped mutation attempt must start");

    assert_eq!(
        std::env::var_os(OVERLAPPING_SCOPES_KEY).as_deref(),
        Some(OsStr::new("first"))
    );
    assert!(matches!(
        second_entered_receiver.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    release_first.release();
    first_thread
        .join()
        .expect("first scoped mutation thread must complete");
    second_entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("second scoped mutation must proceed after restoration");
    second_thread
        .join()
        .expect("second scoped mutation thread must complete");

    assert!(std::env::var_os(OVERLAPPING_SCOPES_KEY).is_none());
}

#[test]
fn scoped_mutation_allows_nested_environment_setup() {
    with_env_var(SCOPED_KEY, "outer", || {
        with_env_var_removed(SCOPED_KEY, || {
            assert!(std::env::var_os(SCOPED_KEY).is_none());
        });

        assert_eq!(
            std::env::var(SCOPED_KEY).expect("outer environment value must be restored"),
            "outer"
        );
    });
}

#[test]
fn locale_scopes_restore_set_and_removed_values() {
    with_env_var_removed("DYLINT_LOCALE", || {
        with_locale(Some("cy"), || {
            assert_eq!(
                std::env::var_os("DYLINT_LOCALE").as_deref(),
                Some(OsStr::new("cy"))
            );
        });
        assert!(std::env::var_os("DYLINT_LOCALE").is_none());
    });

    with_env_var("DYLINT_LOCALE", "en-GB", || {
        with_locale(None, || {
            assert!(std::env::var_os("DYLINT_LOCALE").is_none())
        });
        assert_eq!(
            std::env::var_os("DYLINT_LOCALE").as_deref(),
            Some(OsStr::new("en-GB"))
        );
    });
}

fn transition_exposes_temporary_value() -> Result<(), TestCaseError> {
    let original = std::env::var_os(PROPERTY_KEY);
    let temporary_visible = with_env_var(PROPERTY_KEY, "temporary", || {
        std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("temporary"))
    });
    prop_assert!(temporary_visible);
    prop_assert_eq!(std::env::var_os(PROPERTY_KEY), original);
    Ok(())
}

fn transition_removes_and_restores_value() -> Result<(), TestCaseError> {
    let original = std::env::var_os(PROPERTY_KEY);
    let removed = with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none());
    prop_assert!(removed);
    prop_assert_eq!(std::env::var_os(PROPERTY_KEY), original);
    Ok(())
}

fn transition_restores_outer_value_after_nested_mutation(
    nested_mutation: NestedMutation,
) -> Result<(), TestCaseError> {
    let (inner_mutation_applied, outer_restored) = with_env_var(PROPERTY_KEY, "outer", || {
        let inner_mutation_applied = match nested_mutation {
            NestedMutation::Set => with_env_var(PROPERTY_KEY, "inner", || {
                std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("inner"))
            }),
            NestedMutation::Remove => {
                with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none())
            }
        };
        let outer_restored = std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("outer"));
        (inner_mutation_applied, outer_restored)
    });
    prop_assert!(inner_mutation_applied);
    prop_assert!(outer_restored);
    Ok(())
}

fn transition_restores_value_after_set_callback_panic() -> Result<(), TestCaseError> {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var(PROPERTY_KEY, "temporary", || panic!("test panic"));
    }));
    prop_assert!(panic.is_err());
    Ok(())
}

fn transition_restores_value_after_remove_callback_panic() -> Result<(), TestCaseError> {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var_removed(PROPERTY_KEY, || panic!("test panic"));
    }));
    prop_assert!(panic.is_err());
    Ok(())
}

proptest! {
    #[test]
    fn scoped_environment_transitions_restore_the_initial_value(
        initial in prop_oneof![Just(None), Just(Some("first")), Just(Some("second"))],
        transitions in prop::collection::vec(0_u8..6, 1..8),
    ) {
        with_env_var_removed(PROPERTY_KEY, || {
            let run_transitions = || -> Result<(), TestCaseError> {
                for transition in &transitions {
                    match transition {
                        0 => transition_exposes_temporary_value()?,
                        1 => transition_removes_and_restores_value()?,
                        2 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Remove)?,
                        3 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Set)?,
                        4 => transition_restores_value_after_set_callback_panic()?,
                        5 => transition_restores_value_after_remove_callback_panic()?,
                        _ => prop_assert!(false, "transition is outside the generated range"),
                    }

                    let restored = std::env::var_os(PROPERTY_KEY);
                    let expected = initial.map(OsStr::new);
                    prop_assert_eq!(restored.as_deref(), expected);
                }
                Ok(())
            };

            match initial {
                Some(value) => with_env_var(PROPERTY_KEY, value, run_transitions),
                None => run_transitions(),
            }
        })?;
    }
}
