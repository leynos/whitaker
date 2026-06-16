//! Pure feature-vector extraction over lowered AST trees.

use std::collections::BTreeMap;

use super::{Depth, KindId, NormalisedTree};

/// Exact, depth-resolved syntax-kind counts.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{KindCounts, ast::{Depth, KindId}};
///
/// let counts = KindCounts::default();
/// assert_eq!(counts.count(KindId::new(1), Depth::root()), 0);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindCounts(BTreeMap<(KindId, Depth), u32>);

impl KindCounts {
    /// Returns the exact count for `kind` at `depth`.
    #[must_use]
    pub fn count(&self, kind: KindId, depth: Depth) -> u32 {
        self.0.get(&(kind, depth)).copied().unwrap_or_default()
    }

    /// Iterates over counts in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (KindId, Depth, u32)> + '_ {
        self.0
            .iter()
            .map(|((kind, depth), count)| (*kind, *depth, *count))
    }
}

/// Fixed-point depth weight.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::KindWeight;
///
/// assert_eq!(KindWeight::zero().get(), 0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KindWeight(u64);

impl KindWeight {
    /// Fixed-point scale. Stage C defines the non-zero weighting curve.
    pub const SCALE: u64 = 0;

    /// Returns a zero weight for Stage A skeleton outputs.
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Returns the fixed-point value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Depth-weighted histogram keyed by syntax kind.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{KindHistogram, ast::KindId};
///
/// assert_eq!(KindHistogram::default().get(KindId::new(1)), None);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindHistogram(BTreeMap<KindId, KindWeight>);

impl KindHistogram {
    /// Returns the weight for `kind`, if present.
    #[must_use]
    pub fn get(&self, kind: KindId) -> Option<KindWeight> {
        self.0.get(&kind).copied()
    }

    /// Iterates over kind weights in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (KindId, KindWeight)> + '_ {
        self.0.iter().map(|(kind, weight)| (*kind, *weight))
    }
}

/// Parent/child or parent/child/grandchild production edge.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{Production, ast::KindId};
///
/// let edge = Production::Bigram(KindId::new(1), KindId::new(2));
/// assert_eq!(edge, Production::Bigram(KindId::new(1), KindId::new(2)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Production {
    /// Parent to child edge.
    Bigram(KindId, KindId),
    /// Parent to child to grandchild edge.
    Trigram(KindId, KindId, KindId),
}

/// Multiset of AST production edges.
///
/// # Examples
///
/// ```
/// use whitaker_clones_core::{Production, ProductionMultiset, ast::KindId};
///
/// let production = Production::Bigram(KindId::new(1), KindId::new(2));
/// assert_eq!(ProductionMultiset::default().count(production), 0);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProductionMultiset(BTreeMap<Production, u32>);

impl ProductionMultiset {
    /// Returns the count for `production`.
    #[must_use]
    pub fn count(&self, production: Production) -> u32 {
        self.0.get(&production).copied().unwrap_or_default()
    }

    /// Iterates over bigram entries in deterministic order.
    pub fn bigrams(&self) -> impl Iterator<Item = (Production, u32)> + '_ {
        self.iter()
            .filter(|(production, _)| matches!(production, Production::Bigram(..)))
    }

    /// Iterates over trigram entries in deterministic order.
    pub fn trigrams(&self) -> impl Iterator<Item = (Production, u32)> + '_ {
        self.iter()
            .filter(|(production, _)| matches!(production, Production::Trigram(..)))
    }

    /// Iterates over all productions in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (Production, u32)> + '_ {
        self.0
            .iter()
            .map(|(production, count)| (*production, *count))
    }
}

/// Extracts exact kind counts from `tree`.
///
/// Stage C replaces the empty skeleton with real accumulation logic.
#[must_use]
pub fn kind_counts(_tree: &NormalisedTree) -> KindCounts {
    KindCounts::default()
}

/// Applies depth weighting to exact kind counts.
///
/// Stage C defines the fixed-point weighting curve.
#[must_use]
pub fn weighted_histogram(_counts: &KindCounts) -> KindHistogram {
    KindHistogram::default()
}

/// Extracts a weighted kind histogram from `tree`.
#[must_use]
pub fn kind_histogram(tree: &NormalisedTree) -> KindHistogram {
    weighted_histogram(&kind_counts(tree))
}

/// Extracts AST production counts from `tree`.
///
/// Stage C replaces the empty skeleton with real edge accumulation logic.
#[must_use]
pub fn production_multiset(_tree: &NormalisedTree) -> ProductionMultiset {
    ProductionMultiset::default()
}
