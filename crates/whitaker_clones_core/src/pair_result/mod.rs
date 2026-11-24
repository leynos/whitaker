//! Shared SARIF result helpers for the token-pass and AST-pass emitters.
//!
//! The two emitters build the same kind of object — one result per clone pair,
//! with a stable fragment fingerprint, a token digest, a human-readable
//! message and a canonical ordering — so those pieces live here once rather
//! than diverging between `run0` and `run1`.
//!
//! # Re-use policy
//!
//! This module is owned by the clone-detector emitters. Its only permitted
//! callers are [`run0`](crate::run0) and [`run1`](crate::run1). It composes
//! `whitaker_sarif` builders without adding policy: rule choice, scoring and
//! classification stay in the calling run.

mod fingerprint;
mod location;

pub(crate) use fingerprint::{
    ast_hash,
    ast_pair_fingerprint,
    ensure_non_empty,
    fragment_map,
    node_count,
    pair_fingerprint,
    primary_fingerprint,
    resolve_pair,
    token_hash,
};
pub(crate) use location::{compact_span, region_for_range};
use whitaker_sarif::{Invocation, Run, RunBuilder, SarifResult, WHITAKER_FRAGMENT_KEY};

use crate::clone_rule::CloneRule;

/// Assemble a SARIF run from its invocations and de-duplicated results.
///
/// Both passes share this shape: every Whitaker run names its producer
/// explicitly and carries the three clone rules. Run 0 records no invocation;
/// Run 1 records one successful invocation, as the design requires.
pub(crate) fn assemble_run(
    tool_name: &str,
    tool_version: &str,
    invocations: Vec<Invocation>,
    results: impl IntoIterator<Item = SarifResult>,
) -> Run {
    let rules = RunBuilder::new(tool_name, tool_version).with_rules(whitaker_sarif::all_rules());
    let with_invocations = invocations
        .into_iter()
        .fold(rules, RunBuilder::with_invocation);
    results
        .into_iter()
        .fold(with_invocations, RunBuilder::with_result)
        .build()
}

/// Format the human-readable message for one clone-pair result.
///
/// The shape is `Type-{N} clone: {primary} <-> {peer} (sim = {score})`, which
/// the design fixes for both runs.
pub(crate) fn clone_message(
    rule: CloneRule,
    primary_label: &str,
    peer_label: &str,
    score_text: &str,
) -> String {
    format!(
        "Type-{} clone: {} <-> {} (sim = {})",
        rule.type_number(),
        primary_label,
        peer_label,
        score_text
    )
}

/// Convert a six-decimal score string into the SARIF floating-point property.
///
/// This is the single place where a clone score becomes an `f64`. Decisions
/// never read the result; it exists only for the SARIF `cosine` and `jaccard`
/// properties, which are informational. Returns `None` when the string is not
/// a finite number.
pub(crate) fn decimal_to_f64(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|parsed| parsed.is_finite())
}

/// Order results by rule, fragment fingerprint and message.
///
/// The order is total and content-derived, so two runs over the same input
/// serialize identically.
pub(crate) fn result_sort_key(result: &SarifResult, other: &SarifResult) -> std::cmp::Ordering {
    result
        .rule_id
        .cmp(&other.rule_id)
        .then_with(|| {
            result
                .partial_fingerprints
                .get(WHITAKER_FRAGMENT_KEY)
                .cmp(&other.partial_fingerprints.get(WHITAKER_FRAGMENT_KEY))
        })
        .then_with(|| result.message.text.cmp(&other.message.text))
}

#[cfg(test)]
mod tests {
    //! Message, decimal conversion and ordering tests for the shared helpers.

    use rstest::rstest;

    use super::*;

    /// Check that the message names the rule type and both labels.
    #[test]
    fn message_names_rule_and_labels() {
        let text = clone_message(CloneRule::Type2, "a.rs:1:1-1:9", "b.rs:2:2-2:9", "0.950000");

        assert_eq!(
            text, "Type-2 clone: a.rs:1:1-1:9 <-> b.rs:2:2-2:9 (sim = 0.950000)",
            "the message must follow the documented shape"
        );
    }

    /// Check that decimal conversion accepts scores and rejects non-numbers.
    #[rstest]
    #[case("0.850000", Some(0.85))]
    #[case("1.000000", Some(1.0))]
    #[case("0.000000", Some(0.0))]
    #[case("not a score", None)]
    #[case("NaN", None)]
    #[case("inf", None)]
    fn decimal_conversion(#[case] value: &str, #[case] expected: Option<f64>) {
        let measured = decimal_to_f64(value);
        assert_eq!(
            measured.is_some(),
            expected.is_some(),
            "unexpected conversion verdict for {value}: {measured:?}"
        );
        assert_eq!(
            measured.map(f64::to_bits),
            expected.map(f64::to_bits),
            "unexpected value for {value}"
        );
    }
}
