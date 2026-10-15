//! Verify the installer binary wires CLI verbosity into bounded PATH tracing.

#![cfg(unix)]

use std::{
    error::Error,
    ffi::OsString,
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// Keep an isolated workspace, PATH, and home directories alive for CLI runs.
struct TracingFixture {
    root: tempfile::TempDir,
    workspace: PathBuf,
    environment: Vec<(OsString, OsString)>,
}

impl TracingFixture {
    /// Create temporary binaries and a minimal current-directory workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixture cannot be created or dependency
    /// versions cannot be read from the embedded manifest.
    fn new() -> Result<Self, Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let workspace = root.path().join("workspace");
        let binary_directory = root.path().join("bin");
        let invalid_path_entry = root.path().join("not-a-directory");
        fs::create_dir(&workspace)?;
        fs::create_dir(&binary_directory)?;
        fs::write(&invalid_path_entry, b"PATH entry is a file")?;
        fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"whitaker\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )?;
        let cargo_version = dependency_version("cargo-dylint")?;
        let link_version = dependency_version("dylint-link")?;
        write_executable(
            &binary_directory.join("cargo"),
            &fake_cargo_script(cargo_version, link_version),
        )?;
        write_executable(&binary_directory.join("dylint-link"), "#!/bin/sh\nexit 0\n")?;
        write_executable(
            &binary_directory.join("rustup"),
            "#!/bin/sh\necho 'controlled rustup failure' >&2\nexit 1\n",
        )?;

        let data_home = root.path().join("data");
        let config_home = root.path().join("config");
        let cache_home = root.path().join("cache");
        let rustup_home = root.path().join("rustup");
        let cargo_home = root.path().join("cargo-home");
        for directory in [
            &data_home,
            &config_home,
            &cache_home,
            &rustup_home,
            &cargo_home,
        ] {
            fs::create_dir(directory)?;
        }

        let path = std::env::join_paths([&invalid_path_entry, &binary_directory])?;
        let mut environment = vec![
            (OsString::from("PATH"), path),
            (OsString::from("HOME"), root.path().as_os_str().to_owned()),
            (OsString::from("XDG_DATA_HOME"), data_home.into_os_string()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                config_home.into_os_string(),
            ),
            (
                OsString::from("XDG_CACHE_HOME"),
                cache_home.into_os_string(),
            ),
            (OsString::from("RUSTUP_HOME"), rustup_home.into_os_string()),
            (OsString::from("CARGO_HOME"), cargo_home.into_os_string()),
        ];
        if let Some(ld_library_path) = std::env::var_os("LD_LIBRARY_PATH") {
            environment.push((OsString::from("LD_LIBRARY_PATH"), ld_library_path));
        }

        Ok(Self {
            root,
            workspace,
            environment,
        })
    }
}

/// Exercise the real binary's verbosity parsing and tracing initialization.
///
/// # Errors
///
/// Returns an error if fixture construction or a child process fails.
#[test]
fn main_filters_path_scan_diagnostics_by_cli_verbosity() -> Result<(), Box<dyn Error>> {
    let fixture = TracingFixture::new()?;
    for (mode, arguments, is_verbose) in [
        ("default", &[][..], false),
        ("verbose", &["-v"][..], true),
        ("quiet", &["-q"][..], false),
    ] {
        let output = run_cli_mode(&fixture, arguments)?;
        assert_cli_output(&output, &fixture, mode, is_verbose);
    }
    Ok(())
}

/// Run the installer with an isolated child environment and selected flags.
///
/// # Errors
///
/// Returns an I/O error if the installer process cannot be started or read.
fn run_cli_mode(fixture: &TracingFixture, arguments: &[&str]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_whitaker-installer"))
        .args(["--toolchain", "nightly-test"])
        .args(arguments)
        .current_dir(&fixture.workspace)
        .env_clear()
        .envs(fixture.environment.iter().cloned())
        .output()
}

/// Check the controlled failure and verbosity-filtered diagnostic contract.
fn assert_cli_output(output: &Output, fixture: &TracingFixture, mode: &str, is_verbose: bool) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostic = "skipping PATH scan candidate after lookup failure";

    assert_eq!(
        output.status.code(),
        Some(1),
        "{mode} mode must reach the fake rustup failure; stderr: {stderr}"
    );
    assert!(
        stderr.contains("toolchain nightly-test installation failed: controlled rustup failure"),
        "{mode} mode must reach the controlled failure after dependency checks; stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains(diagnostic),
        is_verbose,
        "{mode} mode stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains("binary_name=\"dylint-link\""),
        is_verbose,
        "{mode} mode stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains("failure_category=\"directory_open\""),
        is_verbose,
        "{mode} mode stderr: {stderr}"
    );
    assert!(!stdout.contains(diagnostic), "diagnostic must be on stderr");
    let fixture_path = fixture.root.path().to_string_lossy();
    assert!(!stdout.contains(fixture_path.as_ref()));
    assert!(!stderr.contains(fixture_path.as_ref()));
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stderr.contains('\u{1b}'));
}

/// Build the fake Cargo responses from the versions required by the installer.
fn fake_cargo_script(cargo_version: &str, link_version: &str) -> String {
    format!(
        concat!(
            "#!/bin/sh\n",
            "case \"$1 $2\" in\n",
            "  \"dylint --version\") echo 'cargo-dylint {}' ;;\n",
            "  \"install --list\") printf '%s\\n' 'cargo-dylint v{}:' ",
            "'dylint-link v{}:' ;;\n",
            "  *) echo 'unexpected fake cargo invocation' >&2; exit 97 ;;\n",
            "esac\n",
        ),
        cargo_version, cargo_version, link_version,
    )
}

/// Return a dependency binary's version from the embedded manifest.
///
/// # Errors
///
/// Returns an error if the manifest cannot be parsed or omits the package.
fn dependency_version(package: &str) -> Result<&'static str, Box<dyn Error>> {
    let dependency = whitaker_installer::dependency_binaries::find_dependency_binary(package)?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("embedded dependency manifest must include {package}"),
            )
        })?;
    Ok(dependency.version())
}

/// Write a Unix script that can be found and run through the isolated PATH.
///
/// # Errors
///
/// Returns a filesystem error if the script or its executable permissions
/// cannot be written.
fn write_executable(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
