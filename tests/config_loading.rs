#![feature(rustc_private)]

use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

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
        Err(payload) => payload.downcast::<&'static str>().map_or_else(
            |_| "configuration loading panicked with a non-string payload".to_string(),
            |message| (*message).to_string(),
        ),
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
    config_source
        .borrow_mut()
        .replace("[module_max_400_lines]\nmax_lines = \"invalid\"\n".to_string());
}

#[given("the workspace config includes unknown fields")]
fn unknown_fields(config_source: &RefCell<Option<String>>) {
    config_source.borrow_mut().replace(
        concat!(
            "unexpected = true\n",
            "[module_max_400_lines]\n",
            "max_lines = 120\n",
        )
        .to_string(),
    );
}

#[when("the shared configuration is loaded")]
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
                    match toml::from_str::<SharedConfig>(input) {
                        Ok(config) => config,
                        Err(error) => panic!("Could not parse shared configuration: {error}"),
                    }
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
