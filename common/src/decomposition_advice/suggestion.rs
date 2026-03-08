//! Suggestion rendering for detected method communities.

use std::collections::BTreeMap;

use super::community::detect_communities;
use super::profile::{DecompositionContext, MethodProfile, SubjectKind};
use super::vector::{FeatureCategory, MethodFeatureVector, build_feature_vector};

/// The extraction shape suggested for a method community.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::SuggestedExtractionKind;
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

/// Structured output from decomposition analysis.
///
/// # Examples
///
/// ```
/// use common::decomposition_advice::{
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
    /// Returns the community label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the suggested extraction kind.
    #[must_use]
    pub fn extraction_kind(&self) -> SuggestedExtractionKind {
        self.extraction_kind
    }

    /// Returns method names in the community.
    #[must_use]
    pub fn methods(&self) -> &[String] {
        &self.methods
    }

    /// Returns the dominant features that motivated the suggestion.
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

/// Generates decomposition suggestions for the provided methods.
///
/// Returns an empty vector when the method set does not yield at least two
/// non-singleton communities.
#[must_use]
pub fn suggest_decomposition(
    context: &DecompositionContext,
    methods: &[MethodProfile],
) -> Vec<DecompositionSuggestion> {
    let mut sorted_methods = methods.to_vec();
    sorted_methods.sort_by(|left, right| left.name().cmp(right.name()));

    let vectors: Vec<_> = sorted_methods.iter().map(build_feature_vector).collect();
    let communities: Vec<_> = detect_communities(&vectors)
        .into_iter()
        .filter(|community| community.len() > 1)
        .collect();

    if communities.len() < 2 {
        return Vec::new();
    }

    let mut suggestions: Vec<_> = communities
        .iter()
        .map(|community| build_suggestion(context, community, &vectors))
        .collect();

    suggestions.sort_by(|left, right| {
        right
            .methods()
            .len()
            .cmp(&left.methods().len())
            .then_with(|| left.label().cmp(right.label()))
    });
    suggestions
}

fn build_suggestion(
    context: &DecompositionContext,
    community: &[usize],
    vectors: &[MethodFeatureVector],
) -> DecompositionSuggestion {
    let aggregated = aggregate_features(community, vectors);
    let label_feature = choose_label_feature(&aggregated);
    let rationale = choose_rationale(&aggregated);

    DecompositionSuggestion {
        label: label_feature.display.clone(),
        extraction_kind: infer_extraction_kind(context.subject_kind(), label_feature.category),
        methods: community
            .iter()
            .map(|index| vectors[*index].method_name().to_owned())
            .collect(),
        rationale,
    }
}

fn aggregate_features(
    community: &[usize],
    vectors: &[MethodFeatureVector],
) -> Vec<AggregatedFeature> {
    let mut aggregated: BTreeMap<String, AggregatedFeature> = BTreeMap::new();

    for method_index in community {
        for (feature_key, weight) in vectors[*method_index].weights() {
            let metadata = &vectors[*method_index].metadata()[feature_key];
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

    aggregated.into_values().collect()
}

fn choose_label_feature(features: &[AggregatedFeature]) -> &AggregatedFeature {
    const LABEL_PRIORITIES: &[FeatureCategory] = &[
        FeatureCategory::Domain,
        FeatureCategory::Field,
        FeatureCategory::Keyword,
        FeatureCategory::SignatureType,
        FeatureCategory::LocalType,
    ];

    for category in LABEL_PRIORITIES {
        let mut matches: Vec<_> = features
            .iter()
            .filter(|feature| feature.category == *category)
            .collect();

        if matches.is_empty() {
            continue;
        }

        matches.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.display.cmp(&right.display))
        });
        return matches[0];
    }

    panic!("community must contain at least one feature");
}

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
