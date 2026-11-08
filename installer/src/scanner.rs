//! Lint scanner for discovering installed libraries.
//!
//! This module provides utilities to scan the staging directory and parse
//! library filenames to extract lint metadata.

use std::{collections::BTreeMap, io};

use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    builder::{library_extension, library_prefix},
    crate_name::CrateName,
    resolution::{EXPERIMENTAL_LINT_CRATES, LINT_CRATES, SUITE_CRATE},
};

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
    pub fn is_empty(&self) -> bool { self.by_toolchain.is_empty() }
}

/// Scan the staging directory for installed libraries.
///
/// Supported staging directory structures are:
/// ```text
/// {target_dir}/{toolchain}/release/lib{crate}@{toolchain}.{ext}
/// {target_dir}/{toolchain}/{target}/lib/lib{crate}@{toolchain}.{ext}
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
    for entry_result in target_dir.read_dir_utf8()? {
        let entry = entry_result?;
        let toolchain_path = entry.path();

        if !toolchain_path.is_dir() {
            continue;
        }

        let toolchain = entry.file_name().to_owned();
        let libraries = scan_toolchain_layouts(toolchain_path, &toolchain)?;
        if !libraries.is_empty() {
            result.by_toolchain.insert(toolchain, libraries);
        }
    }

    Ok(result)
}

fn scan_toolchain_layouts(
    toolchain_path: &Utf8Path,
    toolchain: &str,
) -> io::Result<Vec<InstalledLibrary>> {
    let mut libraries = Vec::new();
    let release_path = toolchain_path.join("release");
    if release_path.is_dir() {
        libraries.extend(scan_toolchain_release(&release_path, toolchain)?);
    }
    for entry_result in toolchain_path.read_dir_utf8()? {
        let entry = entry_result?;
        if !entry.path().is_dir() || entry.file_name() == "release" {
            continue;
        }
        let lib_path = entry.path().join("lib");
        if lib_path.is_dir() && contains_libraries_in_layout(&lib_path)? {
            libraries.extend(scan_toolchain_release(&lib_path, toolchain)?);
        }
    }
    libraries.sort_by(|left, right| left.crate_name.as_str().cmp(right.crate_name.as_str()));
    Ok(libraries)
}

fn contains_libraries_in_layout(lib_path: &Utf8Path) -> io::Result<bool> {
    for entry_result in lib_path.read_dir_utf8()? {
        let entry = entry_result?;
        if entry.path().is_file() && parse_library_filename(entry.file_name()).is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Scan a single toolchain's release directory for libraries.
fn scan_toolchain_release(
    release_path: &Utf8Path,
    toolchain: &str,
) -> io::Result<Vec<InstalledLibrary>> {
    let mut libraries = Vec::new();

    for entry_result in release_path.read_dir_utf8()? {
        let entry = entry_result?;
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

    Ok(libraries)
}

/// Parse a library filename to extract crate name and toolchain.
///
/// Format: `{prefix}{crate_name}@{toolchain}{extension}`
///
/// # Examples
///
/// ```
/// use whitaker_installer::{
///     builder::{library_extension, library_prefix},
///     scanner::parse_library_filename,
/// };
///
/// let prefix = library_prefix();
/// let extension = library_extension();
/// let filename = format!("{prefix}module_max_lines@nightly-2026-05-28{extension}");
/// let result = parse_library_filename(&filename);
/// assert!(result.is_some());
/// let (crate_name, toolchain) = result.expect("valid library filename");
/// assert_eq!(crate_name.as_str(), "module_max_lines");
/// assert_eq!(toolchain, "nightly-2026-05-28");
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
    let (crate_name, toolchain) = without_ext.split_once('@')?;

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
/// use whitaker_installer::{crate_name::CrateName, scanner::lints_for_library};
///
/// let suite_lints = lints_for_library(&CrateName::from("whitaker_suite"));
/// assert!(suite_lints.len() > 1);
///
/// let single_lint = lints_for_library(&CrateName::from("module_max_lines"));
/// assert_eq!(single_lint, vec!["module_max_lines"]);
/// ```
#[must_use]
pub fn lints_for_library(crate_name: &CrateName) -> Vec<&'static str> {
    lints_for_library_with_experimental(crate_name, false)
}

/// Return the list of lints provided by a library with experimental opt-in.
///
/// When `include_experimental` is true and the library is the suite crate,
/// the returned list includes experimental lints that are bundled into the
/// suite. For non-suite crates, the behaviour matches [`lints_for_library`].
///
/// # Examples
///
/// ```
/// use whitaker_installer::{crate_name::CrateName, scanner::lints_for_library_with_experimental};
///
/// let lints = lints_for_library_with_experimental(&CrateName::from("whitaker_suite"), true);
/// assert!(lints.contains(&"bumpy_road_function"));
/// ```
#[must_use]
pub fn lints_for_library_with_experimental(
    crate_name: &CrateName,
    include_experimental: bool,
) -> Vec<&'static str> {
    let name = crate_name.as_str();

    if name == SUITE_CRATE {
        let mut lints = LINT_CRATES.to_vec();
        if include_experimental {
            lints.extend(EXPERIMENTAL_LINT_CRATES);
        }
        lints
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
#[path = "scanner_tests.rs"]
mod tests;
