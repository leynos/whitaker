//! UI test harness for the `bumpy_road_function` lint.
//!
//! These tests execute the curated Dylint fixtures in `ui/` and assert that
//! diagnostics remain stable across changes.
#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
extern crate rustc_driver;

use std::path::Path;

use camino::Utf8Path;
use dylint_testing::ui::Test;
use whitaker_common::test_support::{prepare_fixture, run_fixtures_with, run_test_runner};

/// Run the curated fixtures, then the differential corpus, under the defaults.
///
/// The corpus records what the weighted detector says today, so an unnoticed
/// change to weighted behaviour fails here as a stderr diff. Both directories
/// run inside the one `ui` test so the nextest `serial-dylint-ui` group and its
/// build-sized timeout cover them; a second test name would fall outside that
/// override and could race the first over the shared lint library.
#[test]
fn ui() {
    for directory in ["ui", "corpus"] {
        whitaker_lint_core::testing::ui::run_ui_test(
            env!("CARGO_PKG_NAME"),
            directory,
            run_fixtures,
        );
    }
}

fn run_fixtures(crate_name: &str, directory: &Utf8Path) -> Result<(), String> {
    run_fixtures_with(crate_name, directory, run_fixture)
}

fn run_fixture(crate_name: &str, directory: &Utf8Path, source: &Path) -> Result<(), String> {
    let fixture_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");

    let mut env = prepare_fixture(directory, source)
        .map_err(|error| format!("failed to prepare {fixture_name}: {error}"))?;

    let mut test = Test::src_base(crate_name, env.workdir());
    if let Some(config) = env.take_config() {
        test.dylint_toml(config);
    }

    run_test_runner(fixture_name, || test.run())
}
