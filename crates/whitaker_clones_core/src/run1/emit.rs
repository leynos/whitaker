//! SARIF Run 1 emission.
//!
//! Run 1 is a complete refined view of Run 0: every token-pass result appears
//! once, either carried or reclassified, and token-pass misses that the AST
//! evidence supports are added. A consumer can therefore read Run 1 alone.

use std::collections::BTreeMap;

use whitaker_sarif::{
    AST_HASH_KEY,
    AST_PASS_TOOL_NAME,
    BaselineState,
    Invocation,
    Level,
    LocationBuilder,
    RefinementKind,
    RelatedLocation,
    ResultBuilder,
    Run,
    SarifResult,
    TOKEN_HASH_KEY,
    WHITAKER_FRAGMENT_KEY,
    WhitakerPropertiesBuilder,
    deduplicate_results,
};

use super::{
    config::{AstPassConfig, RefinementInput},
    error::{Run1Error, Run1Result},
    outcome::{AstEvidence, RefinedPair},
};
use crate::{
    AstHash,
    TokenFragment,
    pair_result::{
        assemble_run,
        ast_hash,
        ast_pair_fingerprint,
        clone_message,
        compact_span,
        decimal_to_f64,
        fragment_map,
        pair_fingerprint,
        primary_fingerprint,
        region_for_range,
        resolve_pair,
        result_sort_key,
        token_hash,
    },
};

/// Emit SARIF Run 1 for the refined pairs.
///
/// Results are sorted by `(ruleId, whitakerFragment, message)`, deduplicated,
/// and wrapped in a run named [`AST_PASS_TOOL_NAME`] that carries all three
/// clone rules and one successful invocation.
///
/// # Errors
///
/// Returns a typed error when a refined pair references a missing or empty
/// fragment, when a region cannot be mapped, or when a score cannot be
/// converted into its SARIF property.
pub fn emit_run1(
    input: &RefinementInput<'_>,
    refined: &[RefinedPair],
    config: &AstPassConfig,
) -> Run1Result<Run> {
    let fragments = fragment_map(input.fragments());
    let mut results = refined
        .iter()
        .map(|pair| build_result(&fragments, pair, input))
        .collect::<Run1Result<Vec<_>>>()?;
    results.sort_by(result_sort_key);
    let invocation = Invocation {
        execution_successful: true,
        command_line: None,
    };
    Ok(assemble_run(
        AST_PASS_TOOL_NAME,
        config.tool_version(),
        vec![invocation],
        deduplicate_results(&results),
    ))
}

/// Return the SARIF baseline state this result reports.
///
/// The mapping lives here rather than on `RefinedPair` so the domain outcome
/// stays free of SARIF vocabulary: baseline state is a wire concern, decided
/// where the wire format is written.
const fn baseline_state(refined: &RefinedPair) -> BaselineState {
    match refined {
        RefinedPair::Carried { .. } => BaselineState::Unchanged,
        RefinedPair::Reclassified { .. } => BaselineState::Updated,
        RefinedPair::Added { .. } => BaselineState::New,
    }
}

/// Return the provenance kind recorded in the Whitaker property bag.
const fn refinement_kind(refined: &RefinedPair) -> RefinementKind {
    match refined {
        RefinedPair::Carried { .. } => RefinementKind::Carried,
        RefinedPair::Reclassified { .. } => RefinementKind::Reclassified,
        RefinedPair::Added { .. } => RefinementKind::Added,
    }
}

/// Build one Run 1 result for a refined pair.
fn build_result(
    fragments: &BTreeMap<&str, &TokenFragment>,
    refined: &RefinedPair,
    input: &RefinementInput<'_>,
) -> Run1Result<SarifResult> {
    let (primary, peer) = resolve_pair(fragments, refined.pair()).map_err(Run1Error::from)?;
    let (primary_range, peer_range) = ranges_for(primary, peer, refined)?;
    let primary_region =
        region_for_range(primary.id().as_str(), primary.source_text(), primary_range)?;
    let peer_region = region_for_range(peer.id().as_str(), peer.source_text(), peer_range)?;
    let primary_label = format!("{}:{}", primary.file_uri(), compact_span(&primary_region));
    let peer_label = format!("{}:{}", peer.file_uri(), compact_span(&peer_region));
    let score_text = score_text(refined);

    let mut builder = ResultBuilder::new(refined.rule().rule_id())
        .with_level(Level::Warning)
        .with_message(clone_message(
            refined.rule(),
            &primary_label,
            &peer_label,
            &score_text,
        ))
        .with_location(
            LocationBuilder::new(primary.file_uri())
                .with_region(primary_region)
                .build(),
        )
        .with_related_location(RelatedLocation {
            id: 1,
            message: Some(whitaker_sarif::Message {
                text: format!("Peer clone fragment: {}", peer.id().as_str()),
            }),
            physical_location: whitaker_sarif::PhysicalLocation {
                artefact_location: whitaker_sarif::ArtefactLocation {
                    uri: peer.file_uri().to_owned(),
                    uri_base_id: None,
                },
                region: Some(peer_region),
            },
        })
        .with_fingerprint(WHITAKER_FRAGMENT_KEY, fragment_key(primary, peer, refined))
        .with_fingerprint(TOKEN_HASH_KEY, token_hash(primary, peer))
        .with_baseline_state(baseline_state(refined))
        .with_properties(properties(refined, input)?);

    if let Some(evidence) = refined.ast() {
        let (primary_hash, peer_hash) = ordered_hashes(primary, peer, evidence);
        builder = builder.with_fingerprint(AST_HASH_KEY, ast_hash(primary_hash, peer_hash));
    }
    builder.build().map_err(Run1Error::from)
}

/// Return the byte ranges used for the two result locations.
///
/// A carried or reclassified result keeps its Run 0 primary and related
/// locations, which come from the first retained fingerprint range. An added
/// result has no earlier identity, so it reports the full fragment spans.
fn ranges_for(
    primary: &TokenFragment,
    peer: &TokenFragment,
    refined: &RefinedPair,
) -> Run1Result<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    match refined {
        RefinedPair::Carried { .. } | RefinedPair::Reclassified { .. } => Ok((
            primary_fingerprint(primary)?.range.clone(),
            primary_fingerprint(peer)?.range.clone(),
        )),
        RefinedPair::Added { .. } => Ok((primary.span(), peer.span())),
    }
}

/// Return the stable `whitakerFragment` value for a refined pair.
fn fragment_key(primary: &TokenFragment, peer: &TokenFragment, refined: &RefinedPair) -> String {
    match refined {
        RefinedPair::Carried { token, .. } | RefinedPair::Reclassified { token, .. } => {
            pair_fingerprint(token, token.rule())
        }
        RefinedPair::Added { .. } => ast_pair_fingerprint(primary, peer),
    }
}

/// Order the evidence hashes to match the result's primary and related sides.
///
/// The evidence stores the canonically smaller location first, which is not
/// necessarily the result's primary side.
fn ordered_hashes(
    primary: &TokenFragment,
    peer: &TokenFragment,
    evidence: &AstEvidence,
) -> (AstHash, AstHash) {
    if primary.location() <= peer.location() {
        return (evidence.left_hash(), evidence.right_hash());
    }
    (evidence.right_hash(), evidence.left_hash())
}

/// Return the score rendered in the result message.
///
/// A near-miss message reports the cosine, because that is the score that
/// justified the rule; a carried result keeps its Jaccard score.
fn score_text(refined: &RefinedPair) -> String {
    match refined {
        RefinedPair::Carried { .. } => refined.jaccard().as_decimal_string(),
        RefinedPair::Reclassified { ast, .. } | RefinedPair::Added { ast, .. } => {
            ast.cosine().as_decimal_string()
        }
    }
}

/// Build the Whitaker property bag for one result.
fn properties(refined: &RefinedPair, input: &RefinementInput<'_>) -> Run1Result<serde_json::Value> {
    let token_config = input.token_config();
    let jaccard_text = refined.jaccard().as_decimal_string();
    let mut builder = WhitakerPropertiesBuilder::new(refined.rule().label())
        .with_k(token_config.shingle_size())
        .with_window(token_config.winnow_window())
        .with_jaccard(match decimal_to_f64(&jaccard_text) {
            Some(jaccard) => jaccard,
            None => {
                return Err(Run1Error::InvalidScore {
                    value: jaccard_text,
                });
            }
        })
        .with_group_id(0)
        .with_class_size(2)
        .with_refinement(refinement_kind(refined));

    if let Some(evidence) = refined.ast() {
        let cosine_text = evidence.cosine().as_decimal_string();
        let Some(cosine) = decimal_to_f64(&cosine_text) else {
            return Err(Run1Error::InvalidScore { value: cosine_text });
        };
        builder = builder
            .with_cosine(cosine)
            .with_ast_hashes_equal(evidence.hashes_equal());
    }
    if let Some(token_rule) = refined
        .token_rule()
        .filter(|token_rule| refined.rule() != *token_rule)
    {
        builder = builder.with_token_rule_id(token_rule.rule_id());
    }
    builder.build()?.try_to_value().map_err(Run1Error::from)
}

#[cfg(test)]
#[path = "emit_result_tests.rs"]
mod result_tests;
#[cfg(test)]
#[path = "emit_tests.rs"]
mod tests;
