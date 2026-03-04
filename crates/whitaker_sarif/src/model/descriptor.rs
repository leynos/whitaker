//! SARIF reporting descriptors and message types.
//!
//! [`ReportingDescriptor`] represents a rule or notification in the SARIF
//! schema. [`MultiformatMessageString`] carries a plain-text description.

use serde::{Deserialize, Serialize};

/// A rule or notification defined by a tool.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::ReportingDescriptor;
///
/// let rule = ReportingDescriptor {
///     id: "WHK001".into(),
///     name: Some("Type1Clone".into()),
///     short_description: None,
///     help_uri: None,
/// };
/// assert_eq!(rule.id, "WHK001");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportingDescriptor {
    /// Stable identifier for the rule (e.g. `"WHK001"`).
    pub id: String,

    /// Optional human-readable name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Short plain-text description of the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<MultiformatMessageString>,

    /// URI linking to full documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help_uri: Option<String>,
}

/// A message string that may have multiple representations.
///
/// Only the `text` field is used in this subset.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::MultiformatMessageString;
///
/// let msg = MultiformatMessageString { text: "hello".into() };
/// assert_eq!(msg.text, "hello");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiformatMessageString {
    /// Plain-text representation.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_round_trip() {
        let desc = ReportingDescriptor {
            id: "WHK002".into(),
            name: Some("Type2Clone".into()),
            short_description: Some(MultiformatMessageString {
                text: "Token-equivalent clone".into(),
            }),
            help_uri: Some("https://example.com".into()),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        let parsed: ReportingDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc, parsed);
    }

    #[test]
    fn optional_fields_omitted_when_none() {
        let desc = ReportingDescriptor {
            id: "WHK001".into(),
            name: None,
            short_description: None,
            help_uri: None,
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        assert!(!json.contains("\"name\""));
        assert!(!json.contains("\"shortDescription\""));
        assert!(!json.contains("\"helpUri\""));
    }

    #[test]
    fn camel_case_field_names() {
        let desc = ReportingDescriptor {
            id: "WHK001".into(),
            name: None,
            short_description: Some(MultiformatMessageString {
                text: "desc".into(),
            }),
            help_uri: Some("https://example.com".into()),
        };
        let json = serde_json::to_string(&desc).expect("serialize");
        assert!(json.contains("\"shortDescription\""));
        assert!(json.contains("\"helpUri\""));
    }
}
