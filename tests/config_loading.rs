//! Behaviour-driven tests for shared configuration loading.

use std::any::Any;
use std::cell::RefCell;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::str::FromStr;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::SharedConfig;

#[fixture]
fn config_source() -> RefCell<Option<String>> {
    RefCell::new(None)
}

#[fixture]
fn load_result() -> RefCell<Option<Result<SharedConfig, String>>> {
    RefCell::new(None)
}

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
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl ErrorSnippet {
    fn into_inner(self) -> String {
        self.0
    }
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
fn no_overrides(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().take();
}

#[given("the workspace config sets the module max line limit to {value}")]
fn override_max_lines(config_source: &RefCell<Option<String>>, value: usize) {
    config_source
        .borrow_mut()
        .replace(format!("[module_max_400_lines]\nmax_lines = {value}\n"));
}

#[given("the workspace config sets the module max line limit to an invalid value")]
fn invalid_override(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().replace(String::from(
        "[module_max_400_lines]\nmax_lines = \"invalid\"\n",
    ));
}

#[given("the workspace config includes unknown fields")]
fn unknown_fields(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().replace(
        concat!(
            "unexpected = true\n",
            "[module_max_400_lines]\n",
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
        SharedConfig::load_with("module_max_400_lines", |crate_name| {
            assert_eq!(crate_name, "module_max_400_lines");
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

#[then("the module max line limit is {expected}")]
fn assert_max_lines(load_result: &RefCell<Option<Result<SharedConfig, String>>>, expected: usize) {
    let borrow = load_result.borrow();
    let config = match borrow.as_ref() {
        Some(Ok(config)) => config,
        Some(Err(error)) => panic!("expected configuration loading to succeed: {error}"),
        None => panic!("configuration should be loaded"),
    };

    assert_eq!(config.module_max_400_lines.max_lines, expected);
}

#[then("a configuration error is reported")]
fn assert_error(load_result: &RefCell<Option<Result<SharedConfig, String>>>) {
    let borrow = load_result.borrow();
    match borrow.as_ref() {
        Some(Err(_)) => {}
        Some(Ok(config)) => {
            panic!("expected configuration loading to fail but succeeded with {config:?}")
        }
        None => panic!("configuration should be loaded"),
    }
}

#[then("a configuration error mentioning {snippet} is reported")]
fn assert_error_with_snippet(
    load_result: &RefCell<Option<Result<SharedConfig, String>>>,
    snippet: ErrorSnippet,
) {
    let snippet_value = snippet.into_inner();
    let borrow = load_result.borrow();
    match borrow.as_ref() {
        Some(Err(error)) => {
            assert!(
                error.contains(snippet_value.as_str()),
                "expected error '{error}' to mention '{snippet_value}'",
            );
        }
        Some(Ok(config)) => {
            panic!("expected configuration loading to fail but succeeded with {config:?}")
        }
        None => panic!("configuration should be loaded"),
    }
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
