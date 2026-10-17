//! Behaviour-driven tests for the root `whitaker` command.

use std::{
    cell::{Ref, RefCell},
    path::PathBuf,
    process::{Command, Output},
};
#[cfg(all(unix, not(target_os = "macos")))]
use std::{error::Error, path::Path};

#[cfg(all(unix, not(target_os = "macos")))]
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, Permissions, PermissionsExt},
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use whitaker_installer::builder::{library_extension, library_prefix};
#[cfg(all(unix, not(target_os = "macos")))]
use whitaker_installer::{dirs::BaseDirs, installation_record::record_installation};

#[derive(Default)]
struct CliWorld {
    output: RefCell<Option<Output>>,
    staging: RefCell<Option<TempDir>>,
    fake_cargo: RefCell<Option<TempDir>>,
}

impl CliWorld {
    fn recorded_output(&self) -> Result<Ref<'_, Output>, String> {
        Ref::filter_map(self.output.borrow(), Option::as_ref)
            .map_err(|_| "command output should be recorded".to_owned())
    }

    fn staging_directory(&self) -> Result<Ref<'_, TempDir>, String> {
        Ref::filter_map(self.staging.borrow(), Option::as_ref)
            .map_err(|_| "install target should be recorded".to_owned())
    }
}

#[whitaker_test_macros::allow_fixture_expansion_lints]
#[fixture]
fn world() -> CliWorld {
    CliWorld::default()
}

#[given("the whitaker binary is available")]
fn given_binary_available(world: &CliWorld) {
    let _ = world;
}

#[given("a Whitaker workspace checkout")]
fn given_workspace_checkout(world: &CliWorld) -> Result<(), String> {
    let staging = TempDir::new()
        .map_err(|error| format!("temporary install target should be created: {error}"))?;
    world.staging.replace(Some(staging));
    Ok(())
}

#[cfg(unix)]
#[given("a fake cargo dylint command returning exit code 23")]
fn given_fake_cargo(world: &CliWorld) -> Result<(), String> {
    use cap_std::{
        ambient_authority,
        fs_utf8::{Dir, Permissions, PermissionsExt},
    };

    let directory = TempDir::new()
        .map_err(|error| format!("fake cargo directory should be created: {error}"))?;
    let directory_handle = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(directory.path())
            .ok_or_else(|| "fake cargo directory path should be UTF-8".to_owned())?,
        ambient_authority(),
    )
    .map_err(|error| format!("fake cargo directory should be opened: {error}"))?;
    directory_handle
        .write("cargo", "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 23\n")
        .map_err(|error| format!("fake cargo command should be written: {error}"))?;
    directory_handle
        .set_permissions("cargo", Permissions::from_mode(0o755))
        .map_err(|error| format!("fake cargo should be executable: {error}"))?;
    world.fake_cargo.replace(Some(directory));
    Ok(())
}

#[given("a staging directory containing a staged suite library")]
fn given_staged_suite(world: &CliWorld) -> Result<(), String> {
    use cap_std::{ambient_authority, fs_utf8::Dir};

    let staging = TempDir::new()
        .map_err(|error| format!("temporary staging directory should be created: {error}"))?;
    let staging_dir = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(staging.path())
            .ok_or_else(|| "temporary staging path should be UTF-8".to_owned())?,
        ambient_authority(),
    )
    .map_err(|error| format!("temporary staging directory should be opened: {error}"))?;
    staging_dir
        .create_dir_all("nightly-test/release")
        .map_err(|error| format!("staged release directory should be created: {error}"))?;
    let library_name = format!(
        "{}whitaker_suite@nightly-test{}",
        library_prefix(),
        library_extension()
    );
    staging_dir
        .write(format!("nightly-test/release/{library_name}"), [])
        .map_err(|error| format!("staged suite marker should be written: {error}"))?;
    world.staging.replace(Some(staging));
    Ok(())
}

#[when("I run whitaker with \"{arguments}\"")]
fn when_run_whitaker(world: &CliWorld, arguments: &str) -> Result<(), String> {
    let parsed_arguments: Vec<&str> = arguments.split_ascii_whitespace().collect();
    let mut command = Command::new(env!("CARGO_BIN_EXE_whitaker"));
    command.args(parsed_arguments);
    if let Some(fake_cargo) = world.fake_cargo.borrow().as_ref() {
        command.env("PATH", fake_cargo.path());
    }
    if let Some(staging) = world.staging.borrow().as_ref() {
        let target_dir = staging
            .path()
            .to_str()
            .ok_or_else(|| "temporary path should be UTF-8".to_owned())?;
        command.args(["--target-dir", target_dir]);
    }
    let output = command
        .current_dir(workspace_root()?)
        .output()
        .map_err(|error| format!("whitaker process should start: {error}"))?;
    world.output.replace(Some(output));
    Ok(())
}

#[then("the command succeeds")]
fn then_command_succeeds(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "command should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[then("the command fails")]
fn then_command_fails(world: &CliWorld) -> Result<(), String> {
    if world.recorded_output()?.status.success() {
        Err("command should fail".to_owned())
    } else {
        Ok(())
    }
}

#[then("the exit code is {expected:i32}")]
fn then_exit_code(world: &CliWorld, expected: i32) -> Result<(), String> {
    let actual = world.recorded_output()?.status.code();
    if actual == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "expected process exit code {expected}, found {actual:?}"
        ))
    }
}

#[then("the output lists the subcommand \"{subcommand}\"")]
fn then_output_lists_subcommand(world: &CliWorld, subcommand: &str) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains(subcommand) {
        Ok(())
    } else {
        Err(format!("help output should list {subcommand}"))
    }
}

#[then("the output is valid JSON")]
fn then_output_is_json(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let _json_value = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("ls --json stdout should contain valid JSON: {error}"))?;
    Ok(())
}

#[then("stdout contains no diagnostic text")]
fn then_json_has_no_diagnostics(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.starts_with('{') {
        Ok(())
    } else {
        Err("diagnostics must not precede JSON stdout".to_owned())
    }
}

#[then("no lint library is staged")]
fn then_no_lint_library_is_staged(world: &CliWorld) -> Result<(), String> {
    use cap_std::{ambient_authority, fs_utf8::Dir};

    let staging = world.staging_directory()?;
    let staging_dir = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(staging.path())
            .ok_or_else(|| "temporary staging path should be UTF-8".to_owned())?,
        ambient_authority(),
    )
    .map_err(|error| format!("temporary staging directory should be opened: {error}"))?;
    let entries = staging_dir
        .read_dir(".")
        .map_err(|error| format!("temporary staging directory should be readable: {error}"))?;
    if entries.count() > 0 {
        Err("dry-run should leave the staging directory empty".to_owned())
    } else {
        Ok(())
    }
}

#[then("stdout is empty")]
fn then_stdout_is_empty(world: &CliWorld) -> Result<(), String> {
    if world.recorded_output()?.stdout.is_empty() {
        Ok(())
    } else {
        Err("install diagnostics belong on stderr".to_owned())
    }
}

#[then("stderr contains install diagnostics")]
fn then_stderr_contains_install_diagnostics(world: &CliWorld) -> Result<(), String> {
    if world.recorded_output()?.stderr.is_empty() {
        Err("install progress and diagnostics should be written to stderr".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
#[then("the invocation reaches cargo dylint")]
fn then_invocation_reaches_cargo_dylint(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    if output.status.code() != Some(23) {
        return Err("forwarded status should be preserved".to_owned());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.starts_with("dylint\n--all\n--\n-p\nwhitaker-common\n--all-targets\n") {
        Ok(())
    } else {
        Err(format!(
            "cargo dylint should receive the original argument vector: {stdout:?}"
        ))
    }
}

#[then("the output names the staged suite")]
fn then_output_names_staged_suite(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("whitaker_suite") {
        Ok(())
    } else {
        Err("staged suite should be listed".to_owned())
    }
}

#[then("no wrapper-generation message is reported")]
fn then_no_wrapper_generation_message_is_reported(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Wrapper scripts created") {
        Err("dry-run should not report wrapper generation".to_owned())
    } else {
        Ok(())
    }
}

#[then("the error names both conflicting options")]
fn then_error_names_conflicts(world: &CliWorld) -> Result<(), String> {
    let output = world.recorded_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.contains("--lint") {
        return Err("error should mention --lint".to_owned());
    }
    if !stderr.contains("--individual-lints") {
        return Err("error should mention --individual-lints".to_owned());
    }
    Ok(())
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 0)]
fn scenario_root_command_lists_subcommands(world: CliWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 1)]
fn scenario_help_exits_successfully(world: CliWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 2)]
fn scenario_lists_staged_lints_as_json(world: CliWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 3)]
fn scenario_lists_staged_lints_as_text(world: CliWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 4)]
fn scenario_dry_run_has_no_stdout_or_wrapper(world: CliWorld) {
    let _ = world;
}

#[scenario(path = "tests/features/whitaker_cli.feature", index = 5)]
fn scenario_conflicting_lint_flags_are_rejected(world: CliWorld) {
    let _ = world;
}

#[cfg(unix)]
#[scenario(path = "tests/features/whitaker_cli.feature", index = 6)]
fn scenario_legacy_lint_invocations_are_forwarded(world: CliWorld) {
    let _ = world;
}

fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "root package directory should have a parent".to_owned())
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn forwarding_uses_all_recorded_custom_paths_outside_a_checkout() {
    let temporary = TempDir::new().expect("temporary directory should be created");
    let fixture = recorded_path_fixture(&temporary).expect("recorded-path fixture should exist");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .arg("--all")
        .env("PATH", &fixture.fake_cargo)
        .env_remove("DYLINT_LIBRARY_PATH")
        .env("XDG_DATA_HOME", &fixture.data_home)
        .env("HOME", temporary.path())
        .current_dir(&fixture.external_cwd)
        .output()
        .expect("root CLI should start outside a Whitaker checkout");

    assert!(output.status.success());
    let path_list = String::from_utf8(output.stdout).expect("fake cargo output should be UTF-8");
    let forwarded_paths: Vec<_> = std::env::split_paths(path_list.trim()).collect();
    assert!(forwarded_paths.contains(&fixture.stable));
    assert!(forwarded_paths.contains(&fixture.nightly));
    assert!(!fixture.external_cwd.join("rust-toolchain.toml").exists());
}

#[cfg(all(unix, not(target_os = "macos")))]
struct TestBaseDirs {
    data: PathBuf,
}

#[cfg(all(unix, not(target_os = "macos")))]
impl BaseDirs for TestBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}

#[cfg(all(unix, not(target_os = "macos")))]
struct RecordedPathFixture {
    data_home: PathBuf,
    stable: PathBuf,
    nightly: PathBuf,
    external_cwd: PathBuf,
    fake_cargo: PathBuf,
}

#[cfg(all(unix, not(target_os = "macos")))]
fn recorded_path_fixture(temporary: &TempDir) -> Result<RecordedPathFixture, Box<dyn Error>> {
    let test_root = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(temporary.path())
            .ok_or_else(|| std::io::Error::other("temporary path should be UTF-8"))?,
        ambient_authority(),
    )?;
    create_fixture_directories(&test_root)?;

    let fixture = RecordedPathFixture {
        data_home: temporary.path().join("xdg-data"),
        stable: temporary.path().join("custom-stable"),
        nightly: temporary.path().join("custom-nightly"),
        external_cwd: temporary.path().join("external-project"),
        fake_cargo: temporary.path().join("fake-cargo"),
    };
    record_fixture_toolchains(&fixture)?;
    create_fake_cargo(&fixture.fake_cargo)?;
    Ok(fixture)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn create_fixture_directories(root: &Dir) -> std::io::Result<()> {
    for directory in [
        "xdg-data",
        "custom-stable",
        "custom-nightly",
        "external-project",
        "fake-cargo",
    ] {
        root.create_dir_all(directory)?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn record_fixture_toolchains(fixture: &RecordedPathFixture) -> Result<(), Box<dyn Error>> {
    let dirs = TestBaseDirs {
        data: fixture.data_home.join("whitaker"),
    };
    for (toolchain, path) in [("stable", &fixture.stable), ("nightly", &fixture.nightly)] {
        let staging_path = camino::Utf8Path::from_path(path)
            .ok_or_else(|| std::io::Error::other("staging path should be UTF-8"))?;
        record_installation(&dirs, toolchain, staging_path)?;
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn create_fake_cargo(directory: &Path) -> Result<(), Box<dyn Error>> {
    let path = camino::Utf8Path::from_path(directory)
        .ok_or_else(|| std::io::Error::other("fake cargo path should be UTF-8"))?;
    let fake_cargo_dir = Dir::open_ambient_dir(path, ambient_authority())?;
    fake_cargo_dir.write(
        "cargo",
        "#!/bin/sh\nprintf '%s\\n' \"$DYLINT_LIBRARY_PATH\"\n",
    )?;
    fake_cargo_dir.set_permissions("cargo", Permissions::from_mode(0o755))?;
    Ok(())
}
