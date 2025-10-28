//! BDD-style localisation tests for `no_expect_outside_tests` diagnostics.
//!
//! Validates locale handling, receiver classification, context labelling, and
//! fallback behaviour using `rstest-bdd` scenarios backed by helper fixtures.

use super::{
    Arguments, AttrKey, BundleLookup, ContextLabel, I18nError, Localiser, MESSAGE_KEY,
    NoExpectMessages, ReceiverCategory, ReceiverLabel, context_label, fallback_messages,
    localised_messages,
};
use crate::context::ContextSummary;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

#[derive(Default)]
struct LocalisationWorld {
    localiser: RefCell<Option<Localiser>>,
    receiver: RefCell<ReceiverLabel>,
    summary: RefCell<ContextSummary>,
    failing: RefCell<bool>,
    result: RefCell<Option<Result<NoExpectMessages, I18nError>>>,
}

impl LocalisationWorld {
    fn use_localiser(&self, locale: &str) {
        *self.localiser.borrow_mut() = Some(Localiser::new(Some(locale)));
    }

    fn set_receiver_type(&self, receiver: &str) {
        *self.receiver.borrow_mut() = ReceiverLabel::new(receiver);
    }

    fn set_receiver(&self, receiver: &str) {
        self.set_receiver_type(receiver);
    }

    fn set_function(&self, name: Option<&str>) {
        let mut summary = self.summary.borrow_mut();
        summary.function_name = name.map(ToString::to_string);
    }

    fn get_receiver_type(&self) -> ReceiverLabel {
        self.receiver.borrow().clone()
    }

    fn get_function_context(&self) -> ContextLabel {
        let summary = self.summary.borrow();
        context_label(&summary)
    }

    fn get_bundle_lookup(&self) -> Localiser {
        self.localiser
            .borrow()
            .as_ref()
            .expect("a locale must be selected")
            .clone()
    }

    fn record_result(&self, value: Result<NoExpectMessages, I18nError>) {
        *self.result.borrow_mut() = Some(value);
    }

    fn messages(&self) -> &NoExpectMessages {
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

#[given("the receiver type is {receiver}")]
fn given_receiver(world: &LocalisationWorld, receiver: String) {
    world.set_receiver(&receiver);
}

#[given("the function context is {name}")]
fn given_function(world: &LocalisationWorld, name: String) {
    let value = if name.is_empty() {
        None
    } else {
        Some(name.as_str())
    };
    world.set_function(value);
}

#[given("the receiver type is empty")]
fn given_receiver_type_empty(world: &LocalisationWorld) {
    world.set_receiver_type("");
}

#[given("the receiver type is malformed")]
fn given_receiver_type_malformed(world: &LocalisationWorld) {
    world.set_receiver_type("!!!not_a_type");
}

#[given("the receiver type is unexpected")]
fn given_receiver_type_unexpected(world: &LocalisationWorld) {
    world.set_receiver_type("SomeCompletelyUnexpectedType123");
}

#[given("the call occurs outside any function")]
fn given_no_function(world: &LocalisationWorld) {
    world.set_function(None);
}

#[given("localisation fails")]
fn given_failure(world: &LocalisationWorld) {
    *world.failing.borrow_mut() = true;
}

#[when("I localise the expect diagnostic")]
fn when_localise(world: &LocalisationWorld) {
    let receiver = world.receiver.borrow().clone();
    let summary = world.summary.borrow().clone();
    let context = context_label(&summary);
    let category = ReceiverCategory::for_label(&receiver);

    let result = if *world.failing.borrow() {
        localised_messages(&FailingLookup, &receiver, &context, category)
    } else {
        let localiser = world
            .localiser
            .borrow()
            .as_ref()
            .expect("a locale must be selected");
        localised_messages(localiser, &receiver, &context, category)
    };

    world.record_result(result);
}

#[then("the diagnostic mentions {snippet}")]
fn then_primary(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().primary().contains(&snippet));
}

#[then("the note references {snippet}")]
fn then_note(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().note().contains(&snippet));
}

#[then("the help references {snippet}")]
fn then_help(world: &LocalisationWorld, snippet: String) {
    assert!(world.messages().help().contains(&snippet));
}

#[then("the fallback and localisation logic should handle the receiver type robustly")]
fn then_receiver_type_edge_cases_are_handled(world: &LocalisationWorld) {
    let lookup = world.get_bundle_lookup();
    let context = world.get_function_context();
    let receiver = world.get_receiver_type();
    let category = ReceiverCategory::for_label(&receiver);

    let result = localised_messages(&lookup, &receiver, &context, category);
    assert!(
        result.is_ok(),
        "localisation should succeed for edge case receiver types"
    );
    let messages = result.expect("localisation should succeed");
    assert!(
        !messages.primary().is_empty(),
        "localised message title should never be empty"
    );
}

#[then("localisation fails for {key}")]
fn then_failure(world: &LocalisationWorld, key: String) {
    let error = world.error();
    match error {
        I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, &key),
    }
}

#[scenario(path = "tests/features/localisation.feature", index = 0)]
fn scenario_fallback(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 1)]
fn scenario_cymraeg(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 2)]
fn scenario_unknown_locale(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 3)]
fn scenario_receiver_empty(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 4)]
fn scenario_receiver_malformed(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 5)]
fn scenario_receiver_unexpected(world: LocalisationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/localisation.feature", index = 6)]
fn scenario_failure(world: LocalisationWorld) {
    let _ = world;
}

#[then("the fallback help mentions {snippet}")]
fn then_fallback(world: &LocalisationWorld, snippet: String) {
    let summary = world.summary.borrow().clone();
    let context = context_label(&summary);
    let receiver = world.receiver.borrow();
    let category = ReceiverCategory::for_label(receiver.as_ref());
    let fallback = fallback_messages(receiver.as_ref(), &context, category);
    assert!(fallback.help().contains(&snippet));
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
