//! Verify the installer binary wires CLI verbosity into bounded PATH tracing.

#![cfg(unix)]

use camino::Utf8PathBuf;
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, Permissions, PermissionsExt},
};
use std::{
    error::Error,
    ffi::OsString,
    io,
    process::{Command, Output},
};

/// Keep an isolated workspace, PATH, and home directories alive for CLI runs.
struct TracingFixture {
    _root: tempfile::TempDir,
    root_path: Utf8PathBuf,
    workspace: Utf8PathBuf,
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
        let root_path = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).map_err(|path| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("temporary fixture path is not UTF-8: {}", path.display()),
            )
        })?;
        let root_dir = Dir::open_ambient_dir(&root_path, ambient_authority())?;
        let workspace = root_path.join("workspace");
        let binary_directory = root_path.join("bin");
        root_dir.create_dir("workspace")?;
        root_dir.create_dir("bin")?;
        root_dir.write("not-a-directory", b"PATH entry is a file")?;
        let workspace_dir = root_dir.open_dir("workspace")?;
        let binary_dir = root_dir.open_dir("bin")?;
        workspace_dir.write(
            "Cargo.toml",
            "[package]\nname = \"whitaker\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )?;
        let cargo_version = dependency_version("cargo-dylint")?;
        let link_version = dependency_version("dylint-link")?;
        write_executable(
            &binary_dir,
            "cargo",
            &fake_cargo_script(cargo_version, link_version),
        )?;
        write_executable(&binary_dir, "dylint-link", "#!/bin/sh\nexit 0\n")?;
        write_executable(
            &binary_dir,
            "rustup",
            "#!/bin/sh\necho 'controlled rustup failure' >&2\nexit 1\n",
        )?;

        for directory in ["data", "config", "cache", "rustup", "cargo-home"] {
            root_dir.create_dir(directory)?;
        }

        let data_home = root_path.join("data");
        let config_home = root_path.join("config");
        let cache_home = root_path.join("cache");
        let rustup_home = root_path.join("rustup");
        let cargo_home = root_path.join("cargo-home");
        let invalid_path_entry = root_path.join("not-a-directory");
        let path = std::env::join_paths([
            invalid_path_entry.as_std_path().as_os_str(),
            binary_directory.as_std_path().as_os_str(),
        ])?;
        let mut environment = vec![
            (OsString::from("PATH"), path),
            (
                OsString::from("HOME"),
                root_path.as_std_path().as_os_str().to_owned(),
            ),
            (
                OsString::from("XDG_DATA_HOME"),
                data_home.as_std_path().as_os_str().to_owned(),
            ),
            (
                OsString::from("XDG_CONFIG_HOME"),
                config_home.as_std_path().as_os_str().to_owned(),
            ),
            (
                OsString::from("XDG_CACHE_HOME"),
                cache_home.as_std_path().as_os_str().to_owned(),
            ),
            (
                OsString::from("RUSTUP_HOME"),
                rustup_home.as_std_path().as_os_str().to_owned(),
            ),
            (
                OsString::from("CARGO_HOME"),
                cargo_home.as_std_path().as_os_str().to_owned(),
            ),
        ];
        if let Some(ld_library_path) = std::env::var_os("LD_LIBRARY_PATH") {
            environment.push((OsString::from("LD_LIBRARY_PATH"), ld_library_path));
        }

        Ok(Self {
            _root: root,
            root_path,
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
        .current_dir(fixture.workspace.as_std_path())
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
    assert!(!stdout.contains(fixture.root_path.as_str()));
    assert!(!stderr.contains(fixture.root_path.as_str()));
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
fn write_executable(directory: &Dir, path: &str, contents: &str) -> io::Result<()> {
    directory.write(path, contents)?;
    directory.set_permissions(path, Permissions::from_mode(0o755))
}
