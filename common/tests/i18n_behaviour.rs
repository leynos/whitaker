//! Behaviour-driven coverage for the localisation loader.
//!
//! Scenarios exercise fallback resolution, secondary locale delivery, and
//! missing message handling to ensure lint crates can rely on predictable
//! diagnostics across locales.

use common::i18n::{Arguments, I18nError, Localiser};
use fluent_bundle::FluentValue;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
struct I18nFixture {
    locale: RefCell<Option<String>>,
    localiser: RefCell<Option<Localiser>>,
    outcome: RefCell<Option<Result<String, I18nError>>>,
}

impl I18nFixture {
    fn set_locale(&self, locale: Option<String>) {
        *self.locale.borrow_mut() = locale;
    }

    fn ensure_localiser(&self) -> Localiser {
        self.localiser
            .borrow_mut()
            .get_or_insert_with(|| {
                let locale = self.locale.borrow();
                let locale_clone = locale.clone();
                Localiser::new(locale_clone.as_deref())
            })
            .clone()
    }

    fn store_message(&self, result: Result<String, I18nError>) {
        *self.outcome.borrow_mut() = Some(result);
    }

    fn result(&self) -> Result<String, I18nError> {
        self.outcome
            .borrow()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| panic!("lookup should have been performed"))
    }
}

#[fixture]
fn fixture() -> I18nFixture {
    I18nFixture::default()
}

#[given("no locale preference")]
fn given_no_locale(fixture: &I18nFixture) {
    fixture.set_locale(None);
}

#[given("the locale preference {locale}")]
fn given_locale(fixture: &I18nFixture, locale: String) {
    fixture.set_locale(Some(locale));
}

#[when("I request the message for {key}")]
fn when_message(fixture: &I18nFixture, key: String) {
    let localiser = fixture.ensure_localiser();
    let result = localiser.message(&key);
    fixture.store_message(result);
}

#[when("I request the attribute {attribute} on {key}")]
fn when_attribute(fixture: &I18nFixture, key: String, attribute: String) {
    let localiser = fixture.ensure_localiser();
    let result = localiser.attribute(&key, &attribute);
    fixture.store_message(result);
}

#[when("I request the attribute {attribute} on {key} with branches {count}")]
fn when_attribute_with_branches(fixture: &I18nFixture, key: String, attribute: String, count: u32) {
    let localiser = fixture.ensure_localiser();
    let mut args: Arguments<'static> = HashMap::new();
    args.insert(Cow::Borrowed("branches"), FluentValue::from(count as i64));
    let result = localiser.attribute_with_args(&key, &attribute, &args);
    fixture.store_message(result);
}

#[when("I request the attribute note on common-lint-count with lint count {count}")]
fn when_common_lint_count_note(fixture: &I18nFixture, count: u32) {
    let localiser = fixture.ensure_localiser();
    let mut args: Arguments<'static> = HashMap::new();
    args.insert(Cow::Borrowed("lint"), FluentValue::from(count as i64));
    let result = localiser.attribute_with_args("common-lint-count", "note", &args);
    fixture.store_message(result);
}

#[then("the resolved locale is {expected}")]
fn then_locale(fixture: &I18nFixture, expected: String) {
    let localiser = fixture.ensure_localiser();
    assert_eq!(localiser.locale(), expected);
}

#[then("the loader reports fallback usage")]
fn then_fallback_used(fixture: &I18nFixture) {
    let localiser = fixture.ensure_localiser();
    assert!(localiser.used_fallback());
}

#[then("the message contains {snippet}")]
fn then_contains(fixture: &I18nFixture, snippet: String) {
    let message = fixture.result().expect("message should resolve");
    assert!(message.contains(&snippet));
}

#[then("localisation fails with a missing message error")]
fn then_missing(fixture: &I18nFixture) {
    match fixture.result() {
        Err(I18nError::MissingMessage { .. }) => {}
        other => panic!("unexpected result: {other:?}", other = other),
    }
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 0)]
fn scenario_falls_back(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 1)]
fn scenario_secondary_locale(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 2)]
fn scenario_gaelic_plural(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 3)]
fn scenario_welsh_lint_count_zero(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 4)]
fn scenario_welsh_lint_count_large(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 5)]
fn scenario_welsh_lint_count_one(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 6)]
fn scenario_welsh_lint_count_two(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 7)]
fn scenario_welsh_lint_count_three(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 8)]
fn scenario_welsh_lint_count_six(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 9)]
fn scenario_welsh_lint_count_eleven(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 10)]
fn scenario_attribute_falls_back(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 11)]
fn scenario_missing_message(fixture: I18nFixture) {
    let _ = fixture;
}

#[scenario(path = "tests/features/i18n_loader.feature", index = 12)]
fn scenario_welsh_conditional_note_lenition(fixture: I18nFixture) {
    let _ = fixture;
}
