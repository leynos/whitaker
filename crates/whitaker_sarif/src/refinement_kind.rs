//! Typed provenance for results the AST pass refined.
//!
//! [`RefinementKind`] records, inside the Whitaker property bag, what Run 1 did
//! to a token-pass result. It duplicates the information in `baselineState` on
//! purpose: other SARIF tools treat `baselineState` as cross-commit state and
//! may rewrite it, so Whitaker's own consumers read provenance from here.
//!
//! # Examples
//!
//! ```
//! use whitaker_sarif::RefinementKind;
//!
//! let json = serde_json::to_string(&RefinementKind::Reclassified).expect("serialize");
//! assert_eq!(json, "\"reclassified\"");
//! ```

use serde::{Deserialize, Serialize};

/// What the AST pass did to a result relative to the token pass.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::RefinementKind;
///
/// assert_ne!(RefinementKind::Carried, RefinementKind::Added);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefinementKind {
    /// The token-pass result was carried forward unchanged.
    Carried,
    /// The token-pass result was reclassified under a different rule.
    Reclassified,
    /// The AST pass added a result the token pass did not report.
    Added,
}

#[cfg(test)]
mod tests {
    //! Serialization tests for the refinement provenance vocabulary.

    use rstest::rstest;

    use super::*;
    use crate::test_support::{assert_json_round_trip, assert_serialized_json};

    /// Check that every provenance kind serializes with its documented spelling.
    #[rstest]
    #[case(RefinementKind::Carried, "\"carried\"")]
    #[case(RefinementKind::Reclassified, "\"reclassified\"")]
    #[case(RefinementKind::Added, "\"added\"")]
    fn kinds_serialize_lowercase(#[case] kind: RefinementKind, #[case] expected: &str) {
        assert_serialized_json(&kind, |json| {
            assert_eq!(json, expected, "unexpected serialization of {kind:?}");
        });
    }

    /// Check that every provenance kind survives a JSON round trip.
    #[rstest]
    #[case(RefinementKind::Carried)]
    #[case(RefinementKind::Reclassified)]
    #[case(RefinementKind::Added)]
    fn kinds_round_trip(#[case] kind: RefinementKind) { assert_json_round_trip(&kind); }
}
