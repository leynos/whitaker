//! Regression snapshot for the legacy installer's dry-run output.

use std::{error::Error, io, path::Path, process::Command};

use insta::assert_snapshot;

fn pinned_toolchain_channel() -> Result<String, Box<dyn Error>> {
    let manifest = include_str!("../../rust-toolchain.toml").parse::<toml::Table>()?;

    manifest
        .get("toolchain")
        .and_then(toml::Value::as_table)
        .and_then(|toolchain| toolchain.get("channel"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("workspace toolchain channel is missing or invalid").into())
}

/// Lines whose value depends on the machine or checkout running the test.
///
/// The bootstrap paths are absolute and the snapshot state depends on what a
/// developer has already installed, so neither can be asserted verbatim.
/// Their labels are the stable interface, and the values they carry are
/// covered by the injected-directory unit tests.
const NORMALIZED_PREFIXES: [(&str, &str); 6] = [
    ("Workspace root: ", "Workspace root: [workspace]"),
    (
        "Target directory: ",
        "Target directory: [staging directory]",
    ),
    ("Toolchain: ", "Toolchain: [pinned toolchain]"),
    ("Clippy snapshot: ", "Clippy snapshot: [generation]"),
    ("Clippy snapshot state: ", "Clippy snapshot state: [state]"),
    (
        "Dylint driver directory: ",
        "Dylint driver directory: [driver directory]",
    ),
];

/// Replace a machine-dependent line's value with its placeholder.
fn normalize_line(line: &str) -> &str {
    NORMALIZED_PREFIXES
        .iter()
        .find_map(|(prefix, placeholder)| line.starts_with(prefix).then_some(*placeholder))
        .unwrap_or(line)
}

fn normalized_dry_run_output(output: &str) -> String {
    output
        .lines()
        .map(normalize_line)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn dry_run_output_matches_snapshot() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("installer must be nested under the workspace root");
    let toolchain =
        pinned_toolchain_channel().expect("workspace toolchain channel should be valid");
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"))
        .args(["--dry-run", "--toolchain", &toolchain])
        .current_dir(workspace_root)
        .output()
        .expect("whitaker-installer should run");

    assert!(
        output.status.success(),
        "dry-run must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr).expect("dry-run stderr must be UTF-8");
    assert_snapshot!("dry_run_output", normalized_dry_run_output(&stderr));
}
