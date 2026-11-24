//! SARIF result and supporting types.
//!
//! [`SarifResult`] represents a single finding (e.g. a clone pair).
//! [`Level`] describes the severity. [`Message`] carries the human-readable
//! description.
//!
//! The type is named `SarifResult` rather than `Result` to avoid collision
//! with `std::result::Result`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::location::{Location, RelatedLocation};
use crate::baseline_state::BaselineState;

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
/// let msg = Message {
///     text: "clone detected".into(),
/// };
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
/// use whitaker_sarif::{Level, Message, SarifResult};
///
/// let result = SarifResult {
///     rule_id: "WHK001".into(),
///     level: Level::Warning,
///     message: Message {
///         text: "clone detected".into(),
///     },
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
    ///
    /// Ordered so serialization is deterministic; SARIF consumers compare
    /// logs byte for byte, and ADR 0015 requires a stable order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub partial_fingerprints: BTreeMap<String, String>,

    /// Tool-specific properties (Whitaker metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,

    /// Baseline comparison state (see [`BaselineState`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_state: Option<BaselineState>,
}

#[cfg(test)]
mod tests {
    //! Test `SarifResult` defaults, level encoding and field omission with shared
    //! JSON assertion helpers.

    use super::*;
    use crate::test_support::{assert_json_round_trip, assert_serialized_json};

    #[test]
    fn level_serializes_lowercase() {
        for (level, expected) in [
            (Level::Warning, "\"warning\""),
            (Level::Note, "\"note\""),
            (Level::Error, "\"error\""),
            (Level::None, "\"none\""),
        ] {
            assert_serialized_json(&level, |json| {
                assert_eq!(json, expected, "unexpected serialization of {level:?}");
            });
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
            partial_fingerprints: BTreeMap::new(),
            properties: None,
            baseline_state: None,
        };
        assert_json_round_trip(&result);
    }

    #[test]
    fn empty_collections_omitted() {
        let result = SarifResult {
            rule_id: "WHK001".into(),
            level: Level::Warning,
            message: Message { text: "msg".into() },
            locations: Vec::new(),
            related_locations: Vec::new(),
            partial_fingerprints: BTreeMap::new(),
            properties: None,
            baseline_state: None,
        };
        assert_serialized_json(&result, |json| {
            assert!(
                !json.contains("\"locations\""),
                "empty locations present: {json}"
            );
            assert!(
                !json.contains("\"relatedLocations\""),
                "empty relatedLocations present: {json}"
            );
            assert!(
                !json.contains("\"partialFingerprints\""),
                "empty partialFingerprints present: {json}"
            );
        });
    }

    #[test]
    fn result_with_fingerprints() {
        let mut fps = BTreeMap::new();
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
        assert_serialized_json(&result, |json| {
            assert!(
                json.contains("\"partialFingerprints\""),
                "partialFingerprints missing: {json}"
            );
            assert!(
                json.contains("\"whitakerFragment\""),
                "whitakerFragment key missing: {json}"
            );
        });
    }

    /// Check that fingerprints serialize in key order regardless of insertion
    /// order, so equal results produce equal logs.
    #[test]
    fn partial_fingerprints_serialize_in_key_order() {
        let mut fps = BTreeMap::new();
        fps.insert("tokenHash".to_owned(), "b".to_owned());
        fps.insert("astHash".to_owned(), "a".to_owned());
        fps.insert("whitakerFragment".to_owned(), "c".to_owned());
        let result = SarifResult {
            rule_id: "WHK003".into(),
            level: Level::Warning,
            message: Message { text: "msg".into() },
            locations: Vec::new(),
            related_locations: Vec::new(),
            partial_fingerprints: fps,
            properties: None,
            baseline_state: None,
        };

        assert_serialized_json(&result, |json| {
            let positions =
                ["\"astHash\"", "\"tokenHash\"", "\"whitakerFragment\""].map(|key| json.find(key));
            assert!(
                positions.iter().all(Option::is_some),
                "all fingerprint keys must serialize: {json}"
            );
            let [Some(ast), Some(token), Some(fragment)] = positions else {
                panic!("every fingerprint key must serialize: {json}");
            };
            assert!(
                ast < token && token < fragment,
                "fingerprint keys must serialize in ascending key order: {json}"
            );
        });
    }
}
