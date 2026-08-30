//! Behaviour-driven tests for installer staging.
//!
//! These scenarios cover staged filename conventions and non-writable target
//! handling.

#[cfg(unix)]
use std::cell::Cell;
use std::cell::RefCell;

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker_installer::{builder::CrateName, stager::Stager};

// ---------------------------------------------------------------------------
// Staging world
// ---------------------------------------------------------------------------

#[derive(Default)]
struct StagingWorld {
    crate_name: RefCell<Option<CrateName>>,
    toolchain: RefCell<String>,
    staged_name: RefCell<String>,
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
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
        .replace("nightly-2026-05-28".to_owned());
}

#[when("the library is staged")]
fn when_library_staged(staging_world: &StagingWorld) -> Result<(), String> {
    let crate_name_slot = staging_world.crate_name.borrow();
    let crate_name = crate_name_slot
        .as_ref()
        .ok_or_else(|| String::from("crate name not set"))?;
    let toolchain = staging_world.toolchain.borrow();

    // Use the production Stager to compute the filename.
    let temp_dir = TempDir::new().map_err(|error| format!("failed to create temp dir: {error}"))?;
    let utf8_path = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
        .map_err(|error| format!("temp dir path not UTF-8: {error}"))?;
    let stager = Stager::new(utf8_path, toolchain.as_str());
    let staged_name = stager.staged_filename(crate_name);

    staging_world.staged_name.replace(staged_name);
    Ok(())
}

#[then("the staged filename includes the toolchain")]
fn then_staged_includes_toolchain(staging_world: &StagingWorld) {
    let name = staging_world.staged_name.borrow();
    assert!(name.contains("nightly-2026-05-28"));
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
    //! Unix-only world and steps for staging permission failures.

    use std::{fs, os::unix::fs::PermissionsExt};

    use tempfile::TempDir;
    use whitaker_installer::error::InstallerError;

    use super::*;

    pub struct StagingFailureWorld {
        stager: RefCell<Option<Stager>>,
        result: RefCell<Option<Result<(), InstallerError>>>,
        skip_assertions: Cell<bool>,
        // Keep the temporary directory alive for the lifetime of the test.
        temp_dir: RefCell<Option<TempDir>>,
    }

    impl Default for StagingFailureWorld {
        fn default() -> Self {
            Self {
                stager: RefCell::new(None),
                result: RefCell::new(None),
                skip_assertions: Cell::new(false),
                temp_dir: RefCell::new(None),
            }
        }
    }

    #[whitaker_test_macros::allow_fixture_expansion_lints]
    #[fixture]
    pub fn staging_failure_world() -> StagingFailureWorld {
        StagingFailureWorld::default()
    }

    #[given("a non-writable staging directory")]
    pub fn given_non_writable_dir(
        staging_failure_world: &StagingFailureWorld,
    ) -> Result<(), String> {
        // Create a temp directory and make it read-only.
        let temp_dir =
            TempDir::new().map_err(|error| format!("failed to create temp dir: {error}"))?;
        let dir_path = temp_dir.path();

        // Create the nested staging path structure that Stager expects.
        let staging_path = dir_path.join("nightly-2026-05-28").join("release");
        fs::create_dir_all(&staging_path)
            .map_err(|error| format!("failed to create staging path: {error}"))?;

        // Make the directory read-only (no write permission).
        let mut perms = fs::metadata(&staging_path)
            .map_err(|error| format!("failed to get metadata: {error}"))?
            .permissions();
        perms.set_mode(0o555); // readable/traversable, not writable
        fs::set_permissions(&staging_path, perms)
            .map_err(|error| format!("failed to set permissions: {error}"))?;

        let utf8_path = Utf8PathBuf::try_from(dir_path.to_path_buf())
            .map_err(|error| format!("temp dir path not UTF-8: {error}"))?;
        let stager = Stager::new(utf8_path, "nightly-2026-05-28");

        staging_failure_world.stager.replace(Some(stager));
        staging_failure_world.temp_dir.replace(Some(temp_dir));
        Ok(())
    }

    #[when("the staging directory is prepared")]
    pub fn when_staging_prepared(
        staging_failure_world: &StagingFailureWorld,
    ) -> Result<(), String> {
        let stager_slot = staging_failure_world.stager.borrow();
        let stager = stager_slot
            .as_ref()
            .ok_or_else(|| String::from("stager not set"))?;

        // Best-effort probe to avoid flakes on filesystems that ignore directory
        // permissions. If we can unexpectedly create a file in the staging
        // directory, mark assertions as skipped for this scenario.
        let probe_path = stager.staging_path().as_std_path().join("write-probe");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe_path)
        {
            drop(file);
            std::fs::remove_file(&probe_path).map_err(|error| {
                format!(
                    "failed to remove write probe {}: {error}",
                    probe_path.display()
                )
            })?;
            staging_failure_world.skip_assertions.set(true);
        } else {
            // Expected: directory is not writable, continue.
        }

        let result = stager.prepare();
        staging_failure_world.result.replace(Some(result));
        Ok(())
    }

    #[then("staging fails with a target not writable error")]
    pub fn then_staging_fails_not_writable(
        staging_failure_world: &StagingFailureWorld,
    ) -> Result<(), String> {
        if staging_failure_world.skip_assertions.get() {
            return Ok(());
        }

        // Skip this assertion when running as root (uid 0) since root can bypass
        // filesystem permissions. This is similar to how CI containers often run.
        if rustix::process::geteuid().is_root() {
            return Ok(());
        }

        let result_slot = staging_failure_world.result.borrow();
        let result = result_slot
            .as_ref()
            .ok_or_else(|| String::from("result not set"))?;
        if matches!(result, Err(InstallerError::TargetNotWritable { .. })) {
            Ok(())
        } else {
            Err(format!("expected TargetNotWritable error, got {result:?}"))
        }
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
