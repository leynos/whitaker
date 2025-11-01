//! UI regression tests for the `function_attrs_follow_docs` lint, including
//! locale-specific smoke coverage.
//!
//! The tests serialise execution so temporary `DYLINT_LOCALE` overrides remain
//! race-free while the canonical and Welsh harnesses run against the same
//! fixtures.

use serial_test::serial;
use std::ffi::OsString;

#[test]
#[serial]
fn ui() {
    run_ui_with_locale("ui", None);
}

#[test]
#[serial]
fn ui_runs_in_welsh_locale() {
    run_ui_with_locale("ui-cy", Some("cy"));
}

fn run_ui_with_locale(directory: &str, locale: Option<&str>) {
    let _guard = locale.map(LocaleOverride::set);
    whitaker::run_ui_tests!(directory).expect("UI tests should execute without diffs");
}

struct LocaleOverride {
    previous: Option<OsString>,
}

impl LocaleOverride {
    fn set(locale: &str) -> Self {
        let previous = std::env::var_os("DYLINT_LOCALE");
        // SAFETY: Both UI tests are marked with `serial_test::serial`, so the
        // harness executes them one at a time. The guard returned from this
        // function also keeps the mutation scoped to the current thread.
        unsafe {
            std::env::set_var("DYLINT_LOCALE", locale);
        }
        Self { previous }
    }
}

impl Drop for LocaleOverride {
    fn drop(&mut self) {
        if let Some(value) = &self.previous {
            // SAFETY: Locale mutations remain serialised by the
            // `serial_test::serial` attribute and the guard instance, so
            // restoring the previous value cannot race with another test.
            unsafe {
                std::env::set_var("DYLINT_LOCALE", value);
            }
        } else {
            // SAFETY: The serialised execution guarantees no concurrent access,
            // making removal race-free for the same reason as the setter above.
            unsafe {
                std::env::remove_var("DYLINT_LOCALE");
            }
        }
    }
}
