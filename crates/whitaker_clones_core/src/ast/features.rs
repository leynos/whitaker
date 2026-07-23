//! Pure feature-vector extraction over lowered AST trees.

use std::collections::BTreeMap;

use super::{Depth, KindId, NormalizedNode, NormalizedTree};

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

    fn increment(&mut self, kind: KindId, depth: Depth) {
        self.0
            .entry((kind, depth))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
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
pub struct KindWeight(u128);

impl KindWeight {
    /// Fixed-point scale for `w(depth) = 2^-depth`.
    pub const SCALE: u128 = 1_u128 << 63;

    /// Returns a zero weight.
    #[must_use]
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Returns the fixed-point value.
    #[must_use]
    pub const fn get(self) -> u128 {
        self.0
    }

    const fn from_raw(value: u128) -> Self {
        Self(value)
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

    fn increment(&mut self, production: Production) {
        self.0
            .entry(production)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
}

/// Extracts exact kind counts from `tree`.
///
#[must_use]
pub fn kind_counts(tree: &NormalizedTree) -> KindCounts {
    let mut counts = KindCounts::default();
    count_node_kinds(tree.root(), Depth::root(), &mut counts);
    counts
}

/// Applies depth weighting to exact kind counts.
#[must_use]
pub fn weighted_histogram(counts: &KindCounts) -> KindHistogram {
    let mut histogram = BTreeMap::new();
    for (kind, depth, count) in counts.iter() {
        let contribution = weighted_contribution(depth, count);
        histogram
            .entry(kind)
            .and_modify(|weight: &mut KindWeight| {
                *weight = KindWeight::from_raw(weight.get() + contribution);
            })
            .or_insert_with(|| KindWeight::from_raw(contribution));
    }
    KindHistogram(histogram)
}

/// Extracts a weighted kind histogram from `tree`.
#[must_use]
pub fn kind_histogram(tree: &NormalizedTree) -> KindHistogram {
    weighted_histogram(&kind_counts(tree))
}

/// Extracts AST production counts from `tree`.
#[must_use]
pub fn production_multiset(tree: &NormalizedTree) -> ProductionMultiset {
    let mut productions = ProductionMultiset::default();
    collect_productions(tree.root(), &mut productions);
    productions
}

fn count_node_kinds(node: &NormalizedNode, depth: Depth, counts: &mut KindCounts) {
    let mut pending = vec![(node, depth)];
    while let Some((current, current_depth)) = pending.pop() {
        counts.increment(current.kind(), current_depth);
        let child_depth = next_depth(current_depth);
        pending.extend(
            current
                .children()
                .iter()
                .rev()
                .map(|child| (child, child_depth)),
        );
    }
}

fn next_depth(depth: Depth) -> Depth {
    Depth::new(depth.get().saturating_add(1))
}

// Weights halve with depth (`2^-depth` in fixed point). Below the fixed-point
// resolution the weight intentionally collapses to zero: depths 64..=127 shift
// `SCALE` (`1 << 63`) entirely out through an ordinary in-range shift, and
// depths >= 128 reach `u128`'s bit width, so `checked_shr` returns `None` and
// `unwrap_or_default` supplies the zero. Sub-resolution depths contribute
// nothing rather than panicking or wrapping.
fn depth_weight(depth: Depth) -> u128 {
    KindWeight::SCALE
        .checked_shr(u32::from(depth.get()))
        .unwrap_or_default()
}

fn weighted_contribution(depth: Depth, count: u32) -> u128 {
    depth_weight(depth) * u128::from(count)
}

fn collect_productions(node: &NormalizedNode, productions: &mut ProductionMultiset) {
    let mut pending = vec![node];
    while let Some(parent) = pending.pop() {
        for child in parent.children() {
            productions.increment(Production::Bigram(parent.kind(), child.kind()));
            collect_trigrams(parent, child, productions);
        }
        pending.extend(parent.children().iter().rev());
    }
}

fn collect_trigrams(
    grandparent: &NormalizedNode,
    parent: &NormalizedNode,
    productions: &mut ProductionMultiset,
) {
    for child in parent.children() {
        productions.increment(Production::Trigram(
            grandparent.kind(),
            parent.kind(),
            child.kind(),
        ));
    }
}

#[cfg(test)]
mod tests {
    //! Verifies exact fixed-point depth weights across representable depths.

    use proptest::prelude::*;

    use super::{Depth, depth_weight, weighted_contribution};

    proptest! {
        #[test]
        fn each_representable_count_increases_its_weight(
            depth in 0_u16..64,
            count in 0_u32..u32::MAX
        ) {
            let depth = Depth::new(depth);
            let current_weight = weighted_contribution(depth, count);
            let increased_weight = weighted_contribution(depth, count + 1);

            prop_assert_eq!(increased_weight - current_weight, depth_weight(depth));
        }
    }

    #[test]
    fn weights_below_the_fixed_point_resolution_collapse_to_zero() {
        // Depth 63 keeps the last representable weight; depth 64 is the first
        // whose weight rounds to zero through an ordinary in-range shift. Depth
        // 128 reaches `u128`'s bit width, so `checked_shr` returns `None` and
        // the weight falls back to zero via `unwrap_or_default` rather than
        // panicking, and every deeper depth stays at zero.
        assert_eq!(depth_weight(Depth::new(63)), 1);
        assert_eq!(depth_weight(Depth::new(64)), 0);
        assert_eq!(depth_weight(Depth::new(128)), 0);
        assert_eq!(depth_weight(Depth::new(u16::MAX)), 0);
    }
}
