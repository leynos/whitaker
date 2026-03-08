//! SARIF result and supporting types.
//!
//! [`SarifResult`] represents a single finding (e.g. a clone pair).
//! [`Level`] describes the severity. [`Message`] carries the human-readable
//! description.
//!
//! The type is named `SarifResult` rather than `Result` to avoid collision
//! with `std::result::Result`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::location::{Location, RelatedLocation};

/// Severity level of a SARIF result.
///
/// Serializes as a lowercase string (`"warning"`, `"note"`, etc.).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::Level;
///
/// let json = serde_json::to_string(&Level::Warning).expect("serialize");
/// assert_eq!(json, "\"warning\"");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// No severity assigned.
    None,
    /// Informational finding.
    Note,
    /// Potentially problematic finding (default).
    #[default]
    Warning,
    /// Serious problem.
    Error,
}

/// A human-readable message attached to a result.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::Message;
///
/// let msg = Message { text: "clone detected".into() };
/// assert_eq!(msg.text, "clone detected");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Plain-text content.
    pub text: String,
}

/// A single analysis result (finding) in a SARIF run.
///
/// Named `SarifResult` to avoid collision with `std::result::Result`.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{SarifResult, Level, Message};
///
/// let result = SarifResult {
///     rule_id: "WHK001".into(),
///     level: Level::Warning,
///     message: Message { text: "clone detected".into() },
///     locations: Vec::new(),
///     related_locations: Vec::new(),
///     partial_fingerprints: Default::default(),
///     properties: None,
///     baseline_state: None,
/// };
/// assert_eq!(result.rule_id, "WHK001");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    /// Identifier of the rule that produced this result.
    pub rule_id: String,

    /// Severity level.
    #[serde(default)]
    pub level: Level,

    /// Human-readable description.
    pub message: Message,

    /// Primary locations of the finding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<Location>,

    /// Peer locations related to the finding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_locations: Vec<RelatedLocation>,

    /// Stable fingerprints for deduplication.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub partial_fingerprints: HashMap<String, String>,

    /// Tool-specific properties (Whitaker metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,

    /// Baseline comparison state (e.g. `"updated"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_state: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_serializes_lowercase() {
        match serde_json::to_string(&Level::Warning) {
            Ok(json) => assert_eq!(json, "\"warning\""),
            Err(e) => panic!("failed to serialize Warning: {e}"),
        }
        match serde_json::to_string(&Level::Note) {
            Ok(json) => assert_eq!(json, "\"note\""),
            Err(e) => panic!("failed to serialize Note: {e}"),
        }
        match serde_json::to_string(&Level::Error) {
            Ok(json) => assert_eq!(json, "\"error\""),
            Err(e) => panic!("failed to serialize Error: {e}"),
        }
        match serde_json::to_string(&Level::None) {
            Ok(json) => assert_eq!(json, "\"none\""),
            Err(e) => panic!("failed to serialize None: {e}"),
        }
    }

    #[test]
    fn level_default_is_warning() {
        assert_eq!(Level::default(), Level::Warning);
    }

    #[test]
    fn result_round_trip() {
        let result = SarifResult {
            rule_id: "WHK001".into(),
            level: Level::Warning,
            message: Message {
                text: "clone detected".into(),
            },
            locations: Vec::new(),
            related_locations: Vec::new(),
            partial_fingerprints: HashMap::new(),
            properties: None,
            baseline_state: None,
        };
        match serde_json::to_string(&result) {
            Ok(json) => match serde_json::from_str::<SarifResult>(&json) {
                Ok(parsed) => assert_eq!(result, parsed),
                Err(e) => panic!("failed to deserialize: {e}"),
            },
            Err(e) => panic!("failed to serialize: {e}"),
        }
    }

    #[test]
    fn empty_collections_omitted() {
        let result = SarifResult {
            rule_id: "WHK001".into(),
            level: Level::Warning,
            message: Message { text: "msg".into() },
            locations: Vec::new(),
            related_locations: Vec::new(),
            partial_fingerprints: HashMap::new(),
            properties: None,
            baseline_state: None,
        };
        match serde_json::to_string(&result) {
            Ok(json) => {
                assert!(!json.contains("\"locations\""));
                assert!(!json.contains("\"relatedLocations\""));
                assert!(!json.contains("\"partialFingerprints\""));
            }
            Err(e) => panic!("failed to serialize: {e}"),
        }
    }

    #[test]
    fn result_with_fingerprints() {
        let mut fps = HashMap::new();
        fps.insert("whitakerFragment".into(), "abc123".into());
        let result = SarifResult {
            rule_id: "WHK001".into(),
            level: Level::Warning,
            message: Message { text: "msg".into() },
            locations: Vec::new(),
            related_locations: Vec::new(),
            partial_fingerprints: fps,
            properties: None,
            baseline_state: None,
        };
        match serde_json::to_string(&result) {
            Ok(json) => {
                assert!(json.contains("\"partialFingerprints\""));
                assert!(json.contains("\"whitakerFragment\""));
            }
            Err(e) => panic!("failed to serialize: {e}"),
        }
    }
}
