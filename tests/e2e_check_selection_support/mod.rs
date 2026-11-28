//! Fixture workspaces and real-Cargo runs for the selection differential suite.
//!
//! Each case copies a small workspace out of `tests/data/cargo_selection` and
//! runs two commands in it: `cargo clippy` with the flags under test, and
//! `whitaker check` with the same flags. The Whitaker run goes through the
//! installed-bootstrap fixture, whose stub `cargo` delegates `metadata` and the
//! lint run to a real Cargo, so the compared sets come from Cargo's own
//! `compiler-artifact` messages rather than from Whitaker's idea of them.
//!
//! The copy lives under `CARGO_TARGET_TMPDIR`, inside the repository's target
//! directory: the fixture then inherits the pinned toolchain and Cargo
//! configuration, and a failed run leaves its workspace behind for inspection.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;
use tempfile::TempDir;

use crate::e2e_check_support::BootstrapFixture;

/// The directory holding the checked-in fixture workspaces.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("cargo_selection")
}

/// The directory a fixture copy is created in.
///
/// Cargo supplies `CARGO_TARGET_TMPDIR` when it builds integration-test code,
/// and embedding it keeps the copy inside the package's target directory even
/// when another runner executes the binary: the copy then inherits the
/// repository's toolchain file and Cargo configuration.
fn scratch_root() -> PathBuf { PathBuf::from(env!("CARGO_TARGET_TMPDIR")) }

/// One copied fixture workspace.
pub struct SelectionFixture {
    /// Owns the copy.
    _dir: TempDir,
    /// The copied workspace root.
    workspace: PathBuf,
}

impl SelectionFixture {
    /// Copy the named fixture into a fresh scratch directory.
    ///
    /// # Errors
    ///
    /// Returns a description of the first filesystem step that failed.
    pub fn new(name: &str) -> Result<Self, String> {
        let source = fixture_root().join(name);
        if !source.is_dir() {
            return Err(format!("fixture {} should exist", source.display()));
        }
        fs::create_dir_all(scratch_root())
            .map_err(|error| format!("scratch directory should be created: {error}"))?;
        let dir = TempDir::new_in(scratch_root())
            .map_err(|error| format!("fixture copy should be created: {error}"))?;
        copy_tree(&source, dir.path())?;
        Ok(Self {
            workspace: dir.path().to_path_buf(),
            _dir: dir,
        })
    }

    /// Resolve a directory inside the copied workspace.
    pub fn cwd(&self, relative: &str) -> PathBuf { self.workspace.join(relative) }

    /// Run `cargo clippy` with `flags` and collect its compiler artefacts.
    ///
    /// # Errors
    ///
    /// Returns a description of a failed spawn, a rejected workspace, or an
    /// unreadable message stream.
    pub fn clippy(&self, cwd: &Path, flags: &[&str]) -> Result<Artefacts, String> {
        let mut command = self.cargo_command(cwd);
        command.args(["clippy", "--message-format=json", "--offline"]);
        command.args(flags);
        let output = run(&mut command, "cargo clippy")?;
        Artefacts::parse(&output.stdout)
    }

    /// Run `whitaker check -v` with `flags` through the bootstrap fixture.
    ///
    /// `-q` is passed after the separator so Cargo's own progress lines are
    /// suppressed and every `Checking` line in stderr is Whitaker's listing.
    ///
    /// # Errors
    ///
    /// Returns a description of a failed fixture, spawn or message stream.
    pub fn whitaker(cwd: &Path, flags: &[&str]) -> Result<WhitakerRun, String> {
        let fixture = BootstrapFixture::new()?;
        let mut command = fixture.command()?;
        command
            .current_dir(cwd)
            // The delegated `cargo check` inherits this process's environment,
            // so the flags are cleared here too: both runs must see the same
            // policy as the reference `cargo clippy`.
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTDOCFLAGS")
            .env("STUB_DELEGATE", "1")
            .env("STUB_REAL_CARGO", real_cargo()?)
            .args(["check", "-v"])
            .args(flags)
            .args(["--", "-q", "--message-format=json", "--offline"]);
        let output = run(&mut command, "whitaker check")?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(WhitakerRun {
            artefacts: Artefacts::parse(&output.stdout)?,
            listed: listed_packages(&stderr),
            stderr,
        })
    }

    /// Map Cargo's package ids to package names for the workspace at `cwd`.
    ///
    /// The artefact stream names packages by id; the listing names them by
    /// package name. Asking Cargo for the mapping keeps the comparison from
    /// parsing an id format that is not a stable interface.
    ///
    /// # Errors
    ///
    /// Returns a description of a failed spawn or an unreadable document.
    pub fn package_names(
        &self,
        cwd: &Path,
        manifest: Option<&str>,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut command = self.cargo_command(cwd);
        command.args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ]);
        if let Some(path) = manifest {
            command.args(["--manifest-path", path]);
        }
        let output = run(&mut command, "cargo metadata")?;
        let document: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("cargo metadata should be JSON: {error}"))?;
        let packages = document
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| "cargo metadata should hold packages".to_owned())?;
        let mut names = BTreeMap::new();
        for package in packages {
            let id = package
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "every package should have an id".to_owned())?;
            let name = package
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "every package should have a name".to_owned())?;
            names.insert(id.to_owned(), name.to_owned());
        }
        Ok(names)
    }

    /// Build a Cargo command for `cwd` with a controlled environment.
    ///
    /// `RUSTFLAGS` is cleared because the repository's test flags (`-D
    /// warnings`, dynamic linking) describe how Whitaker is built, not how a
    /// fixture should select packages; retaining them would make the fixtures
    /// depend on lint policy that has nothing to do with selection.
    fn cargo_command(&self, cwd: &Path) -> Command {
        let mut command = Command::new(real_cargo().unwrap_or_else(|_| OsString::from("cargo")));
        command
            .current_dir(cwd)
            .env("CARGO_TARGET_DIR", self.workspace.join("target"))
            .env("CARGO_NET_OFFLINE", "true")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTDOCFLAGS");
        command
    }
}

/// What one `whitaker check` run reported.
pub struct WhitakerRun {
    /// The compiler artefacts the delegated `cargo check` produced.
    pub artefacts: Artefacts,
    /// The package names Whitaker's `-v` listing printed.
    pub listed: BTreeSet<String>,
    /// Everything the child and Whitaker wrote to stderr.
    pub stderr: String,
}

/// Create an empty directory outside any fixture workspace.
///
/// # Errors
///
/// Returns a description of a failed filesystem step.
pub fn scratch_directory() -> Result<TempDir, String> {
    fs::create_dir_all(scratch_root())
        .map_err(|error| format!("scratch directory should be created: {error}"))?;
    TempDir::new_in(scratch_root())
        .map_err(|error| format!("scratch directory should be created: {error}"))
}

/// The `(package_id, target name, target kinds)` tuples Cargo reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Artefacts {
    /// One entry per compiled unit.
    keys: BTreeSet<String>,
}

impl Artefacts {
    /// Parse Cargo's `compiler-artifact` messages.
    ///
    /// # Errors
    ///
    /// Returns a description of a line that is not a JSON message or lacks the
    /// fields the comparison needs.
    pub fn parse(stdout: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(stdout)
            .map_err(|error| format!("Cargo's messages should be UTF-8: {error}"))?;
        let mut keys = BTreeSet::new();
        for line in text.lines().filter(|line| line.starts_with('{')) {
            let message: Value = serde_json::from_str(line)
                .map_err(|error| format!("a Cargo message should be JSON: {error}"))?;
            if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
                continue;
            }
            keys.insert(artefact_key(&message)?);
        }
        Ok(Self { keys })
    }

    /// The package ids named by the compiled units.
    pub fn package_ids(&self) -> BTreeSet<String> {
        self.keys
            .iter()
            .filter_map(|key| key.split('\t').next())
            .map(str::to_owned)
            .collect()
    }

    /// The units, for an assertion message.
    pub const fn keys(&self) -> &BTreeSet<String> { &self.keys }
}

/// Build the comparison key for one artefact message.
fn artefact_key(message: &Value) -> Result<String, String> {
    let package_id = message
        .get("package_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "a compiler artefact should name its package".to_owned())?;
    let target = message
        .get("target")
        .ok_or_else(|| "a compiler artefact should have a target".to_owned())?;
    let name = target
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "a compiler artefact should name its target".to_owned())?;
    let kinds = target
        .get("kind")
        .and_then(Value::as_array)
        .ok_or_else(|| "a compiler artefact should list its target kinds".to_owned())?
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{package_id}\t{name}\t{kinds}"))
}

/// Collect the package names from Whitaker's `Checking` lines.
fn listed_packages(stderr: &str) -> BTreeSet<String> {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix("    Checking "))
        .filter_map(|rest| rest.split_once(' '))
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// Resolve the real Cargo the stub delegates to.
///
/// The compile-time `CARGO` names the Cargo that built this test, which is the
/// pinned toolchain the fixtures must use. The stub consults it by absolute
/// path, so the stub's own directory being first on `PATH` cannot recurse.
fn real_cargo() -> Result<OsString, String> {
    let cargo = Path::new(env!("CARGO"));
    if cargo.is_file() {
        Ok(cargo.as_os_str().to_owned())
    } else {
        Err(format!(
            "the build-time Cargo should exist: {}",
            cargo.display()
        ))
    }
}

/// Run a command and require success.
fn run(command: &mut Command, what: &str) -> Result<std::process::Output, String> {
    let output = command
        .output()
        .map_err(|error| format!("{what} should start: {error}"))?;
    if output.status.success() {
        return Ok(output);
    }
    Err(format!(
        "{what} should succeed (status {:?}), stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Copy a directory tree, creating the destination.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to)
        .map_err(|error| format!("{} should be created: {error}", to.display()))?;
    let entries = fs::read_dir(from)
        .map_err(|error| format!("{} should be readable: {error}", from.display()))?;
    for entry in entries {
        let item = entry.map_err(|error| format!("a fixture entry should be readable: {error}"))?;
        let target = to.join(item.file_name());
        let file_type = item
            .file_type()
            .map_err(|error| format!("a fixture entry should have a type: {error}"))?;
        if file_type.is_dir() {
            copy_tree(&item.path(), &target)?;
        } else {
            copy_file(&item.path(), &target)?;
        }
    }
    Ok(())
}

/// Copy one fixture file.
fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{} should be copied: {error}", from.display()))
}
