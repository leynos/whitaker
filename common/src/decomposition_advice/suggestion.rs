//! Suggestion rendering for detected method communities.

use std::collections::BTreeMap;

use super::community::detect_communities;
use super::error::DecompositionError;
use super::profile::{DecompositionContext, MethodProfile, SubjectKind};
use super::vector::{FeatureCategory, MethodFeatureVector, build_feature_vector};

/// The extraction shape suggested for a method community.
///
/// # Examples
///
/// ```
/// use whitaker_common::decomposition_advice::SuggestedExtractionKind;
///
/// assert_ne!(
///     SuggestedExtractionKind::HelperStruct,
///     SuggestedExtractionKind::Module,
/// );
/// ```
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SuggestedExtractionKind {
    /// Extract a helper struct for stateful type methods.
    HelperStruct,
    /// Move domain-specific helpers into a dedicated module.
    Module,
    /// Split a trait into a smaller focused sub-trait.
    SubTrait,
}

impl std::str::FromStr for SuggestedExtractionKind {
    type Err = String;

    /// Parse a supported extraction-kind label.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "helper struct" => Ok(Self::HelperStruct),
            "module" => Ok(Self::Module),
            "sub trait" | "sub-trait" => Ok(Self::SubTrait),
            _ => Err(format!("unknown extraction kind: {s}")),
        }
    }
}

impl std::fmt::Display for SuggestedExtractionKind {
    /// Format the stable extraction-kind label.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::HelperStruct => "helper struct",
            Self::Module => "module",
            Self::SubTrait => "sub-trait",
        })
    }
}

/// Structured output from decomposition analysis.
///
/// # Examples
///
/// ```
/// use whitaker_common::decomposition_advice::{
///     DecompositionContext, MethodProfileBuilder, SubjectKind, SuggestedExtractionKind,
///     suggest_decomposition,
/// };
///
/// let context = DecompositionContext::new("Store", SubjectKind::Type);
///
/// let mut load = MethodProfileBuilder::new("load");
/// load.record_external_domain("std::fs");
///
/// let mut save = MethodProfileBuilder::new("save");
/// save.record_external_domain("std::fs");
///
/// let mut encode = MethodProfileBuilder::new("encode");
/// encode.record_external_domain("serde::json");
///
/// let mut decode = MethodProfileBuilder::new("decode");
/// decode.record_external_domain("serde::json");
///
/// let suggestions = suggest_decomposition(
///     &context,
///     &[load.build(), save.build(), encode.build(), decode.build()],
/// );
///
/// assert_eq!(suggestions.len(), 2);
/// assert_eq!(suggestions[0].extraction_kind(), SuggestedExtractionKind::Module);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecompositionSuggestion {
    label: String,
    extraction_kind: SuggestedExtractionKind,
    methods: Vec<String>,
    rationale: Vec<String>,
}

impl DecompositionSuggestion {
    /// Return the community label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Return the suggested extraction kind.
    #[must_use]
    pub const fn extraction_kind(&self) -> SuggestedExtractionKind {
        self.extraction_kind
    }

    /// Return method names in the community.
    #[must_use]
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Return the dominant features that motivated the suggestion.
    #[must_use]
    pub fn rationale(&self) -> &[String] {
        &self.rationale
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AggregatedFeature {
    category: FeatureCategory,
    display: String,
    score: u64,
}

/// Generate decomposition suggestions for the provided methods.
///
/// Returns an empty vector when the method set does not yield at least two
/// non-singleton communities. The query is free of logging, metrics, and other
/// side effects; a diagnostic adapter decides how to report malformed state.
///
/// # Errors
///
/// Returns [`DecompositionError`] when community detection or feature
/// aggregation receives inconsistent graph or feature metadata.
///
/// # Examples
///
/// ```
/// use whitaker_common::decomposition_advice::{
///     DecompositionContext, MethodProfileBuilder, SubjectKind, suggest_decomposition,
/// };
///
/// let context = DecompositionContext::new("Parser", SubjectKind::Type);
///
/// let mut parse_tokens = MethodProfileBuilder::new("parse_tokens");
/// parse_tokens.record_accessed_field("grammar");
///
/// let mut parse_nodes = MethodProfileBuilder::new("parse_nodes");
/// parse_nodes.record_accessed_field("grammar");
///
/// let methods = vec![parse_tokens.build(), parse_nodes.build()];
///
/// let suggestions = suggest_decomposition(&context, &methods)
///     .expect("well-formed method profiles should analyse");
///
/// assert!(suggestions.is_empty());
/// ```
pub fn suggest_decomposition(
    context: &DecompositionContext,
    methods: &[MethodProfile],
) -> Result<Vec<DecompositionSuggestion>, DecompositionError> {
    let mut vectors: Vec<_> = methods.iter().map(build_feature_vector).collect();
    vectors.sort();
    let communities: Vec<_> = detect_communities(&vectors)?
        .into_iter()
        .filter(|community| community.len() > 1)
        .collect();

    if communities.len() < 2 {
        return Ok(Vec::new());
    }

    let mut suggestions = build_suggestions(context, &communities, &vectors)?;

    if suggestions.len() < 2 {
        return Ok(Vec::new());
    }

    suggestions.sort_by(|left, right| {
        right
            .methods()
            .len()
            .cmp(&left.methods().len())
            .then_with(|| left.label().cmp(right.label()))
    });
    Ok(suggestions)
}

/// Build suggestions for every community without returning partial output.
///
/// # Errors
///
/// Returns an error when a community index or feature metadata entry is
/// invalid.
fn build_suggestions(
    context: &DecompositionContext,
    communities: &[Vec<usize>],
    vectors: &[MethodFeatureVector],
) -> Result<Vec<DecompositionSuggestion>, DecompositionError> {
    let mut suggestions = Vec::new();
    for community in communities {
        if let Some(suggestion) = build_suggestion(context, community, vectors)? {
            suggestions.push(suggestion);
        }
    }
    Ok(suggestions)
}

/// Render one community when it has a suitable extraction label.
///
/// # Errors
///
/// Returns an error when the community references a missing vector or feature
/// metadata entry.
fn build_suggestion(
    context: &DecompositionContext,
    community: &[usize],
    vectors: &[MethodFeatureVector],
) -> Result<Option<DecompositionSuggestion>, DecompositionError> {
    let aggregated = aggregate_features(community, vectors)?;
    let Some(label_feature) = choose_label_feature(&aggregated) else {
        return Ok(None);
    };
    let rationale = choose_rationale(&aggregated);

    let mut methods = Vec::with_capacity(community.len());
    for &index in community {
        let Some(vector) = vectors.get(index) else {
            return Err(DecompositionError::InvalidCommunityIndex {
                vector_count: vectors.len(),
                community_count: community.len(),
            });
        };
        methods.push(vector.method_name().to_owned());
    }

    Ok(Some(DecompositionSuggestion {
        label: label_feature.display.clone(),
        extraction_kind: infer_extraction_kind(context.subject_kind(), label_feature.category),
        methods,
        rationale,
    }))
}

/// Sum feature weights for a community while validating its source metadata.
///
/// # Errors
///
/// Returns an error when a community index or weighted feature metadata is
/// missing.
fn aggregate_features(
    community: &[usize],
    vectors: &[MethodFeatureVector],
) -> Result<Vec<AggregatedFeature>, DecompositionError> {
    let mut aggregated: BTreeMap<String, AggregatedFeature> = BTreeMap::new();

    for &method_index in community {
        let Some(vector) = vectors.get(method_index) else {
            return Err(DecompositionError::InvalidCommunityIndex {
                vector_count: vectors.len(),
                community_count: community.len(),
            });
        };
        for (feature_key, weight) in vector.weights() {
            let Some(metadata) = vector.metadata().get(feature_key) else {
                return Err(DecompositionError::MissingFeatureMetadata {
                    vector_count: vectors.len(),
                });
            };
            let entry =
                aggregated
                    .entry(feature_key.clone())
                    .or_insert_with(|| AggregatedFeature {
                        category: metadata.category(),
                        display: metadata.display().to_owned(),
                        score: 0,
                    });
            entry.score += *weight;
        }
    }

    Ok(aggregated.into_values().collect())
}

/// Select the highest-priority feature category and its strongest label.
fn choose_label_feature(features: &[AggregatedFeature]) -> Option<&AggregatedFeature> {
    const LABEL_PRIORITIES: &[FeatureCategory] = &[
        FeatureCategory::Domain,
        FeatureCategory::Field,
        FeatureCategory::Keyword,
        FeatureCategory::SignatureType,
        FeatureCategory::LocalType,
    ];

    LABEL_PRIORITIES.iter().find_map(|category| {
        features
            .iter()
            .filter(|feature| feature.category == *category)
            .min_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| left.display.cmp(&right.display))
            })
    })
}

/// Return the three strongest features in deterministic display order.
fn choose_rationale(features: &[AggregatedFeature]) -> Vec<String> {
    let mut ordered: Vec<_> = features.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.category
                    .label_priority()
                    .cmp(&right.category.label_priority())
            })
            .then_with(|| left.display.cmp(&right.display))
    });

    ordered
        .into_iter()
        .take(3)
        .map(|feature| feature.display.clone())
        .collect()
}

/// Choose an extraction shape from the subject and selected feature category.
fn infer_extraction_kind(
    subject_kind: SubjectKind,
    label_category: FeatureCategory,
) -> SuggestedExtractionKind {
    match subject_kind {
        SubjectKind::Trait => SuggestedExtractionKind::SubTrait,
        SubjectKind::Type if label_category == FeatureCategory::Domain => {
            SuggestedExtractionKind::Module
        }
        SubjectKind::Type => SuggestedExtractionKind::HelperStruct,
    }
}

#[cfg(test)]
mod tests;
