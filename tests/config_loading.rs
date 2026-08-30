//! Behaviour-driven tests for shared configuration loading.

use std::{
    any::Any,
    cell::RefCell,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    str::FromStr,
};

mod support;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use support::locale::StepLocale;
use whitaker::SharedConfig;
use whitaker_common::i18n::normalize_locale;

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn config_source() -> RefCell<Option<String>> { RefCell::new(None) }

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn load_result() -> RefCell<Option<Result<SharedConfig, String>>> { RefCell::new(None) }

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(non_string) => non_string.downcast::<&'static str>().map_or_else(
            |_| "configuration loading panicked with a non-string payload".to_owned(),
            |message| (*message).to_owned(),
        ),
    }
}

#[derive(Debug)]
struct ErrorSnippet(String);

impl FromStr for ErrorSnippet {
    type Err = Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input
            .trim()
            .trim_matches(|candidate| matches!(candidate, '"' | '\''));

        Ok(Self(trimmed.to_owned()))
    }
}

impl AsRef<str> for ErrorSnippet {
    fn as_ref(&self) -> &str { self.0.as_str() }
}

impl ErrorSnippet {
    fn into_inner(self) -> String { self.0 }
}

#[given("no configuration state has been prepared")]
fn reset_state(
    config_source: &RefCell<Option<String>>,
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
) {
    config_source.borrow_mut().take();
    load_result.borrow_mut().take();
}

#[given("no workspace configuration overrides are provided")]
fn no_overrides(config_source: &RefCell<Option<String>>) { config_source.borrow_mut().take(); }

#[given("the workspace config sets the module max line limit to {value}")]
fn override_max_lines(config_source: &RefCell<Option<String>>, value: usize) {
    config_source
        .borrow_mut()
        .replace(format!("[module_max_lines]\nmax_lines = {value}\n"));
}

#[given("the workspace config sets the module max line limit to an invalid value")]
fn invalid_override(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().replace(String::from(
        "[module_max_lines]\nmax_lines = \"invalid\"\n",
    ));
}

#[given("the workspace config sets the locale to {value}")]
fn override_locale(config_source: &RefCell<Option<String>>, value: StepLocale) {
    let locale = value.into_inner();
    config_source
        .borrow_mut()
        .replace(format!("locale = \"{locale}\"\n"));
}

#[given("the workspace config includes unknown fields")]
fn unknown_fields(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().replace(
        concat!(
            "unexpected = true\n",
            "[module_max_lines]\n",
            "max_lines = 120\n",
        )
        .to_owned(),
    );
}

#[when("the shared configuration is loaded")]
#[expect(
    clippy::expect_used,
    reason = "`expect` keeps the panic message concise per review guidance"
)]
fn load_config(
    config_source: &RefCell<Option<String>>,
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
) {
    let maybe_source = config_source.borrow().clone();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        SharedConfig::load_with("module_max_lines", |crate_name| {
            assert_eq!(
                crate_name, "module_max_lines",
                "the loader should request configuration for the requested lint",
            );
            maybe_source
                .as_ref()
                .map_or_else(SharedConfig::default, |input| {
                    toml::from_str::<SharedConfig>(input)
                        .expect("Could not parse shared configuration")
                })
        })
    }));

    load_result
        .borrow_mut()
        .replace(outcome.map_err(panic_message));
}

/// Clones the successfully loaded configuration from the step state.
///
/// Returns an error when loading has not run yet or reported a failure.
fn loaded_config(
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
) -> Result<SharedConfig, String> {
    match load_result.borrow().as_ref() {
        Some(Ok(config)) => Ok(config.clone()),
        Some(Err(error)) => Err(format!(
            "expected configuration loading to succeed: {error}"
        )),
        None => Err("configuration should be loaded".to_owned()),
    }
}

/// Clones the loading error from the step state.
///
/// Returns an error when loading has not run yet or unexpectedly succeeded.
fn load_error(
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
) -> Result<String, String> {
    match load_result.borrow().as_ref() {
        Some(Err(error)) => Ok(error.clone()),
        Some(Ok(config)) => Err(format!(
            "expected configuration loading to fail but succeeded with {config:?}"
        )),
        None => Err("configuration should be loaded".to_owned()),
    }
}

#[then("the module max line limit is {expected}")]
fn assert_max_lines(load_result: &RefCell<Option<Result<SharedConfig, String>>>, expected: usize) {
    let config = match loaded_config(load_result) {
        Ok(config) => config,
        Err(message) => panic!("{message}"),
    };

    assert_eq!(config.module_max_lines.max_lines, expected);
}

#[then("the locale override is {expected}")]
fn assert_locale(
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
    expected: StepLocale,
) {
    let raw = expected.into_inner();
    let Some(expected_value) = normalize_locale(Some(raw.as_str())) else {
        panic!("expected the step to provide a locale value");
    };
    let config = match loaded_config(load_result) {
        Ok(config) => config,
        Err(message) => panic!("{message}"),
    };

    assert_eq!(config.locale(), Some(expected_value));
}

#[then("no locale override is configured")]
fn assert_no_locale(load_result: &RefCell<Option<Result<SharedConfig, String>>>) {
    let config = match loaded_config(load_result) {
        Ok(config) => config,
        Err(message) => panic!("{message}"),
    };

    assert!(
        config.locale().is_none(),
        "expected no locale override but found {:?}",
        config.locale(),
    );
}

#[then("a configuration error is reported")]
fn assert_error(load_result: &RefCell<Option<Result<SharedConfig, String>>>) {
    if let Err(message) = load_error(load_result) {
        panic!("{message}");
    }
}

#[then("a configuration error mentioning {snippet} is reported")]
fn assert_error_with_snippet(
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
    snippet: ErrorSnippet,
) {
    let snippet_value = snippet.into_inner();
    let error = match load_error(load_result) {
        Ok(error) => error,
        Err(message) => panic!("{message}"),
    };

    assert!(
        error.contains(snippet_value.as_str()),
        "expected error '{error}' to mention '{snippet_value}'",
    );
}

#[scenario("tests/features/config_loading.feature", index = 0)]
fn scenario_defaults(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, String>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 1)]
fn scenario_override(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, String>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 2)]
fn scenario_errors(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, String>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 3)]
fn scenario_unknown_fields(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, String>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 4)]
fn scenario_locale_override(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, String>>>,
) {
    let _ = (config_source, load_result);
}
