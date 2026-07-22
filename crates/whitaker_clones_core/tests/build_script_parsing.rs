//! Verifies the build script's parser dependency extraction rules.

#[path = "../build_support.rs"]
mod build_support;

use build_support::{
    exact_version, find_workspace_manifest, is_workspace_manifest, parser_dependency_requirement,
    read_workspace_manifest,
};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use proptest::prelude::*;
use rstest::rstest;
use tempfile::tempdir;

/// Opens `directory` as a capability-scoped `cap_std` handle over its UTF-8
/// path, so fixtures write through `cap_std::fs_utf8` rather than ambient
/// `std::fs`. Returns the directory's UTF-8 root and the handle.
fn open_fixture_root(
    directory: &std::path::Path,
) -> Result<(Utf8PathBuf, Dir), Box<dyn std::error::Error>> {
    let root = Utf8Path::from_path(directory).ok_or("temporary path is not valid UTF-8")?;
    let root_dir = Dir::open_ambient_dir(root, ambient_authority())?;
    Ok((root.to_owned(), root_dir))
}

#[rstest]
#[case::inline("[workspace.dependencies]\nra_ap_syntax = \"=0.0.334\"\n", "=0.0.334")]
#[case::table(
    "[workspace.dependencies.ra_ap_syntax]\nversion = \"=0.0.334\"\noptional = true\n",
    "=0.0.334"
)]
fn extracts_supported_dependency_forms(#[case] manifest: &str, #[case] expected: &str) {
    assert_eq!(
        parser_dependency_requirement(manifest).expect("dependency requirement should parse"),
        expected
    );
}

#[rstest]
#[case::missing("[workspace.dependencies]\nserde = \"1\"\n", "is missing")]
#[case::missing_version(
    "[workspace.dependencies.ra_ap_syntax]\noptional = true\n",
    "has no version string"
)]
fn rejects_invalid_dependency_forms(#[case] manifest: &str, #[case] expected: &str) {
    let error = parser_dependency_requirement(manifest)
        .expect_err("invalid dependency form should be rejected");

    assert!(error.to_string().contains(expected));
}

#[rstest]
#[case::caret("0.0.334")]
#[case::caret_explicit("^0.0.334")]
#[case::empty_exact("=")]
fn rejects_non_exact_requirements(#[case] requirement: &str) {
    assert!(exact_version(requirement).is_err());
}

#[rstest]
fn accepts_exact_requirement() {
    assert_eq!(
        exact_version("=0.0.334").expect("exact requirement should parse"),
        "0.0.334"
    );
}

#[rstest]
fn finds_nearest_workspace_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let (root, root_dir) = open_fixture_root(directory.path())?;
    root_dir.create_dir_all("nested-workspace/member")?;
    root_dir.write("Cargo.toml", "[workspace]\nmembers = []\n")?;
    root_dir.write("nested-workspace/Cargo.toml", "[workspace]\nmembers = []\n")?;
    root_dir.write(
        "nested-workspace/member/Cargo.toml",
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
    )?;

    let member = root.join("nested-workspace").join("member");
    let workspace = root.join("nested-workspace").join("Cargo.toml");

    assert_eq!(find_workspace_manifest(&member)?, workspace);
    Ok(())
}

#[rstest]
fn ignores_non_workspace_manifests() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let (root, root_dir) = open_fixture_root(directory.path())?;
    root_dir.write(
        "Cargo.toml",
        "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
    )?;

    let manifest = root.join("Cargo.toml");

    assert!(!is_workspace_manifest(&manifest)?);
    Ok(())
}

#[rstest]
fn reports_missing_workspace_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let (root, root_dir) = open_fixture_root(directory.path())?;
    root_dir.create_dir("member")?;

    let nested = root.join("member");
    let error = find_workspace_manifest(&nested)
        .expect_err("a directory without a workspace manifest should fail");

    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::NotFound)
    );
    Ok(())
}

#[rstest]
fn reads_a_located_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let (root, root_dir) = open_fixture_root(directory.path())?;
    let contents = "[workspace]\nmembers = []\n";
    root_dir.write("Cargo.toml", contents)?;

    let manifest = root.join("Cargo.toml");

    assert_eq!(read_workspace_manifest(&manifest)?, contents);
    Ok(())
}

#[rstest]
fn read_of_absent_manifest_reports_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let manifest = directory.path().join("Cargo.toml");

    let manifest = Utf8PathBuf::from_path_buf(manifest).expect("temporary paths must be UTF-8");
    let error =
        read_workspace_manifest(&manifest).expect_err("reading an absent manifest should fail");

    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::NotFound)
    );
    Ok(())
}

proptest! {
    #[test]
    fn exact_version_accepts_only_non_empty_exact_pins(
        prefix in prop_oneof![Just("=".to_owned()), Just("^".to_owned()), Just("".to_owned())],
        suffix in "[A-Za-z0-9._-]{0,32}",
    ) {
        let requirement = format!("{prefix}{suffix}");
        let is_exact_pin = prefix == "=" && !suffix.is_empty();

        prop_assert_eq!(exact_version(&requirement).is_ok(), is_exact_pin);
    }
}
