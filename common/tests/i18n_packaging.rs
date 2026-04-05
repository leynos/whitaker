//! Regression coverage for crate-relative localization packaging.
//!
//! The `whitaker-common` crate is published independently, so its Fluent
//! bundles must be present in the packaged tarball rather than only in the
//! checkout.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use whitaker_common::i18n::packaged_fallback_locale_path;

use tempfile::{Builder, TempDir};

#[test]
fn fluent_bundles_are_included_in_the_package_tarball() {
    let target_dir = package_target_dir();
    let crate_path = package_crate_path(target_dir.path());
    let tar_listing = package_tar_listing(&crate_path);
    let expected_entry = packaged_fallback_locale_path()
        .to_string_lossy()
        .replace('\\', "/");

    assert!(
        tar_listing.lines().any(|line| line == expected_entry),
        "expected packaged tarball to include the fallback Fluent bundle, but it did not"
    );
}

fn package_target_dir() -> TempDir {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_root = manifest_dir.join("target");
    fs::create_dir_all(&target_root)
        .unwrap_or_else(|error| panic!("target directory should be creatable: {error}"));

    Builder::new()
        .prefix("whitaker-common-package-")
        .tempdir_in(&target_root)
        .unwrap_or_else(|error| panic!("temporary package directory should be creatable: {error}"))
}

fn package_crate_path(target_dir: &Path) -> PathBuf {
    let status = Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", target_dir)
        .args([
            "package",
            "-p",
            "whitaker-common",
            "--allow-dirty",
            "--no-verify",
        ])
        .status()
        .unwrap_or_else(|error| panic!("cargo package should run: {error}"));

    assert!(status.success(), "cargo package should succeed");

    let expected_name = format!(
        "{}-{}.crate",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    let package_dir = target_dir.join("package");
    fs::read_dir(&package_dir)
        .unwrap_or_else(|error| panic!("package directory should be readable: {error}"))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("package directory entry should be readable: {error}")
                })
                .path()
        })
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == expected_name.as_str())
        })
        .unwrap_or_else(|| panic!("cargo package should produce {expected_name}"))
}

#[cfg(unix)]
fn package_tar_listing(crate_path: &Path) -> String {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(crate_path)
        .output()
        .unwrap_or_else(|error| panic!("tar should list package contents: {error}"));

    assert!(
        output.status.success(),
        "tar should succeed when listing the packaged crate: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("tar listing should be valid UTF-8: {error}"))
}
