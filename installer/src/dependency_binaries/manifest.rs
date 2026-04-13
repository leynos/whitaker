//! Parsing for the repository-owned dependency-binary manifest.
//!
//! The committed `installer/dependency-binaries.toml` file is the single source
//! of truth for required dependency-tool versions, licences, and provenance.

use serde::Deserialize;
use std::sync::OnceLock;
use thiserror::Error;

/// One repository-owned dependency binary requirement.
///
/// # Example
///
/// ```
/// use whitaker_installer::dependency_binaries::{
///     parse_manifest, required_dependency_binaries, DependencyBinary
/// };
///
/// // Parse the embedded manifest to obtain dependency binaries
/// let dependencies = required_dependency_binaries()
///     .expect("embedded manifest should be valid");
///
/// // Access fields on a dependency binary
/// if let Some(tool) = dependencies.iter().find(|d| d.package() == "cargo-dylint") {
///     assert_eq!(tool.package(), "cargo-dylint");
///     assert_eq!(tool.binary(), "cargo-dylint");
///     assert!(!tool.version().is_empty());
///     assert!(!tool.license().is_empty());
///     assert!(!tool.repository().is_empty());
/// }
/// ```
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
    ///
    /// # Example
    ///
    /// See the [`DependencyBinary`] type documentation for a complete example.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return the executable basename without any platform suffix.
    ///
    /// # Example
    ///
    /// See the [`DependencyBinary`] type documentation for a complete example.
    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// Return the required upstream version.
    ///
    /// # Example
    ///
    /// See the [`DependencyBinary`] type documentation for a complete example.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Return the upstream licence string recorded in the manifest.
    ///
    /// # Example
    ///
    /// See the [`DependencyBinary`] type documentation for a complete example.
    #[must_use]
    pub fn license(&self) -> &str {
        &self.license
    }

    /// Return the upstream repository URL.
    ///
    /// # Example
    ///
    /// See the [`DependencyBinary`] type documentation for a complete example.
    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

#[derive(Debug, Deserialize)]
struct DependencyBinaryManifest {
    dependency_binaries: Vec<DependencyBinary>,
}

/// Errors that can occur when loading or parsing the dependency manifest.
#[derive(Debug, Clone, Error)]
pub enum ManifestError {
    /// The manifest TOML is malformed or missing required fields.
    #[error("manifest parse error: {0}")]
    ParseError(String),
    /// Duplicate package entries were found in the manifest.
    #[error("duplicate package in manifest: {0}")]
    DuplicatePackage(String),
}

impl From<toml::de::Error> for ManifestError {
    fn from(error: toml::de::Error) -> Self {
        ManifestError::ParseError(error.to_string())
    }
}

/// Return the embedded manifest contents.
///
/// # Example
///
/// ```
/// use whitaker_installer::dependency_binaries::manifest_contents;
///
/// let contents = manifest_contents();
/// assert!(contents.contains("dependency_binaries"));
/// ```
#[must_use]
pub fn manifest_contents() -> &'static str {
    include_str!("../../dependency-binaries.toml")
}

/// Parse manifest TOML into typed dependency entries.
///
/// # Errors
///
/// Returns an error when:
/// - The TOML is malformed or required fields are missing.
/// - Duplicate package entries are found in the manifest.
///
/// # Examples
///
/// Parse the embedded manifest:
///
/// ```
/// use whitaker_installer::dependency_binaries::{
///     manifest_contents, parse_manifest
/// };
///
/// let dependencies = parse_manifest(manifest_contents())
///     .expect("embedded manifest should be valid");
///
/// assert!(!dependencies.is_empty());
/// ```
///
/// Parse a custom manifest string:
///
/// ```
/// use whitaker_installer::dependency_binaries::parse_manifest;
///
/// let manifest = r#"
///     [[dependency_binaries]]
///     package = "cargo-dylint"
///     binary = "cargo-dylint"
///     version = "4.1.0"
///     license = "MIT OR Apache-2.0"
///     repository = "https://github.com/trailofbits/dylint"
/// "#;
///
/// let dependencies = parse_manifest(manifest).expect("valid manifest");
/// assert_eq!(dependencies.len(), 1);
/// assert_eq!(dependencies[0].package(), "cargo-dylint");
/// ```
///
/// Reject duplicate packages:
///
/// ```
/// use whitaker_installer::dependency_binaries::parse_manifest;
///
/// let manifest_with_duplicates = r#"
///     [[dependency_binaries]]
///     package = "cargo-dylint"
///     binary = "cargo-dylint"
///     version = "4.1.0"
///     license = "MIT OR Apache-2.0"
///     repository = "https://github.com/trailofbits/dylint"
///
///     [[dependency_binaries]]
///     package = "cargo-dylint"
///     binary = "cargo-dylint"
///     version = "4.2.0"
///     license = "MIT OR Apache-2.0"
///     repository = "https://github.com/trailofbits/dylint"
/// "#;
///
/// let error = parse_manifest(manifest_with_duplicates)
///     .expect_err("should reject duplicate packages");
/// assert!(error.to_string().contains("cargo-dylint"));
/// ```
pub fn parse_manifest(contents: &str) -> Result<Vec<DependencyBinary>, ManifestError> {
    let manifest: DependencyBinaryManifest = toml::from_str(contents)?;

    // Check for duplicate package entries
    let mut seen_packages = std::collections::HashSet::new();
    for dependency in &manifest.dependency_binaries {
        let package = dependency.package();
        if !seen_packages.insert(package.to_string()) {
            return Err(ManifestError::DuplicatePackage(package.to_string()));
        }
    }

    Ok(manifest.dependency_binaries)
}

/// Return the committed dependency binaries from the embedded manifest.
///
/// # Errors
///
/// Returns an error if the embedded manifest cannot be parsed or contains
/// duplicate package entries.
///
/// # Example
///
/// ```
/// use whitaker_installer::dependency_binaries::required_dependency_binaries;
///
/// let dependencies = required_dependency_binaries()
///     .expect("embedded manifest should be valid");
///
/// // Iterate over all required dependency binaries
/// for tool in dependencies {
///     println!("{} {}: {}", tool.package(), tool.version(), tool.license());
/// }
/// ```
pub fn required_dependency_binaries() -> Result<&'static [DependencyBinary], ManifestError> {
    static MANIFEST: OnceLock<Result<Vec<DependencyBinary>, ManifestError>> = OnceLock::new();

    match MANIFEST.get_or_init(|| parse_manifest(manifest_contents())) {
        Ok(dependencies) => Ok(dependencies.as_slice()),
        Err(error) => Err(error.clone()),
    }
}

/// Find a dependency binary by its Cargo package name.
///
/// # Errors
///
/// Returns an error if the embedded manifest cannot be parsed.
///
/// # Examples
///
/// Find an existing package (returns `Some`):
///
/// ```
/// use whitaker_installer::dependency_binaries::find_dependency_binary;
///
/// let tool = find_dependency_binary("cargo-dylint")
///     .expect("manifest should parse")
///     .expect("cargo-dylint should be in the manifest");
///
/// assert_eq!(tool.package(), "cargo-dylint");
/// assert_eq!(tool.binary(), "cargo-dylint");
/// ```
///
/// Search for a non-existent package (returns `None`):
///
/// ```
/// use whitaker_installer::dependency_binaries::find_dependency_binary;
///
/// let result = find_dependency_binary("non-existent-package")
///     .expect("manifest should parse");
///
/// assert!(result.is_none());
/// ```
///
/// Handle manifest parse errors:
///
/// ```
/// use whitaker_installer::dependency_binaries::find_dependency_binary;
///
/// // This will only fail if the embedded manifest is corrupted
/// match find_dependency_binary("cargo-dylint") {
///     Ok(Some(tool)) => println!("Found {} {}", tool.package(), tool.version()),
///     Ok(None) => println!("Package not found"),
///     Err(error) => eprintln!("Manifest error: {}", error),
/// }
/// ```
#[must_use = "callers should handle missing packages and manifest parse failures"]
pub fn find_dependency_binary(
    package: &str,
) -> Result<Option<&'static DependencyBinary>, ManifestError> {
    Ok(required_dependency_binaries()?
        .iter()
        .find(|dependency| dependency.package() == package))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn missing_field_manifest() -> &'static str {
        r#"
            [[dependency_binaries]]
            package = "cargo-dylint"
            binary = "cargo-dylint"
            version = "4.1.0"
            license = "MIT OR Apache-2.0"
        "#
    }

    #[fixture]
    fn duplicate_packages_manifest() -> &'static str {
        r#"
            [[dependency_binaries]]
            package = "cargo-dylint"
            binary = "cargo-dylint"
            version = "4.1.0"
            license = "MIT OR Apache-2.0"
            repository = "https://github.com/trailofbits/dylint"

            [[dependency_binaries]]
            package = "cargo-dylint"
            binary = "cargo-dylint-alt"
            version = "4.2.0"
            license = "MIT OR Apache-2.0"
            repository = "https://github.com/trailofbits/dylint"
        "#
    }

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

    #[rstest]
    #[case::missing_field(missing_field_manifest(), &["repository"])]
    #[case::duplicate_packages(duplicate_packages_manifest(), &["cargo-dylint", "duplicate"])]
    fn parse_manifest_rejects_invalid_manifests(
        #[case] manifest_fixture: &str,
        #[case] expected_substrings: &[&str],
    ) {
        let error = parse_manifest(manifest_fixture).expect_err("should reject invalid manifest");
        let error_string = error.to_string();
        for substring in expected_substrings {
            assert!(
                error_string.contains(substring),
                "expected error to contain '{substring}'"
            );
        }
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
