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
        delivered: Vec::new(),
    }
}

fn stub_prune_prebuilt_libraries(
    _staging_path: &Utf8Path,
    _toolchain_channel: &str,
    _requested_crates: &[CrateName],
    _delivered: &[String],
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

/// Every file name currently in `staging_path`: what an archive that delivered
/// exactly these files would report.
fn staged_names(staging_path: &Utf8Path) -> Vec<String> {
    fs::read_dir(staging_path.as_std_path())
        .expect("staging directory should be readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
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

/// One pruning scenario: what was asked for, and what that leaves on disk.
///
/// The three lists are only meaningful together — `retained` and `removed` must
/// partition the staged set, and `requested` is what decides the partition — so
/// they travel as one value rather than as three parallel `#[case]` columns.
struct PruneScenario {
    /// The crate names handed to `prune_prebuilt_libraries`.
    requested: &'static [&'static str],
    /// The staged crates the prune must keep.
    retained: &'static [&'static str],
    /// The staged crates the prune must delete.
    removed: &'static [&'static str],
}

#[rstest]
#[case::suite_only(PruneScenario {
    requested: &[SUITE_CRATE],
    retained: &[SUITE_CRATE],
    removed: &["module_max_lines", "no_expect_outside_tests"],
})]
#[case::default_suite(PruneScenario {
    requested: &[],
    retained: &[SUITE_CRATE],
    removed: &["module_max_lines", "no_expect_outside_tests"],
})]
#[case::individual_only(PruneScenario {
    requested: &["module_max_lines"],
    retained: &["module_max_lines"],
    removed: &[SUITE_CRATE, "no_expect_outside_tests"],
})]
fn prune_prebuilt_libraries_keeps_only_requested_crates(
    staging_fixture: StagingFixture,
    #[case] scenario: PruneScenario,
) {
    let PruneScenario {
        requested,
        retained,
        removed,
    } = scenario;
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;

    let other_toolchain_path =
        create_staged_library(&staging_path, "module_max_lines", "nightly-2020-01-01");
    let unrelated_path = staging_path.join("README.txt");
    fs::write(unrelated_path.as_std_path(), b"not a library")
        .expect("test setup should write unrelated file");

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
    let delivered = staged_names(&staging_path);
    prune_prebuilt_libraries(&staging_path, toolchain, &requested_crates, &delivered)
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
        other_toolchain_path.exists(),
        "a library staged for another toolchain should remain untouched"
    );
    assert!(
        unrelated_path.exists(),
        "files that are not staged libraries should remain untouched"
    );
}

/// Regression test for whitaker issue 565.
///
/// A newer archive stages a crate this installer was not compiled to know. The
/// suite registers that crate's lint, so its standalone library must not
/// survive beside the suite: both would load and rustc would reject the
/// duplicate registration.
#[rstest]
fn prune_prebuilt_libraries_removes_unknown_crates_registered_by_the_suite(
    staging_fixture: StagingFixture,
) {
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;
    let suite = create_staged_library(&staging_path, SUITE_CRATE, toolchain);
    let constituent = create_staged_library(&staging_path, "module_max_lines", toolchain);
    let unknown =
        create_staged_library(&staging_path, "a_crate_added_after_this_release", toolchain);

    let delivered = staged_names(&staging_path);
    prune_prebuilt_libraries(&staging_path, toolchain, &[], &delivered)
        .expect("pruning should succeed");

    assert!(suite.exists(), "the suite must survive");
    assert!(!constituent.exists(), "a known constituent must be removed");
    assert!(!unknown.exists(), "an unknown constituent must be removed");
    let remaining: Vec<_> = fs::read_dir(staging_path.as_std_path())
        .expect("staging directory should be readable")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(
        remaining.len(),
        1,
        "only the suite should remain: {remaining:?}"
    );
}

#[rstest]
fn prune_prebuilt_libraries_keeps_an_unknown_crate_that_was_requested(
    staging_fixture: StagingFixture,
) {
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;
    let requested =
        create_staged_library(&staging_path, "a_crate_added_after_this_release", toolchain);
    let suite = create_staged_library(&staging_path, SUITE_CRATE, toolchain);

    let delivered = staged_names(&staging_path);
    prune_prebuilt_libraries(
        &staging_path,
        toolchain,
        &[CrateName::from("a_crate_added_after_this_release")],
        &delivered,
    )
    .expect("pruning should succeed");

    assert!(requested.exists(), "a requested crate must survive");
    assert!(!suite.exists(), "an unrequested suite must be removed");
}

#[rstest]
fn prune_prebuilt_libraries_reports_a_requested_library_the_archive_lacks(
    staging_fixture: StagingFixture,
) {
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;
    create_staged_library(&staging_path, SUITE_CRATE, toolchain);
    let delivered = staged_names(&staging_path);

    let error = prune_prebuilt_libraries(
        &staging_path,
        toolchain,
        &[CrateName::from("string_continuation_style")],
        &delivered,
    )
    .expect_err("a missing requested library must not pass as an install");

    assert!(
        matches!(&error, InstallerError::StagingFailed { reason } if reason.contains("string_continuation_style")),
        "unexpected error: {error:?}"
    );
}

/// The directory is reused between installs (install-whitaker restores it from
/// cache), so a stale library from an earlier install must not satisfy the
/// check when this archive did not deliver it.
#[rstest]
fn prune_prebuilt_libraries_ignores_a_stale_requested_library_the_archive_did_not_deliver(
    staging_fixture: StagingFixture,
) {
    let StagingFixture {
        staging_path,
        toolchain,
        ..
    } = staging_fixture;
    create_staged_library(&staging_path, "string_continuation_style", toolchain);
    let suite = create_staged_library(&staging_path, SUITE_CRATE, toolchain);
    let delivered = vec![
        suite
            .file_name()
            .expect("suite library has a file name")
            .to_owned(),
    ];

    let error = prune_prebuilt_libraries(
        &staging_path,
        toolchain,
        &[CrateName::from("string_continuation_style")],
        &delivered,
    )
    .expect_err("a stale file must not satisfy the requested-library check");

    assert!(
        matches!(&error, InstallerError::StagingFailed { reason } if reason.contains("string_continuation_style")),
        "unexpected error: {error:?}"
    );
}
#[path = "fallback_tests.rs"]
mod fallback_tests;
