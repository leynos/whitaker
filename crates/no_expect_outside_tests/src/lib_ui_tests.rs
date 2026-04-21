//! Additional UI-style regressions that need compiler flags or example-target
//! support beyond the basic `ui/` source fixtures.

use camino::Utf8Path;
use dylint_testing::ui::Test;
use rstest::rstest;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use temp_env::with_vars_unset;
use whitaker_common::test_support::{env_test_guard, prepare_fixture, run_test_runner};

/// Describes a single example-based regression to run under the test harness.
struct ExampleHarnessRun<'a> {
    name: &'a str,
    label: &'a str,
    rustc_flags: &'a [&'a str],
}

impl<'a> ExampleHarnessRun<'a> {
    fn new(name: &'a str, label: &'a str) -> Self {
        Self {
            name,
            label,
            rustc_flags: &["--test"],
        }
    }

    fn with_flags(name: &'a str, label: &'a str, rustc_flags: &'a [&'a str]) -> Self {
        Self {
            name,
            label,
            rustc_flags,
        }
    }
}

/// Describes a fixture-based regression to run under the test harness.
struct FixtureHarnessRun<'a> {
    crate_name: &'a str,
    fixture_name: &'a str,
    rustc_flags: &'a [&'a str],
    extern_crates: &'a [&'a str],
}

#[derive(Debug)]
struct DependencyRlib {
    path: PathBuf,
    modified: SystemTime,
}

fn run_example_under_test_harness(spec: &ExampleHarnessRun<'_>) {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    whitaker::testing::ui::run_with_runner(crate_name, directory, |crate_name, _| {
        run_test_runner(spec.name, || {
            let _guard = env_test_guard();
            with_vars_unset(
                ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER"],
                || {
                    let mut test = Test::example(crate_name, spec.name);
                    test.rustc_flags(spec.rustc_flags);
                    test.run();
                },
            );
        })
    })
    .unwrap_or_else(|error| {
        panic!(
            "{} example regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}",
            spec.label
        )
    });
}

fn run_fixture_under_test_harness(
    spec: &FixtureHarnessRun<'_>,
    directory: &Utf8Path,
) -> Result<(), String> {
    let source = fixture_source_path(directory, spec.fixture_name);
    let mut env = prepare_fixture(directory, &source)
        .map_err(|error| format!("failed to prepare {}: {error}", spec.fixture_name))?;
    let harness_flags = test_harness_flags(spec.rustc_flags, spec.extern_crates)?;
    let harness_flag_refs: Vec<_> = harness_flags.iter().map(String::as_str).collect();

    run_test_runner(spec.fixture_name, || {
        let _guard = env_test_guard();
        with_vars_unset(
            [
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "CARGO_BUILD_RUSTC_WRAPPER",
            ],
            || {
                let mut test = Test::src_base(spec.crate_name, env.workdir());
                if let Some(config) = env.take_config() {
                    test.dylint_toml(config);
                }
                test.rustc_flags(harness_flag_refs.as_slice());
                test.run();
            },
        );
    })
}

fn fixture_source_path(directory: &Utf8Path, fixture_name: &str) -> PathBuf {
    directory.as_std_path().join(format!("{fixture_name}.rs"))
}

fn test_harness_flags(extra_flags: &[&str], extern_crates: &[&str]) -> Result<Vec<String>, String> {
    let mut flags: Vec<String> = extra_flags.iter().map(|flag| (*flag).to_owned()).collect();
    flags.push("--edition=2024".to_owned());
    if extern_crates.is_empty() {
        return Ok(flags);
    }

    let deps_dir = dependency_directory()?;
    flags.extend([
        "-L".to_owned(),
        format!("dependency={}", deps_dir.display()),
    ]);
    for crate_name in extern_crates {
        let dependency_rlib = dependency_rlib(&deps_dir, crate_name)?;
        flags.extend([
            "--extern".to_owned(),
            format!("{crate_name}={}", dependency_rlib.display()),
        ]);
    }
    Ok(flags)
}

fn dependency_directory() -> Result<PathBuf, String> {
    let test_binary = std::env::current_exe()
        .map_err(|error| format!("failed to locate current test binary: {error}"))?;
    test_binary.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "test binary has no parent directory: {}",
            test_binary.display()
        )
    })
}

fn dependency_rlib(deps_dir: &Path, crate_name: &str) -> Result<PathBuf, String> {
    let prefix = format!("lib{crate_name}-");
    let mut matches = dependency_rlib_matches(deps_dir, &prefix)?;
    matches.sort_by(|left, right| {
        // Prefer the artefact produced most recently by the current build, then
        // fall back to a stable path ordering when timestamps tie.
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    matches
        .into_iter()
        .next()
        .map(|artifact| artifact.path)
        .ok_or_else(|| {
            format!(
                "failed to locate `{crate_name}` rlib in dependency directory {}",
                deps_dir.display()
            )
        })
}

fn dependency_rlib_matches(deps_dir: &Path, prefix: &str) -> Result<Vec<DependencyRlib>, String> {
    std::fs::read_dir(deps_dir)
        .map_err(|error| {
            format!(
                "failed to read dependency directory {}: {error}",
                deps_dir.display()
            )
        })?
        .map(|entry_result| {
            let entry = entry_result.map_err(|error| {
                format!(
                    "failed to read dependency entry in {}: {error}",
                    deps_dir.display()
                )
            })?;
            dependency_rlib_candidate(entry.path(), prefix)
        })
        .filter_map(|candidate| candidate.transpose())
        .collect()
}

fn dependency_rlib_candidate(
    path: PathBuf,
    prefix: &str,
) -> Result<Option<DependencyRlib>, String> {
    if !is_dependency_rlib(&path, prefix) {
        return Ok(None);
    }

    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("failed to read metadata for {}: {error}", path.display()))?;
    let modified = metadata.modified().map_err(|error| {
        format!(
            "failed to read modified time for {}: {error}",
            path.display()
        )
    })?;

    Ok(Some(DependencyRlib { path, modified }))
}

fn is_dependency_rlib(path: &Path, prefix: &str) -> bool {
    path.file_name().is_some_and(|name| {
        name.to_str()
            .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".rlib"))
    })
}

#[rstest]
#[case("pass_expect_in_tokio_test_harness", "Tokio")]
#[case(
    "pass_expect_in_tokio_nonstandard_module_harness",
    "Tokio non-standard module"
)]
#[case("pass_expect_in_rstest_harness", "rstest")]
fn example_compiles_under_test_harness(#[case] example_name: &str, #[case] label: &str) {
    run_example_under_test_harness(&ExampleHarnessRun::new(example_name, label));
}

#[test]
fn tokio_path_loaded_module_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "examples";
    let spec = FixtureHarnessRun {
        crate_name,
        fixture_name: "pass_expect_in_tokio_path_module_harness",
        rustc_flags: &["--test"],
        extern_crates: &["tokio"],
    };

    whitaker::testing::ui::run_with_runner(crate_name, directory, |_, dir| {
        run_fixture_under_test_harness(&spec, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "Tokio path module regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

#[test]
fn file_backed_cfg_test_module_compiles_under_test_harness() {
    let crate_name = env!("CARGO_PKG_NAME");
    let directory = "ui";
    let spec = FixtureHarnessRun {
        crate_name,
        fixture_name: "pass_expect_in_file_backed_test_module",
        rustc_flags: &["--test"],
        extern_crates: &[],
    };

    whitaker::testing::ui::run_with_runner(crate_name, directory, |_, dir| {
        run_fixture_under_test_harness(&spec, dir)
    })
    .unwrap_or_else(|error| {
        panic!(
            "File-backed cfg(test) regression should execute without diffs: RunnerFailure {{ crate_name: \"{crate_name}\", directory: \"{directory}\", message: {error:?} }}"
        )
    });
}

#[test]
fn rstest_expect_outside_tests_still_fails_in_non_harness_code() {
    run_example_under_test_harness(&ExampleHarnessRun::with_flags(
        "fail_expect_in_rstest_non_test_module",
        "rstest non-harness",
        &["--test", "-D", "no_expect_outside_tests"],
    ));
}

#[cfg(test)]
mod dependency_rlib_tests {
    //! Coverage for dependency artefact selection and related test fixtures.

    use super::dependency_rlib;
    use rstest::{fixture, rstest};
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    #[derive(Debug)]
    struct TemporaryDirectory(PathBuf);

    #[derive(Clone, Copy, Debug)]
    struct ArtifactSpec<'a> {
        file_name: &'a str,
        seconds_since_epoch: u64,
    }

    impl<'a> ArtifactSpec<'a> {
        const fn new(file_name: &'a str, seconds_since_epoch: u64) -> Self {
            Self {
                file_name,
                seconds_since_epoch,
            }
        }
    }

    #[derive(Debug)]
    struct DependencyRlibSelection {
        _directory: TemporaryDirectory,
        expected: PathBuf,
        selected: PathBuf,
    }

    const NEWEST_ARTIFACTS: [ArtifactSpec<'static>; 2] = [
        ArtifactSpec::new("libtokio-older.rlib", 10),
        ArtifactSpec::new("libtokio-newer.rlib", 20),
    ];
    const TIED_ARTIFACTS: [ArtifactSpec<'static>; 2] = [
        ArtifactSpec::new("libtokio-alpha.rlib", 30),
        ArtifactSpec::new("libtokio-zulu.rlib", 30),
    ];

    #[fixture]
    fn selection_directory() -> TemporaryDirectory {
        TemporaryDirectory::new("selection")
    }

    fn resolve_dependency_rlib_selection(
        directory: TemporaryDirectory,
        artifacts: &[ArtifactSpec<'_>],
        expected_file_name: &str,
    ) -> DependencyRlibSelection {
        for artifact in artifacts {
            let path = create_rlib(directory.path(), artifact.file_name);
            set_modified_time(&path, artifact.seconds_since_epoch);
        }

        let expected = directory.path().join(expected_file_name);
        let selected = dependency_rlib(directory.path(), "tokio")
            .expect("Tokio artefact should resolve from fixture directory");

        DependencyRlibSelection {
            _directory: directory,
            expected,
            selected,
        }
    }

    #[rstest]
    #[case("newest", &NEWEST_ARTIFACTS, "libtokio-newer.rlib")]
    #[case("ties", &TIED_ARTIFACTS, "libtokio-alpha.rlib")]
    fn dependency_rlib_selects_expected_artifact(
        selection_directory: TemporaryDirectory,
        #[case] _directory_name: &str,
        #[case] artifacts: &[ArtifactSpec<'_>],
        #[case] expected_file_name: &str,
    ) {
        let selection =
            resolve_dependency_rlib_selection(selection_directory, artifacts, expected_file_name);
        assert_eq!(selection.selected, selection.expected);
    }

    impl TemporaryDirectory {
        fn new(name: &str) -> Self {
            let unique = format!(
                "no-expect-outside-tests-{name}-{}",
                std::time::UNIX_EPOCH
                    .elapsed()
                    .expect("clock should be after the Unix epoch")
                    .as_nanos()
            );
            let directory = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&directory).expect("temporary directory should be created");
            Self(directory)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn create_rlib(directory: &Path, file_name: &str) -> PathBuf {
        let path = directory.join(file_name);
        File::create(&path).expect("rlib fixture should be created");
        path
    }

    fn set_modified_time(path: &Path, seconds_since_epoch: u64) {
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch);
        let file = File::options()
            .write(true)
            .open(path)
            .expect("rlib fixture should be reopened");
        let existing_accessed = file
            .metadata()
            .expect("rlib fixture metadata should be readable")
            .accessed();
        let times = existing_accessed
            .map(|accessed| std::fs::FileTimes::new().set_accessed(accessed))
            .unwrap_or_else(|_| std::fs::FileTimes::new())
            .set_modified(modified);
        file.set_times(times)
            .expect("rlib fixture modified time should be set");
    }
}
