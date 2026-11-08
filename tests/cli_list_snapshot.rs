//! Snapshot guard for the current, intentionally unstable `whitaker ls` JSON.

use std::process::Command;

use camino::Utf8PathBuf;
use cap_std::{ambient_authority, fs_utf8::Dir};
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use whitaker_installer::builder::{library_extension, library_prefix};

/// Stages an empty suite marker for one fake toolchain.
///
/// Returns the temporary directory alongside its UTF-8 path so the caller
/// keeps the staging area alive for the duration of the command it runs.
fn staged_target_dir() -> (TempDir, Utf8PathBuf) {
    let staging = TempDir::new().expect("temporary staging directory should be created");
    let staging_dir = Dir::open_ambient_dir(
        camino::Utf8Path::from_path(staging.path()).expect("temporary path should be UTF-8"),
        ambient_authority(),
    )
    .expect("temporary staging directory should be opened");
    staging_dir
        .create_dir_all("nightly-test/release")
        .expect("release directory should be created");
    let library_name = format!(
        "{}whitaker_suite@nightly-test{}",
        library_prefix(),
        library_extension()
    );
    staging_dir
        .write(format!("nightly-test/release/{library_name}"), [])
        .expect("suite marker should be written");
    let target_dir = Utf8PathBuf::from_path_buf(staging.path().to_owned())
        .expect("temporary path should be UTF-8");
    (staging, target_dir)
}

#[test]
fn json_inventory_keeps_its_current_shape() {
    let (_staging, target_dir) = staged_target_dir();

    let output = Command::new(env!("CARGO_BIN_EXE_whitaker"))
        .args(["ls", "--json", "--target-dir"])
        .arg(target_dir.as_str())
        .output()
        .expect("whitaker ls process should start");
    assert!(
        output.status.success(),
        "ls should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("ls --json should produce valid JSON");
    let first_toolchain = actual
        .get("toolchains")
        .and_then(serde_json::Value::as_array)
        .and_then(|toolchains| toolchains.first())
        .and_then(|toolchain| toolchain.get("channel"));
    assert_eq!(
        first_toolchain,
        Some(&serde_json::Value::String("nightly-test".to_owned())),
        "snapshot fixture should contain the staged test toolchain",
    );
    insta::assert_json_snapshot!(actual, @r###"
    {
      "toolchains": [
        {
          "active": false,
          "channel": "nightly-test",
          "libraries": [
            {
              "lints": [
                "bumpy_road_function",
                "conditional_max_n_branches",
                "function_attrs_follow_docs",
                "module_max_lines",
                "module_must_have_inner_docs",
                "no_expect_outside_tests",
                "test_must_not_have_example",
                "no_std_fs_operations",
                "no_unwrap_or_else_panic",
                "string_continuation_style"
              ],
              "name": "whitaker_suite"
            }
          ]
        }
      ]
    }
    "###);
}
