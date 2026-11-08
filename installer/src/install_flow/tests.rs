//! Unit tests for install-flow prebuilt staging and fallback behaviour.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use camino::Utf8PathBuf;
use rstest::{fixture, rstest};

use super::*;

struct StagingFixture {
    _temp_dir: tempfile::TempDir,
    staging_path: Utf8PathBuf,
    toolchain: &'static str,
}

struct TestBaseDirs {
    data_dir: Option<PathBuf>,
}

impl BaseDirs for TestBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }
    fn executables(&self) -> Option<PathBuf> { None }
    fn whitaker_data(&self) -> Option<PathBuf> { self.data_dir.clone() }
}

static PRUNE_HOOK_CALLED: AtomicBool = AtomicBool::new(false);

/// The policy these tests assume, stated rather than inherited.
///
/// `InstallArgs::source_policy` reads `WHITAKER_NO_SOURCE_FALLBACK` from the
/// process environment, so a developer or a runner that exported it would turn
/// every fallback here into a refusal and the failures would look like defects
/// in the flow. The policy under test is a parameter, so these tests state it.
fn permissive_policy() -> SourcePolicy {
    SourcePolicy {
        quiet: false,
        no_source_fallback: false,
    }
}

/// The policy a test's own arguments ask for, without reading the environment.
///
/// This reads the `no_source_fallback` field rather than calling
/// `InstallArgs::forbids_source_fallback`, which consults
/// `WHITAKER_NO_SOURCE_FALLBACK`. A test that sets the flag gets the strict
/// policy; a test that does not gets the permissive one, whatever the
/// environment holds.
fn policy_from_flags(args: &InstallArgs) -> SourcePolicy {
    SourcePolicy {
        quiet: args.quiet,
        no_source_fallback: args.no_source_fallback,
    }
}

/// The same, with the refusal enabled.
fn strict_policy() -> SourcePolicy {
    SourcePolicy {
        quiet: false,
        no_source_fallback: true,
    }
}

/// Feed the real parser a canned `rustc -vV` line, so the stub stays fallible.
fn stub_detect_host_target() -> Result<String> {
    parse_host_target("host: x86_64-unknown-linux-gnu\n")
}

fn stub_resolve_destination_dir(
    _dirs: &dyn BaseDirs,
    _toolchain_channel: &str,
    _host_target: &str,
) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(PathBuf::from("/tmp/whitaker-test-data/lints")).map_err(|path| {
        InstallerError::WorkspaceNotFound {
            reason: format!("test destination is not UTF-8: {}", path.display()),
        }
    })
}

fn stub_attempt_prebuilt(_config: &PrebuiltConfig<'_>, _stderr: &mut dyn Write) -> PrebuiltResult {
    PrebuiltResult::Success {
        staging_path: Utf8PathBuf::from("/tmp/whitaker-test-staging"),
    }
}

fn stub_prune_prebuilt_libraries(
    _staging_path: &Utf8Path,
    _toolchain_channel: &str,
    _requested_crates: &[CrateName],
) -> Result<()> {
    PRUNE_HOOK_CALLED.store(true, Ordering::SeqCst);
    Err(InstallerError::StagingFailed {
        reason: "forced prune failure".to_owned(),
    })
}

#[fixture]
fn staging_fixture() -> StagingFixture {
    let temp_dir = tempfile::tempdir().expect("tempdir should be available");
    let staging_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .expect("tempdir path should be utf-8");
    fs::create_dir_all(staging_path.as_std_path()).expect("staging path should be creatable");
    StagingFixture {
        _temp_dir: temp_dir,
        staging_path,
        toolchain: "nightly-2026-05-28",
    }
}

fn create_staged_library(
    staging_path: &Utf8Path,
    crate_name: &str,
    toolchain: &str,
) -> Utf8PathBuf {
    let library_path = staging_path.join(staged_library_filename(crate_name, toolchain));
    fs::write(library_path.as_std_path(), b"fake prebuilt library")
        .expect("test setup should write staged library");
    library_path
}

#[rstest]
#[case::suite_only(
    &[SUITE_CRATE],
    &[SUITE_CRATE],
    &["module_max_lines", "no_expect_outside_tests"]
)]
#[case::default_suite(
    &[],
    &[SUITE_CRATE],
    &["module_max_lines", "no_expect_outside_tests"]
)]
#[case::individual_only(
    &["module_max_lines"],
    &["module_max_lines"],
    &[SUITE_CRATE, "no_expect_outside_tests"]
)]
fn prune_prebuilt_libraries_keeps_only_requested_crates(
    staging_fixture: StagingFixture,
    #[case] requested: &[&str],
    #[case] retained: &[&str],
    #[case] removed: &[&str],
) {
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;

    let foreign_path = staging_path.join("libforeign_lint@nightly-2026-05-28.so");
    fs::write(foreign_path.as_std_path(), b"foreign library")
        .expect("test setup should write foreign library");

    let mut staged = Vec::new();
    for crate_name in retained.iter().chain(removed.iter()) {
        let path = create_staged_library(&staging_path, crate_name, toolchain);
        staged.push(((*crate_name).to_owned(), path));
    }

    let requested_crates: Vec<CrateName> = requested
        .iter()
        .map(|name| CrateName::from(*name))
        .collect();
    if requested.is_empty() {
        let default_requested = requested_crate_names(&requested_crates);
        assert_eq!(
            default_requested,
            HashSet::from([SUITE_CRATE]),
            "empty requested-crate list should default to suite crate"
        );
    }
    prune_prebuilt_libraries(&staging_path, toolchain, &requested_crates)
        .expect("pruning should succeed");

    for crate_name in retained {
        let path = staged
            .iter()
            .find(|(name, _)| name == crate_name)
            .map(|(_, path)| path)
            .expect("retained library should have been staged");
        assert!(path.exists(), "{crate_name} should remain");
    }

    for crate_name in removed {
        let path = staged
            .iter()
            .find(|(name, _)| name == crate_name)
            .map(|(_, path)| path)
            .expect("removed library should have been staged");
        assert!(!path.exists(), "{crate_name} should be removed");
    }

    assert!(
        foreign_path.exists(),
        "non-whitaker libraries should remain untouched"
    );
}

#[path = "fallback_tests.rs"]
mod fallback_tests;
