//! UI harness and helpers for running dylint fixtures against the
//! `module_max_lines` lint. These tests ensure curated fixtures execute without
//! diffs and provide coverage for the fixture discovery helpers.

use camino::{Utf8Path, Utf8PathBuf};
use common::test_support::copy_fixture;
use dylint_testing::ui::Test;
use glob::glob;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

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

fn run_fixtures_with<F>(crate_name: &str, directory: &Utf8Path, mut runner: F) -> Result<(), String>
where
    F: FnMut(&str, &Utf8Path, &Path) -> Result<(), String>,
{
    let mut fixtures = discover_fixtures(directory).map_err(|error| error.to_string())?;
    fixtures.sort();

    for source in fixtures {
        runner(crate_name, directory, &source)?;
    }

    Ok(())
}

fn discover_fixtures(directory: &Utf8Path) -> io::Result<Vec<PathBuf>> {
    let pattern = directory.join("*.rs").to_string();
    let walker =
        glob(&pattern).map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
    let mut fixtures = Vec::new();

    for entry in walker {
        let path =
            entry.map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;
        if path.is_file() {
            fixtures.push(path);
        }
    }

    Ok(fixtures)
}

struct FixtureEnvironment {
    tempdir: TempDir,
    workdir: PathBuf,
    config: Option<String>,
}

fn prepare_fixture(directory: &Utf8Path, source: &Path) -> io::Result<FixtureEnvironment> {
    let tempdir = tempdir()?;
    copy_fixture(directory.as_std_path(), source, tempdir.path())?;
    let config = resolve_fixture_config(directory, source)?;
    Ok(FixtureEnvironment {
        workdir: tempdir.path().to_path_buf(),
        tempdir,
        config,
    })
}

fn run_fixture(crate_name: &str, directory: &Utf8Path, source: &Path) -> Result<(), String> {
    let fixture_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");
    let env = prepare_fixture(directory, source)
        .map_err(|error| format!("failed to prepare {fixture_name}: {error}"))?;

    let mut test = Test::src_base(crate_name, &env.workdir);
    if let Some(config) = env.config {
        test.dylint_toml(config);
    }

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test.run())).map_err(|payload| {
        match payload.downcast::<String>() {
            Ok(message) => format!("{fixture_name}: {message}"),
            Err(payload) => match payload.downcast::<&'static str>() {
                Ok(message) => format!("{fixture_name}: {message}"),
                Err(_) => format!("{fixture_name}: dylint UI tests panicked without a message"),
            },
        }
    })
}

fn read_fixture_config(source: &Path) -> io::Result<Option<String>> {
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "fixture missing name"))?;
    let config_path = source.with_file_name(format!("{stem}.dylint.toml"));

    if config_path.exists() {
        fs::read_to_string(config_path).map(Some)
    } else {
        Ok(None)
    }
}

fn read_directory_config(directory: &Utf8Path) -> io::Result<Option<String>> {
    let path = directory.as_std_path().join("dylint.toml");
    if path.exists() {
        fs::read_to_string(path).map(Some)
    } else {
        Ok(None)
    }
}

fn resolve_fixture_config(directory: &Utf8Path, source: &Path) -> io::Result<Option<String>> {
    if let Some(config) = read_fixture_config(source)? {
        Ok(Some(config))
    } else {
        read_directory_config(directory)
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    fn utf8_path(buf: &Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(buf.to_path_buf()).expect("utf8 path")
    }

    #[test]
    fn run_fixtures_sorts_and_runs_all_cases() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("b.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("a.rs"), "fn main() {}").unwrap();
        let directory = utf8_path(dir.path());
        let mut visited = Vec::new();

        run_fixtures_with("crate", &directory, |_, _, source| {
            let name = source.file_name().unwrap().to_string_lossy().into_owned();
            visited.push(name);
            Ok(())
        })
        .unwrap();

        assert_eq!(visited, vec!["a.rs".to_string(), "b.rs".to_string()]);
    }

    #[test]
    fn discover_fixtures_filters_rust_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("first.rs"), "").unwrap();
        fs::write(dir.path().join("second.txt"), "").unwrap();
        let directory = utf8_path(dir.path());

        let mut fixtures = discover_fixtures(&directory).unwrap();
        fixtures.sort();

        assert_eq!(fixtures.len(), 1);
        assert!(fixtures[0].ends_with("first.rs"));
    }

    #[test]
    fn read_fixture_config_loads_optional_file() {
        let dir = tempdir().unwrap();
        let fixture = dir.path().join("case.rs");
        fs::write(&fixture, "").unwrap();
        let config = dir.path().join("case.dylint.toml");
        fs::write(&config, "key = 1").unwrap();

        let contents = read_fixture_config(&fixture).unwrap();
        assert_eq!(contents.as_deref(), Some("key = 1"));
    }

    #[test]
    fn read_directory_config_loads_global_file() {
        let dir = tempdir().unwrap();
        let directory = utf8_path(dir.path());
        fs::write(directory.as_std_path().join("dylint.toml"), "max_lines = 5").unwrap();

        let contents = read_directory_config(&directory).unwrap();
        assert_eq!(contents.as_deref(), Some("max_lines = 5"));
    }

    #[test]
    fn resolve_fixture_config_prefers_fixture_specific_file() {
        let dir = tempdir().unwrap();
        let directory = utf8_path(dir.path());
        let fixture = directory.as_std_path().join("case.rs");
        fs::write(&fixture, "").unwrap();
        fs::write(
            directory.as_std_path().join("case.dylint.toml"),
            "fixture = true",
        )
        .unwrap();
        fs::write(directory.as_std_path().join("dylint.toml"), "global = true").unwrap();

        let contents = resolve_fixture_config(&directory, &fixture).unwrap();
        assert_eq!(contents.as_deref(), Some("fixture = true"));
    }

    #[test]
    fn resolve_fixture_config_falls_back_to_directory_file() {
        let dir = tempdir().unwrap();
        let directory = utf8_path(dir.path());
        let fixture = directory.as_std_path().join("case.rs");
        fs::write(&fixture, "").unwrap();
        fs::write(directory.as_std_path().join("dylint.toml"), "max_lines = 5").unwrap();

        let contents = resolve_fixture_config(&directory, &fixture).unwrap();
        assert_eq!(contents.as_deref(), Some("max_lines = 5"));
    }
}
