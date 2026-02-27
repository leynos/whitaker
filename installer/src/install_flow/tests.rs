//! Unit tests for install-flow prebuilt staging and fallback behaviour.

use super::*;
use camino::Utf8PathBuf;
use rstest::{fixture, rstest};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

struct StagingFixture {
    _temp_dir: tempfile::TempDir,
    staging_path: Utf8PathBuf,
    toolchain: &'static str,
}

struct TestBaseDirs {
    data_dir: Option<PathBuf>,
}

impl BaseDirs for TestBaseDirs {
    fn home_dir(&self) -> Option<PathBuf> {
        None
    }
    fn bin_dir(&self) -> Option<PathBuf> {
        None
    }
    fn whitaker_data_dir(&self) -> Option<PathBuf> {
        self.data_dir.clone()
    }
}

static PRUNE_HOOK_CALLED: AtomicBool = AtomicBool::new(false);

fn stub_detect_host_target() -> Result<String> {
    Ok("x86_64-unknown-linux-gnu".to_owned())
}

fn stub_resolve_destination_dir(
    _dirs: &dyn BaseDirs,
    _toolchain_channel: &str,
    _host_target: &str,
) -> Result<Utf8PathBuf> {
    Ok(Utf8PathBuf::from("/tmp/whitaker-test-data/lints"))
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
        toolchain: "nightly-2025-09-18",
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
        _temp_dir: _,
        staging_path,
        toolchain,
    } = staging_fixture;

    let foreign_path = staging_path.join("libforeign_lint@nightly-2025-09-18.so");
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

#[test]
fn try_prebuilt_installation_prune_error_falls_back_to_local_build() {
    let dirs = TestBaseDirs {
        data_dir: Some(PathBuf::from("/tmp/whitaker-test-data")),
    };

    let args = InstallArgs::default();
    let requested_crates = vec![CrateName::from(SUITE_CRATE)];
    let context = PrebuiltInstallationContext {
        args: &args,
        dirs: &dirs,
        requested_crates: &requested_crates,
        toolchain_channel: "nightly-2025-09-18",
    };

    let mut stderr = Vec::new();
    PRUNE_HOOK_CALLED.store(false, Ordering::SeqCst);
    let result = try_prebuilt_installation_with(
        &context,
        &mut stderr,
        PrebuiltInstallationHooks {
            detect_host_target: stub_detect_host_target,
            resolve_destination_dir: stub_resolve_destination_dir,
            attempt_prebuilt: stub_attempt_prebuilt,
            prune_prebuilt_libraries: stub_prune_prebuilt_libraries,
        },
    );

    assert!(
        matches!(result, Ok(None)),
        "prune failure should trigger fallback to local compilation"
    );
    assert!(
        PRUNE_HOOK_CALLED.load(Ordering::SeqCst),
        "prune hook should be invoked"
    );
    let stderr = String::from_utf8(stderr).expect("stderr should be utf-8");
    assert!(
        stderr.contains("Prebuilt download unavailable: staging failed: forced prune failure"),
        "fallback reason should include prune error, stderr: {stderr}"
    );
    assert!(
        stderr.contains("Falling back to local compilation."),
        "fallback message should be emitted, stderr: {stderr}"
    );
}
