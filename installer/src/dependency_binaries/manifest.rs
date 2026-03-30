//! Parsing for the repository-owned dependency-binary manifest.
//!
//! The committed `installer/dependency-binaries.toml` file is the single source
//! of truth for required dependency-tool versions, licences, and provenance.

use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::sync::OnceLock;

/// One repository-owned dependency binary requirement.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DependencyBinary {
    package: String,
    binary: String,
    version: String,
    license: String,
    repository: String,
}

impl DependencyBinary {
    /// Return the Cargo package name.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return the executable basename without any platform suffix.
    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// Return the required upstream version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Return the upstream licence string recorded in the manifest.
    #[must_use]
    pub fn license(&self) -> &str {
        &self.license
    }

    /// Return the upstream repository URL.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

#[derive(Debug, Deserialize)]
struct DependencyBinaryManifest {
    dependency_binaries: Vec<DependencyBinary>,
}

#[derive(Debug, Clone)]
struct ManifestLoadError(String);

impl fmt::Display for ManifestLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ManifestLoadError {}

/// Return the embedded manifest contents.
#[must_use]
pub fn manifest_contents() -> &'static str {
    include_str!("../../dependency-binaries.toml")
}

/// Parse manifest TOML into typed dependency entries.
///
/// # Errors
///
/// Returns an error when the TOML is malformed or required fields are missing.
pub fn parse_manifest(contents: &str) -> Result<Vec<DependencyBinary>, toml::de::Error> {
    let manifest: DependencyBinaryManifest = toml::from_str(contents)?;
    Ok(manifest.dependency_binaries)
}

/// Return the committed dependency binaries from the embedded manifest.
pub fn required_dependency_binaries() -> Result<&'static [DependencyBinary], Box<dyn Error>> {
    static MANIFEST: OnceLock<Result<Vec<DependencyBinary>, ManifestLoadError>> = OnceLock::new();

    match MANIFEST.get_or_init(|| {
        parse_manifest(manifest_contents()).map_err(|error| ManifestLoadError(error.to_string()))
    }) {
        Ok(dependencies) => Ok(dependencies.as_slice()),
        Err(error) => Err(Box::new(error.clone())),
    }
}

/// Find a dependency binary by its Cargo package name.
pub fn find_dependency_binary(
    package: &str,
) -> Result<Option<&'static DependencyBinary>, Box<dyn Error>> {
    Ok(required_dependency_binaries()?
        .iter()
        .find(|dependency| dependency.package() == package))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_contains_expected_tools() {
        let dependencies =
            required_dependency_binaries().expect("embedded manifest should stay parseable");
        assert_eq!(dependencies.len(), 2);
        assert!(
            dependencies
                .iter()
                .any(|tool| tool.package() == "cargo-dylint")
        );
        assert!(
            dependencies
                .iter()
                .any(|tool| tool.package() == "dylint-link")
        );
    }

    #[test]
    fn parse_manifest_rejects_missing_required_field() {
        let manifest = r#"
            [[dependency_binaries]]
            package = "cargo-dylint"
            binary = "cargo-dylint"
            version = "4.1.0"
            license = "MIT OR Apache-2.0"
        "#;

        let error =
            parse_manifest(manifest).expect_err("manifest should reject missing repository");
        assert!(error.to_string().contains("repository"));
    }

    #[test]
    fn find_dependency_binary_returns_matching_package() {
        let tool = find_dependency_binary("cargo-dylint")
            .expect("embedded manifest should stay parseable")
            .expect("tool should exist");
        assert_eq!(tool.binary(), "cargo-dylint");
        assert_eq!(tool.version(), "4.1.0");
    }
}
