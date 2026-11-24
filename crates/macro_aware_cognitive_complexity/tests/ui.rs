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
use whitaker_common::test_support::{prepare_fixture, run_fixtures_with, run_test_runner};
use whitaker_lint_core::testing::ui::{HarnessError, child_test_path, run_in_locale_child};

/// Run the curated fixtures under the default locale.
#[test]
#[serial]
fn ui() { run_in_locale("ui", "ui", None); }

/// Run the Welsh fixtures under `cy`.
#[test]
#[serial]
fn ui_cy() { run_in_locale("ui_cy", "ui-cy", Some("cy")); }

/// Run the Gaelic fixtures under `gd`.
#[test]
#[serial]
fn ui_gd() { run_in_locale("ui_gd", "ui-gd", Some("gd")); }

/// Run the curated fixtures under an unknown locale, which falls back to English.
#[test]
#[serial]
fn ui_unknown_locale_falls_back_to_english() {
    run_in_locale("ui_unknown_locale_falls_back_to_english", "ui", Some("zz"));
}

/// Run the fixtures that need a `--test` build, where the harness consumes
/// `#[test]` during expansion.
#[test]
#[serial]
fn ui_test_build() {
    let test_path = child_test_path(module_path!(), "ui_test_build");
    run_in_locale_child(&test_path, None, || {
        run_directory("ui-test", true).expect("`--test` UI fixtures should match");
    });
}

/// Run one fixture directory in a child process whose `DYLINT_LOCALE` is
/// `locale`.
///
/// The child re-runs the test named `test_fn`, so the locale is set when the
/// process starts rather than changed in this one.
fn run_in_locale(test_fn: &str, directory: &str, locale: Option<&str>) {
    let test_path = child_test_path(module_path!(), test_fn);
    run_in_locale_child(&test_path, locale, || {
        run_directory(directory, false)
            .unwrap_or_else(|error| panic!("UI fixtures should match: {error}"));
    });
}

/// Run every fixture in `directory` through the Dylint UI harness, as a
/// `--test` build when `is_test_build` is set.
fn run_directory(directory: &str, is_test_build: bool) -> Result<(), HarnessError> {
    whitaker_lint_core::testing::ui::run_with_runner(
        env!("CARGO_PKG_NAME"),
        directory,
        |crate_name, fixture_directory| {
            run_lane(FixtureSpec {
                crate_name,
                directory: fixture_directory,
                is_test_build,
            })
        },
    )
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
///
/// The callback rebuilds the spec from the runner's own crate and directory and
/// keeps this lane's build mode, so a lane is described once and applied to
/// every fixture it discovers.
fn run_lane(spec: FixtureSpec<'_>) -> Result<(), String> {
    run_fixtures_with(
        spec.crate_name,
        spec.directory,
        |crate_name, directory, source| {
            let fixture = FixtureSpec {
                crate_name,
                directory,
                is_test_build: spec.is_test_build,
            };
            run_fixture_in(fixture, source)
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
