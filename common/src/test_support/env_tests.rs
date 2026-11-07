//! Regression tests for process-wide environment synchronization.

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use proptest::prelude::*;

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
const OVERLAPPING_KEY: &str = "WHITAKER_TEST_SUPPORT_OVERLAPPING";
const SELECTED_KEY: &str = "WHITAKER_TEST_SUPPORT_SELECTED";
const VARS_FIRST_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_FIRST";
const VARS_SECOND_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_SECOND";
const VARS_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_VARS_PANIC";
const PROPERTY_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY";
const PROPERTY_COMPANION_KEY: &str = "WHITAKER_TEST_SUPPORT_PROPERTY_COMPANION";

#[derive(Clone, Copy)]
enum NestedMutation {
    Set,
    Remove,
}

struct ReleaseOnDrop(Option<mpsc::Sender<()>>);

impl ReleaseOnDrop {
    /// Release a worker blocked inside a scoped mutation.
    fn release(mut self) { self.signal_worker(); }

    /// Unblock the worker, tolerating one that has already finished.
    ///
    /// Both the explicit release and the unwind-time drop send the same
    /// signal, so the outcome is discarded rather than propagated: a closed
    /// channel only means the worker left first.
    fn signal_worker(&mut self) {
        if let Some(sender) = self.0.take() {
            let _send_outcome = sender.send(());
        }
    }
}

impl Drop for ReleaseOnDrop {
    /// Unblock a worker if an earlier assertion unwinds the test.
    fn drop(&mut self) { self.signal_worker(); }
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
fn spawn_scoped_mutation(key: &'static str, value: &'static str) -> ScopedMutationThread {
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (restored_sender, restored_receiver) = mpsc::channel();
    let thread = thread::spawn(move || {
        with_env_var(key, value, || {
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

/// Check that concurrent scoped mutations wait for the current callback.
#[test]
fn scoped_mutations_wait_for_callback_restoration() {
    let original_value = std::env::var_os(OVERLAPPING_KEY);
    let first = spawn_scoped_mutation(OVERLAPPING_KEY, "first");
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

/// Check that the selection waits for the current scoped mutation to restore.
///
/// Regression: the UI harness chose which variables to scope before taking
/// the environment lock, and skipped the lock entirely when it chose none. A
/// runner that overlapped another therefore saw the first runner's temporary
/// flag, selected nothing, and then ran after the flag had been removed, so
/// `dylint_testing` copied its default-feature stub over the prepared library.
#[test]
fn env_selection_waits_for_callback_restoration() {
    let original_value = std::env::var_os(SELECTED_KEY);
    let first = spawn_scoped_mutation(SELECTED_KEY, "first");
    let release_first = ReleaseOnDrop(Some(first.release_sender.clone()));
    first.wait_for_entry("first callback should enter before the selection starts");

    let (attempted_sender, attempted_receiver) = mpsc::channel();
    let (selected_sender, selected_receiver) = mpsc::channel();
    let selector = thread::spawn(move || {
        attempted_sender
            .send(())
            .expect("selection attempt must be reported");
        with_env_vars_selected(
            || {
                selected_sender
                    .send(())
                    .expect("selection evaluation must be reported");
                // Mirrors the harness's "only supply what is absent" rule.
                if std::env::var_os(SELECTED_KEY).is_none() {
                    vec![(SELECTED_KEY, "selected")]
                } else {
                    Vec::new()
                }
            },
            || std::env::var_os(SELECTED_KEY),
        )
    });
    attempted_receiver
        .recv()
        .expect("selection attempt should start");

    assert!(
        selected_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the selection must wait for the first scoped mutation to restore"
    );

    release_first.release();
    first.wait_for_restoration("the first value should restore before the selection runs");
    selected_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("the selection should run after restoration");

    let seen_by_callback = selector.join().expect("selection thread should complete");
    first.join();

    assert_eq!(seen_by_callback.as_deref(), Some(OsStr::new("selected")));
    assert_eq!(std::env::var_os(SELECTED_KEY), original_value);
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

/// Baseline value of one property variable, where `None` means absent.
type BaselineValue = Option<&'static str>;

/// Starting environment the generated transitions must restore.
#[derive(Clone, Copy)]
struct Baseline {
    primary: BaselineValue,
    companion: BaselineValue,
}

/// Apply one baseline value, removing the key when the value is absent.
fn with_baseline_value<R>(
    key: &'static str,
    value: BaselineValue,
    callback: impl FnOnce() -> R,
) -> R {
    match value {
        Some(set_value) => with_env_var(key, set_value, callback),
        None => with_env_var_removed(key, callback),
    }
}

/// Run `callback` with both property variables at their generated baseline.
fn with_baseline<R>(baseline: Baseline, callback: impl FnOnce() -> R) -> R {
    with_baseline_value(PROPERTY_KEY, baseline.primary, || {
        with_baseline_value(PROPERTY_COMPANION_KEY, baseline.companion, callback)
    })
}

/// Check that both property variables match the generated baseline.
fn assert_baseline_restored(baseline: Baseline) -> Result<(), TestCaseError> {
    let primary = std::env::var_os(PROPERTY_KEY);
    prop_assert_eq!(primary.as_deref(), baseline.primary.map(OsStr::new));

    let companion = std::env::var_os(PROPERTY_COMPANION_KEY);
    prop_assert_eq!(companion.as_deref(), baseline.companion.map(OsStr::new));
    Ok(())
}

/// Build the settings that set every generated key to the same value.
fn settings_for(keys: &[&'static str], value: &'static str) -> Vec<(&'static str, &'static str)> {
    keys.iter().map(|key| (*key, value)).collect()
}

/// Check that every generated key currently holds `value`.
fn every_key_has(keys: &[&'static str], value: &'static str) -> bool {
    keys.iter()
        .all(|key| std::env::var_os(key).as_deref() == Some(OsStr::new(value)))
}

/// Exercise `with_env_vars` over a generated key list.
fn transition_with_env_vars_sets_every_key(keys: &[&'static str]) -> bool {
    let mut callback_ran = false;
    let applied = with_env_vars(settings_for(keys, "vars temporary"), || {
        callback_ran = true;
        every_key_has(keys, "vars temporary")
    });
    // An empty list takes the helper's lock-free short circuit, so the callback
    // must still run for the transition to be meaningful.
    callback_ran && applied
}

/// Exercise `with_env_vars_selected` over a generated key list.
fn transition_with_env_vars_selected_sets_every_key(keys: &[&'static str]) -> bool {
    let mut callback_ran = false;
    let applied = with_env_vars_selected(
        || settings_for(keys, "selected temporary"),
        || {
            callback_ran = true;
            every_key_has(keys, "selected temporary")
        },
    );
    callback_ran && applied
}

/// Exercise nested multi-variable scopes and restoration of the outer values.
fn transition_nests_multi_variable_scopes(keys: &[&'static str]) -> bool {
    with_env_vars(settings_for(keys, "vars outer"), || {
        let inner_applied = with_env_vars(settings_for(keys, "vars inner"), || {
            every_key_has(keys, "vars inner")
        });
        let outer_restored = every_key_has(keys, "vars outer");
        inner_applied && outer_restored
    })
}

/// Exercise a repeated key inside one multi-variable scope.
fn transition_keeps_the_last_repeated_value(keys: &[&'static str]) -> bool {
    let mut settings = settings_for(keys, "vars temporary");
    settings.push((PROPERTY_KEY, "repeated first"));
    settings.push((PROPERTY_KEY, "repeated second"));
    with_env_vars(settings, || {
        let repeated_kept =
            std::env::var_os(PROPERTY_KEY).as_deref() == Some(OsStr::new("repeated second"));
        let others_kept = keys
            .iter()
            .copied()
            .filter(|key| *key != PROPERTY_KEY)
            .all(|key| std::env::var_os(key).as_deref() == Some(OsStr::new("vars temporary")));
        repeated_kept && others_kept
    })
}

/// Exercise restoration after a multi-variable callback panics.
fn transition_multi_variable_restores_after_panic(keys: &[&'static str]) -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_vars(settings_for(keys, "vars temporary"), || {
            panic!("test panic")
        });
    }));
    panic.is_err()
}

/// Exercise restoration after a selected multi-variable callback panics.
fn transition_selected_multi_variable_restores_after_panic(keys: &[&'static str]) -> bool {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_env_vars_selected(
            || settings_for(keys, "selected temporary"),
            || panic!("test panic"),
        );
    }));
    panic.is_err()
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

/// Apply one generated transition and report whether its own checks passed.
fn apply_transition(transition: u8, keys: &[&'static str]) -> Result<bool, TestCaseError> {
    let succeeded = match transition {
        0 => transition_exposes_temporary_value(),
        1 => transition_removes_temporary_value(),
        2 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Remove),
        3 => transition_restores_outer_value_after_nested_mutation(NestedMutation::Set),
        4 => transition_restores_value_after_set_callback_panic(),
        5 => transition_restores_value_after_remove_callback_panic(),
        6 => transition_with_env_vars_sets_every_key(keys),
        7 => transition_with_env_vars_selected_sets_every_key(keys),
        8 => transition_nests_multi_variable_scopes(keys),
        9 => transition_keeps_the_last_repeated_value(keys),
        10 => transition_multi_variable_restores_after_panic(keys),
        11 => transition_selected_multi_variable_restores_after_panic(keys),
        other => {
            return Err(TestCaseError::fail(format!(
                "the transition strategy only yields values below twelve, got {other}"
            )));
        }
    };
    Ok(succeeded)
}

/// Run a generated sequence and confirm every scoped transition restores its baseline.
fn run_transitions(
    baseline: Baseline,
    keys: &[&'static str],
    transitions: &[u8],
) -> Result<(), TestCaseError> {
    for transition in transitions {
        let transition_succeeded = apply_transition(*transition, keys)?;
        prop_assert!(transition_succeeded);

        assert_baseline_restored(baseline)?;
    }
    Ok(())
}

proptest! {
    /// Check that varied scoped mutations restore the original environment values.
    #[test]
    fn scoped_environment_transitions_restore_the_initial_value(
        primary in prop_oneof![Just(None), Just(Some("first")), Just(Some("second"))],
        companion in prop_oneof![Just(None), Just(Some("companion"))],
        keys in prop::collection::vec(
            prop_oneof![Just(PROPERTY_KEY), Just(PROPERTY_COMPANION_KEY)],
            0..4,
        ),
        transitions in prop::collection::vec(0_u8..12, 1..8),
    ) {
        let baseline = Baseline { primary, companion };
        with_baseline(baseline, || run_transitions(baseline, &keys, &transitions))?;
    }
}
