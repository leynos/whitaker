//! Exercises the parser-pin build script through isolated Cargo workspaces.

use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::tempdir;

const FIXTURE_PACKAGE: &str = "parser_pin_build_script_fixture";

struct BuildFixture {
    _directory: tempfile::TempDir,
    manifest_path: PathBuf,
}

#[test]
fn build_script_accepts_an_exact_workspace_parser_pin() -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture(Some("=0.0.334"))?;
    let output = cargo_check(&fixture.manifest_path)?;

    assert!(
        output.status.success(),
        "exact parser pin should pass the build script:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn build_script_rejects_a_loose_workspace_parser_pin() -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture(Some("0.0.334"))?;
    let output = cargo_check(&fixture.manifest_path)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "loose parser pin should fail the build script"
    );
    assert!(
        stderr.contains("must be exact-pinned"),
        "build-script failure should explain the parser-pin rule:\n{stderr}"
    );
    Ok(())
}

#[test]
fn build_script_rejects_a_missing_workspace_parser_pin() -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture(None)?;
    let output = cargo_check(&fixture.manifest_path)?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "missing parser pin should fail the build script"
    );
    assert!(
        stderr.contains("workspace dependency `ra_ap_syntax` is missing"),
        "build-script failure should explain the missing parser pin:\n{stderr}"
    );
    Ok(())
}

fn build_fixture(requirement: Option<&str>) -> Result<BuildFixture, Box<dyn Error>> {
    let directory = tempdir()?;
    let fixture_root = directory.path();
    let package_dir = fixture_root.join("fixture");
    let manifest_path = package_dir.join("Cargo.toml");

    fs::create_dir_all(package_dir.join("src"))?;
    let parser_dependency = requirement.map_or_else(String::new, |version| {
        format!("ra_ap_syntax = \"{version}\"\n")
    });
    fs::write(
        fixture_root.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [\"fixture\"]\nresolver = \"2\"\n\n\
             [workspace.dependencies]\n{parser_dependency}"
        ),
    )?;
    fs::write(
        &manifest_path,
        format!(
            "[package]\nname = \"{FIXTURE_PACKAGE}\"\nversion = \"0.0.0\"\n\
             edition = \"2024\"\npublish = false\nbuild = \"build.rs\"\n\n\
             [build-dependencies]\ntoml = \"1.1.2\"\n"
        ),
    )?;
    fs::write(
        package_dir.join("src/lib.rs"),
        "pub const PARSER_VERSION: &str = env!(\"WHITAKER_RA_AP_SYNTAX_VERSION\");\n",
    )?;

    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::copy(crate_root.join("build.rs"), package_dir.join("build.rs"))?;
    fs::copy(
        crate_root.join("build_support.rs"),
        package_dir.join("build_support.rs"),
    )?;

    Ok(BuildFixture {
        _directory: directory,
        manifest_path,
    })
}

fn cargo_check(manifest_path: &Path) -> Result<Output, Box<dyn Error>> {
    Ok(
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .arg("check")
            .arg("--offline")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(manifest_path)
            .output()?,
    )
}
