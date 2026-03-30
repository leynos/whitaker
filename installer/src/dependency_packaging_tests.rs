//! Unit tests for dependency-binary packaging helpers.

use crate::dependency_binaries::find_dependency_binary;
use crate::dependency_packaging::{
    ArchiveFormat, DependencyPackageParams, archive_format, inner_dir_name,
    package_dependency_binary, render_provenance_markdown,
};
use crate::installer_packaging::TargetTriple;
use std::fs;

#[test]
fn archive_format_matches_target_platform() {
    let linux = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let windows = TargetTriple::try_from("x86_64-pc-windows-msvc").expect("valid target");

    assert_eq!(archive_format(&linux), ArchiveFormat::Tgz);
    assert_eq!(archive_format(&windows), ArchiveFormat::Zip);
}

#[test]
fn inner_dir_name_uses_dependency_version() {
    let dependency = find_dependency_binary("cargo-dylint").expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");

    assert_eq!(
        inner_dir_name(dependency, &target),
        "cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0"
    );
}

#[test]
fn package_dependency_binary_rejects_missing_binary() {
    let dependency = find_dependency_binary("dylint-link").expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let temp_dir = tempfile::tempdir().expect("temp dir");

    let error = package_dependency_binary(DependencyPackageParams {
        dependency: dependency.clone(),
        target,
        binary_path: temp_dir.path().join("missing"),
        output_dir: temp_dir.path().join("dist"),
    })
    .expect_err("missing binary should fail");

    assert!(error.to_string().contains("binary file not found"));
}

#[test]
fn package_dependency_binary_creates_expected_archive() {
    let dependency = find_dependency_binary("cargo-dylint").expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let binary_path = temp_dir.path().join("cargo-dylint");
    fs::write(&binary_path, b"binary").expect("write fake binary");

    let output = package_dependency_binary(DependencyPackageParams {
        dependency: dependency.clone(),
        target: target.clone(),
        binary_path,
        output_dir: temp_dir.path().join("dist"),
    })
    .expect("packaging should succeed");

    assert_eq!(
        output.archive_name,
        "cargo-dylint-x86_64-unknown-linux-gnu-v4.1.0.tgz"
    );
    assert!(output.archive_path.is_file());
}

#[test]
fn provenance_markdown_includes_all_dependency_fields() {
    let dependencies = [
        find_dependency_binary("cargo-dylint")
            .expect("dependency should exist")
            .clone(),
        find_dependency_binary("dylint-link")
            .expect("dependency should exist")
            .clone(),
    ];

    let markdown = render_provenance_markdown(&dependencies);

    assert!(markdown.contains("# Dependency binary licences and provenance"));
    assert!(markdown.contains("cargo-dylint"));
    assert!(markdown.contains("dylint-link"));
    assert!(markdown.contains("MIT OR Apache-2.0"));
    assert!(markdown.contains("https://github.com/trailofbits/dylint"));
}
