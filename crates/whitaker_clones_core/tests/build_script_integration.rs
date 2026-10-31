//! Exercises the parser-pin build script through isolated Cargo workspaces.

use std::{
    env,
    error::Error,
    ffi::OsStr,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest::{fixture, rstest};
use tempfile::tempdir_in;

const FIXTURE_PACKAGE: &str = "parser_pin_build_script_fixture";

struct BuildFixture {
    _directory: tempfile::TempDir,
    manifest_path: Utf8PathBuf,
    target_dir: Utf8PathBuf,
}

#[rstest]
fn build_script_accepts_an_exact_workspace_parser_pin(
    #[with(Some("=0.0.334"))] build_fixture: Result<BuildFixture, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture?;
    let output = cargo_check(&fixture)?;

    if !output.status.success() {
        return Err(format!(
            "exact parser pin must pass the build script:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let run_output = cargo_run(&fixture)?;
    if !run_output.status.success() {
        return Err(format!(
            "parser-version fixture must run:\n{}",
            String::from_utf8_lossy(&run_output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8(run_output.stdout)?;
    if stdout.trim() != "0.0.334" {
        return Err(format!(
            "parser-version fixture must print 0.0.334, printed `{}`",
            stdout.trim()
        )
        .into());
    }
    Ok(())
}

/// Reject missing, loose, partial, and ranged workspace parser pins.
#[rstest]
#[case::loose(Some("0.0.334"), "must be exact-pinned")]
#[case::missing(None, "workspace dependency `ra_ap_syntax` is missing")]
#[case::partial_major(Some("=1"), "must be exact-pinned")]
#[case::partial_minor(Some("=1.2"), "must be exact-pinned")]
#[case::additional_comparator(Some("=1.2.3, <1.2.4"), "must be exact-pinned")]
fn build_script_rejects_invalid_parser_pins(
    #[case] requirement: Option<&str>,
    #[case] expected_error: &str,
) -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture(requirement)?;
    let output = cargo_check(&fixture)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let pin_description = requirement.map_or_else(
        || "missing parser pin".to_owned(),
        |version| format!("parser pin `{version}`"),
    );

    if output.status.success() {
        return Err(format!("{pin_description} must fail the build script").into());
    }
    if !stderr.contains(expected_error) {
        return Err(format!(
            "build-script failure for {pin_description} must explain the parser-pin rule:\n{stderr}"
        )
        .into());
    }
    Ok(())
}

/// Build a temporary crate with the requested parser dependency declaration.
///
/// # Errors
///
/// Returns an error if the fixture directory cannot be prepared or its files
/// cannot be written.
#[fixture]
fn build_fixture(
    #[default(None)] requirement: Option<&str>,
) -> Result<BuildFixture, Box<dyn Error>> {
    let directory = parser_pin_fixture_directory()?;
    let fixture_root =
        Utf8Path::from_path(directory.path()).ok_or("temporary path is not valid UTF-8")?;
    let manifest_path = fixture_root.join("fixture").join("Cargo.toml");
    let target_dir = fixture_root.join("target");

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
                "semver = \"1.0.28\"\n",
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
        target_dir,
    })
}

/// Allocate an isolated parser-pin fixture beneath the workspace target tree.
///
/// Keep nested Cargo output out of the shared system temporary filesystem while
/// preserving a distinct directory for each fixture.
///
/// # Errors
///
/// Returns an error if the workspace root cannot be found or the fixture
/// directory cannot be created.
fn parser_pin_fixture_directory() -> Result<tempfile::TempDir, Box<dyn Error>> {
    let crate_directory = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_directory
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or("workspace root not found above CARGO_MANIFEST_DIR")?;
    let workspace = Dir::open_ambient_dir(workspace_root, ambient_authority())?;
    workspace.create_dir_all("target/tests/build_script_integration")?;
    Ok(tempdir_in(
        workspace_root.join("target/tests/build_script_integration"),
    )?)
}

fn cargo_check(fixture: &BuildFixture) -> Result<Output, Box<dyn Error>> {
    Ok(cargo_command("check", fixture).output()?)
}

fn cargo_run(fixture: &BuildFixture) -> Result<Output, Box<dyn Error>> {
    Ok(cargo_command("run", fixture).output()?)
}

fn cargo_command(command: &str, fixture: &BuildFixture) -> Command {
    let mut cargo = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .arg(command)
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&fixture.manifest_path)
        // Each fixture owns a workspace with the same temporary package name.
        // Keep Cargo outputs separate so concurrent checks cannot reuse another
        // fixture's build-script result through an inherited coverage target.
        .arg("--target-dir")
        .arg(&fixture.target_dir);
    cargo
}

#[test]
fn cargo_command_scopes_build_output_to_its_fixture() -> Result<(), Box<dyn Error>> {
    let fixture = build_fixture(Some("=0.0.334"))?;
    let command = cargo_command("check", &fixture);
    let arguments: Vec<_> = command.get_args().collect();

    assert!(arguments.windows(2).any(|pair| {
        pair == [
            OsStr::new("--target-dir"),
            fixture.target_dir.as_std_path().as_os_str(),
        ]
    }));
    Ok(())
}
