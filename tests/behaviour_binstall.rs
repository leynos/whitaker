//! Behavioural checks for the root package's cargo-binstall metadata.

use std::path::Path;

use rstest::rstest;
use toml::{Table, Value};
use whitaker_installer::binstall_metadata::{
    BIN_DIR_TEMPLATE,
    DEFAULT_PKG_FMT,
    PKG_URL_TEMPLATE,
    WINDOWS_OVERRIDE_TARGET,
    WINDOWS_PKG_FMT,
    expand_pkg_url_for,
    extract_binstall_table,
    load_cargo_toml_from,
};

const RELEASE_TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    WINDOWS_OVERRIDE_TARGET,
];

fn required_table<'a>(table: &'a Table, key: &str) -> Result<&'a Table, String> {
    table
        .get(key)
        .and_then(Value::as_table)
        .ok_or_else(|| format!("{key} table not found"))
}

fn required_string<'a>(table: &'a Table, key: &str) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} string not found"))
}

#[rstest]
fn whitaker_package_metadata() {
    let validation = validate_whitaker_package_metadata();
    assert!(
        validation.is_ok(),
        "root package metadata is inconsistent: {validation:?}"
    );
}

fn validate_whitaker_package_metadata() -> Result<(), String> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = load_cargo_toml_from(&manifest_path)?;
    let package = required_table(&manifest, "package")?;
    let package_name = required_string(package, "name")?;
    let version = required_string(package, "version")?;
    let binstall = extract_binstall_table(&manifest)?;

    if package_name != "whitaker" {
        return Err(format!("root package name changed to `{package_name}`"));
    }
    if required_string(&binstall, "pkg-url")? != PKG_URL_TEMPLATE {
        return Err("root pkg-url must match the shared template".to_owned());
    }
    if required_string(&binstall, "bin-dir")? != BIN_DIR_TEMPLATE {
        return Err("root bin-dir must match the shared template".to_owned());
    }
    if required_string(&binstall, "pkg-fmt")? != DEFAULT_PKG_FMT {
        return Err("root package default archive format changed".to_owned());
    }

    let overrides = required_table(&binstall, "overrides")?;
    let windows = required_table(overrides, WINDOWS_OVERRIDE_TARGET)?;
    if required_string(windows, "pkg-fmt")? != WINDOWS_PKG_FMT {
        return Err("root Windows archive override changed".to_owned());
    }
    if overrides.len() != 1 {
        return Err(format!(
            "expected one root override, found {}",
            overrides.len()
        ));
    }

    for target in RELEASE_TARGETS {
        let whitaker_url = expand_pkg_url_for(package_name, version, target);
        let installer_url = expand_pkg_url_for("whitaker-installer", version, target);
        if whitaker_url == installer_url {
            return Err(format!(
                "package URLs must remain distinct for release target {target}"
            ));
        }
    }

    Ok(())
}
