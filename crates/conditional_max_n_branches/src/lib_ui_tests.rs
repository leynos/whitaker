//! UI test helpers for `conditional_max_n_branches`.
//!
//! This module centralises the UI test helpers to avoid repetition
//! between the integration test binaries.

use std::fs;
use std::path::Path;

use camino::Utf8PathBuf;
use dylint_testing::{ui::Test, ui_test};
use glob::glob;
use tempfile::tempdir;

/// Harness entry-point for unit tests that wish to run the UI suite.
pub fn run_ui_tests() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let ui_glob = format!("{manifest_dir}/ui/**/*.rs");

    let temp_target = tempdir().expect("temporary target directory");

    for ui_entry in glob(&ui_glob).expect("valid glob") {
        let ui_entry = ui_entry.expect("UI test path");
        let ui_test = Test::from_path(&ui_entry);

        ui_test
            .name(&format!("ui_{}", ui_entry.file_stem().unwrap().to_string_lossy()))
            .program("dylint")
            .args(&[
                "--libs",
                &ui_entry.to_string_lossy(),
                "--",
                "--target-dir",
                temp_target.path().to_str().unwrap(),
            ])
            .run();
    }
}

/// Guards against accidental deletion or renaming of UI directories.
#[test]
fn ui_directories_exist() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let ui_path = Path::new(manifest_dir).join("ui");
    assert!(
        ui_path.exists(),
        "UI directory should exist: {}",
        ui_path.display()
    );

    let ui_cy_path = Path::new(manifest_dir).join("ui-cy");
    // Optional: only check if it exists
    if ui_cy_path.exists() {
        assert!(
            ui_cy_path.is_dir(),
            "UI-cy path should be a directory: {}",
            ui_cy_path.display()
        );
    }
}