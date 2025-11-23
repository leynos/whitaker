//! Behaviour-driven localization tests for the `no_std_fs_operations` lint.

use crate::diagnostics::{StdFsMessages, localised_messages};
use common::i18n::testing::FailingLookup;
use common::i18n::{I18nError, Localizer};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct LocalisationWorld {
    localizer: Option<Localizer>,
    operation: String,
    failing: bool,
    result: Option<Result<StdFsMessages, I18nError>>,
}

impl LocalisationWorld {
    fn select_locale(&mut self, locale: &str) {
        self.localizer = Some(Localizer::new(Some(locale)));
    }

    fn set_operation(&mut self, operation: &str) {
        self.operation = operation.to_owned();
    }

    fn mark_failure(&mut self) {
        self.failing = true;
    }

    fn resolve(&mut self) {
        let op = self.operation.clone();
        let result = if self.failing {
            localised_messages(&FailingLookup::new("no_std_fs_operations"), &op)
        } else {
            let localizer = self.localizer.as_ref().expect("a locale must be selected");
            localised_messages(localizer, &op)
        };
        self.result = Some(result);
    }

    fn messages(&self) -> &StdFsMessages {
        self.result
            .as_ref()
            .expect("localisation result should be recorded")
            .as_ref()
            .expect("localisation should succeed")
    }

    fn error(&self) -> &I18nError {
        self.result
            .as_ref()
            .expect("localisation result should be recorded")
            .as_ref()
            .expect_err("localisation should fail")
    }
}

type WorldCell = RefCell<LocalisationWorld>;

#[fixture]
fn world() -> WorldCell {
    RefCell::new(LocalisationWorld {
        operation: String::from("std::fs::read"),
        ..LocalisationWorld::default()
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

#[given("localisation fails")]
fn given_failure(world: &WorldCell) {
    world.borrow_mut().mark_failure();
}

#[when("I localise the std::fs diagnostic")]
fn when_localise(world: &WorldCell) {
    world.borrow_mut().resolve();
}

#[then("the primary mentions {snippet}")]
fn then_primary(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(borrow.messages().primary().contains(needle));
}

#[then("the note references {snippet}")]
fn then_note(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(borrow.messages().note().contains(needle));
}

#[then("the help references {snippet}")]
fn then_help(world: &WorldCell, snippet: String) {
    let needle = snippet.trim_matches('"');
    let borrow = world.borrow();
    assert!(borrow.messages().help().contains(needle));
}

#[then("localisation fails for {key}")]
fn then_failure(world: &WorldCell, key: String) {
    let borrow = world.borrow();
    match borrow.error() {
        I18nError::MissingMessage { key: missing, .. } => {
            assert_eq!(missing, &key.trim_matches('"'))
        }
    }
}

#[scenario(path = "tests/features/localisation.feature", index = 0)]
fn scenario_english(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 1)]
fn scenario_welsh(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 2)]
fn scenario_gaelic(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 3)]
fn scenario_fallback(world: WorldCell) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 4)]
fn scenario_failure(world: WorldCell) {
    let _ = world;
}
