#![feature(rustc_private)]

use std::cell::RefCell;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use whitaker::SharedConfig;

#[fixture]
fn config_source() -> RefCell<Option<String>> {
    RefCell::new(None)
}

#[fixture]
fn load_result() -> RefCell<Option<Result<SharedConfig, toml::de::Error>>> {
    RefCell::new(None)
}

#[given("no configuration state has been prepared")]
fn reset_state(
    config_source: &RefCell<Option<String>>,
    load_result: &RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
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
            "[module_max_400_lines]\n",
            "max_lines = 120\n",
            "unexpected = true\n",
        )
        .to_string(),
    );
}

#[when("the shared configuration is loaded")]
fn load_config(
    config_source: &RefCell<Option<String>>,
    load_result: &RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
) {
    let result = config_source.borrow().as_ref().map_or_else(
        || Ok(SharedConfig::default()),
        |input| toml::from_str::<SharedConfig>(input),
    );

    load_result.borrow_mut().replace(result);
}

#[then("the module max line limit is {expected}")]
fn assert_max_lines(
    load_result: &RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
    expected: usize,
) {
    let borrow = load_result.borrow();
    let config = match borrow.as_ref() {
        Some(Ok(config)) => config,
        Some(Err(error)) => panic!("expected configuration loading to succeed: {error}"),
        None => panic!("configuration should be loaded"),
    };

    assert_eq!(config.module_max_400_lines.max_lines, expected);
}

#[then("a configuration error is reported")]
fn assert_error(load_result: &RefCell<Option<Result<SharedConfig, toml::de::Error>>>) {
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
    load_result: RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 1)]
fn scenario_override(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 2)]
fn scenario_errors(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
) {
    let _ = (config_source, load_result);
}

#[scenario("tests/features/config_loading.feature", index = 3)]
fn scenario_unknown_fields(
    config_source: RefCell<Option<String>>,
    load_result: RefCell<Option<Result<SharedConfig, toml::de::Error>>>,
) {
    let _ = (config_source, load_result);
}
