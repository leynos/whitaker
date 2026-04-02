//! Unit tests for dependency-binary packaging helpers.

use crate::dependency_binaries::find_dependency_binary;
use crate::dependency_packaging::{
    ArchiveFormat, DependencyPackageParams, DependencyPackagingError, archive_format,
    inner_dir_name, package_dependency_binary, render_provenance_markdown,
    write_provenance_markdown,
};
use crate::installer_packaging::TargetTriple;
use rstest::{fixture, rstest};
use std::fs;

struct PackagingCase<'a> {
    package: &'a str,
    binary_name: &'a str,
    should_create_binary: bool,
    expect_success: bool,
}

#[fixture]
fn linux_target() -> TargetTriple {
    TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target")
}

#[fixture]
fn windows_target() -> TargetTriple {
    TargetTriple::try_from("x86_64-pc-windows-msvc").expect("valid target")
}

#[fixture]
fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn archive_format_matches_target_platform() {
    let linux = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");
    let windows = TargetTriple::try_from("x86_64-pc-windows-msvc").expect("valid target");

    assert_eq!(archive_format(&linux), ArchiveFormat::Tgz);
    assert_eq!(archive_format(&windows), ArchiveFormat::Zip);
}

#[test]
fn inner_dir_name_uses_dependency_version() {
    let dependency = find_dependency_binary("cargo-dylint")
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    let target = TargetTriple::try_from("x86_64-unknown-linux-gnu").expect("valid target");

    assert_eq!(
        inner_dir_name(dependency, &target),
        format!("cargo-dylint-{}-v{}", target, dependency.version())
    );
}

#[rstest]
#[case(PackagingCase {
    package: "dylint-link",
    binary_name: "missing",
    should_create_binary: false,
    expect_success: false,
})]
#[case(PackagingCase {
    package: "cargo-dylint",
    binary_name: "cargo-dylint",
    should_create_binary: true,
    expect_success: true,
})]
fn package_dependency_binary_handles_binary_presence(
    linux_target: TargetTriple,
    temp_dir: tempfile::TempDir,
    #[case] case: PackagingCase<'_>,
) {
    let dependency = find_dependency_binary(case.package)
        .expect("dependency manifest should load")
        .expect("dependency should exist");
    let binary_path = temp_dir.path().join(case.binary_name);
    if case.should_create_binary {
        fs::write(&binary_path, b"binary").expect("write fake binary");
    }

    let result = package_dependency_binary(DependencyPackageParams {
        dependency: dependency.clone(),
        target: linux_target.clone(),
        binary_path: binary_path.clone(),
        output_dir: temp_dir.path().join("dist"),
    });

    if case.expect_success {
        let output = result.expect("packaging should succeed");
        let expected_archive_name = format!(
            "{}-{}-v{}.tgz",
            dependency.package(),
            linux_target,
            dependency.version()
        );
        assert_eq!(output.archive_name, expected_archive_name);
        assert!(output.archive_path.is_file());
        assert!(output.archive_path.is_absolute());
    } else {
        let error = result.expect_err("missing binary should fail");
        assert!(matches!(
            error,
            DependencyPackagingError::BinaryNotFound(path) if path == binary_path
        ));
    }
}

#[test]
fn provenance_markdown_includes_all_dependency_fields() {
    let cargo_dylint = find_dependency_binary("cargo-dylint")
        .expect("dependency manifest should load")
        .expect("cargo-dylint should exist")
        .clone();
    let dylint_link = find_dependency_binary("dylint-link")
        .expect("dependency manifest should load")
        .expect("dylint-link should exist")
        .clone();

    // Save values before moving into vec
    let cargo_dylint_package = cargo_dylint.package().to_string();
    let cargo_dylint_version = cargo_dylint.version().to_string();
    let cargo_dylint_license = cargo_dylint.license().to_string();
    let cargo_dylint_repository = cargo_dylint.repository().to_string();
    let dylint_link_package = dylint_link.package().to_string();
    let dylint_link_version = dylint_link.version().to_string();
    let dylint_link_license = dylint_link.license().to_string();
    let dylint_link_repository = dylint_link.repository().to_string();

    let dependencies = vec![cargo_dylint, dylint_link];

    let markdown = render_provenance_markdown(&dependencies);

    // Check header and shared values
    assert!(markdown.contains("# Dependency binary licences and provenance"));
    assert!(markdown.contains("https://github.com/trailofbits/dylint"));

    // Check cargo-dylint fields
    assert!(markdown.contains(&cargo_dylint_package));
    assert!(markdown.contains(&cargo_dylint_version));
    assert!(markdown.contains(&cargo_dylint_license));
    assert!(markdown.contains(&cargo_dylint_repository));

    // Check dylint-link fields
    assert!(markdown.contains(&dylint_link_package));
    assert!(markdown.contains(&dylint_link_version));
    assert!(markdown.contains(&dylint_link_license));
    assert!(markdown.contains(&dylint_link_repository));
}

#[test]
fn write_provenance_markdown_writes_expected_file() {
    let dependencies = [
        find_dependency_binary("cargo-dylint")
            .expect("dependency manifest should load")
            .expect("dependency should exist")
            .clone(),
        find_dependency_binary("dylint-link")
            .expect("dependency manifest should load")
            .expect("dependency should exist")
            .clone(),
    ];
    let temp_dir = tempfile::tempdir().expect("temp dir");

    let path = write_provenance_markdown(temp_dir.path(), &dependencies)
        .expect("provenance file should be written");

    assert!(path.is_file());
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(crate::dependency_binaries::provenance_filename())
    );
    assert_eq!(
        fs::read_to_string(&path).expect("provenance should be readable"),
        render_provenance_markdown(&dependencies)
    );
}
