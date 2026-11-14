//! UI harness for the macro-aware cognitive complexity lint.
//!
//! The curated fixtures run once per supported locale. `prepare_fixture`
//! copies each source, its expected stderr, and its configuration into a fresh
//! temporary directory, so `ui/` can be reused for the unknown-locale
//! fallback run without disturbing the default-locale expectations.
//!
//! `ui-test/` needs a separate lane because its fixtures compile as `--test`
//! crates. That build consumes `#[test]` during expansion, which is the only
//! way to exercise the harness-test exemption.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use std::path::Path;

use camino::Utf8Path;
use dylint_testing::ui::Test;
use serial_test::serial;
use whitaker_common::test_support::{
    prepare_fixture,
    run_fixtures_with,
    run_test_runner,
    with_locale,
};
use whitaker_lint_core::testing::ui::HarnessError;

/// Run the curated fixtures under the default locale.
#[test]
#[serial]
fn ui() { run_in_locale("ui", None).expect("default-locale UI fixtures should match"); }

/// Run the Welsh fixtures under `cy`.
#[test]
#[serial]
fn ui_cy() { run_in_locale("ui-cy", Some("cy")).expect("Welsh UI fixtures should match"); }

/// Run the Gaelic fixtures under `gd`.
#[test]
#[serial]
fn ui_gd() { run_in_locale("ui-gd", Some("gd")).expect("Gaelic UI fixtures should match"); }

/// Run the curated fixtures under an unknown locale, which falls back to English.
#[test]
#[serial]
fn ui_unknown_locale_falls_back_to_english() {
    run_in_locale("ui", Some("zz")).expect("an unknown locale should reuse English output");
}

/// Run the fixtures that need a `--test` build, where the harness consumes
/// `#[test]` during expansion.
#[test]
#[serial]
fn ui_test_build() {
    with_locale(None, || {
        whitaker_lint_core::testing::ui::run_with_runner(
            env!("CARGO_PKG_NAME"),
            "ui-test",
            run_test_build_fixtures,
        )
    })
    .expect("`--test` UI fixtures should match");
}

/// Run one fixture directory under `locale`.
fn run_in_locale(directory: &str, locale: Option<&str>) -> Result<(), HarnessError> {
    with_locale(locale, || run_directory(directory))
}

/// Run every fixture in `directory` through the Dylint UI harness.
fn run_directory(directory: &str) -> Result<(), HarnessError> {
    whitaker_lint_core::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        directory,
        run_fixtures,
    )
}

/// Iterate the fixtures in `directory`, preparing one isolated copy each.
fn run_fixtures(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    let spec = FixtureSpec {
        crate_name,
        directory,
        is_test_build: false,
    };
    run_lane(spec)
}

/// Run every fixture in `directory` as a `--test` build.
fn run_test_build_fixtures(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    let spec = FixtureSpec {
        crate_name,
        directory,
        is_test_build: true,
    };
    run_lane(spec)
}

/// One UI lane: the crate, its fixture directory, and its build mode.
///
/// The three travel together through every stage of a run, and pairing them
/// here keeps the per-fixture helper from taking separate string arguments.
#[derive(Clone, Copy)]
struct FixtureSpec<'a> {
    /// Name of the lint crate under test.
    crate_name: &'a str,
    /// Fixture directory, such as `ui` or `ui-test`.
    directory: &'a Utf8Path,
    /// Whether the lane compiles its fixtures as `--test` crates.
    is_test_build: bool,
}

/// Run every fixture in `spec`'s directory, preparing one isolated copy each.
fn run_lane(spec: FixtureSpec<'_>) -> Result<(), String> {
    let is_test_build = spec.is_test_build;
    run_fixtures_with(
        spec.crate_name,
        spec.directory,
        |crate_name, directory, source| {
            let spec = FixtureSpec {
                crate_name,
                directory,
                is_test_build,
            };
            run_fixture_in(spec, source)
        },
    )
}

/// Prepare `source`, optionally as a `--test` build, and compare its output.
fn run_fixture_in(spec: FixtureSpec<'_>, source: &Path) -> Result<(), String> {
    let fixture_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");

    let mut environment = prepare_fixture(spec.directory, source)
        .map_err(|error| format!("failed to prepare {fixture_name}: {error}"))?;

    let mut test = Test::src_base(spec.crate_name, environment.workdir());
    if let Some(config) = environment.take_config() {
        test.dylint_toml(config);
    }
    if spec.is_test_build {
        test.rustc_flags(["--test"]);
    }

    run_test_runner(fixture_name, || test.run())
}
