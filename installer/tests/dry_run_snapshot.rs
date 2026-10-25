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

fn normalized_dry_run_output(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            if line.starts_with("Workspace root: ") {
                "Workspace root: [workspace]"
            } else if line.starts_with("Target directory: ") {
                "Target directory: [staging directory]"
            } else if line.starts_with("Toolchain: ") {
                "Toolchain: [pinned toolchain]"
            } else {
                line
            }
        })
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
