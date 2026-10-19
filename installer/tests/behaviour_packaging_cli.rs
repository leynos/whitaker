//! Process-level checks for the release packaging command-line tools.

use std::fs;
use std::process::{Command, Output};
use tempfile::tempdir;
use whitaker_installer::dependency_binaries::find_dependency_binary;

fn assert_success(output: Output, command: &str) {
    assert!(
        output.status.success(),
        "{command} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Created "),
        "{command} did not report its output path"
    );
    assert!(
        output.stderr.is_empty(),
        "{command} wrote to stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_missing_input_is_rejected(command: &str) {
    let output = Command::new(command)
        .output()
        .expect("packaging CLI should launch");

    assert!(
        !output.status.success(),
        "{command} should reject missing required input"
    );
    assert!(
        output.stdout.is_empty(),
        "{command} wrote unexpected stdout"
    );
    assert!(
        !output.stderr.is_empty(),
        "{command} should explain the missing input"
    );
}

#[test]
fn packaging_binaries_reject_missing_required_input() {
    assert_missing_input_is_rejected(env!("CARGO_BIN_EXE_whitaker-package-lints"));
    assert_missing_input_is_rejected(env!("CARGO_BIN_EXE_whitaker-package-installer"));
    assert_missing_input_is_rejected(env!("CARGO_BIN_EXE_whitaker-package-dependency-binary"));
}

#[test]
fn package_lints_binary_creates_archive_and_manifest() {
    let temp_dir = tempdir().expect("temporary directory");
    let library = temp_dir.path().join("libfixture.so");
    let output_dir = temp_dir.path().join("dist");
    fs::write(&library, b"fixture lint library").expect("write fixture library");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-package-lints"))
        .args([
            "--git-sha",
            "abc1234",
            "--toolchain",
            "nightly-2026-05-28",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--output-dir",
        ])
        .arg(&output_dir)
        .args(["--generated-at", "2026-02-12T10:00:00Z"])
        .arg(&library)
        .output()
        .expect("run package-lints binary");

    assert_success(output, "whitaker-package-lints");
    assert!(
        output_dir
            .join("manifest-x86_64-unknown-linux-gnu.json")
            .is_file()
    );
    assert!(
        fs::read_dir(output_dir)
            .expect("read distribution directory")
            .any(|entry| entry
                .is_ok_and(|entry| entry.path().extension().is_some_and(|ext| ext == "zst")))
    );
}

#[test]
fn package_installer_binary_creates_release_archive() {
    let temp_dir = tempdir().expect("temporary directory");
    let binary = temp_dir.path().join("whitaker-installer");
    let output_dir = temp_dir.path().join("dist");
    fs::write(&binary, b"fixture installer executable").expect("write fixture binary");

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-package-installer"))
        .args([
            "--crate-version",
            "0.2.9",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--binary-path",
        ])
        .arg(&binary)
        .arg("--output-dir")
        .arg(&output_dir)
        .output()
        .expect("run package-installer binary");

    assert_success(output, "whitaker-package-installer");
    assert!(
        output_dir
            .join("whitaker-installer-x86_64-unknown-linux-gnu-v0.2.9.tgz")
            .is_file()
    );
}

#[test]
fn package_dependency_binary_creates_release_archive() {
    let temp_dir = tempdir().expect("temporary directory");
    let binary = temp_dir.path().join("cargo-dylint");
    let output_dir = temp_dir.path().join("dist");
    fs::write(&binary, b"fixture dependency executable").expect("write fixture binary");
    let dependency = find_dependency_binary("cargo-dylint")
        .expect("dependency manifest should load")
        .expect("cargo-dylint should be present");
    let archive_name = format!(
        "cargo-dylint-x86_64-unknown-linux-gnu-v{}.tgz",
        dependency.version()
    );

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-package-dependency-binary"))
        .args([
            "package",
            "--package",
            "cargo-dylint",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--binary-path",
        ])
        .arg(&binary)
        .arg("--output-dir")
        .arg(&output_dir)
        .output()
        .expect("run package-dependency-binary");

    assert_success(output, "whitaker-package-dependency-binary");
    assert!(output_dir.join(archive_name).is_file());
}
