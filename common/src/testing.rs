#![cfg(any(test, feature = "dylint"))]

use std::fmt;

use camino::Utf8Path;
use dylint_testing::ui;

pub struct UiTestHarness {
    test: ui::Test,
}

impl fmt::Debug for UiTestHarness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiTestHarness").finish_non_exhaustive()
    }
}

impl UiTestHarness {
    #[must_use]
    pub fn new(name: &str, src_base: &Utf8Path) -> Self {
        Self {
            test: ui::Test::src_base(name, src_base.as_std_path()),
        }
    }

    #[must_use]
    pub fn with_rustc_flags(mut self, flags: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.test.rustc_flags(flags);
        self
    }

    #[must_use]
    pub fn with_dylint_toml(mut self, toml: impl AsRef<str>) -> Self {
        self.test.dylint_toml(toml);
        self
    }

    pub fn run(mut self) {
        self.test.run();
    }

    #[cfg(test)]
    pub fn into_inner(self) -> ui::Test {
        self.test
    }
}

#[must_use]
pub fn harness(name: &str, src_base: &Utf8Path) -> UiTestHarness {
    UiTestHarness::new(name, src_base)
}

#[must_use]
pub fn default_ui_harness(name: &str) -> UiTestHarness {
    UiTestHarness::new(name, Utf8Path::new("tests/ui"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use rstest_bdd::{StepError, assert_step_ok};
    use rstest_bdd_macros::{given, then, when};
    use std::cell::RefCell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;

    #[derive(Clone)]
    struct HarnessHandle(Rc<RefCell<Option<UiTestHarness>>>);

    impl HarnessHandle {
        fn new(harness: UiTestHarness) -> Self {
            Self(Rc::new(RefCell::new(Some(harness))))
        }

        fn update(&self, f: impl FnOnce(UiTestHarness) -> UiTestHarness) {
            let mut slot = self.0.borrow_mut();
            let harness = slot.take().expect("harness should be available");
            *slot = Some(f(harness));
        }

        fn into_inner(self) -> UiTestHarness {
            Rc::try_unwrap(self.0)
                .expect("no other references to harness")
                .into_inner()
                .expect("harness should still be present")
        }
    }

    #[rstest]
    fn chains_builder_calls() {
        let harness = UiTestHarness::new("demo", Utf8Path::new("tests/ui"))
            .with_rustc_flags(["--test"])
            .with_dylint_toml("[lints]\nallow = []");
        let _ = harness.into_inner();
    }

    #[rstest]
    fn accepts_mixed_flag_iter() {
        let harness = UiTestHarness::new("demo", Utf8Path::new("tests/ui"))
            .with_rustc_flags(vec!["--cfg", "test"])
            .with_dylint_toml("[lints]\nallow = []");
        let _ = harness.into_inner();
    }

    #[given("a harness initialised for {suite}")]
    fn given_harness(suite: String) -> Result<HarnessHandle, StepError> {
        Ok(HarnessHandle::new(default_ui_harness(&suite)))
    }

    #[when("the harness is configured without flags")]
    fn when_configured(handle: HarnessHandle) -> Result<HarnessHandle, StepError> {
        handle.update(|harness| harness.with_dylint_toml("[lints]\nallow = []"));
        Ok(handle)
    }

    #[then("the harness should still construct successfully")]
    fn then_builds(handle: HarnessHandle) -> Result<(), StepError> {
        let harness = handle.into_inner();
        let _ = harness.into_inner();
        Ok(())
    }

    #[rstest]
    fn run_panics_for_missing_path() {
        let harness = UiTestHarness::new("missing", Utf8Path::new("tests/ui/__missing_suite"));
        let outcome = catch_unwind(AssertUnwindSafe(|| harness.run()));
        assert!(
            outcome.is_err(),
            "expected harness.run() to panic when fixtures are missing"
        );
    }

    #[given("a harness initialised for missing suite {suite}")]
    fn given_missing_harness(suite: String) -> Result<HarnessHandle, StepError> {
        Ok(HarnessHandle::new(UiTestHarness::new(
            &suite,
            Utf8Path::new("tests/ui/__missing_suite"),
        )))
    }

    #[when("the harness is executed against missing fixtures")]
    fn when_run_panics(handle: HarnessHandle) -> Result<bool, StepError> {
        let harness = handle.into_inner();
        let outcome = catch_unwind(AssertUnwindSafe(|| harness.run()));
        Ok(outcome.is_err())
    }

    #[then("the harness run should be reported as failure")]
    fn then_run_fails(did_fail: bool) -> Result<(), StepError> {
        if did_fail {
            Ok(())
        } else {
            Err(StepError::ExecutionError {
                pattern: "the harness run should be reported as failure".into(),
                function: "then_run_fails".into(),
                message: "expected harness.run() to panic for missing fixtures".into(),
            })
        }
    }

    #[rstest]
    fn bdd_harness_run_fails() {
        let handle = assert_step_ok!(given_missing_harness("suite".to_owned()));
        let did_fail = assert_step_ok!(when_run_panics(handle));
        assert_step_ok!(then_run_fails(did_fail));
    }

    #[rstest]
    fn bdd_harness_builds() {
        let handle = assert_step_ok!(given_harness("suite".to_owned()));
        let configured = assert_step_ok!(when_configured(handle));
        assert_step_ok!(then_builds(configured));
    }
}
