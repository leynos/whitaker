//! Manifest deserialization for prebuilt artefact consumption.
//!
//! Parses the `manifest.json` sidecar file obtained from the GitHub
//! rolling release into the validated [`Manifest`] type. All newtype
//! validation runs during deserialization, rejecting malformed fields
//! at parse time.

use super::manifest::Manifest;

/// Errors arising from manifest parsing.
#[derive(Debug, thiserror::Error)]
pub enum ManifestParseError {
    /// JSON deserialization or field validation failed.
    #[error("manifest parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parse a JSON string into a validated [`Manifest`].
///
/// All newtype validation (e.g. supported target triple, valid SHA-256
/// digest) runs during deserialization. Invalid fields produce a
/// [`ManifestParseError`].
///
/// # Errors
///
/// Returns an error if the JSON is malformed or any field fails
/// newtype validation.
///
/// # Examples
///
/// ```
/// use whitaker_installer::artefact::manifest_parser::parse_manifest;
///
/// let json = concat!(
///     r#"{"git_sha":"abc1234","schema_version":1,"#,
///     r#""toolchain":"nightly-2025-09-18","#,
///     r#""target":"x86_64-unknown-linux-gnu","#,
///     r#""generated_at":"2026-02-03T00:00:00Z","#,
///     r#""files":["lib.so"],"#,
///     r#""sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
/// );
/// let manifest = parse_manifest(json).expect("valid manifest");
/// assert_eq!(manifest.git_sha().as_str(), "abc1234");
/// ```
pub fn parse_manifest(json: &str) -> Result<Manifest, ManifestParseError> {
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn valid_manifest_json() -> String {
        concat!(
            r#"{"git_sha":"abc1234","schema_version":1,"#,
            r#""toolchain":"nightly-2025-09-18","#,
            r#""target":"x86_64-unknown-linux-gnu","#,
            r#""generated_at":"2026-02-03T00:00:00Z","#,
            r#""files":["lib.so"],"#,
            r#""sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        )
        .to_owned()
    }

    #[test]
    fn parses_valid_manifest() {
        let manifest = parse_manifest(&valid_manifest_json()).expect("valid");
        assert_eq!(manifest.git_sha().as_str(), "abc1234");
        assert_eq!(manifest.toolchain().as_str(), "nightly-2025-09-18");
        assert_eq!(manifest.target().as_str(), "x86_64-unknown-linux-gnu");
        assert_eq!(manifest.schema_version().as_u32(), 1);
        assert_eq!(manifest.files(), &["lib.so"]);
        assert_eq!(manifest.sha256().as_str().len(), 64);
    }

    #[test]
    fn rejects_invalid_json_syntax() {
        let result = parse_manifest("{not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let json = valid_manifest_json().replace("\"schema_version\":1", "\"schema_version\":99");
        let result = parse_manifest(&json);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unsupported_target() {
        let json =
            valid_manifest_json().replace("x86_64-unknown-linux-gnu", "wasm32-unknown-unknown");
        let result = parse_manifest(&json);
        assert!(result.is_err());
    }

    #[rstest]
    #[case::bad_sha(r#""git_sha":"abc1234""#, r#""git_sha":"AB""#)]
    #[case::bad_digest(
        r#""sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#,
        r#""sha256":"short""#
    )]
    fn rejects_invalid_field_values(#[case] from: &str, #[case] to: &str) {
        let json = valid_manifest_json().replace(from, to);
        let result = parse_manifest(&json);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_missing_required_fields() {
        let json = r#"{"git_sha":"abc1234"}"#;
        let result = parse_manifest(json);
        assert!(result.is_err());
    }
}
