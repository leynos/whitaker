//! Regression tests for process-wide environment synchronization.

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{env_test_guard, with_env_var, with_env_var_removed, with_locale};
use proptest::prelude::*;

const ABSENT_SET_KEY: &str = "WHITAKER_TEST_SUPPORT_ABSENT_SET";
const REMOVED_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVED";
const SET_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_SET_PANIC";
const REMOVE_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVE_PANIC";
const SCOPED_KEY: &str = "WHITAKER_TEST_SUPPORT_SCOPED";
const OVERLAPPING_KEY: &str = "WHITAKER_TEST_SUPPORT_OVERLAPPING";
const PROPERTY_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY";

enum NestedMutation {
    Set,
    Remove,
}

struct ReleaseOnDrop(Option<mpsc::Sender<()>>);

impl ReleaseOnDrop {
    /// Release a worker blocked inside a scoped mutation.
    fn release(mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for ReleaseOnDrop {
    /// Unblock a worker if an earlier assertion unwinds the test.
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

struct ScopedMutationThread {
    release_sender: mpsc::Sender<()>,
    entered_receiver: mpsc::Receiver<()>,
    restored_receiver: mpsc::Receiver<()>,
    thread: JoinHandle<()>,
}

impl ScopedMutationThread {
    /// Waits until the scoped environment callback has entered.
    fn wait_for_entry(&self, description: &str) {
        self.entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect(description);
    }

    /// Waits until the scoped environment value has been restored.
    fn wait_for_restoration(&self, description: &str) {
        self.restored_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect(description);
    }

    /// Joins the worker after its environment callback has completed.
    fn join(self) {
        self.thread
            .join()
            .expect("scoped environment thread should complete");
    }
}

/// Starts a thread that holds a scoped value until the test releases it.
fn spawn_scoped_mutation(value: &'static str) -> ScopedMutationThread {
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (restored_sender, restored_receiver) = mpsc::channel();
    let thread = thread::spawn(move || {
        with_env_var(OVERLAPPING_KEY, value, || {
            entered_sender
                .send(())
                .expect("scoped callback entry must be reported");
            release_receiver
                .recv()
                .expect("scoped callback release must be received");
        });
        restored_sender
            .send(())
            .expect("environment restoration must be reported");
    });
    ScopedMutationThread {
        release_sender,
        entered_receiver,
        restored_receiver,
        thread,
    }
}

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

/// Check that concurrent scoped mutations wait for the current callback.
#[test]
fn scoped_mutations_wait_for_callback_restoration() {
    let original_value = std::env::var_os(OVERLAPPING_KEY);
    let first = spawn_scoped_mutation("first");
    let release_first = ReleaseOnDrop(Some(first.release_sender.clone()));
    first.wait_for_entry("first callback should enter before the second starts");

    let (attempted_sender, attempted_receiver) = mpsc::channel();
    let second = {
        let (entered_sender, entered_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let (restored_sender, restored_receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            attempted_sender
                .send(())
                .expect("second mutation attempt must be reported");
            with_env_var(OVERLAPPING_KEY, "second", || {
                entered_sender
                    .send(())
                    .expect("second callback entry must be reported");
                release_receiver
                    .recv()
                    .expect("second callback release must be received");
            });
            restored_sender
                .send(())
                .expect("second environment restoration must be reported");
        });
        ScopedMutationThread {
            release_sender,
            entered_receiver,
            restored_receiver,
            thread,
        }
    };
    let release_second = ReleaseOnDrop(Some(second.release_sender.clone()));
    attempted_receiver
        .recv()
        .expect("second mutation attempt should start");

    assert_eq!(
        std::env::var_os(OVERLAPPING_KEY).as_deref(),
        Some(OsStr::new("first"))
    );
    assert!(
        second
            .entered_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the second callback must wait for the first scoped mutation"
    );

    release_first.release();
    first.wait_for_restoration("the first value should restore before the second enters");
    second.wait_for_entry("the second callback should enter after restoration");
    assert_eq!(
        std::env::var_os(OVERLAPPING_KEY).as_deref(),
        Some(OsStr::new("second"))
    );

    release_second.release();
    second.wait_for_restoration("the second value should restore after release");
    first.join();
    second.join();
    assert_eq!(std::env::var_os(OVERLAPPING_KEY), original_value);
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
            assert!(std::env::var_os("DYLINT_LOCALE").is_none())
        });
        assert_eq!(
            std::env::var_os("DYLINT_LOCALE").as_deref(),
            Some(OsStr::new("en-GB"))
        );
    });
}

/// Exercise scoped environment transitions from several starting values.
fn transition_exposes_temporary_value() -> bool {
    with_env_var(PROPERTY_KEY, "temporary", || {
        std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("temporary"))
    })
}

/// Exercise scoped removal while the generated baseline is active.
fn transition_removes_temporary_value() -> bool {
    with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none())
}

/// Exercise nested mutation and restoration of the outer value.
fn transition_restores_outer_value_after_nested_mutation(nested_mutation: NestedMutation) -> bool {
    with_env_var(PROPERTY_KEY, "outer", || {
        let inner_mutation_applied = match nested_mutation {
            NestedMutation::Set => with_env_var(PROPERTY_KEY, "inner", || {
                std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("inner"))
            }),
            NestedMutation::Remove => {
                with_env_var_removed(PROPERTY_KEY, || std::env::var_os(PROPERTY_KEY).is_none())
            }
        };
        let outer_restored = std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("outer"));
        inner_mutation_applied && outer_restored
    })
}

/// Exercise restoration after a set callback panics.
fn transition_restores_value_after_set_callback_panic() -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var(PROPERTY_KEY, "temporary", || panic!("test panic"));
    }));
    panic.is_err()
}

/// Exercise restoration after a removal callback panics.
fn transition_restores_value_after_remove_callback_panic() -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_var_removed(PROPERTY_KEY, || panic!("test panic"));
    }));
    panic.is_err()
}

/// Run a generated sequence and confirm every scoped transition restores its baseline.
fn run_transitions(initial: Option<&str>, transitions: &[u8]) -> Result<(), TestCaseError> {
    for transition in transitions {
        let transition_succeeded = match transition {
            0 => transition_exposes_temporary_value(),
            1 => transition_removes_temporary_value(),
            2 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Remove),
            3 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Set),
            4 => transition_restores_value_after_set_callback_panic(),
            5 => transition_restores_value_after_remove_callback_panic(),
            _ => unreachable!("the transition strategy only yields values below six"),
        };
        prop_assert!(transition_succeeded);

        let restored = std::env::var_os(PROPERTY_KEY);
        prop_assert_eq!(restored.as_deref(), initial.map(OsStr::new));
    }
    Ok(())
}

proptest! {
    /// Check that varied scoped mutations restore the original environment value.
    #[test]
    fn scoped_environment_transitions_restore_the_initial_value(
        initial in prop_oneof![Just(None), Just(Some("first")), Just(Some("second"))],
        transitions in prop::collection::vec(0_u8..6, 1..8),
    ) {
        let result = match initial {
            Some(value) => with_env_var(PROPERTY_KEY, value, || run_transitions(Some(value), &transitions)),
            None => with_env_var_removed(PROPERTY_KEY, || run_transitions(None, &transitions)),
        };
        result?;
    }
}
