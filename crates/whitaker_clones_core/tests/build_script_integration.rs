//! Exercises the parser-pin build script through isolated Cargo workspaces.

use std::{
    env,
    error::Error,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use tempfile::tempdir;

const FIXTURE_PACKAGE: &str = "parser_pin_build_script_fixture";

struct BuildFixture {
    _directory: tempfile::TempDir,
    manifest_path: Utf8PathBuf,
}

#[rstest]
fn build_script_accepts_an_exact_workspace_parser_pin(
    #[with(Some("=0.0.334"))] build_fixture: Result<BuildFixture, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture?;
    let output = cargo_check(&fixture.manifest_path)?;

    assert!(
        output.status.success(),
        "exact parser pin should pass the build script:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run_output = cargo_run(&fixture.manifest_path)?;
    assert!(
        run_output.status.success(),
        "parser-version fixture should run:\n{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_eq!(String::from_utf8(run_output.stdout)?.trim(), "0.0.334");
    Ok(())
}

#[rstest]
fn build_script_rejects_a_loose_workspace_parser_pin(
    #[with(Some("0.0.334"))] build_fixture: Result<BuildFixture, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture?;
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

#[rstest]
fn build_script_rejects_a_missing_workspace_parser_pin(
    #[with(None)] build_fixture: Result<BuildFixture, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture?;
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

#[fixture]
fn build_fixture(
    #[default(None)] requirement: Option<&str>,
) -> Result<BuildFixture, Box<dyn Error>> {
    let directory = tempdir()?;
    let fixture_root =
        Utf8Path::from_path(directory.path()).ok_or("temporary path is not valid UTF-8")?;
    let manifest_path = fixture_root.join("fixture").join("Cargo.toml");

    // Capability-scope every write to the fixture tree rather than reaching for
    // ambient `std::fs`. The tempdir handle keeps the directory alive; `root`
    // can only touch paths beneath it.
    let root = Dir::open_ambient_dir(fixture_root, ambient_authority())?;
    root.create_dir_all("fixture/src")?;

    let parser_dependency = requirement.map_or_else(String::new, |version| {
        format!("ra_ap_syntax = \"{version}\"\n")
    });
    root.write(
        "Cargo.toml",
        // `format!` cannot capture variables when the format string comes from a
        // macro such as `concat!`, so the interpolations are passed explicitly.
        format!(
            concat!(
                "[workspace]\n",
                "members = [\"fixture\"]\n",
                "resolver = \"2\"\n",
                "\n",
                "[workspace.dependencies]\n",
                "{parser_dependency}",
            ),
            parser_dependency = parser_dependency,
        ),
    )?;
    root.write(
        "fixture/Cargo.toml",
        format!(
            concat!(
                "[package]\n",
                "name = \"{FIXTURE_PACKAGE}\"\n",
                "version = \"0.0.0\"\n",
                "edition = \"2024\"\n",
                "publish = false\n",
                "build = \"build.rs\"\n",
                "\n",
                "[build-dependencies]\n",
                "camino = \"1.2.1\"\n",
                "cap-std = {{ version = \"4.0.2\", features = [\"fs_utf8\"] }}\n",
                "toml = \"1.1.2\"\n",
            ),
            FIXTURE_PACKAGE = FIXTURE_PACKAGE,
        ),
    )?;
    root.write(
        "fixture/src/lib.rs",
        "pub const PARSER_VERSION: &str = env!(\"WHITAKER_RA_AP_SYNTAX_VERSION\");\n",
    )?;
    root.write(
        "fixture/src/main.rs",
        format!("fn main() {{ println!(\"{{}}\", {FIXTURE_PACKAGE}::PARSER_VERSION); }}\n"),
    )?;

    // Copy the build script and its support module out of this crate's source
    // tree through a second capability-scoped directory handle.
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())?;
    root.write("fixture/build.rs", crate_root.read_to_string("build.rs")?)?;
    root.write(
        "fixture/build_support.rs",
        crate_root.read_to_string("build_support.rs")?,
    )?;

    Ok(BuildFixture {
        _directory: directory,
        manifest_path,
    })
}

fn cargo_check(manifest_path: &Utf8Path) -> Result<Output, Box<dyn Error>> {
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

fn cargo_run(manifest_path: &Utf8Path) -> Result<Output, Box<dyn Error>> {
    Ok(
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .arg("run")
            .arg("--offline")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(manifest_path)
            .output()?,
    )
}
