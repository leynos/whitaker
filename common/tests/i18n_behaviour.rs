//! Behaviour-driven coverage for the localisation loader.
//!
//! Scenarios exercise fallback resolution, secondary locale delivery, and
//! missing message handling to ensure lint crates can rely on predictable
//! diagnostics across locales.

use common::i18n::{Arguments, FluentValue, I18nError, Localizer};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

#[path = "support/i18n_helpers.rs"]
mod i18n_helpers;
use i18n_helpers::{default_arguments, strip_isolation_marks};

#[derive(Clone, Debug, Default)]
struct I18nFixture {
    locale: RefCell<Option<String>>,
    localizer: RefCell<Option<Localizer>>,
    outcome: RefCell<Option<Result<String, I18nError>>>,
}

impl I18nFixture {
    fn set_locale(&self, locale: Option<String>) {
        *self.locale.borrow_mut() = locale;
    }

    fn ensure_localizer(&self) -> Localizer {
        let locale = self.locale.borrow().clone();
        self.localizer
            .borrow_mut()
            .get_or_insert_with(|| Localizer::new(locale.as_deref()))
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

fn branch_phrase_for(locale: &str, branches: u32) -> String {
    match locale {
        "cy" => match branches {
            0 => "dim canghennau".to_string(),
            1 => "un gangen".to_string(),
            2 => "dwy gangen".to_string(),
            3 => "tri changen".to_string(),
            6 => "chwe changen".to_string(),
            4 | 5 => format!("{branches} cangen"),
            _ => format!("{branches} canghennau"),
        },
        "gd" => match branches {
            1 | 2 => format!("{branches} mheur"),
            3 => format!("{branches} meuran"),
            _ => format!("{branches} meur"),
        },
        _ => format!("{branches} branches"),
    }
}

fn lint_count_from_key(key: &str) -> Option<(String, u32)> {
    let suffix = " with lint count ";
    let (base, count) = key.rsplit_once(suffix)?;
    let value = count.trim().parse().ok()?;
    Some((base.to_string(), value))
}

fn branch_count_from_key(key: &str) -> Option<(String, u32)> {
    let suffix = " with branches ";
    let (base, count) = key.rsplit_once(suffix)?;
    let value = count.trim().parse().ok()?;
    Some((base.to_string(), value))
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
    let localizer = fixture.ensure_localizer();
    let args = default_arguments();
    let result = localizer.message_with_args(&key, &args);
    fixture.store_message(result);
}

#[when("I request the attribute {attribute} on {key}")]
fn when_attribute(fixture: &I18nFixture, attribute: String, key: String) {
    let localizer = fixture.ensure_localizer();

    if let Some((base_key, branches)) = branch_count_from_key(&key) {
        let mut args = default_arguments();
        args.insert(
            Cow::Borrowed("branches"),
            FluentValue::from(branches as i64),
        );
        let phrase = branch_phrase_for(localizer.locale(), branches);
        args.insert(Cow::Borrowed("branch_phrase"), FluentValue::from(phrase));
        let result = localizer.attribute_with_args(&base_key, &attribute, &args);
        fixture.store_message(result);
        return;
    }

    if let Some((base_key, lint_count)) = lint_count_from_key(&key) {
        let mut args: Arguments<'static> = HashMap::new();
        args.insert(Cow::Borrowed("lint"), FluentValue::from(lint_count as i64));
        let result = localizer.attribute_with_args(&base_key, &attribute, &args);
        fixture.store_message(result);
        return;
    }

    let args = default_arguments();
    let result = localizer.attribute_with_args(&key, &attribute, &args);
    fixture.store_message(result);
}

#[then("the resolved locale is {expected}")]
fn then_locale(fixture: &I18nFixture, expected: String) {
    let localizer = fixture.ensure_localizer();
    assert_eq!(localizer.locale(), expected);
}

#[then("the loader reports fallback usage")]
fn then_fallback_used(fixture: &I18nFixture) {
    let localizer = fixture.ensure_localizer();
    assert!(localizer.used_fallback());
}

#[then("the message contains {snippet}")]
fn then_contains(fixture: &I18nFixture, snippet: String) {
    let message = fixture
        .result()
        .unwrap_or_else(|error| panic!("message should resolve: {error}"));
    let message = strip_isolation_marks(&message);
    let snippet = strip_isolation_marks(&snippet);
    assert!(
        message.contains(snippet.as_ref()),
        "expected `{message}` to contain `{snippet}`",
    );
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

#[cfg(test)]
mod tests {
    use super::lint_count_from_key;
    use rstest::rstest;

    #[rstest]
    #[case("foo with lint count 42", Some(("foo".to_string(), 42)))]
    #[case("foo with lint 42", None)]
    #[case("foo with lint count ", None)]
    #[case("foo with lint count abc", None)]
    #[case("", None)]
    #[case(" with lint count 10", Some(("".to_string(), 10)))]
    fn lint_count_from_key_parsing(#[case] input: &str, #[case] expected: Option<(String, u32)>) {
        assert_eq!(lint_count_from_key(input), expected);
    }
}
