//! BDD-style localization tests for function attribute diagnostic messages.
//!
//! Exercises locale selection, attribute fallback, and missing-message paths via
//! `rstest-bdd` scenarios and a custom failing lookup to validate fallbacks.

use super::{
    FunctionAttrsMessages, FunctionKind, Localizer, MESSAGE_KEY, attribute_fallback,
    localised_messages,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::{Cell, Ref, RefCell};
use whitaker_common::i18n::I18nError;
use whitaker_common::i18n::testing::FailingLookup;

#[derive(Default)]
struct LocalizationWorld {
    localizer: RefCell<Option<Localizer>>,
    subject: RefCell<FunctionKind>,
    attribute: RefCell<String>,
    use_attribute_fallback: Cell<bool>,
    failing: Cell<bool>,
    result: RefCell<Option<Result<FunctionAttrsMessages, I18nError>>>,
}

impl LocalizationWorld {
    fn use_localizer(&self, locale: &str) {
        *self.localizer.borrow_mut() = Some(Localizer::new(Some(locale)));
    }

    fn with_localizer<T>(&self, f: impl FnOnce(&Localizer) -> T) -> T {
        let borrow = self.localizer.borrow();
        let localizer = borrow.as_ref().expect("a locale must be selected");
        f(localizer)
    }

    fn messages(&self) -> Ref<'_, FunctionAttrsMessages> {
        Ref::map(
            Ref::map(self.result.borrow(), |opt| {
                opt.as_ref().expect("result recorded")
            }),
            |res| res.as_ref().expect("expected localization to succeed"),
        )
    }

    fn error(&self) -> Ref<'_, I18nError> {
        Ref::map(
            Ref::map(self.result.borrow(), |opt| {
                opt.as_ref().expect("result recorded")
            }),
            |res| res.as_ref().expect_err("expected localization to fail"),
        )
    }
}

#[fixture]
fn world() -> LocalizationWorld {
    LocalizationWorld::default()
}

#[given("the locale {locale} is selected")]
fn given_locale(world: &LocalizationWorld, locale: String) {
    world.use_localizer(&locale);
}

#[given("the subject kind is {kind}")]
fn given_subject(world: &LocalizationWorld, kind: String) {
    *world.subject.borrow_mut() = match kind.as_str() {
        "function" => FunctionKind::Function,
        "method" => FunctionKind::Method,
        "trait method" => FunctionKind::TraitMethod,
        other => panic!("unknown subject kind: {other}"),
    };
}

#[given("the attribute label is {label}")]
fn given_attribute(world: &LocalizationWorld, label: String) {
    *world.attribute.borrow_mut() = label;
}

#[given("the attribute snippet cannot be retrieved")]
fn given_attribute_fallback(world: &LocalizationWorld) {
    world.use_attribute_fallback.set(true);
}

#[given("localization fails")]
fn given_failure(world: &LocalizationWorld) {
    world.failing.set(true);
}

#[when("I localise the diagnostic")]
fn when_localise(world: &LocalizationWorld) {
    let kind = *world.subject.borrow();
    let failing = world.failing.get();
    let attribute = resolve_attribute(world, failing);
    let result = resolve_localization(world, kind, attribute.as_str(), failing);

    world.result.replace(Some(result));
}

fn resolve_attribute(world: &LocalizationWorld, failing: bool) -> String {
    match (world.use_attribute_fallback.get(), failing) {
        (true, true) => {
            let lookup = failing_lookup();
            attribute_fallback(&lookup)
        }
        (true, false) => world.with_localizer(attribute_fallback),
        (false, _) => world.attribute.borrow().clone(),
    }
}

fn resolve_localization(
    world: &LocalizationWorld,
    kind: FunctionKind,
    attribute: &str,
    failing: bool,
) -> Result<FunctionAttrsMessages, I18nError> {
    if failing {
        let lookup = failing_lookup();
        localised_messages(&lookup, kind, attribute)
    } else {
        world.with_localizer(|localizer| localised_messages(localizer, kind, attribute))
    }
}

#[then("the primary message contains {snippet}")]
fn then_primary(world: &LocalizationWorld, snippet: String) {
    assert!(world.messages().primary().contains(&snippet));
}

#[then("the note mentions {snippet}")]
fn then_note(world: &LocalizationWorld, snippet: String) {
    assert!(world.messages().note().contains(&snippet));
}

#[then("the help mentions {snippet}")]
fn then_help(world: &LocalizationWorld, snippet: String) {
    assert!(world.messages().help().contains(&snippet));
}

#[then("localization fails for {key}")]
fn then_failure(world: &LocalizationWorld, key: String) {
    let error = world.error();
    match &*error {
        I18nError::MissingMessage { key: missing, .. } => assert_eq!(missing, &key),
    }
}

#[scenario(path = "tests/features/function_attrs_localization.feature", index = 0)]
fn scenario_fallback(world: LocalizationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localization.feature", index = 1)]
fn scenario_welsh(world: LocalizationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localization.feature", index = 2)]
fn scenario_attribute_fallback(world: LocalizationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localization.feature", index = 3)]
fn scenario_unknown_locale(world: LocalizationWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/function_attrs_localization.feature", index = 4)]
fn scenario_failure(world: LocalizationWorld) {
    let _ = world;
}

fn failing_lookup() -> FailingLookup {
    FailingLookup::new(MESSAGE_KEY.as_ref())
}
