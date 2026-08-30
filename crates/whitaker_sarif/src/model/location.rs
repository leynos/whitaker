//! SARIF location and region types.
//!
//! These types describe where a result was found in source code. A
//! [`Location`] wraps a [`PhysicalLocation`] which combines an
//! [`ArtefactLocation`] (file URI) with an optional [`Region`] (line and
//! column spans).

use serde::{Deserialize, Serialize};

/// A location within an artefact (source file).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{ArtefactLocation, Location, PhysicalLocation, Region};
///
/// let loc = Location {
///     physical_location: PhysicalLocation {
///         artefact_location: ArtefactLocation {
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
/// assert_eq!(loc.physical_location.artefact_location.uri, "src/main.rs");
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
    /// Identifies the artefact (file).
    ///
    /// The SARIF 2.1.0 schema spells this property `artifactLocation`, so the
    /// wire name is pinned here rather than derived from the field name.
    #[serde(rename = "artifactLocation")]
    pub artefact_location: ArtefactLocation,

    /// Optional region within the artefact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

/// A reference to an artefact by URI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtefactLocation {
    /// Relative or absolute URI of the artefact.
    pub uri: String,

    /// Base identifier for resolving relative URIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri_base_id: Option<String>,
}

/// A region within an artefact, identified by line and column numbers.
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

    /// Optional byte offset from the start of the artefact.
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
    //! Behavioural tests for the SARIF location model.

    use super::*;
    use crate::test_support::{assert_json_round_trip, assert_serialized_json};

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
        assert_json_round_trip(&region);
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
        assert_serialized_json(&region, |json| {
            assert!(
                !json.contains("startColumn"),
                "unset startColumn present: {json}"
            );
            assert!(!json.contains("endLine"), "unset endLine present: {json}");
        });
    }

    #[test]
    fn location_round_trip() {
        let loc = Location {
            physical_location: PhysicalLocation {
                artefact_location: ArtefactLocation {
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
        assert_json_round_trip(&loc);
    }

    #[test]
    fn related_location_round_trip() {
        let rl = RelatedLocation {
            id: 1,
            message: Some(super::super::result::Message {
                text: "peer fragment".into(),
            }),
            physical_location: PhysicalLocation {
                artefact_location: ArtefactLocation {
                    uri: "src/other.rs".into(),
                    uri_base_id: None,
                },
                region: None,
            },
        };
        assert_json_round_trip(&rl);
    }
}
