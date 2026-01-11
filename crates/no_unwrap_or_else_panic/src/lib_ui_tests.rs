//! UI harness for `no_unwrap_or_else_panic` fixtures.

use camino::Utf8Path;
use common::test_support::{prepare_fixture, run_fixtures_with, run_test_runner};
use dylint_testing::ui::Test;
use std::path::Path;
use std::{fs, io};

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
    if let Some(flags) = read_rustc_flags(source)
        .map_err(|error| format!("failed to load rustc flags for {fixture_name}: {error}"))?
    {
        test.rustc_flags(flags);
    }

    run_test_runner(fixture_name, || test.run())
}

/// Load optional rustc flags from a `.rustc-flags` sidecar file.
///
/// Each non-empty line is treated as whitespace-delimited flags. Lines may
/// include comments after `#`, which are stripped before parsing.
fn read_rustc_flags(source: &Path) -> io::Result<Option<Vec<String>>> {
    let path = source.with_extension("rustc-flags");
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)?;
    let flags = contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .flat_map(|line| line.split_whitespace().map(str::to_owned))
        .collect();

    Ok(Some(flags))
}
