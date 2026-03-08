//! Whitaker clone detection rule definitions.
//!
//! The clone detector uses three rule identifiers matching the clone type
//! taxonomy:
//!
//! - [`WHK001_ID`] — Type-1 clone (token exact after trivia removal).
//! - [`WHK002_ID`] — Type-2 clone (token equivalent under renaming).
//! - [`WHK003_ID`] — Type-3 clone (near-miss; AST similar).

use crate::model::descriptor::{MultiformatMessageString, ReportingDescriptor};

/// Rule identifier for Type-1 clones.
pub const WHK001_ID: &str = "WHK001";

/// Rule identifier for Type-2 clones.
pub const WHK002_ID: &str = "WHK002";

/// Rule identifier for Type-3 clones.
pub const WHK003_ID: &str = "WHK003";

/// Builds the [`ReportingDescriptor`] for WHK001 (Type-1 clone).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::whk001_rule;
///
/// let rule = whk001_rule();
/// assert_eq!(rule.id, "WHK001");
/// ```
#[must_use]
pub fn whk001_rule() -> ReportingDescriptor {
    ReportingDescriptor {
        id: WHK001_ID.into(),
        name: Some("Type1Clone".into()),
        short_description: Some(MultiformatMessageString {
            text: "Token-exact clone detected after whitespace and comment removal".into(),
        }),
        help_uri: Some(
            concat!(
                "https://github.com/leynos/whitaker/blob/main/",
                "docs/whitaker-clone-detector-design.md#rules"
            )
            .into(),
        ),
    }
}

/// Builds the [`ReportingDescriptor`] for WHK002 (Type-2 clone).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::whk002_rule;
///
/// let rule = whk002_rule();
/// assert_eq!(rule.id, "WHK002");
/// ```
#[must_use]
pub fn whk002_rule() -> ReportingDescriptor {
    ReportingDescriptor {
        id: WHK002_ID.into(),
        name: Some("Type2Clone".into()),
        short_description: Some(MultiformatMessageString {
            text: concat!(
                "Token-equivalent clone detected under identifier ",
                "and literal renaming"
            )
            .into(),
        }),
        help_uri: Some(
            concat!(
                "https://github.com/leynos/whitaker/blob/main/",
                "docs/whitaker-clone-detector-design.md#rules"
            )
            .into(),
        ),
    }
}

/// Builds the [`ReportingDescriptor`] for WHK003 (Type-3 clone).
///
/// # Examples
///
/// ```
/// use whitaker_sarif::whk003_rule;
///
/// let rule = whk003_rule();
/// assert_eq!(rule.id, "WHK003");
/// ```
#[must_use]
pub fn whk003_rule() -> ReportingDescriptor {
    ReportingDescriptor {
        id: WHK003_ID.into(),
        name: Some("Type3Clone".into()),
        short_description: Some(MultiformatMessageString {
            text: "Near-miss clone detected with similar AST structure".into(),
        }),
        help_uri: Some(
            concat!(
                "https://github.com/leynos/whitaker/blob/main/",
                "docs/whitaker-clone-detector-design.md#rules"
            )
            .into(),
        ),
    }
}

/// Returns all three Whitaker clone detection rules.
///
/// # Examples
///
/// ```
/// use whitaker_sarif::all_rules;
///
/// let rules = all_rules();
/// assert_eq!(rules.len(), 3);
/// ```
#[must_use]
pub fn all_rules() -> Vec<ReportingDescriptor> {
    vec![whk001_rule(), whk002_rule(), whk003_rule()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_rules_returns_three() {
        assert_eq!(all_rules().len(), 3);
    }

    #[test]
    fn rule_ids_are_correct() {
        let rules = all_rules();
        assert_eq!(rules.first().map(|r| r.id.as_str()), Some("WHK001"));
        assert_eq!(rules.get(1).map(|r| r.id.as_str()), Some("WHK002"));
        assert_eq!(rules.get(2).map(|r| r.id.as_str()), Some("WHK003"));
    }

    #[test]
    fn rule_names_are_correct() {
        assert_eq!(whk001_rule().name.as_deref(), Some("Type1Clone"));
        assert_eq!(whk002_rule().name.as_deref(), Some("Type2Clone"));
        assert_eq!(whk003_rule().name.as_deref(), Some("Type3Clone"));
    }

    #[test]
    fn rules_have_descriptions() {
        for rule in all_rules() {
            assert!(rule.short_description.is_some());
        }
    }

    #[test]
    fn rules_have_help_uris() {
        for rule in all_rules() {
            assert!(rule.help_uri.is_some());
        }
    }
}
