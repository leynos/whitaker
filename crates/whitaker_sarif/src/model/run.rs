//! SARIF run, tool, invocation, and artefact types.
//!
//! A [`Run`] represents a single execution of an analysis tool. It contains
//! the [`Tool`] that produced the results, optional [`Invocation`] metadata,
//! the [`SarifResult`] findings, and any referenced
//! [`Artefact`]s.

use serde::{Deserialize, Serialize};

use super::descriptor::ReportingDescriptor;
use super::result::SarifResult;

/// A single analysis tool execution.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{Run, Tool, ToolComponent};
///
/// let run = Run {
///     tool: Tool {
///         driver: ToolComponent {
///             name: "whitaker_clones_cli".into(),
///             version: Some("0.2.1".into()),
///             information_uri: None,
///             rules: Vec::new(),
///         },
///     },
///     invocations: Vec::new(),
///     results: Vec::new(),
///     artefacts: Vec::new(),
/// };
/// assert_eq!(run.tool.driver.name, "whitaker_clones_cli");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// The tool that produced this run.
    pub tool: Tool,

    /// Metadata about the tool's invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invocations: Vec<Invocation>,

    /// Analysis findings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<SarifResult>,

    /// Referenced source artefacts.
    ///
    /// The SARIF 2.1.0 schema spells this property `artifacts`, so the wire
    /// name is pinned here rather than derived from the field name.
    #[serde(default, rename = "artifacts", skip_serializing_if = "Vec::is_empty")]
    pub artefacts: Vec<Artefact>,
}

/// The analysis tool that produced a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// The primary tool component (driver).
    pub driver: ToolComponent,
}

/// A component of a tool (the driver or an extension).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::ToolComponent;
///
/// let tc = ToolComponent {
///     name: "example".into(),
///     version: Some("1.0.0".into()),
///     information_uri: None,
///     rules: Vec::new(),
/// };
/// assert_eq!(tc.name, "example");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolComponent {
    /// Tool component name.
    pub name: String,

    /// Optional version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Optional URI to documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub information_uri: Option<String>,

    /// Rules defined by this component.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ReportingDescriptor>,
}

/// Metadata about a tool invocation.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::Invocation;
///
/// let inv = Invocation {
///     execution_successful: true,
///     command_line: Some("cargo whitaker clones scan".into()),
/// };
/// assert!(inv.execution_successful);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    /// Whether the tool completed without internal errors.
    pub execution_successful: bool,

    /// Optional command line used to invoke the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_line: Option<String>,
}

/// A source artefact referenced by results.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::{Artefact, ArtefactLocation};
///
/// let artefact = Artefact {
///     location: ArtefactLocation {
///         uri: "src/main.rs".into(),
///         uri_base_id: None,
///     },
///     mime_type: Some("text/x-rust".into()),
/// };
/// assert_eq!(artefact.location.uri, "src/main.rs");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artefact {
    /// Location of the artefact.
    pub location: super::location::ArtefactLocation,

    /// Optional MIME type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::location::ArtefactLocation;
    use crate::test_support::{assert_json_round_trip, assert_serialized_json};

    #[test]
    fn run_round_trip() {
        let run = Run {
            tool: Tool {
                driver: ToolComponent {
                    name: "test".into(),
                    version: Some("1.0".into()),
                    information_uri: None,
                    rules: Vec::new(),
                },
            },
            invocations: vec![Invocation {
                execution_successful: true,
                command_line: None,
            }],
            results: Vec::new(),
            artefacts: Vec::new(),
        };
        assert_json_round_trip(&run);
    }

    #[test]
    fn empty_collections_omitted_from_run() {
        let run = Run {
            tool: Tool {
                driver: ToolComponent {
                    name: "test".into(),
                    version: None,
                    information_uri: None,
                    rules: Vec::new(),
                },
            },
            invocations: Vec::new(),
            results: Vec::new(),
            artefacts: Vec::new(),
        };
        assert_serialized_json(&run, |json| {
            assert!(
                !json.contains("\"invocations\""),
                "empty invocations present: {json}"
            );
            assert!(
                !json.contains("\"results\""),
                "empty results present: {json}"
            );
            assert!(
                !json.contains("\"artifacts\""),
                "empty artefacts present: {json}"
            );
            assert!(!json.contains("\"rules\""), "empty rules present: {json}");
        });
    }

    #[test]
    fn artefact_round_trip() {
        let artefact = Artefact {
            location: ArtefactLocation {
                uri: "src/lib.rs".into(),
                uri_base_id: Some("%SRCROOT%".into()),
            },
            mime_type: Some("text/x-rust".into()),
        };
        assert_json_round_trip(&artefact);
    }
}
