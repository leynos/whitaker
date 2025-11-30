//! UI harness for `no_unwrap_or_else_panic` fixtures.

use camino::Utf8Path;
use common::test_support::{prepare_fixture, run_fixtures_with, run_test_runner};
use dylint_testing::ui::Test;
use std::path::Path;

#[test]
fn ui() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "ui";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, dir| {
        run_fixtures(crate_name, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "UI tests should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error} }}"
        )
    });
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
