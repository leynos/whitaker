//! Compile-time regression guard for the `dylint-driver`-disabled build.
//!
//! The crate is a Dylint lint library: the lint logic lives behind the
//! optional `dylint-driver` feature, while the default feature set is empty so
//! the crate can be built by tools that only need a plain library. Issue #322
//! removed the private stub that previously existed solely to keep that
//! empty build warning-free. This test re-checks the configuration the stub
//! existed for: `cargo check --no-default-features --lib` must still succeed
//! with warnings denied, now without relying on that stub.
//!
//! The check is deliberately restricted to the library target: the crate's
//! integration test binaries are dylint harnesses that require `cargo-dylint`
//! and `dylint-link`, which must not be assumed here. The nested `cargo`
//! invocation therefore inherits the outer `RUSTFLAGS` (so `-D warnings` from
//! the Makefile gate applies) but uses an isolated target directory so it never
//! contends with the outer build.

use std::process::Command;

use anyhow::Context as _;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use serde_json::Value;
use tempfile::TempDir;

/// Top-level name of the package under test.
const CRATE: &str = "no_std_fs_operations";

/// Runs `cargo check --no-default-features --lib` for this crate and asserts
/// that the build succeeds with warnings denied (via inherited `RUSTFLAGS`).
///
/// The workspace manifest is located by walking up from this test crate's
/// manifest directory, mirroring `integration_exclusion.rs`, so the nested
/// invocation resolves the same workspace this test builds under.
#[test]
fn crate_builds_without_dylint_driver_feature() -> anyhow::Result<()> {
    let workspace_root = workspace_root()?;
    let target_dir = TempDir::new().context("failed to create isolated target directory")?;

    let output = Command::new("cargo")
        .arg("check")
        .arg("--package")
        .arg(CRATE)
        .arg("--no-default-features")
        .arg("--lib")
        .arg("--message-format=json")
        .current_dir(&workspace_root)
        .env("CARGO_TARGET_DIR", target_dir.path())
        .output()
        .context("failed to execute nested cargo check")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "`cargo check --no-default-features --lib` for `{CRATE}` failed (it must compile \
             without the `dylint-driver` feature now that the no-driver stub is gone):\n{stderr}"
        );
    }

    let diagnostics = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-message")
        .collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        anyhow::bail!(
            "`cargo check --no-default-features --lib` for `{CRATE}` emitted compiler diagnostics \
             under `-D warnings`: {diagnostics:#?}"
        );
    }

    Ok(())
}

/// Returns the workspace root containing this crate's manifest.
fn workspace_root() -> anyhow::Result<Utf8PathBuf> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidate = manifest_dir.as_path();
    loop {
        if is_workspace_root(candidate) {
            return Ok(candidate.to_path_buf());
        }
        candidate = candidate
            .parent()
            .context("workspace root not found above CARGO_MANIFEST_DIR")?;
    }
}

/// Returns whether `candidate` contains a workspace manifest.
fn is_workspace_root(candidate: &Utf8Path) -> bool {
    let Ok(directory) = Dir::open_ambient_dir(candidate, ambient_authority()) else {
        return false;
    };
    let Ok(manifest) = directory.read_to_string("Cargo.toml") else {
        return false;
    };
    manifest.contains("[workspace]")
}
