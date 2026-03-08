//! Top-level SARIF log container.
//!
//! [`SarifLog`] is the root object of a SARIF 2.1.0 document. It holds the
//! schema URL, version string, and one or more [`Run`] entries.

use serde::{Deserialize, Serialize};

use super::run::Run;

/// SARIF 2.1.0 schema URL.
pub const SARIF_SCHEMA: &str = concat!(
    "https://docs.oasis-open.org/sarif/sarif/",
    "v2.1.0/os/schemas/sarif-schema-2.1.0.json"
);

/// SARIF specification version.
pub const SARIF_VERSION: &str = "2.1.0";

/// Root object of a SARIF 2.1.0 document.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::SarifLog;
///
/// let log = SarifLog::default();
/// assert_eq!(log.version, "2.1.0");
/// assert!(log.runs.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog {
    /// JSON schema URI (`$schema` in SARIF).
    #[serde(rename = "$schema")]
    pub schema: String,

    /// SARIF specification version (always `"2.1.0"`).
    pub version: String,

    /// Ordered collection of runs in the document.
    #[serde(default)]
    pub runs: Vec<Run>,
}

impl Default for SarifLog {
    fn default() -> Self {
        Self {
            schema: SARIF_SCHEMA.into(),
            version: SARIF_VERSION.into(),
            runs: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_log_has_correct_version() {
        let log = SarifLog::default();
        assert_eq!(log.version, "2.1.0");
    }

    #[test]
    fn default_log_has_schema_url() {
        let log = SarifLog::default();
        assert!(log.schema.contains("sarif-schema-2.1.0"));
    }

    #[test]
    fn round_trip_empty_log() {
        let log = SarifLog::default();
        match serde_json::to_string(&log) {
            Ok(json) => match serde_json::from_str::<SarifLog>(&json) {
                Ok(parsed) => assert_eq!(log, parsed),
                Err(e) => panic!("deserialization failed: {e}"),
            },
            Err(e) => panic!("serialization failed: {e}"),
        }
    }

    #[test]
    fn schema_field_serializes_as_dollar_schema() {
        let log = SarifLog::default();
        match serde_json::to_string(&log) {
            Ok(json) => assert!(json.contains("\"$schema\"")),
            Err(e) => panic!("serialization failed: {e}"),
        }
    }

    #[test]
    fn empty_runs_present_in_json() {
        let log = SarifLog::default();
        match serde_json::to_string(&log) {
            Ok(json) => {
                assert!(json.contains("\"runs\":[]") || json.contains("\"runs\": []"));
            }
            Err(e) => panic!("serialization failed: {e}"),
        }
    }
}
