//! Regression snapshot for the legacy installer's dry-run output.

use std::{path::Path, process::Command};

use insta::assert_snapshot;

const fn pinned_toolchain_channel() -> &'static str { "nightly-2026-05-28" }

fn normalized_dry_run_output(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            if line.starts_with("Workspace root: ") {
                "Workspace root: [workspace]"
            } else if line.starts_with("Target directory: ") {
                "Target directory: [staging directory]"
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
    let output = Command::new(env!("CARGO_BIN_EXE_whitaker-installer"))
        .args(["--dry-run", "--toolchain", pinned_toolchain_channel()])
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
