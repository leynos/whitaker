//! Tests for the root installation policy.
//!
//! The root policy installs `whitaker` itself rather than the
//! `whitaker-installer` dispatcher, so it must never overwrite the executable
//! it is running from and must not generate a lister script it cannot support.

use tempfile::TempDir;

use super::super::{WrapperScripts, generate_wrapper_scripts_with};

/// The root install must not overwrite the `whitaker` executable it runs from.
#[test]
fn root_policy_generation_preserves_an_existing_whitaker_binary() {
    let temp = TempDir::new().expect("temporary bin parent should be created");
    let bin_dir = temp.path().join("bin");
    let whitaker_path = bin_dir.join(if cfg!(windows) {
        "whitaker.ps1"
    } else {
        "whitaker"
    });
    std::fs::create_dir_all(&bin_dir).expect("bin directory should be created");
    std::fs::write(&whitaker_path, "installed binary sentinel")
        .expect("existing executable sentinel should be written");

    let mut dirs = crate::dirs::MockBaseDirs::new();
    let expected_bin_dir = bin_dir.clone();
    dirs.expect_executables()
        .return_once(move || Some(expected_bin_dir));

    let result = generate_wrapper_scripts_with(&dirs, WrapperScripts::None)
        .expect("the root policy needs no script to succeed");

    assert_eq!(
        std::fs::read(&whitaker_path).expect("existing executable should remain readable"),
        b"installed binary sentinel",
        "root install must not overwrite its own executable",
    );
    assert!(
        !result.whitaker_ls_path.exists(),
        "the lister script would need a separately installed whitaker-installer",
    );
}

/// The root policy writes nothing into an empty bin directory either.
#[test]
fn root_policy_generation_writes_no_scripts() {
    let temp = TempDir::new().expect("temporary bin parent should be created");
    let bin_dir = temp.path().join("bin");
    std::fs::create_dir_all(&bin_dir).expect("bin directory should be created");

    let mut dirs = crate::dirs::MockBaseDirs::new();
    let expected_bin_dir = bin_dir.clone();
    dirs.expect_executables()
        .return_once(move || Some(expected_bin_dir));

    let result = generate_wrapper_scripts_with(&dirs, WrapperScripts::None)
        .expect("the root policy needs no script to succeed");

    assert!(
        !result.whitaker_path.exists() && !result.whitaker_ls_path.exists(),
        "the root policy must leave the bin directory empty",
    );
}
