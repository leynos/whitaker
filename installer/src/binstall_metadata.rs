//! Cargo-binstall metadata constants and template expansion helpers.
//!
//! This module defines the expected cargo-binstall metadata values for the
//! `whitaker-installer` crate, as specified in the design document
//! (§ Installer release artefacts). The constants serve as a single source
//! of truth for both unit tests and BDD scenarios.

/// The GitHub repository URL prefix used in the `pkg-url` template.
pub const REPO_URL: &str = "https://github.com/leynos/whitaker";

/// The expected `pkg-url` template from `[package.metadata.binstall]`.
pub const PKG_URL_TEMPLATE: &str = concat!(
    "https://github.com/leynos/whitaker/releases/download/",
    "v{version}/{name}-{target}-v{version}.{archive-format}"
);

/// The expected `bin-dir` template from `[package.metadata.binstall]`.
pub const BIN_DIR_TEMPLATE: &str = "{name}-{target}-v{version}/{bin}";

/// The default `pkg-fmt` value.
pub const DEFAULT_PKG_FMT: &str = "tgz";

/// The `pkg-fmt` override for Windows targets.
pub const WINDOWS_PKG_FMT: &str = "zip";

/// The Windows target triple that receives the `pkg-fmt` override.
pub const WINDOWS_OVERRIDE_TARGET: &str = "x86_64-pc-windows-msvc";

/// Expand the `pkg-url` template for a given version and target.
///
/// Replaces `{name}`, `{version}`, `{target}`, and `{archive-format}`
/// with the supplied values. The `{archive-format}` is derived from the
/// `pkg-fmt` for the target: `"tgz"` for all targets except
/// `x86_64-pc-windows-msvc`, which uses `"zip"`.
///
/// # Examples
///
/// ```
/// use whitaker_installer::binstall_metadata::expand_pkg_url;
///
/// let url = expand_pkg_url("0.2.0", "x86_64-unknown-linux-gnu");
/// assert!(url.ends_with(".tgz"));
/// ```
#[must_use]
pub fn expand_pkg_url(version: &str, target: &str) -> String {
    let archive_format = if target == WINDOWS_OVERRIDE_TARGET {
        WINDOWS_PKG_FMT
    } else {
        DEFAULT_PKG_FMT
    };
    PKG_URL_TEMPLATE
        .replace("{name}", "whitaker-installer")
        .replace("{version}", version)
        .replace("{target}", target)
        .replace("{archive-format}", archive_format)
}

/// Expand the `bin-dir` template for a given version and target.
///
/// Replaces `{name}`, `{version}`, `{target}`, and `{bin}` with the
/// supplied values. The `{bin}` value is `whitaker-installer.exe` for
/// Windows targets and `whitaker-installer` otherwise.
///
/// # Examples
///
/// ```
/// use whitaker_installer::binstall_metadata::expand_bin_dir;
///
/// let dir = expand_bin_dir("0.2.0", "x86_64-unknown-linux-gnu");
/// assert!(dir.ends_with("/whitaker-installer"));
/// ```
#[must_use]
pub fn expand_bin_dir(version: &str, target: &str) -> String {
    let bin = if target == WINDOWS_OVERRIDE_TARGET {
        "whitaker-installer.exe"
    } else {
        "whitaker-installer"
    };
    BIN_DIR_TEMPLATE
        .replace("{name}", "whitaker-installer")
        .replace("{version}", version)
        .replace("{target}", target)
        .replace("{bin}", bin)
}

// ---------------------------------------------------------------------------
// Test helpers (available to unit tests and integration tests via
// the `test-support` feature)
// ---------------------------------------------------------------------------

/// Load and parse the installer's `Cargo.toml`.
///
/// Returns the full TOML table for `installer/Cargo.toml`, located via
/// `CARGO_MANIFEST_DIR`. This helper is shared by unit tests and
/// behaviour-driven scenarios to avoid duplicating manifest-loading logic.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn load_cargo_toml() -> toml::Table {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml_path = manifest_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", cargo_toml_path.display()));
    content.parse::<toml::Table>().unwrap_or_else(|err| {
        panic!(
            "failed to parse {} as TOML: {err}",
            cargo_toml_path.display()
        )
    })
}

/// Extract the `[package.metadata.binstall]` sub-table from a parsed
/// `Cargo.toml`.
///
/// Panics if the expected table path is missing.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn extract_binstall_table(table: &toml::Table) -> toml::Table {
    table
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("binstall"))
        .and_then(|b| b.as_table())
        .expect("[package.metadata.binstall] table not found")
        .clone()
}

#[cfg(test)]
#[path = "binstall_metadata_tests.rs"]
mod tests;
