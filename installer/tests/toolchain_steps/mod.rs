//! Step definitions for toolchain behavioural tests.
//!
//! These step implementations are used by the scenarios in
//! `behaviour_toolchain.rs` via rstest-bdd macros.

mod scenario_setup;

use std::process::Command;

use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
pub use scenario_setup::{FAKE_TOOLCHAIN, ToolchainWorld, setup_install_scenario};
use scenario_setup::{
    ensure_toolchain_installed_in_isolated_env, get_combined_output_string, get_output,
    get_stderr_string, setup_auto_install_scenario, setup_dry_run_scenario, setup_failure_scenario,
};

use super::{prebuilt_markers::PREBUILT_INSTALL_MARKER, support::workspace_root};

/// Output marker indicating successful library staging (build-from-source path).
const STAGING_OUTPUT_MARKER: &str = "Staging libraries to";

/// Output marker indicating successful toolchain installation.
const TOOLCHAIN_INSTALLED_MARKER: &str = "installed successfully";

/// Canonical error marker for toolchain installation failures.
const TOOLCHAIN_ERROR_MARKER: &str = "installation failed";

/// Maximum output lines expected in quiet mode error scenarios.
const QUIET_MODE_MAX_LINES: usize = 5;

macro_rules! skip_if_needed {
    ($world:expr) => {
        if $world.should_skip_assertions.get() {
            return Ok(());
        }
    };
}

fn assert_toolchain_install_message_presence(
    world: &ToolchainWorld,
    expected_presence: bool,
) -> Result<(), String> {
    skip_if_needed!(world);

    let output = get_combined_output_string(world)?;
    let channel = world.pinned_channel.borrow().clone();
    let output_lowercase = output.to_lowercase();
    let channel_lowercase = channel.to_lowercase();
    let expected_message = format!("toolchain {channel} installed successfully").to_lowercase();
    let has_install_message = output_lowercase.contains(&expected_message)
        || output_lowercase.contains(&channel_lowercase)
            && output_lowercase.contains(TOOLCHAIN_INSTALLED_MARKER);

    if has_install_message == expected_presence {
        Ok(())
    } else if expected_presence {
        Err(format!(
            "expected success marker for channel '{channel}' in output, got:\n{output}"
        ))
    } else {
        Err(format!(
            "expected no installation message for channel '{channel}' in output, got:\n{output}"
        ))
    }
}

fn assert_stderr_contains(
    world: &ToolchainWorld,
    expected: &str,
    failure_message: impl FnOnce(&str) -> String,
) -> Result<(), String> {
    skip_if_needed!(world);

    let stderr = get_stderr_string(world)?;
    if stderr.contains(expected) {
        Ok(())
    } else {
        Err(failure_message(&stderr))
    }
}

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

/// Creates the world shared by a toolchain scenario's setup and assertions.
#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
pub fn world() -> ToolchainWorld {
    ToolchainWorld::default()
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

/// Configures an auto-detection dry-run scenario.
#[given("the installer is invoked with auto-detect toolchain")]
pub fn given_auto_detect_toolchain(world: &ToolchainWorld) -> Result<(), String> {
    setup_dry_run_scenario(world, &["--dry-run"])
}

/// Configures an auto-detection dry-run scenario in quiet mode.
#[given("the installer is invoked with auto-detect toolchain in quiet mode")]
pub fn given_auto_detect_toolchain_quiet(world: &ToolchainWorld) -> Result<(), String> {
    setup_dry_run_scenario(world, &["--dry-run", "--quiet"])
}

/// Configures an auto-detection installation scenario.
#[given("the installer is invoked with auto-detect toolchain to a temporary directory")]
pub fn given_auto_detect_toolchain_install(world: &ToolchainWorld) -> Result<(), String> {
    setup_auto_install_scenario(world)
}

/// Configures an isolated rustup scenario that exercises auto-installation.
#[given("the installer is invoked with isolated rustup to force auto-install")]
pub fn given_isolated_rustup_auto_install(world: &ToolchainWorld) -> Result<(), String> {
    setup_auto_install_scenario(world)
}

/// Configures an isolated rustup installation scenario in quiet mode.
#[given("the installer is invoked with isolated rustup in quiet mode")]
pub fn given_isolated_rustup_quiet(world: &ToolchainWorld) -> Result<(), String> {
    // Use --skip-wrapper to prevent writing to the user's real ~/.local/bin.
    setup_install_scenario(
        world,
        &["--jobs", "1", "--quiet", "--skip-deps", "--skip-wrapper"],
    )
}

/// Configures a scenario for a missing toolchain.
#[given("the installer is invoked with a non-existent toolchain")]
pub fn given_nonexistent_toolchain(world: &ToolchainWorld) -> Result<(), String> {
    setup_failure_scenario(world, &[])
}

/// Configures a quiet-mode scenario for a missing toolchain.
#[given("the installer is invoked with a non-existent toolchain in quiet mode")]
pub fn given_nonexistent_toolchain_quiet(world: &ToolchainWorld) -> Result<(), String> {
    setup_failure_scenario(world, &["--quiet"])
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

/// Runs the installer with the scenario's isolated rustup environment.
#[when("the installer CLI is run")]
pub fn when_installer_cli_run(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);

    let args = world.args.borrow();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"));
    cmd.args(args.iter());
    cmd.current_dir(workspace_root()?);

    // Sanitize rustup environment to prevent host settings from leaking
    // into tests: always disable auto-install and remove toolchain overrides
    cmd.env("RUSTUP_AUTO_INSTALL", "0");
    cmd.env_remove("RUSTUP_TOOLCHAIN");

    if let Some(ref rustup_home) = *world.rustup_home.borrow() {
        cmd.env("RUSTUP_HOME", rustup_home.path());
    }
    if let Some(ref cargo_home) = *world.cargo_home.borrow() {
        cmd.env("CARGO_HOME", cargo_home.path());
    }

    let output = cmd
        .output()
        .map_err(|error| format!("failed to run whitaker-installer: {error}"))?;
    world.output.replace(Some(output));
    Ok(())
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

/// Asserts that the installer command completed successfully.
#[then("the CLI exits successfully")]
pub fn then_cli_exits_successfully(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    let output = get_output(world)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "expected success, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Asserts that dry-run output names the expected toolchain.
#[then("dry-run output shows the detected toolchain")]
pub fn then_dry_run_shows_toolchain(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_channel = world.pinned_channel.borrow().clone();
    if stderr.contains(&expected_channel) {
        Ok(())
    } else {
        Err(format!(
            "expected toolchain '{expected_channel}' in output, stderr: {stderr}"
        ))
    }
}

/// Asserts that no toolchain installation message was emitted.
#[then("no toolchain installation message is shown")]
pub fn then_no_install_message(world: &ToolchainWorld) -> Result<(), String> {
    assert_toolchain_install_message_presence(world, false)
}

/// Asserts that the toolchain installation message was emitted.
#[then("the toolchain installation message is shown")]
pub fn then_install_message_shown(world: &ToolchainWorld) -> Result<(), String> {
    assert_toolchain_install_message_presence(world, true)
}

/// Asserts that installation succeeded, allowing the isolated-toolchain skip.
#[then("installation succeeds or is skipped")]
pub fn then_installation_succeeds_or_is_skipped(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    {
        let output = get_output(world)?;
        if !output.status.success() {
            return Err(format!(
                "installation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    ensure_toolchain_installed_in_isolated_env(world)
}

/// Asserts that the requested toolchain is present in the isolated environment.
#[then("the toolchain is installed in the isolated environment")]
pub fn then_toolchain_installed_in_isolated_env(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    ensure_toolchain_installed_in_isolated_env(world)
}

/// Asserts that the suite library was staged or prebuilt installation succeeded.
#[then("the suite library is staged")]
pub fn then_suite_library_is_staged(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Accept either the build-from-source staging marker or the prebuilt
    // success marker — when prebuilt artefacts are available the installer
    // downloads them instead of building locally.
    let has_local_staging_marker = stderr.contains(STAGING_OUTPUT_MARKER);
    let has_prebuilt_staging_marker = stderr.contains(PREBUILT_INSTALL_MARKER);
    if has_local_staging_marker || has_prebuilt_staging_marker {
        Ok(())
    } else {
        Err(format!(
            "expected '{STAGING_OUTPUT_MARKER}' or '{PREBUILT_INSTALL_MARKER}' in staging output, \
             stderr: {stderr}"
        ))
    }
}

/// Asserts that the installer command failed.
#[then("the CLI exits with an error")]
pub fn then_cli_exits_with_error(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    let output = get_output(world)?;
    if output.status.success() {
        return Err(String::from(
            "expected failure exit code, but command succeeded",
        ));
    }
    Ok(())
}

/// Asserts that stderr reports a toolchain installation failure.
#[then("the error mentions toolchain installation failure")]
pub fn then_error_mentions_install_failure(world: &ToolchainWorld) -> Result<(), String> {
    assert_stderr_contains(world, TOOLCHAIN_ERROR_MARKER, |stderr| {
        format!("expected '{TOOLCHAIN_ERROR_MARKER}' in stderr: {stderr}")
    })
}

/// Asserts that the failure output includes the selected toolchain name.
#[then("the error includes the toolchain name")]
pub fn then_error_includes_toolchain_name(world: &ToolchainWorld) -> Result<(), String> {
    assert_stderr_contains(world, FAKE_TOOLCHAIN, |stderr| {
        format!("expected toolchain name '{FAKE_TOOLCHAIN}' in error output, stderr: {stderr}")
    })
}

/// Asserts that quiet-mode failure output stays within the line-count limit.
#[then("the error output is minimal")]
pub fn then_error_output_is_minimal(world: &ToolchainWorld) -> Result<(), String> {
    skip_if_needed!(world);
    let output = get_output(world)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line_count = stderr.lines().count();
    if line_count <= QUIET_MODE_MAX_LINES {
        Ok(())
    } else {
        Err(format!(
            "expected at most {QUIET_MODE_MAX_LINES} lines in quiet mode, got {line_count}: \
             {stderr}"
        ))
    }
}
