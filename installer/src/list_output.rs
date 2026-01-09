//! Output formatting for lint listing.
//!
//! This module provides utilities to format installed lint information for
//! human-readable or JSON output.

use serde::Serialize;

use crate::scanner::{InstalledLints, lints_for_library};

/// Format installed lints for human-readable output.
///
/// # Examples
///
/// ```
/// use whitaker_installer::list_output::format_human;
/// use whitaker_installer::scanner::InstalledLints;
///
/// let lints = InstalledLints::default();
/// let output = format_human(&lints, None);
/// assert!(output.contains("No lints installed"));
/// ```
#[must_use]
pub fn format_human(lints: &InstalledLints, active_toolchain: Option<&str>) -> String {
    if lints.is_empty() {
        return String::from(
            "No lints installed.\n\nRun `whitaker-installer` to install the default lint suite.",
        );
    }

    let mut output = String::from("Installed lints:\n");

    for (toolchain, libraries) in &lints.by_toolchain {
        output.push('\n');

        let active_marker = active_toolchain
            .filter(|active| *active == toolchain)
            .map_or(String::new(), |_| " (active)".to_owned());

        output.push_str(&format!("Toolchain: {toolchain}{active_marker}\n"));
        output.push_str("  Libraries:\n");

        for library in libraries {
            output.push_str(&format!("    {}\n", library.crate_name));

            let lint_names = lints_for_library(&library.crate_name);
            for lint in lint_names {
                output.push_str(&format!("      - {lint}\n"));
            }
        }
    }

    output
}

/// Format installed lints as JSON.
///
/// # Examples
///
/// ```
/// use whitaker_installer::list_output::format_json;
/// use whitaker_installer::scanner::InstalledLints;
///
/// let lints = InstalledLints::default();
/// let json = format_json(&lints, None);
/// assert!(json.contains("\"toolchains\""));
/// ```
#[must_use]
pub fn format_json(lints: &InstalledLints, active_toolchain: Option<&str>) -> String {
    let json_data = InstalledLintsJson::from_installed(lints, active_toolchain);

    // Use pretty printing for readability
    serde_json::to_string_pretty(&json_data).unwrap_or_else(|_| "{}".to_owned())
}

/// JSON-serializable representation of installed lints.
#[derive(Debug, Serialize)]
pub struct InstalledLintsJson {
    /// List of toolchains with installed lints.
    pub toolchains: Vec<ToolchainEntry>,
}

impl InstalledLintsJson {
    /// Create from `InstalledLints`.
    fn from_installed(lints: &InstalledLints, active_toolchain: Option<&str>) -> Self {
        let toolchains = lints
            .by_toolchain
            .iter()
            .map(|(toolchain, libraries)| {
                let active = active_toolchain.is_some_and(|active| active == toolchain);

                let libs = libraries
                    .iter()
                    .map(|lib| {
                        let lint_names = lints_for_library(&lib.crate_name);
                        LibraryEntry {
                            name: lib.crate_name.as_str().to_owned(),
                            lints: lint_names.iter().map(|s| (*s).to_owned()).collect(),
                        }
                    })
                    .collect();

                ToolchainEntry {
                    channel: toolchain.clone(),
                    active,
                    libraries: libs,
                }
            })
            .collect();

        Self { toolchains }
    }
}

/// JSON entry for a toolchain.
#[derive(Debug, Serialize)]
pub struct ToolchainEntry {
    /// Toolchain channel name.
    pub channel: String,
    /// Whether this is the currently active toolchain.
    pub active: bool,
    /// Libraries installed for this toolchain.
    pub libraries: Vec<LibraryEntry>,
}

/// JSON entry for a library.
#[derive(Debug, Serialize)]
pub struct LibraryEntry {
    /// Library crate name.
    pub name: String,
    /// Lints provided by this library.
    pub lints: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::CrateName;
    use crate::scanner::InstalledLibrary;
    use camino::Utf8PathBuf;
    use std::collections::BTreeMap;

    fn sample_lints() -> InstalledLints {
        let mut by_toolchain = BTreeMap::new();
        by_toolchain.insert(
            "nightly-2025-09-18".to_owned(),
            vec![InstalledLibrary {
                crate_name: CrateName::from("whitaker_suite"),
                toolchain: "nightly-2025-09-18".to_owned(),
                path: Utf8PathBuf::from("/fake/path/libwhitaker_suite@nightly-2025-09-18.so"),
            }],
        );
        InstalledLints { by_toolchain }
    }

    #[test]
    fn format_human_empty_shows_no_lints() {
        let lints = InstalledLints::default();
        let output = format_human(&lints, None);
        assert!(output.contains("No lints installed"));
        assert!(output.contains("whitaker-installer"));
    }

    #[test]
    fn format_human_shows_toolchain_and_lints() {
        let lints = sample_lints();
        let output = format_human(&lints, None);

        assert!(output.contains("Installed lints:"));
        assert!(output.contains("Toolchain: nightly-2025-09-18"));
        assert!(output.contains("whitaker_suite"));
        assert!(output.contains("module_max_lines"));
    }

    #[test]
    fn format_human_marks_active_toolchain() {
        let lints = sample_lints();
        let output = format_human(&lints, Some("nightly-2025-09-18"));

        assert!(output.contains("(active)"));
    }

    #[test]
    fn format_human_does_not_mark_inactive_toolchain() {
        let lints = sample_lints();
        let output = format_human(&lints, Some("other-toolchain"));

        assert!(!output.contains("(active)"));
    }

    #[test]
    fn format_json_empty_has_empty_toolchains() {
        let lints = InstalledLints::default();
        let json = format_json(&lints, None);

        assert!(json.contains("\"toolchains\""));
        assert!(json.contains("[]"));
    }

    #[test]
    fn format_json_includes_all_fields() {
        let lints = sample_lints();
        let json = format_json(&lints, Some("nightly-2025-09-18"));

        assert!(json.contains("\"channel\""));
        assert!(json.contains("\"active\": true"));
        assert!(json.contains("\"libraries\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"lints\""));
        assert!(json.contains("\"whitaker_suite\""));
    }

    #[test]
    fn format_json_is_valid_json() {
        let lints = sample_lints();
        let json = format_json(&lints, None);

        let parsed: serde_json::Value = serde_json::from_str(&json).expect("should be valid JSON");
        assert!(parsed.is_object());
        assert!(parsed.get("toolchains").is_some());
    }
}
