//! BDD-style localisation tests for function attribute diagnostic messages.
//!
//! Exercises locale selection, attribute fallback, and missing-message paths via
//! `rstest-bdd` scenarios and a custom failing lookup to validate fallbacks.

use super::{
    Arguments, BundleLookup, FunctionAttrsMessages, FunctionKind, Localiser, MESSAGE_KEY,
    attribute_fallback, localised_messages,
};
use common::i18n::{AttrKey, I18nError, MessageKey};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct LocalisationWorld {
    localiser: RefCell<Option<Localiser>>,
    subject: RefCell<FunctionKind>,
    attribute: RefCell<String>,
    use_attribute_fallback: RefCell<bool>,
    failing: RefCell<bool>,
    result: RefCell<Option<Result<FunctionAttrsMessages, I18nError>>>,
}

impl LocalisationWorld {
    fn use_localiser(&self, locale: &str) {
        *self.localiser.borrow_mut() = Some(Localiser::new(Some(locale)));
    }

    fn trigger_attribute_fallback(&self) {
        *self.use_attribute_fallback.borrow_mut() = true;
    }

    fn record_result(&self, value: Result<FunctionAttrsMessages, I18nError>) {
        *self.result.borrow_mut() = Some(value);
    }

    fn messages(&self) -> &FunctionAttrsMessages {
        self.result
            .borrow()
            .as_ref()
            .expect("result recorded")
            .as_ref()
            .expect("expected localisation to succeed")
    }

    fn error(&self) -> &I18nError {
        self.result
            .borrow()
            .as_ref()
            .expect("result recorded")
            .as_ref()
            .expect_err("expected localisation to fail")
    }
}

#[fixture]
fn world() -> LocalisationWorld {
    LocalisationWorld::default()
}

#[given("the locale {locale} is selected")]
fn given_locale(world: &LocalisationWorld, locale: String) {
    world.use_localiser(&locale);
}

#[given("the subject kind is {kind}")]
fn given_subject(world: &LocalisationWorld, kind: String) {
    *world.subject.borrow_mut() = match kind.as_str() {
        "function" => FunctionKind::Function,
        "method" => FunctionKind::Method,
        "trait method" => FunctionKind::TraitMethod,
        other => panic!("unknown subject kind: {other}"),
    };
}

#[given("the attribute label is {label}")]
fn given_attribute(world: &LocalisationWorld, label: String) {
    *world.attribute.borrow_mut() = label;
}

#[given("the attribute snippet cannot be retrieved")]
fn given_attribute_fallback(world: &LocalisationWorld) {
    world.trigger_attribute_fallback();
}

#[given("localisation fails")]
fn given_failure(world: &LocalisationWorld) {
    *world.failing.borrow_mut() = true;
}

#[when("I localise the diagnostic")]
fn when_localise(world: &LocalisationWorld) {
    let kind = *world.subject.borrow();
    let attribute = if *world.use_attribute_fallback.borrow() {
        if *world.failing.borrow() {
            attribute_fallback(&FailingLookup)
        } else {
            let localiser = world
                .localiser
                .borrow()
                .as_ref()
                .expect("a locale must be selected");
            attribute_fallback(localiser)
        }
    } else {
        world.attribute.borrow().clone()
    };

    let result = if *world.failing.borrow() {
        localised_messages(&FailingLookup, kind, attribute.as_str())
    } else {
        let localiser = world
            .localiser
            .borrow()
            .as_ref()
            .expect("a locale must be selected");
        localised_messages(localiser, kind, attribute.as_str())
    };

    world.record_result(result);
}

#[then("the primary message contains {snippet}")]
fn then_primary(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().primary().contains(&snippet));
}

#[then("the note mentions {snippet}")]
fn then_note(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().note().contains(&snippet));
}

#[then("the help mentions {snippet}")]
fn then_help(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().help().contains(&snippet));
}

#[then("localisation fails for {key}")]
fn then_failure(world: &LocalisationWorld, key: String) {
    let error = world.error();
    match error {
        I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, &key),
    }
}

#[scenario(path = "tests/features/function_attrs_localisation.feature", index = 0)]
fn scenario_fallback(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localisation.feature", index = 1)]
fn scenario_welsh(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localisation.feature", index = 2)]
fn scenario_attribute_fallback(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localisation.feature", index = 3)]
fn scenario_unknown_locale(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localisation.feature", index = 4)]
fn scenario_failure(world: LocalisationWorld) {
    let _ = world;
}

struct FailingLookup;

impl BundleLookup for FailingLookup {
    fn message(&self, _key: MessageKey<'_>, _args: &Arguments<'_>) -> Result<String, I18nError> {
        Err(I18nError::MissingMessage {
            key: MESSAGE_KEY.to_string(),
            locale: "test".to_string(),
        })
    }

    fn attribute(
        &self,
        _key: MessageKey<'_>,
        _attribute: AttrKey<'_>,
        _args: &Arguments<'_>,
    ) -> Result<String, I18nError> {
        Err(I18nError::MissingMessage {
            key: MESSAGE_KEY.to_string(),
            locale: "test".to_string(),
        })
    }
}
