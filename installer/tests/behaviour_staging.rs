//! Behaviour-driven tests for installer staging.
//!
//! These scenarios cover staged filename conventions and non-writable target
//! handling.

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
#[cfg(unix)]
use std::cell::Cell;
use std::cell::RefCell;
use whitaker_installer::builder::CrateName;
use whitaker_installer::stager::Stager;

// ---------------------------------------------------------------------------
// Staging world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct StagingWorld {
    crate_name: RefCell<Option<CrateName>>,
    toolchain: RefCell<String>,
    staged_name: RefCell<String>,
}

#[fixture]
fn staging_world() -> StagingWorld {
    StagingWorld::default()
}

#[given("a built library")]
fn given_built_library(staging_world: &StagingWorld) {
    staging_world
        .crate_name
        .replace(Some(CrateName::from("module_max_lines")));
}

#[given("a staging directory")]
fn given_staging_dir(staging_world: &StagingWorld) {
    staging_world
        .toolchain
        .replace("nightly-2025-09-18".to_owned());
}

#[when("the library is staged")]
fn when_library_staged(staging_world: &StagingWorld) {
    let crate_name = staging_world.crate_name.borrow();
    let crate_name = crate_name.as_ref().expect("crate name not set");
    let toolchain = staging_world.toolchain.borrow();

    // Use the production Stager to compute the filename.
    let stager = Stager::new(Utf8PathBuf::from("/tmp/test"), &toolchain);
    let staged_name = stager.staged_filename(crate_name);

    staging_world.staged_name.replace(staged_name);
}

#[then("the staged filename includes the toolchain")]
fn then_staged_includes_toolchain(staging_world: &StagingWorld) {
    let name = staging_world.staged_name.borrow();
    assert!(name.contains("nightly-2025-09-18"));
    assert!(name.contains("module_max_lines"));
}

// ---------------------------------------------------------------------------
// Staging failure world (Unix only - relies on Unix file permissions)
// ---------------------------------------------------------------------------

#[cfg(unix)]
use staging_failure::StagingFailureWorld;
#[cfg(unix)]
use staging_failure::staging_failure_world;

#[cfg(unix)]
mod staging_failure {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;
    use whitaker_installer::error::InstallerError;

    pub struct StagingFailureWorld {
        stager: RefCell<Option<Stager>>,
        result: RefCell<Option<Result<(), InstallerError>>>,
        skip_assertions: Cell<bool>,
        // Keep temp_dir alive for the lifetime of the test.
        _temp_dir: RefCell<Option<TempDir>>,
    }

    impl Default for StagingFailureWorld {
        fn default() -> Self {
            Self {
                stager: RefCell::new(None),
                result: RefCell::new(None),
                skip_assertions: Cell::new(false),
                _temp_dir: RefCell::new(None),
            }
        }
    }

    #[fixture]
    pub fn staging_failure_world() -> StagingFailureWorld {
        StagingFailureWorld::default()
    }

    #[given("a non-writable staging directory")]
    pub fn given_non_writable_dir(staging_failure_world: &StagingFailureWorld) {
        // Create a temp directory and make it read-only.
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // Create the nested staging path structure that Stager expects.
        let staging_path = dir_path.join("nightly-2025-09-18").join("release");
        fs::create_dir_all(&staging_path).expect("failed to create staging path");

        // Make the directory read-only (no write permission).
        let mut perms = fs::metadata(&staging_path)
            .expect("failed to get metadata")
            .permissions();
        perms.set_mode(0o444); // read-only
        fs::set_permissions(&staging_path, perms).expect("failed to set permissions");

        let utf8_path =
            Utf8PathBuf::try_from(dir_path.to_path_buf()).expect("temp dir path not UTF-8");
        let stager = Stager::new(utf8_path, "nightly-2025-09-18");

        staging_failure_world.stager.replace(Some(stager));
        staging_failure_world._temp_dir.replace(Some(temp_dir));
    }

    #[when("the staging directory is prepared")]
    pub fn when_staging_prepared(staging_failure_world: &StagingFailureWorld) {
        let stager = staging_failure_world.stager.borrow();
        let stager = stager.as_ref().expect("stager not set");

        // Best-effort probe to avoid flakes on filesystems that ignore directory
        // permissions. If we can unexpectedly create a file in the staging
        // directory, mark assertions as skipped for this scenario.
        let probe_path = stager.staging_path().as_std_path().join("write-probe");
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe_path)
        {
            Ok(file) => {
                drop(file);
                let _ = std::fs::remove_file(&probe_path);
                staging_failure_world.skip_assertions.set(true);
            }
            Err(_) => {
                // Expected: directory is not writable, continue.
            }
        }

        let result = stager.prepare();
        staging_failure_world.result.replace(Some(result));
    }

    #[then("staging fails with a target not writable error")]
    pub fn then_staging_fails_not_writable(staging_failure_world: &StagingFailureWorld) {
        if staging_failure_world.skip_assertions.get() {
            return;
        }

        // Skip this assertion when running as root (uid 0) since root can bypass
        // filesystem permissions. This is similar to how CI containers often run.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }

        let result = staging_failure_world.result.borrow();
        let result = result.as_ref().expect("result not set");
        assert!(
            matches!(result, Err(InstallerError::TargetNotWritable { .. })),
            "expected TargetNotWritable error, got {result:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario bindings
// ---------------------------------------------------------------------------

#[scenario(path = "tests/features/installer.feature", index = 10)]
fn scenario_stage_with_toolchain_suffix(staging_world: StagingWorld) {
    let _ = staging_world;
}

#[cfg(unix)]
#[scenario(path = "tests/features/installer.feature", index = 11)]
fn scenario_reject_staging_non_writable(staging_failure_world: StagingFailureWorld) {
    let _ = staging_failure_world;
}
