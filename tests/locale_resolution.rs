//! Behaviour-driven tests covering locale resolution semantics.

use std::cell::RefCell;
use std::convert::Infallible;
use std::str::FromStr;

use common::i18n::{LocaleResolution, LocaleSource, resolve_localiser};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[derive(Default)]
struct LocaleWorld {
    explicit: RefCell<Option<String>>,
    environment: RefCell<Option<String>>,
    configuration: RefCell<Option<String>>,
    resolution: RefCell<Option<LocaleResolution>>,
}

#[fixture]
fn world() -> LocaleWorld {
    LocaleWorld::default()
}

#[derive(Debug)]
struct StepLocale(String);

impl FromStr for StepLocale {
    type Err = Infallible;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input
            .trim()
            .trim_matches(|candidate| matches!(candidate, '"' | '\''));

        Ok(Self(trimmed.to_owned()))
    }
}

impl StepLocale {
    fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Debug)]
struct StepSource(LocaleSource);

impl FromStr for StepSource {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_lowercase().as_str() {
            "explicit" => Ok(Self(LocaleSource::ExplicitArgument)),
            "environment" => Ok(Self(LocaleSource::EnvironmentVariable)),
            "configuration" => Ok(Self(LocaleSource::Configuration)),
            "fallback" => Ok(Self(LocaleSource::Fallback)),
            other => Err(format!("unknown locale source '{other}'")),
        }
    }
}

impl StepSource {
    const fn into_inner(self) -> LocaleSource {
        self.0
    }
}

fn resolved(world: &LocaleWorld) -> LocaleResolution {
    let borrow = world.resolution.borrow();
    borrow.as_ref().map_or_else(
        || panic!("the locale should have been resolved"),
        LocaleResolution::clone,
    )
}

#[given("no explicit locale override is provided")]
fn no_explicit(world: &LocaleWorld) {
    world.explicit.borrow_mut().take();
}

#[given("the explicit locale override is {value}")]
fn set_explicit(world: &LocaleWorld, value: StepLocale) {
    world.explicit.borrow_mut().replace(value.into_inner());
}

#[given("DYLINT_LOCALE is not set")]
fn clear_environment(world: &LocaleWorld) {
    world.environment.borrow_mut().take();
}

#[given("DYLINT_LOCALE is {value}")]
fn set_environment(world: &LocaleWorld, value: StepLocale) {
    world.environment.borrow_mut().replace(value.into_inner());
}

#[given("no configuration locale is provided")]
fn clear_configuration(world: &LocaleWorld) {
    world.configuration.borrow_mut().take();
}

#[given("the configuration locale is {value}")]
fn set_configuration(world: &LocaleWorld, value: StepLocale) {
    world.configuration.borrow_mut().replace(value.into_inner());
}

#[when("the locale is resolved")]
fn resolve_locale(world: &LocaleWorld) {
    let explicit = world.explicit.borrow().clone();
    let environment = world.environment.borrow().clone();
    let configuration = world.configuration.borrow().clone();

    let resolution = resolve_localiser(explicit.as_deref(), environment, configuration.as_deref());
    world.resolution.borrow_mut().replace(resolution);
}

#[then("the locale source is {source}")]
fn assert_source(world: &LocaleWorld, source: StepSource) {
    let resolution = resolved(world);

    assert_eq!(resolution.source(), source.into_inner());
}

#[then("the resolved locale is {value}")]
fn assert_locale(world: &LocaleWorld, value: StepLocale) {
    let resolution = resolved(world);

    assert_eq!(resolution.locale(), value.into_inner());
}

#[then("the fallback locale is used")]
fn assert_fallback_used(world: &LocaleWorld) {
    let resolution = resolved(world);

    assert!(resolution.used_fallback());
}

#[then("the fallback locale is not used")]
fn assert_fallback_not_used(world: &LocaleWorld) {
    let resolution = resolved(world);

    assert!(!resolution.used_fallback());
}

#[then("no locale rejections are recorded")]
fn assert_no_rejections(world: &LocaleWorld) {
    let resolution = resolved(world);

    assert!(resolution.rejections().is_empty());
}

#[then("the locale rejections include {source} {value}")]
fn assert_rejection(world: &LocaleWorld, source: StepSource, value: StepLocale) {
    let resolution = resolved(world);
    let expected_source = source.into_inner();
    let expected_value = value.into_inner();

    assert!(resolution.rejections().iter().any(|rejection| {
        rejection.source() == expected_source && rejection.value() == expected_value
    }));
}

#[scenario("tests/features/locale_resolution.feature", index = 0)]
fn scenario_fallback(world: LocaleWorld) {
    let _ = world;
}

#[scenario("tests/features/locale_resolution.feature", index = 1)]
fn scenario_environment(world: LocaleWorld) {
    let _ = world;
}

#[scenario("tests/features/locale_resolution.feature", index = 2)]
fn scenario_configuration(world: LocaleWorld) {
    let _ = world;
}

#[scenario("tests/features/locale_resolution.feature", index = 3)]
fn scenario_explicit(world: LocaleWorld) {
    let _ = world;
}

#[scenario("tests/features/locale_resolution.feature", index = 4)]
fn scenario_whitespace(world: LocaleWorld) {
    let _ = world;
}
