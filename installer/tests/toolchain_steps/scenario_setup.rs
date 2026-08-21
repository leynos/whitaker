//! Scenario setup helpers and shared world state for toolchain behaviour tests.
//!
//! The step definitions in the parent module drive these helpers; keeping them
//! here keeps each module within the repository's size budget.

use std::{
    cell::{Cell, RefCell},
    io::Write as _,
    process::Output,
};

use tempfile::TempDir;

use crate::support::{
    is_toolchain_installed,
    is_toolchain_installed_in_env,
    pinned_toolchain_channel,
    setup_isolated_rustup,
};

/// Non-existent toolchain channel used to exercise auto-install failure paths.
pub const FAKE_TOOLCHAIN: &str = "nonexistent-nightly-2024-01-01";

#[derive(Default)]
pub struct ToolchainWorld {
    pub args: RefCell<Vec<String>>,
    pub output: RefCell<Option<Output>>,
    pub should_skip_assertions: Cell<bool>,
    pub temp_dir: RefCell<Option<TempDir>>,
    pub rustup_home: RefCell<Option<TempDir>>,
    pub cargo_home: RefCell<Option<TempDir>>,
    pub pinned_channel: RefCell<String>,
}

/// Borrows the captured CLI output, failing when no command has run yet.
pub(super) fn get_output(world: &ToolchainWorld) -> Result<std::cell::Ref<'_, Output>, String> {
    let output = world.output.borrow();
    std::cell::Ref::filter_map(output, Option::as_ref)
        .map_err(|_| String::from("CLI output not set; run the installer step first"))
}

pub(super) fn get_combined_output_string(world: &ToolchainWorld) -> Result<String, String> {
    let output = get_output(world)?;
    Ok(format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    ))
}

pub(super) fn get_stderr_string(world: &ToolchainWorld) -> Result<String, String> {
    let output = get_output(world)?;
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

/// Reports a skipped scenario on stderr without tripping `print_stderr`.
fn report_skip(reason: &str) -> Result<(), String> {
    writeln!(std::io::stderr(), "{reason}")
        .map_err(|error| format!("failed to report skipped scenario: {error}"))
}

fn skip_scenario_when_toolchain_missing(
    world: &ToolchainWorld,
    channel: &str,
) -> Result<(), String> {
    if !is_toolchain_installed(channel) {
        report_skip(&format!(
            "Skipping scenario: toolchain '{channel}' not installed."
        ))?;
        world.should_skip_assertions.set(true);
        rstest_bdd::skip!("toolchain '{channel}' is not installed.", channel = channel);
    }
    Ok(())
}

fn setup_temp_dir(world: &ToolchainWorld) -> Result<String, String> {
    let temp_dir = TempDir::new().map_err(|error| format!("failed to create temp dir: {error}"))?;
    let target_dir = temp_dir.path().to_string_lossy().to_string();
    world.temp_dir.replace(Some(temp_dir));
    Ok(target_dir)
}

pub(super) fn setup_dry_run_scenario(
    world: &ToolchainWorld,
    extra_args: &[&str],
) -> Result<(), String> {
    let channel = pinned_toolchain_channel()?;
    skip_scenario_when_toolchain_missing(world, &channel)?;
    world.pinned_channel.replace(channel.clone());

    let target_dir = setup_temp_dir(world)?;
    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
    Ok(())
}

/// Prepares an isolated rustup environment and CLI arguments for an install run.
///
/// # Errors
///
/// Returns an error when the isolated environment or temporary directory
/// cannot be created, or the pinned toolchain cannot be resolved.
pub fn setup_install_scenario(world: &ToolchainWorld, extra_args: &[&str]) -> Result<(), String> {
    let env = setup_isolated_rustup()?;
    world.rustup_home.replace(Some(env.rustup_home));
    world.cargo_home.replace(Some(env.cargo_home));
    world.pinned_channel.replace(pinned_toolchain_channel()?);

    let target_dir = setup_temp_dir(world)?;
    let mut args: Vec<String> = extra_args.iter().map(|s| (*s).to_owned()).collect();
    args.extend(["--target-dir".to_owned(), target_dir]);
    world.args.replace(args);
    Ok(())
}

pub(super) fn setup_failure_scenario(
    world: &ToolchainWorld,
    extra_args: &[&str],
) -> Result<(), String> {
    // Use isolated rustup environment so the install failure path is exercised
    // without affecting the host system.
    let env = setup_isolated_rustup()?;
    world.rustup_home.replace(Some(env.rustup_home));
    world.cargo_home.replace(Some(env.cargo_home));

    let target_dir = setup_temp_dir(world)?;
    // Filter out --dry-run to exercise the real install path
    let mut args: Vec<String> = extra_args
        .iter()
        .filter(|s| **s != "--dry-run")
        .map(|s| (*s).to_owned())
        .collect();
    args.extend([
        "--toolchain".to_owned(),
        FAKE_TOOLCHAIN.to_owned(),
        "--target-dir".to_owned(),
        target_dir,
        "--skip-deps".to_owned(),
    ]);
    world.args.replace(args);
    Ok(())
}

pub(super) fn ensure_toolchain_installed_in_isolated_env(
    world: &ToolchainWorld,
) -> Result<(), String> {
    let rustup_home = world.rustup_home.borrow();
    let cargo_home = world.cargo_home.borrow();
    let (Some(rustup), Some(cargo)) = (rustup_home.as_ref(), cargo_home.as_ref()) else {
        return Err(String::from(
            "isolated rustup environment must be configured for install scenario",
        ));
    };
    let channel = pinned_toolchain_channel()?;
    if is_toolchain_installed_in_env(&channel, rustup, cargo) {
        Ok(())
    } else {
        Err(format!(
            "toolchain '{channel}' was not installed in isolated environment"
        ))
    }
}

pub(super) fn setup_auto_install_scenario(world: &ToolchainWorld) -> Result<(), String> {
    // Skip auto-install tests on Windows - toolchain downloads are extremely slow
    // due to Windows Defender scanning and larger binaries. The code path is
    // identical to Linux; we're testing rustup behaviour rather than installer logic.
    if cfg!(windows) {
        report_skip("Skipping auto-install scenario on Windows (toolchain downloads too slow).")?;
        world.should_skip_assertions.set(true);
        rstest_bdd::skip!("auto-install tests skipped on Windows");
    }
    // Use --skip-wrapper to prevent writing to the user's real ~/.local/bin.
    setup_install_scenario(world, &["--jobs", "1", "--skip-deps", "--skip-wrapper"])
}
