//! Sparse feature-vector construction and similarity helpers.

use std::collections::BTreeMap;

use super::profile::MethodProfile;

const FIELD_WEIGHT: u64 = 6;
const DOMAIN_WEIGHT: u64 = 5;
const SIGNATURE_TYPE_WEIGHT: u64 = 4;
const LOCAL_TYPE_WEIGHT: u64 = 3;
const KEYWORD_WEIGHT: u64 = 2;

const STOP_WORDS: &[&str] = &[
    "build", "create", "do", "get", "handle", "make", "process", "render", "run", "set", "update",
];

pub(crate) const MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED: u64 = 1;
pub(crate) const MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum FeatureCategory {
    Domain,
    Field,
    Keyword,
    SignatureType,
    LocalType,
}

impl FeatureCategory {
    fn prefix(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Field => "field",
            Self::Keyword => "keyword",
            Self::SignatureType => "sig",
            Self::LocalType => "local",
        }
    }

    fn weight(self) -> u64 {
        match self {
            Self::Domain => DOMAIN_WEIGHT,
            Self::Field => FIELD_WEIGHT,
            Self::Keyword => KEYWORD_WEIGHT,
            Self::SignatureType => SIGNATURE_TYPE_WEIGHT,
            Self::LocalType => LOCAL_TYPE_WEIGHT,
        }
    }

    pub(crate) fn label_priority(self) -> usize {
        match self {
            Self::Domain => 0,
            Self::Field => 1,
            Self::Keyword => 2,
            Self::SignatureType => 3,
            Self::LocalType => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct FeatureMetadata {
    category: FeatureCategory,
    display: String,
}

struct FeatureIdentity {
    canonical: String,
    display: String,
}

impl FeatureMetadata {
    pub(crate) fn category(&self) -> FeatureCategory {
        self.category
    }

    pub(crate) fn display(&self) -> &str {
        &self.display
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct MethodFeatureVector {
    method_name: String,
    weights: BTreeMap<String, u64>,
    metadata: BTreeMap<String, FeatureMetadata>,
}

impl MethodFeatureVector {
    pub(crate) fn method_name(&self) -> &str {
        &self.method_name
    }

    pub(crate) fn weights(&self) -> &BTreeMap<String, u64> {
        &self.weights
    }

    pub(crate) fn metadata(&self) -> &BTreeMap<String, FeatureMetadata> {
        &self.metadata
    }

    pub(crate) fn norm_squared(&self) -> u64 {
        self.weights.values().map(|weight| weight * weight).sum()
    }
}

pub(crate) fn build_feature_vector(profile: &MethodProfile) -> MethodFeatureVector {
    let mut weights = BTreeMap::new();
    let mut metadata = BTreeMap::new();

    for field in profile.accessed_fields() {
        add_feature(
            &mut weights,
            &mut metadata,
            FeatureCategory::Field,
            feature_identity(field),
        );
    }

    for domain in profile.external_domains() {
        add_feature(
            &mut weights,
            &mut metadata,
            FeatureCategory::Domain,
            feature_identity(domain),
        );
    }

    for type_name in profile.signature_types() {
        add_feature(
            &mut weights,
            &mut metadata,
            FeatureCategory::SignatureType,
            type_identity(type_name),
        );
    }

    for type_name in profile.local_types() {
        add_feature(
            &mut weights,
            &mut metadata,
            FeatureCategory::LocalType,
            type_identity(type_name),
        );
    }

    for keyword in identifier_keywords(profile.name()) {
        add_feature(
            &mut weights,
            &mut metadata,
            FeatureCategory::Keyword,
            FeatureIdentity {
                canonical: keyword.clone(),
                display: keyword,
            },
        );
    }

    MethodFeatureVector {
        method_name: profile.name().to_owned(),
        weights,
        metadata,
    }
}

/// Evaluates Whitaker's shipped cosine threshold for two method profiles.
///
/// The runtime compares the squared cosine form `25 * dot^2 >= left_norm *
/// right_norm`, which is equivalent to `cosine >= 0.20` when both norms are
/// non-zero.
#[must_use]
pub(crate) fn methods_meet_cosine_threshold(left: &MethodProfile, right: &MethodProfile) -> bool {
    let left_vector = build_feature_vector(left);
    let right_vector = build_feature_vector(right);
    cosine_threshold_met(
        &left_vector,
        &right_vector,
        MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED,
        MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED,
    )
}

pub(crate) fn cosine_threshold_met(
    left: &MethodFeatureVector,
    right: &MethodFeatureVector,
    min_similarity_numerator: u64,
    min_similarity_denominator: u64,
) -> bool {
    let dot = dot_product(left.weights(), right.weights());
    if dot == 0 {
        return false;
    }

    let left_norm = left.norm_squared();
    let right_norm = right.norm_squared();
    if left_norm == 0 || right_norm == 0 {
        return false;
    }

    let dot_squared = u128::from(dot) * u128::from(dot);
    let left_side = u128::from(min_similarity_denominator) * dot_squared;
    let right_side =
        u128::from(min_similarity_numerator) * u128::from(left_norm) * u128::from(right_norm);
    left_side >= right_side
}

#[cfg(test)]
pub(crate) fn test_feature_vector(
    method_name: &str,
    weights: &[(&str, u64)],
) -> MethodFeatureVector {
    MethodFeatureVector {
        method_name: method_name.to_owned(),
        weights: weights
            .iter()
            .map(|(feature, weight)| ((*feature).to_owned(), *weight))
            .collect(),
        metadata: BTreeMap::new(),
    }
}

pub(crate) fn dot_product(left: &BTreeMap<String, u64>, right: &BTreeMap<String, u64>) -> u64 {
    let (smaller, larger) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };

    smaller
        .iter()
        .filter_map(|(feature, left_weight)| {
            larger
                .get(feature)
                .map(|right_weight| left_weight * right_weight)
        })
        .sum()
}

pub(crate) fn identifier_keywords(identifier: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = identifier.chars().collect();

    for (index, character) in chars.iter().enumerate() {
        if !character.is_alphanumeric() {
            push_token(&mut tokens, &mut current);
            continue;
        }

        if should_split_before(&chars, index) {
            push_token(&mut tokens, &mut current);
        }

        current.extend(character.to_lowercase());
    }

    push_token(&mut tokens, &mut current);

    tokens.retain(|token| !STOP_WORDS.contains(&token.as_str()));
    tokens
}

fn add_feature(
    weights: &mut BTreeMap<String, u64>,
    metadata: &mut BTreeMap<String, FeatureMetadata>,
    category: FeatureCategory,
    identity: FeatureIdentity,
) {
    if identity.canonical.is_empty() {
        return;
    }

    let key = format!("{}:{}", category.prefix(), identity.canonical);
    *weights.entry(key.clone()).or_insert(0) += category.weight();
    metadata.entry(key).or_insert_with(|| FeatureMetadata {
        category,
        display: identity.display,
    });
}

fn canonical_feature_value(value: &str) -> String {
    value.trim().to_lowercase()
}

fn feature_identity(value: &str) -> FeatureIdentity {
    let canonical = canonical_feature_value(value);
    FeatureIdentity {
        display: canonical.clone(),
        canonical,
    }
}

fn type_identity(type_name: &str) -> FeatureIdentity {
    FeatureIdentity {
        canonical: canonical_feature_value(type_name),
        display: type_name.trim().to_owned(),
    }
}

fn should_split_before(chars: &[char], index: usize) -> bool {
    if index == 0 {
        return false;
    }

    let current = chars[index];
    if !current.is_uppercase() {
        return false;
    }

    let previous = chars[index - 1];
    previous.is_lowercase()
        || chars
            .get(index + 1)
            .is_some_and(|next| previous.is_uppercase() && next.is_lowercase())
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}
