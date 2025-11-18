//! Behaviour-driven localisation tests for the `no_std_fs_operations` lint.

use crate::diagnostics::{StdFsMessages, localised_messages};
use common::i18n::testing::FailingLookup;
use common::i18n::{I18nError, Localizer};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Ref, RefCell};

#[derive(Default)]
struct LocalisationWorld {
    localizer: RefCell<Option<Localizer>>,
    operation: RefCell<String>,
    failing: RefCell<bool>,
    result: RefCell<Option<Result<StdFsMessages, I18nError>>>,
}

impl LocalisationWorld {
    fn select_locale(&self, locale: &str) {
        *self.localizer.borrow_mut() = Some(Localizer::new(Some(locale)));
    }

    fn set_operation(&self, operation: &str) {
        *self.operation.borrow_mut() = operation.to_owned();
    }

    fn mark_failure(&self) {
        *self.failing.borrow_mut() = true;
    }

    fn with_localizer<T>(&self, f: impl FnOnce(&Localizer) -> T) -> T {
        let borrow = self.localizer.borrow();
        let localizer = borrow.as_ref().expect("a locale must be selected");
        f(localizer)
    }

    fn resolve(&self) {
        let op = self.operation.borrow().clone();
        let result = if *self.failing.borrow() {
            localised_messages(&FailingLookup::new("no_std_fs_operations"), &op)
        } else {
            self.with_localizer(|localizer| localised_messages(localizer, &op))
        };
        self.result.borrow_mut().replace(result);
    }

    fn messages(&self) -> Ref<'_, StdFsMessages> {
        Ref::map(self.result.borrow(), |maybe| {
            maybe
                .as_ref()
                .expect("localisation result should be recorded")
                .as_ref()
                .expect("localisation should succeed")
        })
    }

    fn error(&self) -> Ref<'_, I18nError> {
        Ref::map(self.result.borrow(), |maybe| {
            maybe
                .as_ref()
                .expect("localisation result should be recorded")
                .as_ref()
                .expect_err("localisation should fail")
        })
    }
}

#[fixture]
fn world() -> LocalisationWorld {
    let world = LocalisationWorld::default();
    world.set_operation("std::fs::read");
    world
}

#[given("the locale {locale} is selected")]
fn given_locale(world: &LocalisationWorld, locale: String) {
    world.select_locale(locale.trim_matches('"'));
}

#[given("the operation is {operation}")]
fn given_operation(world: &LocalisationWorld, operation: String) {
    world.set_operation(operation.trim_matches('"'));
}

#[given("localisation fails")]
fn given_failure(world: &LocalisationWorld) {
    world.mark_failure();
}

#[when("I localise the std::fs diagnostic")]
fn when_localise(world: &LocalisationWorld) {
    world.resolve();
}

#[then("the primary mentions {snippet}")]
fn then_primary(world: &LocalisationWorld, snippet: String) {
    let needle = snippet.trim_matches('"');
    assert!(world.messages().primary().contains(needle));
}

#[then("the note references {snippet}")]
fn then_note(world: &LocalisationWorld, snippet: String) {
    let needle = snippet.trim_matches('"');
    assert!(world.messages().note().contains(needle));
}

#[then("the help references {snippet}")]
fn then_help(world: &LocalisationWorld, snippet: String) {
    let needle = snippet.trim_matches('"');
    assert!(world.messages().help().contains(needle));
}

#[then("localisation fails for {key}")]
fn then_failure(world: &LocalisationWorld, key: String) {
    match &*world.error() {
        I18nError::MissingMessage { key: missing, .. } => {
            assert_eq!(missing, &key.trim_matches('"'))
        }
    }
}

#[scenario(path = "tests/features/localisation.feature", index = 0)]
fn scenario_english(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 1)]
fn scenario_welsh(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 2)]
fn scenario_gaelic(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 3)]
fn scenario_fallback(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 4)]
fn scenario_failure(world: LocalisationWorld) {
    let _ = world;
}
