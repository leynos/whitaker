//! Candidate acceptance and SARIF Run 0 emission.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use whitaker_sarif::{
    Level, LocationBuilder, RelatedLocation, ResultBuilder, Run, RunBuilder, WHITAKER_FRAGMENT_KEY,
    WHK001_ID, WHK002_ID, WhitakerPropertiesBuilder, all_rules, deduplicate_results,
};

use crate::{CandidatePair, NormProfile};

use super::{
    error::{Run0Error, Run0Result},
    score::{SimilarityRatio, jaccard_similarity, select_rule_profile},
    span::region_for_range,
    types::{AcceptedPair, TokenFragment, TokenPassConfig},
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
        ensure_non_empty(left)?;
        ensure_non_empty(right)?;

        let Some(score) =
            jaccard_similarity(left.retained_fingerprints(), right.retained_fingerprints())
        else {
            return Err(Run0Error::EmptyFingerprintSet {
                fragment_id: left.id().as_str().to_owned(),
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
    let mut results = Vec::new();
    for accepted in accepted_pairs {
        let (left, right) = resolve_pair(&fragment_map, accepted.pair())?;
        ensure_non_empty(left)?;
        ensure_non_empty(right)?;
        results.push(build_result(left, right, accepted, config)?);
    }
    results.sort_by(result_sort_key);
    let deduplicated = deduplicate_results(&results);

    let mut run =
        RunBuilder::new(config.tool_name(), config.tool_version()).with_rules(all_rules());
    for result in deduplicated {
        run = run.with_result(result);
    }
    Ok(run.build())
}

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

    let rule_id = match accepted.profile() {
        NormProfile::T1 => WHK001_ID,
        NormProfile::T2 => WHK002_ID,
    };
    let score = accepted.score();
    let score_text = score.as_decimal_string();
    let pair_fingerprint = pair_fingerprint(accepted);
    let token_hash = token_hash(primary, peer);
    let properties = WhitakerPropertiesBuilder::new(profile_name(accepted.profile()))
        .with_k(config.shingle_size())
        .with_window(config.winnow_window())
        .with_jaccard(score_to_f64(score)?)
        .with_cosine(0.0)
        .with_group_id(0)
        .with_class_size(2)
        .build()?
        .try_to_value()?;
    let primary_label = format!("{}:{}", primary.file_uri(), compact_span(&primary_region));
    let peer_label = format!("{}:{}", peer.file_uri(), compact_span(&peer_region));

    ResultBuilder::new(rule_id)
        .with_level(Level::Warning)
        .with_message(build_message(
            accepted.profile(),
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
                artifact_location: whitaker_sarif::ArtifactLocation {
                    uri: peer.file_uri().to_owned(),
                    uri_base_id: None,
                },
                region: Some(peer_region),
            },
        })
        .with_fingerprint(WHITAKER_FRAGMENT_KEY, pair_fingerprint)
        .with_fingerprint("tokenHash", token_hash)
        .with_properties(properties)
        .build()
        .map_err(Run0Error::from)
}

fn build_message(
    profile: NormProfile,
    primary_label: &str,
    peer_label: &str,
    score_text: &str,
) -> String {
    format!(
        "Type-{} clone: {} <-> {} (sim = {})",
        profile_number(profile),
        primary_label,
        peer_label,
        score_text
    )
}

fn compact_span(region: &whitaker_sarif::Region) -> String {
    format!(
        "{}:{}-{}:{}",
        region.start_line,
        region.start_column.unwrap_or(1),
        region.end_line.unwrap_or(region.start_line),
        region
            .end_column
            .unwrap_or(region.start_column.unwrap_or(1))
    )
}

fn profile_number(profile: NormProfile) -> &'static str {
    match profile {
        NormProfile::T1 => "1",
        NormProfile::T2 => "2",
    }
}

fn profile_name(profile: NormProfile) -> &'static str {
    match profile {
        NormProfile::T1 => "T1",
        NormProfile::T2 => "T2",
    }
}

fn profile_sort_key(profile: NormProfile) -> u8 {
    match profile {
        NormProfile::T1 => 1,
        NormProfile::T2 => 2,
    }
}

fn score_to_f64(score: SimilarityRatio) -> Run0Result<f64> {
    let value = score.as_decimal_string();
    value
        .parse::<f64>()
        .map_err(|_| Run0Error::InvalidScore { value })
}

fn fragment_map(fragments: &[TokenFragment]) -> BTreeMap<&str, &TokenFragment> {
    fragments
        .iter()
        .map(|fragment| (fragment.id().as_str(), fragment))
        .collect()
}

fn resolve_pair<'a>(
    fragment_map: &'a BTreeMap<&str, &'a TokenFragment>,
    pair: &CandidatePair,
) -> Run0Result<(&'a TokenFragment, &'a TokenFragment)> {
    let left = fragment_map
        .get(pair.left().as_str())
        .copied()
        .ok_or_else(|| Run0Error::MissingFragment {
            fragment_id: pair.left().as_str().to_owned(),
        })?;
    let right = fragment_map
        .get(pair.right().as_str())
        .copied()
        .ok_or_else(|| Run0Error::MissingFragment {
            fragment_id: pair.right().as_str().to_owned(),
        })?;
    Ok((left, right))
}

fn ensure_non_empty(fragment: &TokenFragment) -> Run0Result<()> {
    if fragment.retained_fingerprints().is_empty() {
        return Err(Run0Error::EmptyFingerprintSet {
            fragment_id: fragment.id().as_str().to_owned(),
        });
    }
    Ok(())
}

fn primary_fingerprint(fragment: &TokenFragment) -> Run0Result<&crate::Fingerprint> {
    fragment
        .retained_fingerprints()
        .first()
        .ok_or_else(|| Run0Error::EmptyFingerprintSet {
            fragment_id: fragment.id().as_str().to_owned(),
        })
}

fn pair_fingerprint(accepted: &AcceptedPair) -> String {
    let mut hasher = Sha256::new();
    hasher.update(accepted.pair().left().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(accepted.pair().right().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(profile_name(accepted.profile()).as_bytes());
    digest_hex(hasher.finalize())
}

fn token_hash(left: &TokenFragment, right: &TokenFragment) -> String {
    let mut values = left
        .retained_fingerprints()
        .iter()
        .chain(right.retained_fingerprints().iter())
        .map(|fingerprint| fingerprint.hash)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();

    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_be_bytes());
    }
    digest_hex(hasher.finalize())
}

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut output = String::new();
    for byte in bytes.as_ref() {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0'.saturating_add(value)),
        _ => char::from(b'a'.saturating_add(value.saturating_sub(10))),
    }
}

fn result_sort_key(
    result: &whitaker_sarif::SarifResult,
    other: &whitaker_sarif::SarifResult,
) -> std::cmp::Ordering {
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
