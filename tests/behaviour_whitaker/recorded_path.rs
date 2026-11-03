//! Tests recorded toolchain path forwarding from outside a workspace.

use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, Permissions, PermissionsExt},
};
use tempfile::TempDir;
use whitaker_installer::{dirs::BaseDirs, installation_record::record_installation};

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

struct TestBaseDirs {
    data: PathBuf,
}

impl BaseDirs for TestBaseDirs {
    fn home(&self) -> Option<PathBuf> { None }

    fn executables(&self) -> Option<PathBuf> { None }

    fn whitaker_data(&self) -> Option<PathBuf> { Some(self.data.clone()) }
}

struct RecordedPathFixture {
    data_home: PathBuf,
    stable: PathBuf,
    nightly: PathBuf,
    external_cwd: PathBuf,
    fake_cargo: PathBuf,
}

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
