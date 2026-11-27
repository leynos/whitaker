//! Candidate acceptance and SARIF Run 0 emission.

use whitaker_sarif::{
    Level,
    LocationBuilder,
    RelatedLocation,
    ResultBuilder,
    Run,
    TOKEN_HASH_KEY,
    WHITAKER_FRAGMENT_KEY,
    WhitakerPropertiesBuilder,
    deduplicate_results,
};

use super::{
    error::{Run0Error, Run0Result},
    score::{jaccard_similarity, select_rule_profile},
    types::{AcceptedPair, TokenFragment, TokenPassConfig},
};
use crate::{
    CandidatePair,
    CloneRule,
    NormProfile,
    pair_result::{
        assemble_run,
        clone_message,
        compact_span,
        decimal_to_f64,
        ensure_non_empty,
        fragment_map,
        pair_fingerprint,
        primary_fingerprint,
        region_for_range,
        resolve_pair,
        result_sort_key,
        token_hash,
    },
};

/// Accepts canonical candidate pairs for later Run 0 emission.
///
/// The resolved rule profile matches each fragment's normalization profile:
/// Type-1 fragments can only emit `WHK001`, and Type-2 fragments can only emit
/// `WHK002`.
///
/// # Errors
///
/// Returns a typed error when fragments are missing, empty, mixed-profile, or
/// thresholds are malformed.
pub fn accept_candidate_pairs(
    fragments: &[TokenFragment],
    candidates: &[CandidatePair],
    config: &TokenPassConfig,
) -> Run0Result<Vec<AcceptedPair>> {
    config.type1_threshold().validate()?;
    config.type2_threshold().validate()?;

    let fragment_map = fragment_map(fragments);
    let mut accepted = Vec::new();
    for pair in candidates {
        let (left, right) = resolve_pair(&fragment_map, pair)?;
        if left.profile() != right.profile() {
            return Err(Run0Error::MixedProfiles {
                left_fragment: left.id().as_str().to_owned(),
                right_fragment: right.id().as_str().to_owned(),
            });
        }

        let Some(score) =
            jaccard_similarity(left.retained_fingerprints(), right.retained_fingerprints())
        else {
            let empty_fragment_id = if left.retained_fingerprints().is_empty() {
                left.id().as_str().to_owned()
            } else {
                right.id().as_str().to_owned()
            };
            return Err(Run0Error::EmptyFingerprintSet {
                fragment_id: empty_fragment_id,
            });
        };
        let Some(profile) = select_rule_profile(
            left.profile(),
            score,
            config.type1_threshold(),
            config.type2_threshold(),
        ) else {
            continue;
        };
        accepted.push(AcceptedPair::new(pair.clone(), profile, score));
    }

    accepted.sort_by(|left, right| {
        left.pair()
            .cmp(right.pair())
            .then_with(|| profile_sort_key(left.profile()).cmp(&profile_sort_key(right.profile())))
    });
    accepted
        .dedup_by(|left, right| left.pair() == right.pair() && left.profile() == right.profile());
    Ok(accepted)
}

/// Emits SARIF Run 0 results for accepted token-pass pairs.
///
/// # Errors
///
/// Returns a typed error when accepted pairs reference missing fragments or
/// malformed fingerprint ranges.
pub fn emit_run0(
    fragments: &[TokenFragment],
    accepted_pairs: &[AcceptedPair],
    config: &TokenPassConfig,
) -> Run0Result<Run> {
    let fragment_map = fragment_map(fragments);
    let mut results = accepted_pairs
        .iter()
        .map(|accepted| build_pair_result(&fragment_map, accepted, config))
        .collect::<Run0Result<Vec<_>>>()?;
    results.sort_by(result_sort_key);
    Ok(assemble_run(
        config.tool_name(),
        config.tool_version(),
        Vec::new(),
        deduplicate_results(&results),
    ))
}

/// Resolve both fragments of an accepted pair and build its SARIF result.
fn build_pair_result(
    fragment_map: &std::collections::BTreeMap<&str, &TokenFragment>,
    accepted: &AcceptedPair,
    config: &TokenPassConfig,
) -> Run0Result<whitaker_sarif::SarifResult> {
    let (left, right) = resolve_pair(fragment_map, accepted.pair())?;
    [left, right].into_iter().try_for_each(ensure_non_empty)?;
    build_result(left, right, accepted, config)
}

/// Build one SARIF result for an accepted pair of clone fragments.
///
/// The primary fragment becomes the result location and the peer becomes a
/// related location. Fingerprint, region, score-conversion, or property-build
/// failures are returned to the caller rather than emitting a partial result.
fn build_result(
    primary: &TokenFragment,
    peer: &TokenFragment,
    accepted: &AcceptedPair,
    config: &TokenPassConfig,
) -> Run0Result<whitaker_sarif::SarifResult> {
    let primary_range = primary_fingerprint(primary)?.range.clone();
    let peer_range = primary_fingerprint(peer)?.range.clone();
    let primary_region =
        region_for_range(primary.id().as_str(), primary.source_text(), primary_range)?;
    let peer_region = region_for_range(peer.id().as_str(), peer.source_text(), peer_range)?;

    let rule = CloneRule::from(accepted.profile());
    let score = accepted.score();
    let score_text = score.as_decimal_string();
    let pair_fingerprint = pair_fingerprint(accepted, rule);
    let token_hash = token_hash(primary, peer);
    let properties = WhitakerPropertiesBuilder::new(rule.label())
        .with_k(config.shingle_size())
        .with_window(config.winnow_window())
        .with_jaccard(
            decimal_to_f64(&score_text).ok_or_else(|| Run0Error::InvalidScore {
                value: score_text.clone(),
            })?,
        )
        .with_group_id(0)
        .with_class_size(2)
        .build()?
        .try_to_value()?;
    let primary_label = format!("{}:{}", primary.file_uri(), compact_span(&primary_region));
    let peer_label = format!("{}:{}", peer.file_uri(), compact_span(&peer_region));

    ResultBuilder::new(rule.rule_id())
        .with_level(Level::Warning)
        .with_message(clone_message(
            rule,
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
        .with_fingerprint(WHITAKER_FRAGMENT_KEY, pair_fingerprint)
        .with_fingerprint(TOKEN_HASH_KEY, token_hash)
        .with_properties(properties)
        .build()
        .map_err(Run0Error::from)
}

/// Return the stable ordering key for a normalization profile.
const fn profile_sort_key(profile: NormProfile) -> u8 {
    match profile {
        NormProfile::T1 => 1,
        NormProfile::T2 => 2,
    }
}
