//! SARIF location and region types.
//!
//! These types describe where a result was found in source code. A
//! [`Location`] wraps a [`PhysicalLocation`] which combines an
//! [`ArtifactLocation`] (file URI) with an optional [`Region`] (line and
//! column spans).

use serde::{Deserialize, Serialize};

/// A location within an artifact (source file).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{Location, PhysicalLocation, ArtifactLocation, Region};
///
/// let loc = Location {
///     physical_location: PhysicalLocation {
///         artifact_location: ArtifactLocation {
///             uri: "src/main.rs".into(),
///             uri_base_id: None,
///         },
///         region: Some(Region {
///             start_line: 10,
///             start_column: None,
///             end_line: Some(15),
///             end_column: None,
///             byte_offset: None,
///             byte_length: None,
///         }),
///     },
/// };
/// assert_eq!(loc.physical_location.artifact_location.uri, "src/main.rs");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    /// Physical file and region within it.
    pub physical_location: PhysicalLocation,
}

/// A physical location combining a file reference and optional region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLocation {
    /// Identifies the artifact (file).
    pub artifact_location: ArtifactLocation,

    /// Optional region within the artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

/// A reference to an artifact by URI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLocation {
    /// Relative or absolute URI of the artifact.
    pub uri: String,

    /// Base identifier for resolving relative URIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri_base_id: Option<String>,
}

/// A region within an artifact, identified by line and column numbers.
///
/// `start_line` is always required (1-based). All other fields are optional.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::Region;
///
/// let region = Region {
///     start_line: 42,
///     start_column: Some(5),
///     end_line: Some(42),
///     end_column: Some(30),
///     byte_offset: None,
///     byte_length: None,
/// };
/// assert_eq!(region.start_line, 42);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    /// 1-based start line number.
    pub start_line: usize,

    /// Optional 1-based start column number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_column: Option<usize>,

    /// Optional 1-based end line number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,

    /// Optional 1-based end column number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<usize>,

    /// Optional byte offset from the start of the artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<usize>,

    /// Optional length in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<usize>,
}

/// A location related to the primary result location.
///
/// Used for peer fragments in a clone class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedLocation {
    /// Sequence number (1-based) within the result's related locations.
    pub id: usize,

    /// Optional descriptive message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<super::result::Message>,

    /// Physical file and region.
    pub physical_location: PhysicalLocation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_round_trip() {
        let region = Region {
            start_line: 10,
            start_column: Some(1),
            end_line: Some(15),
            end_column: Some(20),
            byte_offset: None,
            byte_length: None,
        };
        let json = serde_json::to_string(&region).expect("serialize");
        let parsed: Region = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(region, parsed);
    }

    #[test]
    fn optional_region_fields_omitted() {
        let region = Region {
            start_line: 1,
            start_column: None,
            end_line: None,
            end_column: None,
            byte_offset: None,
            byte_length: None,
        };
        let json = serde_json::to_string(&region).expect("serialize");
        assert!(!json.contains("startColumn"));
        assert!(!json.contains("endLine"));
    }

    #[test]
    fn location_round_trip() {
        let loc = Location {
            physical_location: PhysicalLocation {
                artifact_location: ArtifactLocation {
                    uri: "src/lib.rs".into(),
                    uri_base_id: Some("%SRCROOT%".into()),
                },
                region: Some(Region {
                    start_line: 5,
                    start_column: None,
                    end_line: None,
                    end_column: None,
                    byte_offset: None,
                    byte_length: None,
                }),
            },
        };
        let json = serde_json::to_string(&loc).expect("serialize");
        let parsed: Location = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loc, parsed);
    }

    #[test]
    fn related_location_round_trip() {
        let rl = RelatedLocation {
            id: 1,
            message: Some(super::super::result::Message {
                text: "peer fragment".into(),
            }),
            physical_location: PhysicalLocation {
                artifact_location: ArtifactLocation {
                    uri: "src/other.rs".into(),
                    uri_base_id: None,
                },
                region: None,
            },
        };
        let json = serde_json::to_string(&rl).expect("serialize");
        let parsed: RelatedLocation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rl, parsed);
    }
}
