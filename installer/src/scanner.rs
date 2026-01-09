//! Lint scanner for discovering installed libraries.
//!
//! This module provides utilities to scan the staging directory and parse
//! library filenames to extract lint metadata.

use std::collections::BTreeMap;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};

use crate::builder::{library_extension, library_prefix};
use crate::crate_name::CrateName;
use crate::resolution::{EXPERIMENTAL_LINT_CRATES, LINT_CRATES, SUITE_CRATE};

/// Metadata about an installed lint library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLibrary {
    /// Name of the library crate.
    pub crate_name: CrateName,
    /// Toolchain the library was built for.
    pub toolchain: String,
    /// Full path to the library file.
    pub path: Utf8PathBuf,
}

/// Metadata about installed lints grouped by toolchain.
#[derive(Debug, Clone, Default)]
pub struct InstalledLints {
    /// Map from toolchain channel to installed libraries.
    pub by_toolchain: BTreeMap<String, Vec<InstalledLibrary>>,
}

impl InstalledLints {
    /// Returns true if no lints are installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_toolchain.is_empty()
    }
}

/// Scan the staging directory for installed libraries.
///
/// The staging directory structure is:
/// ```text
/// {target_dir}/{toolchain}/release/lib{crate}@{toolchain}.{ext}
/// ```
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn scan_installed(target_dir: &Utf8Path) -> io::Result<InstalledLints> {
    let mut result = InstalledLints::default();

    if !target_dir.exists() {
        return Ok(result);
    }

    // Iterate over toolchain subdirectories
    for entry in target_dir.read_dir_utf8()? {
        let entry = entry?;
        let toolchain_path = entry.path();

        if !toolchain_path.is_dir() {
            continue;
        }

        let toolchain = entry.file_name().to_owned();
        let release_path = toolchain_path.join("release");

        if !release_path.exists() || !release_path.is_dir() {
            continue;
        }

        let libraries = scan_toolchain_release(&release_path, &toolchain)?;
        if !libraries.is_empty() {
            result.by_toolchain.insert(toolchain, libraries);
        }
    }

    Ok(result)
}

/// Scan a single toolchain's release directory for libraries.
fn scan_toolchain_release(
    release_path: &Utf8Path,
    toolchain: &str,
) -> io::Result<Vec<InstalledLibrary>> {
    let mut libraries = Vec::new();

    for entry in release_path.read_dir_utf8()? {
        let entry = entry?;
        let file_name = entry.file_name();

        if let Some((crate_name, parsed_toolchain)) = parse_library_filename(file_name) {
            // Only include libraries matching this toolchain
            if parsed_toolchain == toolchain {
                libraries.push(InstalledLibrary {
                    crate_name,
                    toolchain: parsed_toolchain,
                    path: entry.path().to_owned(),
                });
            }
        }
    }

    // Sort by crate name for consistent output
    libraries.sort_by_key(|lib| lib.crate_name.as_str().to_owned());

    Ok(libraries)
}

/// Parse a library filename to extract crate name and toolchain.
///
/// Format: `{prefix}{crate_name}@{toolchain}{extension}`
///
/// # Examples
///
/// ```
/// use whitaker_installer::scanner::parse_library_filename;
///
/// let result = parse_library_filename("libmodule_max_lines@nightly-2025-09-18.so");
/// assert!(result.is_some());
/// let (crate_name, toolchain) = result.expect("valid library filename");
/// assert_eq!(crate_name.as_str(), "module_max_lines");
/// assert_eq!(toolchain, "nightly-2025-09-18");
/// ```
#[must_use]
pub fn parse_library_filename(filename: &str) -> Option<(CrateName, String)> {
    let prefix = library_prefix();
    let extension = library_extension();

    // Strip prefix
    let without_prefix = filename.strip_prefix(prefix)?;

    // Strip extension
    let without_ext = without_prefix.strip_suffix(extension)?;

    // Split on @ to get crate name and toolchain
    let at_pos = without_ext.find('@')?;
    let crate_name = &without_ext[..at_pos];
    let toolchain = &without_ext[at_pos + 1..];

    if crate_name.is_empty() || toolchain.is_empty() {
        return None;
    }

    Some((CrateName::from(crate_name), toolchain.to_owned()))
}

/// Return the list of lints provided by a library.
///
/// For the suite library, returns all standard lints. Experimental lints are
/// only reported when installed as individual crates, not when part of the
/// suite, because the suite may or may not have been built with experimental
/// features enabled.
///
/// For individual lint crates, returns a single-element vector with the lint
/// name.
///
/// # Examples
///
/// ```
/// use whitaker_installer::scanner::lints_for_library;
/// use whitaker_installer::crate_name::CrateName;
///
/// let suite_lints = lints_for_library(&CrateName::from("suite"));
/// assert!(suite_lints.len() > 1);
///
/// let single_lint = lints_for_library(&CrateName::from("module_max_lines"));
/// assert_eq!(single_lint, vec!["module_max_lines"]);
/// ```
#[must_use]
pub fn lints_for_library(crate_name: &CrateName) -> Vec<&'static str> {
    let name = crate_name.as_str();

    if name == SUITE_CRATE {
        // Suite contains standard lints; experimental lints may or may not be
        // present depending on build-time flags, so we report only standard.
        LINT_CRATES.to_vec()
    } else if let Some(&static_name) = LINT_CRATES.iter().find(|&&s| s == name) {
        // Individual lint crate - 1:1 mapping; return static reference
        vec![static_name]
    } else if let Some(&static_name) = EXPERIMENTAL_LINT_CRATES.iter().find(|&&s| s == name) {
        // Experimental lint crate - 1:1 mapping; return static reference
        vec![static_name]
    } else {
        // Unknown crate
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::TempDir;

    /// Skip test execution on non-Linux platforms where library extensions differ.
    macro_rules! skip_unless_linux {
        () => {
            if !cfg!(target_os = "linux") {
                return;
            }
        };
    }

    #[rstest]
    #[case::standard_linux(
        "libmodule_max_lines@nightly-2025-09-18.so",
        "module_max_lines",
        "nightly-2025-09-18"
    )]
    #[case::suite("libsuite@nightly-2025-09-18.so", "suite", "nightly-2025-09-18")]
    #[case::stable_toolchain(
        "libno_expect_outside_tests@stable-1.80.0.so",
        "no_expect_outside_tests",
        "stable-1.80.0"
    )]
    fn parse_library_filename_valid(
        #[case] filename: &str,
        #[case] expected_crate: &str,
        #[case] expected_toolchain: &str,
    ) {
        skip_unless_linux!();

        let result = parse_library_filename(filename);
        assert!(result.is_some(), "expected Some for {filename}");

        let (crate_name, toolchain) = result.expect("already checked");
        assert_eq!(crate_name.as_str(), expected_crate);
        assert_eq!(toolchain, expected_toolchain);
    }

    #[rstest]
    #[case::no_at_sign("libmodule_max_lines.so")]
    #[case::empty_crate("lib@nightly-2025-09-18.so")]
    #[case::empty_toolchain("libmodule_max_lines@.so")]
    #[case::wrong_prefix("module_max_lines@nightly-2025-09-18.so")]
    #[case::wrong_extension("libmodule_max_lines@nightly-2025-09-18.dll")]
    #[case::random_file("readme.txt")]
    fn parse_library_filename_invalid(#[case] filename: &str) {
        skip_unless_linux!();

        let result = parse_library_filename(filename);
        assert!(result.is_none(), "expected None for {filename}");
    }

    #[test]
    fn lints_for_suite_returns_standard_lints_only() {
        let lints = lints_for_library(&CrateName::from("suite"));
        // Suite reports only standard lints; experimental lints depend on build flags
        assert_eq!(lints.len(), LINT_CRATES.len());

        for lint in LINT_CRATES {
            assert!(lints.contains(lint), "missing standard lint: {lint}");
        }
        for lint in EXPERIMENTAL_LINT_CRATES {
            assert!(
                !lints.contains(lint),
                "suite should not report experimental lint: {lint}"
            );
        }
    }

    #[test]
    fn lints_for_individual_crate_returns_single_lint() {
        let lints = lints_for_library(&CrateName::from("module_max_lines"));
        assert_eq!(lints, vec!["module_max_lines"]);
    }

    #[test]
    fn lints_for_experimental_crate_returns_single_lint() {
        let lints = lints_for_library(&CrateName::from("bumpy_road_function"));
        assert_eq!(lints, vec!["bumpy_road_function"]);
    }

    #[test]
    fn lints_for_unknown_crate_returns_empty() {
        let lints = lints_for_library(&CrateName::from("unknown_crate"));
        assert!(lints.is_empty());
    }

    #[test]
    fn scan_empty_directory_returns_empty() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let target_dir = Utf8Path::from_path(temp.path()).expect("non-UTF8 path");

        let result = scan_installed(target_dir).expect("scan should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn scan_nonexistent_directory_returns_empty() {
        let result =
            scan_installed(Utf8Path::new("/nonexistent/path")).expect("scan should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn scan_finds_installed_libraries() {
        skip_unless_linux!();

        let temp = TempDir::new().expect("failed to create temp dir");
        let target_dir = Utf8Path::from_path(temp.path()).expect("non-UTF8 path");

        // Create toolchain directory structure
        let toolchain = "nightly-2025-09-18";
        let release_dir = target_dir.join(toolchain).join("release");
        std::fs::create_dir_all(&release_dir).expect("failed to create dirs");

        // Create fake library files
        let lib_name = format!("libsuite@{toolchain}.so");
        std::fs::write(release_dir.join(&lib_name), b"fake").expect("failed to write file");

        let result = scan_installed(target_dir).expect("scan should succeed");
        assert!(!result.is_empty());
        assert!(result.by_toolchain.contains_key(toolchain));

        let libs = result
            .by_toolchain
            .get(toolchain)
            .expect("toolchain should exist");
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].crate_name.as_str(), "suite");
        assert_eq!(libs[0].toolchain, toolchain);
    }
}
