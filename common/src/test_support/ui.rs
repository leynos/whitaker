//! Shared UI harness helpers for dylint fixture runners.
//!
//! These utilities encapsulate the boilerplate required to discover fixtures,
//! clone them into an isolated workspace, and execute each case via
//! `dylint_testing` while capturing panics into deterministic error messages.

use crate::test_support::copy_fixture;
use camino::Utf8Path;
use glob::glob;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::{TempDir, tempdir};

/// Temporary workspace prepared for a single UI fixture run.
pub struct FixtureEnvironment {
    _tempdir: TempDir,
    workdir: PathBuf,
    config: Option<String>,
}

impl FixtureEnvironment {
    /// Returns the root directory containing the cloned fixture files.
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// Moves the optional `dylint.toml` contents out of the environment.
    pub fn take_config(&mut self) -> Option<String> {
        self.config.take()
    }
}

/// Discovers `.rs` fixtures inside `directory`, returning the paths unsorted.
pub fn discover_fixtures(directory: &Utf8Path) -> io::Result<Vec<PathBuf>> {
    let pattern = directory.join("*.rs").to_string();
    let walker = glob(&pattern).map_err(|error| io::Error::other(error.to_string()))?;
    let mut fixtures = Vec::new();

    for entry in walker {
        let path = entry.map_err(|error| io::Error::other(error.to_string()))?;
        if path.is_file() {
            fixtures.push(path);
        }
    }

    Ok(fixtures)
}

/// Runs fixtures discovered under `directory` using the provided `runner`.
pub fn run_fixtures_with<F>(
    crate_name: &str,
    directory: &Utf8Path,
    mut runner: F,
) -> Result<(), String>
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

/// Copies `source` into a temporary directory, including stderr/config files.
pub fn prepare_fixture(directory: &Utf8Path, source: &Path) -> io::Result<FixtureEnvironment> {
    let tempdir = tempdir()?;
    copy_fixture(directory.as_std_path(), source, tempdir.path())?;
    let config = resolve_fixture_config(directory, source)?;
    Ok(FixtureEnvironment {
        workdir: tempdir.path().to_path_buf(),
        _tempdir: tempdir,
        config,
    })
}

/// Executes `runner`, capturing unwinds into deterministic error strings.
pub fn run_test_runner<F>(fixture_name: &str, runner: F) -> Result<(), String>
where
    F: FnOnce(),
{
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(runner)).map_err(|payload| match payload
        .downcast::<String>(
    ) {
        Ok(message) => format!("{fixture_name}: {message}"),
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => format!("{fixture_name}: {message}"),
            Err(_) => format!("{fixture_name}: dylint UI tests panicked without a message"),
        },
    })
}

/// Resolves the configuration content for `source`, preferring per-fixture files.
pub fn resolve_fixture_config(directory: &Utf8Path, source: &Path) -> io::Result<Option<String>> {
    if let Some(config) = read_fixture_config(source)? {
        Ok(Some(config))
    } else {
        read_directory_config(directory)
    }
}

/// Loads `case.dylint.toml` for a fixture when present.
pub fn read_fixture_config(source: &Path) -> io::Result<Option<String>> {
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

/// Loads `ui/dylint.toml` style directory-level configuration when present.
pub fn read_directory_config(directory: &Utf8Path) -> io::Result<Option<String>> {
    let path = directory.as_std_path().join("dylint.toml");
    if path.exists() {
        fs::read_to_string(path).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use std::fs;

    fn utf8_path(buf: &Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(buf.to_path_buf()).expect("utf8 path")
    }

    #[test]
    fn run_fixtures_sorts_and_runs_all_cases() {
        let dir = tempdir().expect("fixture directory");
        fs::write(dir.path().join("b.rs"), "fn main() {}").expect("write first fixture");
        fs::write(dir.path().join("a.rs"), "fn main() {}").expect("write second fixture");
        let directory = utf8_path(dir.path());
        let mut visited = Vec::new();

        run_fixtures_with("crate", &directory, |_, _, source| {
            let name = source
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| "utf8 file name".to_string())?
                .to_owned();
            visited.push(name);
            Ok(())
        })
        .expect("fixtures run");

        assert_eq!(visited, vec!["a.rs".to_string(), "b.rs".to_string()]);
    }

    #[test]
    fn discover_fixtures_filters_rust_files() {
        let dir = tempdir().expect("fixture directory");
        fs::write(dir.path().join("first.rs"), "").expect("first fixture");
        fs::write(dir.path().join("second.txt"), "").expect("second fixture");
        let directory = utf8_path(dir.path());

        let mut fixtures = discover_fixtures(&directory).expect("discover fixtures");
        fixtures.sort();

        assert_eq!(fixtures.len(), 1);
        assert!(fixtures[0].ends_with("first.rs"));
    }

    #[test]
    fn discover_fixtures_returns_empty_directory() {
        let dir = tempdir().expect("fixture directory");
        let directory = utf8_path(dir.path());

        let fixtures = discover_fixtures(&directory).expect("discover fixtures");

        assert!(fixtures.is_empty());
    }

    #[test]
    fn read_fixture_config_loads_optional_file() {
        let dir = tempdir().expect("fixture directory");
        let fixture = dir.path().join("case.rs");
        fs::write(&fixture, "").expect("fixture file");
        let config = dir.path().join("case.dylint.toml");
        fs::write(&config, "key = 1").expect("config file");

        let contents = read_fixture_config(&fixture).expect("config contents");
        assert_eq!(contents.as_deref(), Some("key = 1"));
    }

    #[test]
    fn read_directory_config_loads_global_file() {
        let dir = tempdir().expect("fixture directory");
        let directory = utf8_path(dir.path());
        fs::write(directory.as_std_path().join("dylint.toml"), "max_lines = 5")
            .expect("global config");

        let contents = read_directory_config(&directory).expect("config contents");
        assert_eq!(contents.as_deref(), Some("max_lines = 5"));
    }

    #[test]
    fn resolve_fixture_config_prefers_fixture_specific_file() {
        let dir = tempdir().expect("fixture directory");
        let directory = utf8_path(dir.path());
        let fixture = directory.as_std_path().join("case.rs");
        fs::write(&fixture, "").expect("fixture");
        fs::write(
            directory.as_std_path().join("case.dylint.toml"),
            "fixture = true",
        )
        .expect("fixture config");
        fs::write(directory.as_std_path().join("dylint.toml"), "global = true")
            .expect("global config");

        let contents = resolve_fixture_config(&directory, &fixture).expect("config contents");
        assert_eq!(contents.as_deref(), Some("fixture = true"));
    }
}
