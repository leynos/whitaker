//! Verify the installer binary wires CLI verbosity into bounded PATH tracing.

#![cfg(unix)]

use camino::{Utf8Path, Utf8PathBuf};
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

/// Select one supported installer CLI verbosity mode for the fixture.
#[derive(Clone, Copy)]
enum CliMode {
    Default,
    Verbose,
    Quiet,
}

impl CliMode {
    /// Return the arguments that select this mode.
    fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Default => &[],
            Self::Verbose => &["-v"],
            Self::Quiet => &["-q"],
        }
    }

    /// Return the short label used in assertion messages.
    fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Verbose => "verbose",
            Self::Quiet => "quiet",
        }
    }

    /// Report whether this mode enables debug diagnostics.
    fn is_verbose(self) -> bool {
        matches!(self, Self::Verbose)
    }
}

/// Identify one pinned installer dependency without passing package strings.
#[derive(Clone, Copy)]
enum DependencyPackage {
    CargoDylint,
    DylintLink,
}

impl DependencyPackage {
    /// Return this dependency's embedded manifest key.
    fn manifest_name(self) -> &'static str {
        match self {
            Self::CargoDylint => "cargo-dylint",
            Self::DylintLink => "dylint-link",
        }
    }
}

/// Hold the pinned versions used to synthesize fake Cargo output.
struct DependencyVersions {
    cargo_dylint: &'static str,
    dylint_link: &'static str,
}

/// Select a fake executable whose behaviour is controlled by this fixture.
enum FixtureExecutable {
    Cargo(DependencyVersions),
    DylintLink,
    Rustup,
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
        create_workspace(&root_dir)?;
        create_binaries(&root_dir)?;
        let environment = create_environment(&root_dir, &root_path, &binary_directory)?;

        Ok(Self {
            _root: root,
            root_path,
            workspace,
            environment,
        })
    }
}

/// Create the minimal workspace passed to the installer child process.
///
/// # Errors
///
/// Returns a filesystem error if the workspace directory or manifest cannot
/// be created.
fn create_workspace(root_dir: &Dir) -> io::Result<()> {
    root_dir.create_dir("workspace")?;
    root_dir.open_dir("workspace")?.write(
        "Cargo.toml",
        "[package]\nname = \"whitaker\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
}

/// Create fake dependency binaries and the controlled PATH file entry.
///
/// # Errors
///
/// Returns an error if dependency versions cannot be read or fixture files
/// cannot be created.
fn create_binaries(root_dir: &Dir) -> Result<(), Box<dyn Error>> {
    root_dir.create_dir("bin")?;
    root_dir.write("not-a-directory", b"PATH entry is a file")?;
    let binary_dir = root_dir.open_dir("bin")?;
    let versions = DependencyVersions {
        cargo_dylint: dependency_version(DependencyPackage::CargoDylint)?,
        dylint_link: dependency_version(DependencyPackage::DylintLink)?,
    };
    write_executable(&binary_dir, FixtureExecutable::Cargo(versions))?;
    write_executable(&binary_dir, FixtureExecutable::DylintLink)?;
    write_executable(&binary_dir, FixtureExecutable::Rustup)?;
    Ok(())
}

/// Assemble isolated paths and environment variables for child invocations.
///
/// # Errors
///
/// Returns an error if a home directory cannot be created or the child PATH
/// cannot be assembled.
fn create_environment(
    root_dir: &Dir,
    root_path: &Utf8Path,
    binary_directory: &Utf8Path,
) -> Result<Vec<(OsString, OsString)>, Box<dyn Error>> {
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

    Ok(environment)
}

/// Exercise the real binary's verbosity parsing and tracing initialization.
///
/// # Errors
///
/// Returns an error if fixture construction or a child process fails.
#[test]
fn main_filters_path_scan_diagnostics_by_cli_verbosity() -> Result<(), Box<dyn Error>> {
    let fixture = TracingFixture::new()?;
    for mode in [CliMode::Default, CliMode::Verbose, CliMode::Quiet] {
        let output = run_cli_mode(&fixture, mode)?;
        assert_cli_output(&output, &fixture, mode);
    }
    Ok(())
}

/// Run the installer with an isolated child environment and selected flags.
///
/// # Errors
///
/// Returns an I/O error if the installer process cannot be started or read.
fn run_cli_mode(fixture: &TracingFixture, mode: CliMode) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_whitaker-installer"))
        .args(["--toolchain", "nightly-test"])
        .args(mode.arguments())
        .current_dir(fixture.workspace.as_std_path())
        .env_clear()
        .envs(fixture.environment.iter().cloned())
        .output()
}

/// Check the controlled failure and verbosity-filtered diagnostic contract.
fn assert_cli_output(output: &Output, fixture: &TracingFixture, mode: CliMode) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostic = "skipping PATH scan candidate after lookup failure";
    let mode_label = mode.label();
    let is_verbose = mode.is_verbose();

    assert_eq!(
        output.status.code(),
        Some(1),
        "{mode_label} mode must reach the fake rustup failure; stderr: {stderr}"
    );
    assert!(
        stderr.contains("toolchain nightly-test installation failed: controlled rustup failure"),
        "{mode_label} mode must reach the controlled failure after dependency checks; stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains(diagnostic),
        is_verbose,
        "{mode_label} mode stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains("binary_name=\"dylint-link\""),
        is_verbose,
        "{mode_label} mode stderr: {stderr}"
    );
    assert_eq!(
        stderr.contains("failure_category=\"directory_open\""),
        is_verbose,
        "{mode_label} mode stderr: {stderr}"
    );
    assert!(!stdout.contains(diagnostic), "diagnostic must be on stderr");
    assert!(!stdout.contains(fixture.root_path.as_str()));
    assert!(!stderr.contains(fixture.root_path.as_str()));
    assert!(!stdout.contains('\u{1b}'));
    assert!(!stderr.contains('\u{1b}'));
}

/// Build the fake Cargo responses from the versions required by the installer.
fn fake_cargo_script(versions: DependencyVersions) -> String {
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
        versions.cargo_dylint, versions.cargo_dylint, versions.dylint_link,
    )
}

/// Return a dependency binary's version from the embedded manifest.
///
/// # Errors
///
/// Returns an error if the manifest cannot be parsed or omits the package.
fn dependency_version(package: DependencyPackage) -> Result<&'static str, Box<dyn Error>> {
    let dependency =
        whitaker_installer::dependency_binaries::find_dependency_binary(package.manifest_name())?
            .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "embedded dependency manifest must include {}",
                    package.manifest_name()
                ),
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
fn write_executable(directory: &Dir, executable: FixtureExecutable) -> io::Result<()> {
    let (path, contents) = match executable {
        FixtureExecutable::Cargo(versions) => ("cargo", fake_cargo_script(versions)),
        FixtureExecutable::DylintLink => ("dylint-link", String::from("#!/bin/sh\nexit 0\n")),
        FixtureExecutable::Rustup => (
            "rustup",
            String::from("#!/bin/sh\necho 'controlled rustup failure' >&2\nexit 1\n"),
        ),
    };
    directory.write(path, &contents)?;
    directory.set_permissions(path, Permissions::from_mode(0o755))
}
