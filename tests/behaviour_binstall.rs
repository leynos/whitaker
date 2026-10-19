//! Behavioural checks for the root package's cargo-binstall metadata.

use std::path::Path;

use rstest::rstest;
use toml::Value;
use whitaker_installer::binstall_metadata as metadata;

const RELEASE_TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    metadata::WINDOWS_OVERRIDE_TARGET,
];

#[rstest]
fn whitaker_package_metadata_matches_binstall_templates() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = metadata::load_cargo_toml_from(&manifest_path).expect("root manifest");
    let package = manifest
        .get("package")
        .and_then(Value::as_table)
        .expect("package table");
    let package_name = package
        .get("name")
        .and_then(Value::as_str)
        .expect("package name");
    let version = package
        .get("version")
        .and_then(Value::as_str)
        .expect("package version");
    assert_eq!(package_name, "whitaker");

    let binstall = metadata::extract_binstall_table(&manifest).expect("binstall metadata");
    for (key, expected) in [
        ("pkg-url", metadata::PKG_URL_TEMPLATE),
        ("bin-dir", metadata::BIN_DIR_TEMPLATE),
        ("pkg-fmt", metadata::DEFAULT_PKG_FMT),
    ] {
        assert_eq!(
            binstall.get(key).and_then(Value::as_str),
            Some(expected),
            "root {key}"
        );
    }

    let overrides = binstall
        .get("overrides")
        .and_then(Value::as_table)
        .expect("root overrides");
    assert_eq!(overrides.len(), 1);
    let windows_override = overrides
        .get(metadata::WINDOWS_OVERRIDE_TARGET)
        .and_then(Value::as_table)
        .expect("Windows package override");
    assert_eq!(
        windows_override.get("pkg-fmt").and_then(Value::as_str),
        Some(metadata::WINDOWS_PKG_FMT)
    );

    for target in RELEASE_TARGETS {
        let release = metadata::BinstallRelease {
            package_name,
            version,
            target,
        };
        assert_ne!(
            metadata::expand_pkg_url_for(release),
            metadata::expand_pkg_url_for(metadata::BinstallRelease {
                package_name: "whitaker-installer",
                ..release
            }),
            "package URLs must remain distinct for release target {target}"
        );
    }
}
