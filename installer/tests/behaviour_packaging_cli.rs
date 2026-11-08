//! Process-level checks for the release packaging command-line tools.

use std::{
    fs,
    process::{Command, Output},
};

use tempfile::tempdir;
use whitaker_installer::dependency_binaries::find_dependency_binary;

/// Assert that a packaging CLI succeeded and reported its output path.
fn assert_success(output: &Output, command: &str) {
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

/// Launch a packaging CLI with no arguments, capturing its output.
fn run_without_arguments(command: &str) -> std::io::Result<Output> {
    Command::new(command).output()
}

/// Assert that a packaging CLI rejects missing input with a diagnostic.
fn assert_missing_input_is_rejected(output: &Output, command: &str) {
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
    for command in [
        env!("CARGO_BIN_EXE_whitaker-package-lints"),
        env!("CARGO_BIN_EXE_whitaker-package-installer"),
        env!("CARGO_BIN_EXE_whitaker-package-dependency-binary"),
    ] {
        let output = run_without_arguments(command).expect("packaging CLI should launch");
        assert_missing_input_is_rejected(&output, command);
    }
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

    assert_success(&output, "whitaker-package-lints");
    assert!(
        output_dir
            .join("manifest-x86_64-unknown-linux-gnu.json")
            .is_file()
    );
    assert!(
        fs::read_dir(output_dir)
            .expect("read distribution directory")
            .any(|entry| entry.is_ok_and(|dir_entry| dir_entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "zst")))
    );
}

/// The libraries a rolling archive must hold, named independently of the
/// packaging code: the suite and the nine standard lints, which installers
/// v0.2.9 to v0.2.11 can prune.
const EXPECTED_ARCHIVE_CRATES: [&str; 10] = [
    "bumpy_road_function",
    "conditional_max_n_branches",
    "function_attrs_follow_docs",
    "module_max_lines",
    "module_must_have_inner_docs",
    "no_expect_outside_tests",
    "no_std_fs_operations",
    "no_unwrap_or_else_panic",
    "test_must_not_have_example",
    "whitaker_suite",
];

/// Experimental crate whose standalone library installers v0.2.9 to v0.2.11
/// cannot prune (whitaker #565), so the archive must not carry it.
const UNPRUNABLE_CRATE: &str = "string_continuation_style";

/// Return the crate name behind an archive or manifest entry such as
/// `libfoo@nightly-2026-05-28-x86_64-unknown-linux-gnu.so`.
fn crate_of_entry(entry: &str) -> String {
    let stem = entry.strip_prefix("lib").unwrap_or(entry);
    stem.split(['@', '.']).next().unwrap_or(stem).to_owned()
}

/// Sort a list of entry names reduced to crate names.
fn sorted_crates<'a>(entries: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut crates: Vec<String> = entries.into_iter().map(crate_of_entry).collect();
    crates.sort();
    crates
}

/// Boxed error for the helpers below, which propagate rather than panic.
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Read the `files` list of the linux manifest written to `output_dir`.
fn read_manifest_files(output_dir: &std::path::Path) -> TestResult<Vec<String>> {
    let bytes = fs::read(output_dir.join("manifest-x86_64-unknown-linux-gnu.json"))?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes)?;
    let files = manifest
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or("manifest has no files array")?;
    files
        .iter()
        .map(|file| {
            file.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "manifest file name is not a string".into())
        })
        .collect()
}

/// List the entry paths of the `.tar.zst` archive written to `output_dir`.
fn read_archive_entries(output_dir: &std::path::Path) -> TestResult<Vec<String>> {
    let archive_path = fs::read_dir(output_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "zst"))
        .ok_or("no archive written")?;
    let decoder = zstd::Decoder::new(fs::File::open(archive_path)?)?;
    let mut archive = tar::Archive::new(decoder);
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        paths.push(entry?.path()?.to_string_lossy().into_owned());
    }
    Ok(paths)
}

#[test]
fn package_lints_release_dir_ships_only_libraries_the_pinned_installer_prunes() {
    let temp_dir = tempdir().expect("temporary directory");
    let release_dir = temp_dir.path().join("release");
    let output_dir = temp_dir.path().join("dist");
    fs::create_dir(&release_dir).expect("create release directory");
    for name in EXPECTED_ARCHIVE_CRATES.iter().chain(&[UNPRUNABLE_CRATE]) {
        fs::write(release_dir.join(format!("lib{name}.so")), b"fixture").expect("write library");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-package-lints"))
        .args([
            "--git-sha",
            "abc1234",
            "--toolchain",
            "nightly-2026-05-28",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--generated-at",
            "2026-02-12T10:00:00Z",
            "--output-dir",
        ])
        .arg(&output_dir)
        .arg("--release-dir")
        .arg(&release_dir)
        .output()
        .expect("run package-lints binary");
    assert_success(&output, "whitaker-package-lints");

    let expected: Vec<String> = EXPECTED_ARCHIVE_CRATES
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let manifest_files = read_manifest_files(&output_dir).expect("read manifest files");
    assert_eq!(
        sorted_crates(manifest_files.iter().map(String::as_str)),
        expected,
        "manifest files"
    );
    let entries = read_archive_entries(&output_dir).expect("read archive entries");
    assert_eq!(
        sorted_crates(entries.iter().map(String::as_str)),
        expected,
        "archive entries"
    );
    assert!(
        entries
            .iter()
            .all(|entry| !entry.contains(UNPRUNABLE_CRATE)),
        "the archive must not carry {UNPRUNABLE_CRATE}"
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

    assert_success(&output, "whitaker-package-installer");
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

    assert_success(&output, "whitaker-package-dependency-binary");
    assert!(output_dir.join(archive_name).is_file());
}
