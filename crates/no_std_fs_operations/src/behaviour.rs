//! Behaviour-driven localization tests for the `no_std_fs_operations` lint.

use crate::diagnostics::{StdFsMessages, localized_messages};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use whitaker_common::i18n::testing::FailingLookup;
use whitaker_common::i18n::{I18nError, Localizer};

#[derive(Default)]
struct LocalizationWorld {
    localizer: Option<Localizer>,
    operation: String,
    failing: bool,
    result: Option<Result<StdFsMessages, I18nError>>,
}

impl LocalizationWorld {
    fn select_locale(&mut self, locale: &str) {
        self.localizer = Some(Localizer::new(Some(locale)));
    }

    fn set_operation(&mut self, operation: &str) {
        operation.clone_into(&mut self.operation);
    }

    const fn mark_failure(&mut self) {
        self.failing = true;
    }

    fn resolve(&mut self) {
        let op = self.operation.clone();
        let result = if self.failing {
            localized_messages(&FailingLookup::new("no_std_fs_operations"), &op)
        } else {
            let Some(localizer) = self.localizer.as_ref() else {
                panic!("a locale must be selected before resolving messages")
            };
            localized_messages(localizer, &op)
        };
        self.result = Some(result);
    }

    fn messages(&self) -> &StdFsMessages {
        let Some(Ok(messages)) = self.result.as_ref().map(Result::as_ref) else {
            panic!("localization should have been resolved successfully")
        };
        messages
    }

    fn error(&self) -> &I18nError {
        let Some(Err(error)) = self.result.as_ref().map(Result::as_ref) else {
            panic!("localization should have been resolved to a failure")
        };
        error
    }
}

type WorldCell = RefCell<LocalizationWorld>;

#[fixture]
fn world() -> WorldCell {
    RefCell::new(LocalizationWorld {
        operation: String::from("std::fs::read"),
        ..LocalizationWorld::default()
    })
}

#[given("the locale {locale} is selected")]
fn given_locale(world: &WorldCell, locale: String) {
    world.borrow_mut().select_locale(locale.trim_matches('"'));
}

#[given("the operation is {operation}")]
fn given_operation(world: &WorldCell, operation: String) {
    world
        .borrow_mut()
        .set_operation(operation.trim_matches('"'));
}

#[given("localization fails")]
fn given_failure(world: &WorldCell) {
    world.borrow_mut().mark_failure();
}

#[when("I localize the std::fs diagnostic")]
fn when_localize(world: &WorldCell) {
    world.borrow_mut().resolve();
}

#[then("the primary mentions {snippet}")]
fn then_primary(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(
        borrow.messages().primary().contains(needle),
        "primary message should mention `{needle}`"
    );
}

#[then("the note references {snippet}")]
fn then_note(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(
        borrow.messages().note().contains(needle),
        "note message should mention `{needle}`"
    );
}

#[then("the help references {snippet}")]
fn then_help(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(
        borrow.messages().help().contains(needle),
        "help message should mention `{needle}`"
    );
}

#[then("localization fails for {key}")]
fn then_failure(world: &WorldCell, key: String) {
    let borrow = world.borrow();
    match borrow.error() {
        I18nError::MissingMessage { key: missing, .. } => {
            assert_eq!(
                missing,
                &key.trim_matches('"'),
                "localization should fail for the requested key"
            );
        }
    }
}

#[scenario(path = "tests/features/localization.feature", index = 0)]
fn scenario_english(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localization.feature", index = 1)]
fn scenario_welsh(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localization.feature", index = 2)]
fn scenario_gaelic(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localization.feature", index = 3)]
fn scenario_fallback(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localization.feature", index = 4)]
fn scenario_failure(world: WorldCell) {
    let _ = world;
}
