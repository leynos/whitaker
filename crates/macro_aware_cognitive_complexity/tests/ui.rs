//! UI harness for the macro-aware cognitive complexity lint.
//!
//! The curated fixtures run once per supported locale. `prepare_fixture`
//! copies each source, its expected stderr, and its configuration into a fresh
//! temporary directory, so `ui/` can be reused for the unknown-locale
//! fallback run without disturbing the default-locale expectations.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use std::path::Path;

use camino::Utf8Path;
use dylint_testing::ui::Test;
use serial_test::serial;
use whitaker::testing::ui::HarnessError;
use whitaker_common::test_support::{
    prepare_fixture,
    run_fixtures_with,
    run_test_runner,
    with_locale,
};

#[test]
#[serial]
fn ui() { run_in_locale("ui", None).expect("default-locale UI fixtures should match"); }

#[test]
#[serial]
fn ui_cy() { run_in_locale("ui-cy", Some("cy")).expect("Welsh UI fixtures should match"); }

#[test]
#[serial]
fn ui_gd() { run_in_locale("ui-gd", Some("gd")).expect("Gaelic UI fixtures should match"); }

#[test]
#[serial]
fn ui_unknown_locale_falls_back_to_english() {
    run_in_locale("ui", Some("zz")).expect("an unknown locale should reuse English output");
}

/// Run one fixture directory under `locale`.
fn run_in_locale(directory: &str, locale: Option<&str>) -> Result<(), HarnessError> {
    with_locale(locale, || run_directory(directory))
}

/// Run every fixture in `directory` through the Dylint UI harness.
fn run_directory(directory: &str) -> Result<(), HarnessError> {
    whitaker::testing::ui::run_with_runner(env!("CARGO_PKG_NAME"), directory, run_fixtures)
}

/// Iterate the fixtures in `directory`, preparing one isolated copy each.
fn run_fixtures(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    run_fixtures_with(crate_name, directory, run_fixture)
}

/// Run one prepared fixture and compare its diagnostic output.
fn run_fixture(crate_name: &str, directory: &Utf8Path, source: &Path) -> Result<(), String> {
    let fixture_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");

    let mut environment = prepare_fixture(directory, source)
        .map_err(|error| format!("failed to prepare {fixture_name}: {error}"))?;

    let mut test = Test::src_base(crate_name, environment.workdir());
    if let Some(config) = environment.take_config() {
        test.dylint_toml(config);
    }

    run_test_runner(fixture_name, || test.run())
}
