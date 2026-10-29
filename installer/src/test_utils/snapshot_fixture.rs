//! A small, real Clippy snapshot for integration tests of the installer.
//!
//! The published snapshot is the full Clippy history, far too large for a
//! test. This fixture commits two rustups to a local repository, bundles it
//! with a `.sha256` sidecar, and writes a manifest pinning it. Debug builds of
//! the installer read that manifest from
//! [`crate::test_support::TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV`] and install the
//! bundle through `--clippy-snapshot-bundle`, so the real verification and
//! installation code runs without network access.

use crate::clippy_snapshot::{
    BundleArtefact, ClippySnapshotManifest, install_bundle, parse_manifest,
};
use camino::{Utf8Path, Utf8PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The nightly the fixture snapshot pins.
pub const FIXTURE_TOOLCHAIN: &str = "nightly-2026-05-28";

/// A fixture bundle, its sidecar, and the manifest that pins it.
#[derive(Debug)]
pub struct SnapshotFixture {
    /// Owns every path below.
    _dir: TempDir,
    bundle: Utf8PathBuf,
    manifest_path: Utf8PathBuf,
    manifest: ClippySnapshotManifest,
}

impl SnapshotFixture {
    /// Commit a two-rustup history and publish it as a bundle and manifest.
    ///
    /// # Errors
    ///
    /// Returns a description of the failed filesystem or Git step.
    pub fn new() -> Result<Self, String> {
        let dir = TempDir::new().map_err(|error| format!("create fixture directory: {error}"))?;
        let base = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| format!("fixture path is not UTF-8: {}", path.display()))?;
        let work = base.join("work");
        let commit = commit_history(&work)?;
        let bundle = base.join(format!("whitaker-clippy-snapshot-{commit}.bundle"));
        git(
            &work,
            &[
                "bundle",
                "create",
                "--quiet",
                bundle.as_str(),
                "refs/heads/main",
                "HEAD",
            ],
        )?;
        write_sidecar(&bundle)?;
        // The fixture's tip introduced the pinned nightly, so Dylint selects it.
        let text = format!(
            concat!(
                "schema_version = 1\n",
                "upstream = \"https://example.invalid/rust-clippy\"\n",
                "branch = \"main\"\n",
                "commit = \"{commit}\"\n",
                "toolchain = \"{toolchain}\"\n",
                "selected_revision = \"{commit}\"\n",
                "dylint_version = \"6.3.0\"\n",
            ),
            commit = commit,
            toolchain = FIXTURE_TOOLCHAIN,
        );
        let manifest = parse_manifest(&text).map_err(|error| error.to_string())?;
        let manifest_path = base.join("clippy-snapshot.toml");
        write(&manifest_path, &text)?;
        Ok(Self {
            _dir: dir,
            bundle,
            manifest_path,
            manifest,
        })
    }

    /// The pinned commit.
    #[must_use]
    pub fn commit(&self) -> &str {
        self.manifest.commit.as_str()
    }

    /// The fixture's bundle, with its `.sha256` sidecar beside it.
    #[must_use]
    pub fn bundle(&self) -> &Utf8Path {
        &self.bundle
    }

    /// The fixture's manifest file, for the test manifest override.
    #[must_use]
    pub fn manifest_path(&self) -> &Utf8Path {
        &self.manifest_path
    }

    /// The fixture's source repository, from which a producer build clones.
    #[must_use]
    pub fn source(&self) -> Utf8PathBuf {
        self.manifest_path.with_file_name("work")
    }

    /// Point a debug installer at this fixture instead of the published snapshot.
    pub fn configure(&self, command: &mut Command) {
        command
            .env(
                crate::test_support::TEST_CLIPPY_SNAPSHOT_MANIFEST_ENV,
                &self.manifest_path,
            )
            .arg("--clippy-snapshot-bundle")
            .arg(&self.bundle);
    }

    /// Install the fixture as a generation under `clippy_root`.
    ///
    /// # Errors
    ///
    /// Returns the installer's own rejection, which would be a fixture bug.
    pub fn install(&self, clippy_root: &Utf8Path) -> Result<Utf8PathBuf, String> {
        let artefact = BundleArtefact::from_sidecar(&self.bundle, &self.manifest)
            .map_err(|error| error.to_string())?;
        install_bundle(&artefact, &self.manifest, clippy_root).map_err(|error| error.to_string())
    }
}

/// Commit the nightly before the pinned one, then the pinned one; return the tip.
fn commit_history(work: &Utf8Path) -> Result<String, String> {
    std::fs::create_dir_all(work.join("clippy_utils"))
        .map_err(|error| format!("create work tree: {error}"))?;
    git(work, &["init", "--quiet", "--initial-branch=main"])?;
    for (index, channel) in ["nightly-2026-05-13", FIXTURE_TOOLCHAIN].iter().enumerate() {
        let cargo = format!("[package]\nname = \"clippy_utils\"\nversion = \"0.1.{index}\"\n");
        write(&work.join("clippy_utils/Cargo.toml"), &cargo)?;
        write(
            &work.join("rust-toolchain.toml"),
            &format!("[toolchain]\nchannel = \"{channel}\"\n"),
        )?;
        git(work, &["add", "--all"])?;
        git(work, &["commit", "--quiet", "--message", channel])?;
    }
    git(work, &["rev-parse", "HEAD"])
}

/// Write `<bundle>.sha256` in `sha256sum` format.
fn write_sidecar(bundle: &Utf8Path) -> Result<(), String> {
    let bytes = std::fs::read(bundle).map_err(|error| format!("read {bundle}: {error}"))?;
    let name = bundle
        .file_name()
        .ok_or_else(|| format!("{bundle} has no name"))?;
    write(
        &Utf8PathBuf::from(format!("{bundle}.sha256")),
        &format!("{}  {name}\n", super::sha256_hex(&bytes)),
    )
}

/// Write a fixture file, describing any failure.
fn write(path: &Utf8Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|error| format!("write {path}: {error}"))
}

/// Run fixture Git with a fixed identity and no ambient configuration.
///
/// # Errors
///
/// Returns Git's standard error when the command fails.
pub fn git(dir: &Utf8Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_COUNT", "0")
        .output()
        .map_err(|error| format!("run git {args:?}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
