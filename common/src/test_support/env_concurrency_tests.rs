//! Regression tests for cross-thread environment serialization.
//!
//! Each case holds a scoped mutation on one thread and proves that another
//! thread's mutation, or a variable selection, waits for it to restore. Single
//! threaded behaviour lives in [`super::env_tests`].

use std::{
    ffi::OsStr,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{with_env_var, with_env_var_removed, with_env_vars_selected};

const OVERLAPPING_KEY: &str = "WHITAKER_TEST_SUPPORT_OVERLAPPING";
const SELECTED_KEY: &str = "WHITAKER_TEST_SUPPORT_SELECTED";
const SELECTED_PANIC_KEY: &str = "WHITAKER_TEST_SUPPORT_SELECTED_PANIC";
const SELECTED_PANIC_EXISTING_KEY: &str = "WHITAKER_TEST_SUPPORT_SELECTED_PANIC_EXISTING";
const EMPTY_SELECTION_KEY: &str = "WHITAKER_TEST_SUPPORT_EMPTY_SELECTION";

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

/// Check that an empty selection holds the lock for the whole callback.
///
/// This is the case [`with_env_vars`] short-circuits. The selecting helper must
/// not: its callback runs on the decision the selection made, so another
/// thread's scoped mutation has to wait until the callback returns even though
/// nothing was applied.
#[test]
fn empty_selection_holds_the_lock_for_its_callback() {
    let original_value = std::env::var_os(EMPTY_SELECTION_KEY);
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (finished_sender, finished_receiver) = mpsc::channel();
    let holder = thread::spawn(move || {
        with_env_vars_selected(Vec::new, || {
            entered_sender
                .send(())
                .expect("empty selection entry must be reported");
            release_receiver
                .recv()
                .expect("empty selection release must be received");
        });
        finished_sender
            .send(())
            .expect("empty selection completion must be reported");
    });
    let release_holder = ReleaseOnDrop(Some(release_sender));
    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("the empty selection callback should enter");

    let contender = spawn_scoped_mutation(EMPTY_SELECTION_KEY, "contender");
    let release_contender = ReleaseOnDrop(Some(contender.release_sender.clone()));
    assert!(
        contender
            .entered_receiver
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "a scoped mutation must wait for an empty selection's callback",
    );

    release_holder.release();
    finished_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("the empty selection should finish after release");
    contender.wait_for_entry("the contender should enter once the empty selection returns");
    release_contender.release();
    contender.wait_for_restoration("the contender should restore its value");
    holder
        .join()
        .expect("empty selection thread should complete");
    contender.join();
    assert_eq!(std::env::var_os(EMPTY_SELECTION_KEY), original_value);
}

/// Check that a panicking selection restores both value shapes and releases
/// the lock.
///
/// The selection takes `temp_env`'s lock before applying anything, so a panic
/// has to release it as well as restore every value: the originally absent one
/// and the pre-existing one alike. A lock left held would block every later
/// scoped mutation on any thread for ever.
#[test]
fn with_env_vars_selected_restores_and_releases_after_a_panic() {
    let absent_original = std::env::var_os(SELECTED_PANIC_KEY);
    let existing_original = std::env::var_os(SELECTED_PANIC_EXISTING_KEY);

    with_selection_panic_baseline(assert_selection_panic_restores_values);

    assert_eq!(std::env::var_os(SELECTED_PANIC_KEY), absent_original);
    assert_eq!(
        std::env::var_os(SELECTED_PANIC_EXISTING_KEY),
        existing_original
    );

    let after_panic = spawn_scoped_mutation(SELECTED_PANIC_KEY, "after panic");
    let release = ReleaseOnDrop(Some(after_panic.release_sender.clone()));
    after_panic.wait_for_entry("the lock must be available after a panicking selection");
    release.release();
    after_panic.wait_for_restoration("the later mutation must restore its value");
    after_panic.join();
    assert_eq!(std::env::var_os(SELECTED_PANIC_KEY), absent_original);
}

/// Run `callback` with one key absent and another holding a known value.
fn with_selection_panic_baseline(callback: impl FnOnce()) {
    with_env_var_removed(SELECTED_PANIC_KEY, || {
        with_env_var(SELECTED_PANIC_EXISTING_KEY, "original value", callback);
    });
}

/// Panic inside a two-key selection and assert both value shapes restore.
fn assert_selection_panic_restores_values() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_env_vars_selected(selected_panic_settings, || panic!("intentional test panic"));
    }));

    assert!(result.is_err());
    assert!(std::env::var_os(SELECTED_PANIC_KEY).is_none());
    assert_eq!(
        std::env::var_os(SELECTED_PANIC_EXISTING_KEY).as_deref(),
        Some(OsStr::new("original value"))
    );
}

/// The two keys the panicking selection scopes, one absent, one pre-existing.
fn selected_panic_settings() -> Vec<(&'static str, &'static str)> {
    vec![
        (SELECTED_PANIC_KEY, "temporary"),
        (SELECTED_PANIC_EXISTING_KEY, "temporary"),
    ]
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
