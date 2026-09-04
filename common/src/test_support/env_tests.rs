//! Regression tests for process-wide environment synchronization.

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{EnvVarGuard, env_test_guard, with_env_var, with_env_var_removed, with_locale};
use proptest::prelude::*;

const ABSENT_SET_KEY: &str = "WHITAKER_TEST_SUPPORT_ABSENT_SET";
const REMOVED_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVED";
const SET_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_SET_PANIC";
const REMOVE_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_REMOVE_PANIC";
const SCOPED_KEY: &str = "WHITAKER_TEST_SUPPORT_SCOPED";
const OVERLAPPING_GUARDS_KEY: &str = "WHITAKER_TEST_SUPPORT_OVERLAPPING_GUARDS";
const PROPERTY_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY";

struct GuardThread {
    release_sender: mpsc::Sender<()>,
    created_receiver: mpsc::Receiver<()>,
    dropped_receiver: mpsc::Receiver<()>,
    thread: JoinHandle<()>,
}

impl GuardThread {
    fn release(&self, description: &str) {
        self.release_sender.send(()).expect(description);
    }

    fn wait_for_creation(&self, description: &str) {
        self.created_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect(description);
    }

    fn wait_for_drop(&self, description: &str) {
        self.dropped_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect(description);
    }

    fn join(self, description: &str) {
        self.thread.join().expect(description);
    }
}

fn spawn_first_guard() -> GuardThread {
    let (created_sender, created_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (dropped_sender, dropped_receiver) = mpsc::channel();
    let thread = thread::spawn(move || {
        let guard = EnvVarGuard::set(OVERLAPPING_GUARDS_KEY, "first");
        created_sender
            .send(())
            .expect("first guard creation must be reported");
        release_receiver
            .recv()
            .expect("first guard release must be received");
        drop(guard);
        dropped_sender
            .send(())
            .expect("first guard restoration must be reported");
    });
    GuardThread {
        release_sender,
        created_receiver,
        dropped_receiver,
        thread,
    }
}

fn spawn_second_guard() -> (GuardThread, mpsc::Receiver<()>) {
    let (attempted_sender, attempted_receiver) = mpsc::channel();
    let (created_sender, created_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (dropped_sender, dropped_receiver) = mpsc::channel();
    let thread = thread::spawn(move || {
        attempted_sender
            .send(())
            .expect("second guard attempt must be reported");
        let guard = EnvVarGuard::set(OVERLAPPING_GUARDS_KEY, "second");
        created_sender
            .send(())
            .expect("second guard mutation must be reported");
        release_receiver
            .recv()
            .expect("second guard release must be received");
        drop(guard);
        dropped_sender
            .send(())
            .expect("second guard restoration must be reported");
    });
    (
        GuardThread {
            release_sender,
            created_receiver,
            dropped_receiver,
            thread,
        },
        attempted_receiver,
    )
}

#[test]
fn with_env_var_exposes_and_restores_an_absent_value() {
    let baseline = EnvVarGuard::remove(ABSENT_SET_KEY);

    with_env_var(ABSENT_SET_KEY, "temporary", || {
        assert_eq!(
            std::env::var_os(ABSENT_SET_KEY).as_deref(),
            Some(OsStr::new("temporary"))
        );
    });

    assert!(std::env::var_os(ABSENT_SET_KEY).is_none());
    drop(baseline);
}

#[test]
fn with_env_var_removed_removes_and_restores_the_exact_value() {
    let baseline = EnvVarGuard::set(REMOVED_KEY, "original value");

    with_env_var_removed(REMOVED_KEY, || {
        assert!(std::env::var_os(REMOVED_KEY).is_none());
    });

    assert_eq!(
        std::env::var_os(REMOVED_KEY).as_deref(),
        Some(OsStr::new("original value"))
    );
    drop(baseline);
}

#[test]
fn with_env_var_restores_an_absent_value_after_a_panic() {
    let baseline = EnvVarGuard::remove(SET_PANIC_KEY);

    let result = catch_unwind(AssertUnwindSafe(|| {
        with_env_var(SET_PANIC_KEY, "temporary", || {
            panic!("intentional test panic")
        });
    }));

    assert!(result.is_err());
    assert!(std::env::var_os(SET_PANIC_KEY).is_none());
    drop(baseline);
}

#[test]
fn with_env_var_removed_restores_the_exact_value_after_a_panic() {
    let baseline = EnvVarGuard::set(REMOVE_PANIC_KEY, "original value");

    let result = catch_unwind(AssertUnwindSafe(|| {
        with_env_var_removed(REMOVE_PANIC_KEY, || panic!("intentional test panic"));
    }));

    assert!(result.is_err());
    assert_eq!(
        std::env::var_os(REMOVE_PANIC_KEY).as_deref(),
        Some(OsStr::new("original value"))
    );
    drop(baseline);
}

#[test]
fn scoped_mutation_blocks_env_var_guard_until_callback_finishes() {
    let (scoped_entered_sender, scoped_entered_receiver) = mpsc::channel();
    let (release_scoped_sender, release_scoped_receiver) = mpsc::channel();
    let scoped_thread = thread::spawn(move || {
        with_env_var(SCOPED_KEY, "scoped", || {
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
        let _guard = EnvVarGuard::set(SCOPED_KEY, "guarded");
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
    with_env_var(SCOPED_KEY, "scoped", || {
        let _nested_guard = env_test_guard();
        assert_eq!(
            std::env::var(SCOPED_KEY).expect("scoped environment variable must remain available"),
            "scoped"
        );
    });
}

#[test]
fn overlapping_env_var_guards_wait_for_mutation_and_restoration() {
    {
        let _guard = env_test_guard();
        assert!(
            std::env::var_os(OVERLAPPING_GUARDS_KEY).is_none(),
            "the unique regression key must start absent"
        );
    }
    let first_guard = spawn_first_guard();
    first_guard.wait_for_creation("first guard must be created before the second starts");
    let (second_guard, second_attempted_receiver) = spawn_second_guard();
    second_attempted_receiver
        .recv()
        .expect("second guard must attempt construction");

    assert_eq!(
        std::env::var_os(OVERLAPPING_GUARDS_KEY).as_deref(),
        Some(OsStr::new("first"))
    );
    assert!(
        second_guard
            .created_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the second guard must not mutate before the first restores"
    );
    assert!(
        second_guard
            .dropped_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the second guard cannot restore before it acquires the shared guard"
    );

    first_guard.release("first guard must be released");
    first_guard.wait_for_drop("first guard must restore before the second can proceed");
    second_guard.wait_for_creation("second guard must mutate after the first restores");
    assert_eq!(
        std::env::var_os(OVERLAPPING_GUARDS_KEY).as_deref(),
        Some(OsStr::new("second"))
    );

    second_guard.release("second guard must be released");
    second_guard.wait_for_drop("second guard must restore after release");
    first_guard.join("first guard thread must complete");
    second_guard.join("second guard thread must complete");
    let _guard = env_test_guard();
    assert!(std::env::var_os(OVERLAPPING_GUARDS_KEY).is_none());
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

proptest! {
    #[test]
    fn scoped_environment_transitions_restore_the_initial_value(
        initial in prop_oneof![Just(None), Just(Some("first")), Just(Some("second"))],
        transitions in prop::collection::vec(0_u8..6, 1..8),
    ) {
        let _protocol = env_test_guard();
        let baseline = match initial {
            Some(value) => EnvVarGuard::set(PROPERTY_KEY, value),
            None => EnvVarGuard::remove(PROPERTY_KEY),
        };

        for transition in transitions {
            match transition {
                0 => {
                    let temporary_visible = with_env_var(PROPERTY_KEY, "temporary", || {
                        std::env::var_os(PROPERTY_KEY).as_deref()
                            == Some(OsStr::new("temporary"))
                    });
                    prop_assert!(temporary_visible);
                }
                1 => {
                    let removed = with_env_var_removed(PROPERTY_KEY, || {
                        std::env::var_os(PROPERTY_KEY).is_none()
                    });
                    prop_assert!(removed);
                }
                2 => {
                    let (inner_removed, outer_restored) = with_env_var(PROPERTY_KEY, "outer", || {
                        let inner_removed = with_env_var_removed(PROPERTY_KEY, || {
                            std::env::var_os(PROPERTY_KEY).is_none()
                        });
                        let outer_restored = std::env::var_os(PROPERTY_KEY).as_deref()
                            == Some(OsStr::new("outer"));
                        (inner_removed, outer_restored)
                    });
                    prop_assert!(inner_removed);
                    prop_assert!(outer_restored);
                }
                3 => {
                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        with_env_var(PROPERTY_KEY, "temporary", || panic!("test panic"));
                    }));
                    prop_assert!(panic.is_err());
                }
                4 => {
                    let competing_guard = EnvVarGuard::set(PROPERTY_KEY, "guarded");
                    drop(competing_guard);
                }
                5 => {
                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        with_env_var_removed(PROPERTY_KEY, || panic!("test panic"));
                    }));
                    prop_assert!(panic.is_err());
                }
                _ => unreachable!("the transition strategy only yields values below six"),
            }

            let restored = std::env::var_os(PROPERTY_KEY);
            prop_assert_eq!(restored.as_deref(), initial.map(OsStr::new));
        }

        drop(baseline);
    }
}
