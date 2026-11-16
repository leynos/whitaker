//! The suite features that turn experimental lints on for a source build.
//!
//! `whitaker-installer --experimental` builds the suite with every experimental
//! lint enabled. The feature that does so changed shape in 0.3.0: a suite from
//! 0.3.0 onward declares `experimental-enable-all`, while an older one,
//! reachable through `--suite-version`, declares one `experimental-<lint>`
//! feature per lint it already shipped. Cargo rejects a feature the manifest
//! does not declare, so the choice is read from the suite's own manifest
//! rather than assumed.
//!
//! Scope and reuse policy: this module only reads `suite/Cargo.toml` and names
//! features. It owns no build policy; the builder composes its answer into the
//! feature list it passes to Cargo.

use camino::Utf8Path;

use crate::{
    error::{InstallerError, Result},
    resolution::EXPERIMENTAL_LINT_CRATES,
};

/// The suite feature, from 0.3.0, that starts every experimental lint enabled.
pub(crate) const ENABLE_ALL_FEATURE: &str = "experimental-enable-all";

/// The comma-separated experimental features the suite at `workspace_root`
/// declares, or an empty string when it declares none.
///
/// A suite that declares [`ENABLE_ALL_FEATURE`] gets only that. An older one
/// gets whichever per-lint `experimental-<lint>` features it declares.
///
/// # Errors
///
/// Returns an error when `suite/Cargo.toml` cannot be read or parsed.
///
/// # Examples
///
/// ```ignore
/// let features = experimental_features(Utf8Path::new("/work/whitaker"))?;
/// assert_eq!(features, "experimental-enable-all");
/// ```
pub(crate) fn experimental_features(workspace_root: &Utf8Path) -> Result<String> {
    let manifest = workspace_root.join("suite").join("Cargo.toml");
    let declared = declared_features(&manifest)?;
    Ok(select_features(&declared))
}

/// Pick the experimental features out of the features a suite declares.
fn select_features(declared: &[String]) -> String {
    if declared.iter().any(|name| name == ENABLE_ALL_FEATURE) {
        return ENABLE_ALL_FEATURE.to_owned();
    }
    EXPERIMENTAL_LINT_CRATES
        .iter()
        .map(|lint| format!("experimental-{}", lint.replace('_', "-")))
        .filter(|feature| declared.contains(feature))
        .collect::<Vec<_>>()
        .join(",")
}

/// The names under `[features]` in the manifest at `manifest`.
fn declared_features(manifest: &Utf8Path) -> Result<Vec<String>> {
    let contents = std::fs::read_to_string(manifest)?;
    let table =
        contents
            .parse::<toml::Table>()
            .map_err(|error| InstallerError::InvalidCargoToml {
                path: manifest.to_owned(),
                reason: error.to_string(),
            })?;
    Ok(table
        .get("features")
        .and_then(toml::Value::as_table)
        .map(|features| features.keys().cloned().collect())
        .unwrap_or_default())
}

#[cfg(test)]
#[path = "suite_features_tests.rs"]
mod tests;
