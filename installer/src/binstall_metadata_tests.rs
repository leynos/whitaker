//! Unit tests for cargo-binstall metadata validation.
//!
//! These tests parse the actual `installer/Cargo.toml` file and verify that
//! the `[package.metadata.binstall]` section matches the specification in
//! the design document (§ Installer release artefacts).

use super::*;
use rstest::rstest;
use std::path::PathBuf;
use toml::Table;

/// Load and parse the installer's `Cargo.toml`.
fn load_cargo_toml() -> Table {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let content =
        std::fs::read_to_string(&cargo_toml_path).expect("failed to read installer/Cargo.toml");
    content
        .parse::<Table>()
        .expect("failed to parse installer/Cargo.toml as TOML")
}

/// Navigate to `package.metadata.binstall` in the parsed TOML table.
fn binstall_table(table: &Table) -> &Table {
    table
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("binstall"))
        .and_then(|b| b.as_table())
        .expect("[package.metadata.binstall] table not found")
}

#[rstest]
fn pkg_url_matches_design_document() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let pkg_url = binstall
        .get("pkg-url")
        .and_then(|v| v.as_str())
        .expect("pkg-url not found");
    assert_eq!(pkg_url, PKG_URL_TEMPLATE);
}

#[rstest]
fn bin_dir_matches_design_document() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let bin_dir = binstall
        .get("bin-dir")
        .and_then(|v| v.as_str())
        .expect("bin-dir not found");
    assert_eq!(bin_dir, BIN_DIR_TEMPLATE);
}

#[rstest]
fn default_pkg_fmt_is_tgz() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let pkg_fmt = binstall
        .get("pkg-fmt")
        .and_then(|v| v.as_str())
        .expect("pkg-fmt not found");
    assert_eq!(pkg_fmt, DEFAULT_PKG_FMT);
}

#[rstest]
fn windows_override_uses_zip() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let overrides = binstall
        .get("overrides")
        .and_then(|o| o.as_table())
        .expect("overrides table not found");
    let windows = overrides
        .get(WINDOWS_OVERRIDE_TARGET)
        .and_then(|w| w.as_table())
        .expect("Windows override table not found");
    let pkg_fmt = windows
        .get("pkg-fmt")
        .and_then(|v| v.as_str())
        .expect("pkg-fmt not found in Windows override");
    assert_eq!(pkg_fmt, WINDOWS_PKG_FMT);
}

#[rstest]
fn no_unexpected_overrides() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let overrides = binstall
        .get("overrides")
        .and_then(|o| o.as_table())
        .expect("overrides table not found");
    assert_eq!(
        overrides.len(),
        1,
        "expected exactly one override (Windows), found {}",
        overrides.len()
    );
    assert!(
        overrides.contains_key(WINDOWS_OVERRIDE_TARGET),
        "expected override key to be {WINDOWS_OVERRIDE_TARGET}"
    );
}

#[rstest]
fn essential_binstall_fields_present() {
    let table = load_cargo_toml();
    let binstall = binstall_table(&table);
    let required = ["pkg-url", "bin-dir", "pkg-fmt"];
    for key in &required {
        assert!(
            binstall.contains_key(*key),
            "missing required binstall field: {key}"
        );
    }
}

#[rstest]
#[case::linux_x86("x86_64-unknown-linux-gnu")]
#[case::linux_arm("aarch64-unknown-linux-gnu")]
#[case::macos_x86("x86_64-apple-darwin")]
#[case::macos_arm("aarch64-apple-darwin")]
fn non_windows_targets_expand_to_tgz(#[case] target: &str) {
    let url = expand_pkg_url("0.2.0", target);
    assert!(
        url.ends_with(".tgz"),
        "expected URL for {target} to end with .tgz, got {url}"
    );
    assert!(url.contains(target));
    assert!(url.contains("v0.2.0"));
}

#[rstest]
fn windows_target_expands_to_zip() {
    let url = expand_pkg_url("0.2.0", WINDOWS_OVERRIDE_TARGET);
    assert!(
        url.ends_with(".zip"),
        "expected URL for Windows to end with .zip, got {url}"
    );
}

#[rstest]
fn bin_dir_expands_correctly_for_unix() {
    let dir = expand_bin_dir("0.2.0", "x86_64-unknown-linux-gnu");
    assert_eq!(
        dir,
        "whitaker-installer-x86_64-unknown-linux-gnu-v0.2.0/whitaker-installer"
    );
}

#[rstest]
fn bin_dir_expands_correctly_for_windows() {
    let dir = expand_bin_dir("0.2.0", WINDOWS_OVERRIDE_TARGET);
    assert_eq!(
        dir,
        concat!(
            "whitaker-installer-x86_64-pc-windows-msvc-v0.2.0/",
            "whitaker-installer.exe"
        )
    );
}

#[rstest]
fn pkg_url_contains_repo_url() {
    let url = expand_pkg_url("0.2.0", "x86_64-unknown-linux-gnu");
    assert!(
        url.starts_with(REPO_URL),
        "expected URL to start with {REPO_URL}, got {url}"
    );
}
