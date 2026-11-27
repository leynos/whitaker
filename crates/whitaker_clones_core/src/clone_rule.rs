//! The three clone rules the detector reports.
//!
//! [`CloneRule`] names a rule once, so the token pass and the AST pass cannot
//! disagree about an identifier, a display label or a message number.
//!
//! # Examples
//!
//! ```
//! use whitaker_clones_core::CloneRule;
//!
//! assert_eq!(CloneRule::Type1.rule_id(), "WHK001");
//! assert_eq!(CloneRule::Type3.label(), "T3");
//! assert_eq!(CloneRule::Type2.type_number(), 2);
//! ```

use whitaker_sarif::{WHK001_ID, WHK002_ID, WHK003_ID};

use crate::NormProfile;

/// A clone rule: Type-1 (token exact), Type-2 (rename equivalent) or Type-3
/// (AST near-miss).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CloneRule {
    /// Token-exact clone.
    Type1,
    /// Rename-equivalent clone.
    Type2,
    /// AST near-miss clone.
    Type3,
}

impl CloneRule {
    /// Every rule, in ascending type order.
    pub const ALL: [Self; 3] = [Self::Type1, Self::Type2, Self::Type3];

    /// Returns the SARIF rule identifier.
    #[must_use]
    pub const fn rule_id(self) -> &'static str {
        match self {
            Self::Type1 => WHK001_ID,
            Self::Type2 => WHK002_ID,
            Self::Type3 => WHK003_ID,
        }
    }

    /// Returns the profile label recorded in Whitaker properties.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Type1 => "T1",
            Self::Type2 => "T2",
            Self::Type3 => "T3",
        }
    }

    /// Returns the clone-type number used in result messages.
    #[must_use]
    pub const fn type_number(self) -> usize {
        match self {
            Self::Type1 => 1,
            Self::Type2 => 2,
            Self::Type3 => 3,
        }
    }

    /// Returns the rule a token-pass normalization profile reports.
    #[must_use]
    pub const fn from_profile(profile: NormProfile) -> Self {
        match profile {
            NormProfile::T1 => Self::Type1,
            NormProfile::T2 => Self::Type2,
        }
    }
}

impl From<NormProfile> for CloneRule {
    fn from(profile: NormProfile) -> Self { Self::from_profile(profile) }
}

#[cfg(test)]
mod tests {
    //! Identity and label tests that pin the rule vocabulary.

    use rstest::rstest;

    use super::*;

    /// Check that each rule maps to its documented identifier and label.
    #[rstest]
    #[case(CloneRule::Type1, "WHK001", "T1", 1)]
    #[case(CloneRule::Type2, "WHK002", "T2", 2)]
    #[case(CloneRule::Type3, "WHK003", "T3", 3)]
    fn rules_map_to_their_identifiers(
        #[case] rule: CloneRule,
        #[case] rule_id: &str,
        #[case] label: &str,
        #[case] number: usize,
    ) {
        assert_eq!(rule.rule_id(), rule_id, "unexpected rule id for {rule:?}");
        assert_eq!(rule.label(), label, "unexpected label for {rule:?}");
        assert_eq!(rule.type_number(), number, "unexpected number for {rule:?}");
    }

    /// Check that the rule list is ordered and complete.
    #[test]
    fn rule_list_is_ordered() {
        let ids = CloneRule::ALL.map(CloneRule::rule_id);

        assert_eq!(
            ids,
            ["WHK001", "WHK002", "WHK003"],
            "the rule list must be complete and ordered"
        );
    }
}
