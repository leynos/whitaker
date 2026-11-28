//! Property tests for the classification policy.
//!
//! The UI suite proves that one literal with several joins yields one warning,
//! but it can only pin the join counts someone wrote a fixture for. These
//! properties state the same rule over the generated join-count range, so a
//! policy change that started emitting one disposition per join would fail
//! even if every committed fixture still had only two joins.

use proptest::prelude::*;

use super::{ContinuationDisposition, classify};
use crate::{
    facts::{LiteralBody, LiteralContext, LiteralFacts},
    props::body_with_continuations,
};

/// The fragment alphabet this suite generates.
///
/// Narrower than the rewrite suite's: classification never decodes a value, so
/// multi-byte characters and escaped spellings are enough to vary the source
/// around each join.
const ALPHABET: &[&str] = &["alpha", "λ", "🦄", "\\\"", "\\\\", "{{", "}}"];

proptest! {
    #[test]
    fn any_positive_join_count_yields_exactly_one_diagnostic(
        (body, expected_joins) in body_with_continuations(ALPHABET)
    ) {
        let facts = LiteralFacts::Cooked {
            body: LiteralBody::new(&body),
            context: LiteralContext::Expression,
        };
        let rewrite = match classify(&facts) {
            ContinuationDisposition::PreferConcat(rewrite) => rewrite,
            other => {
                return Err(TestCaseError::fail(format!(
                    "expected one PreferConcat disposition, got {other:?}"
                )));
            }
        };

        // The driver emits one diagnostic for each `PreferConcat`, so these
        // two assertions together state "one diagnostic for any positive
        // number of joins in one literal".
        prop_assert_eq!(rewrite.join_offsets().len(), expected_joins);
        prop_assert_eq!(
            rewrite.fragments().len(),
            expected_joins.saturating_add(1)
        );
    }
}
