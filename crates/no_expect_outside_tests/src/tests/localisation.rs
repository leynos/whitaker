//! BDD-style localisation tests for no_expect_outside_tests diagnostic
//! messages.
//!
//! Exercises localisation scenarios including locale selection, receiver type
//! handling, context label generation, and error paths using `rstest-bdd` and a
//! `FailingLookup` test double.

use super::{
    I18nError, Localizer, MESSAGE_KEY, NoExpectMessages, ReceiverCategory, ReceiverLabel,
    context_label, fallback_messages, localised_messages,
};
use crate::context::ContextSummary;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, Ref, RefCell};
use whitaker_common::i18n::BundleLookup;
use whitaker_common::i18n::testing::FailingLookup;

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|stripped| stripped.strip_suffix('"'))
        .unwrap_or(value)
}

fn format_receiver(receiver: &str) -> String {
    if receiver.is_empty() || receiver.starts_with('`') {
        receiver.to_string()
    } else {
        format!("`{receiver}`")
    }
}

// Strip Fluent's bidi isolation marks (U+2068 FSI and U+2069 PDI) before assertions.
fn normalize_for_assertion(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[derive(Default)]
struct LocalisationWorld {
    localizer: RefCell<Option<Localizer>>,
    receiver: RefCell<ReceiverLabel>,
    summary: RefCell<ContextSummary>,
    failing: Cell<bool>,
    result: RefCell<Option<Result<NoExpectMessages, I18nError>>>,
}

impl LocalisationWorld {
    fn use_localizer(&self, locale: &str) {
        *self.localizer.borrow_mut() = Some(Localizer::new(Some(locale)));
    }

    fn with_localizer<T>(&self, f: impl FnOnce(&Localizer) -> T) -> T {
        let borrow = self.localizer.borrow();
        let localizer = borrow.as_ref().expect("a locale must be selected");
        f(localizer)
    }

    fn set_receiver_type(&self, receiver: &str) {
        *self.receiver.borrow_mut() = ReceiverLabel::new(format_receiver(receiver));
    }

    fn set_function(&self, name: Option<&str>) {
        let mut summary = self.summary.borrow_mut();
        summary.function_name = name.map(ToString::to_string);
    }

    fn get_receiver_type(&self) -> ReceiverLabel {
        self.receiver.borrow().clone()
    }

    fn record_result(&self, value: Result<NoExpectMessages, I18nError>) {
        *self.result.borrow_mut() = Some(value);
    }

    fn messages(&self) -> Ref<'_, NoExpectMessages> {
        Ref::map(
            Ref::map(self.result.borrow(), |opt| {
                opt.as_ref().expect("result recorded")
            }),
            |res| res.as_ref().expect("expected localisation to succeed"),
        )
    }

    fn error(&self) -> Ref<'_, I18nError> {
        Ref::map(
            Ref::map(self.result.borrow(), |opt| {
                opt.as_ref().expect("result recorded")
            }),
            |res| res.as_ref().expect_err("expected localisation to fail"),
        )
    }
}

#[fixture]
fn world() -> LocalisationWorld {
    LocalisationWorld::default()
}

#[given("the locale {locale} is selected")]
fn given_locale(world: &LocalisationWorld, locale: String) {
    world.use_localizer(unquote(&locale));
}

#[given("the receiver type is {receiver}")]
fn given_receiver(world: &LocalisationWorld, receiver: String) {
    world.set_receiver_type(unquote(&receiver));
}

#[given("the function context is {name}")]
fn given_function(world: &LocalisationWorld, name: String) {
    let name = unquote(&name);
    let value = if name.is_empty() { None } else { Some(name) };
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
    world.failing.set(true);
}

#[when("I localise the expect diagnostic")]
fn when_localise(world: &LocalisationWorld) {
    let receiver = world.receiver.borrow().clone();
    let summary = world.summary.borrow().clone();

    let result = if world.failing.get() {
        let lookup = failing_lookup();
        execute_localisation(&lookup, &receiver, &summary)
    } else {
        world.with_localizer(|localizer| execute_localisation(localizer, &receiver, &summary))
    };

    world.record_result(result);
}

#[then("the diagnostic mentions {snippet}")]
fn then_primary(world: &LocalisationWorld, snippet: String) {
    let snippet = normalize_for_assertion(unquote(&snippet));
    let primary = normalize_for_assertion(world.messages().primary());
    assert!(
        primary.contains(&snippet),
        "primary message `{primary}` did not contain `{snippet}`"
    );
}

#[then("the note references {snippet}")]
fn then_note(world: &LocalisationWorld, snippet: String) {
    let snippet = normalize_for_assertion(unquote(&snippet));
    let note = normalize_for_assertion(world.messages().note());
    assert!(
        note.contains(&snippet),
        "note `{note}` did not contain `{snippet}`"
    );
}

#[then("the help references {snippet}")]
fn then_help(world: &LocalisationWorld, snippet: String) {
    let snippet = normalize_for_assertion(unquote(&snippet));
    let help = normalize_for_assertion(world.messages().help());
    assert!(
        help.contains(&snippet),
        "help `{help}` did not contain `{snippet}`"
    );
}

#[then("the fallback and localisation logic should handle the receiver type robustly")]
fn then_receiver_type_edge_cases_are_handled(world: &LocalisationWorld) {
    let receiver_label = world.get_receiver_type();
    let summary = world.summary.borrow().clone();

    let messages = world
        .with_localizer(|localizer| execute_localisation(localizer, &receiver_label, &summary))
        .expect("localisation should succeed");
    assert!(
        !messages.primary().is_empty(),
        "localised message title should never be empty"
    );
}

#[then("localisation fails for {key}")]
fn then_failure(world: &LocalisationWorld, key: String) {
    let key = unquote(&key);
    let error = world.error();
    match &*error {
        I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, key),
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
    let snippet = normalize_for_assertion(unquote(&snippet));
    let summary = world.summary.borrow().clone();
    let context = context_label(&summary);
    let receiver = world.receiver.borrow().clone();
    let category = ReceiverCategory::for_label(&receiver);
    let fallback = fallback_messages(&receiver, &context, category);
    let help = normalize_for_assertion(fallback.help());
    assert!(
        help.contains(&snippet),
        "fallback help `{help}` did not contain `{snippet}`"
    );
}

fn execute_localisation(
    lookup: &impl BundleLookup,
    receiver: &ReceiverLabel,
    summary: &ContextSummary,
) -> Result<NoExpectMessages, I18nError> {
    let context = context_label(summary);
    let category = ReceiverCategory::for_label(receiver);
    localised_messages(lookup, receiver, &context, category)
}

fn failing_lookup() -> FailingLookup {
    FailingLookup::new(MESSAGE_KEY.as_ref())
}
