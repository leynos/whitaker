//! Locality-sensitive hashing over fixed-width MinHash signatures.

#[cfg(not(kani))]
use std::collections::{BTreeMap, BTreeSet};

use super::{CandidatePair, FragmentId, LshConfig, MinHashSignature};

/// Bucketed LSH index that emits canonical candidate fragment pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LshIndex {
    config: LshConfig,
    #[cfg(not(kani))]
    buckets: BTreeMap<BandBucketKey, BTreeSet<FragmentId>>,
    #[cfg(kani)]
    inserted_fragments: [Option<InsertedFragmentForKani>; KANI_MAX_INSERTED_FRAGMENTS],
    #[cfg(kani)]
    inserted_len: usize,
}

#[cfg(not(kani))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BandBucketKey {
    band_index: usize,
    values: Vec<u64>,
}

#[cfg(not(kani))]
impl BandBucketKey {
    fn new(band_index: usize, values: &[u64]) -> Self {
        Self {
            band_index,
            values: values.to_vec(),
        }
    }
}

impl LshIndex {
    /// Creates an empty LSH index for the supplied configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use whitaker_clones_core::{
    ///     Fingerprint, FragmentId, LshConfig, LshIndex, MinHasher, MINHASH_SIZE,
    /// };
    ///
    /// let hasher = MinHasher::new();
    /// let config = LshConfig::new(1, MINHASH_SIZE)?;
    /// let mut index = LshIndex::new(config);
    /// let retained = [Fingerprint::new(11, 0..1), Fingerprint::new(22, 1..2)];
    /// let alpha = FragmentId::from("alpha");
    /// let beta = FragmentId::from("beta");
    ///
    /// index.insert(&alpha, &hasher.sketch(&retained)?);
    /// index.insert(&beta, &hasher.sketch(&retained)?);
    ///
    /// assert_eq!(index.candidate_pairs().len(), 1);
    /// # Ok::<(), whitaker_clones_core::IndexError>(())
    /// ```
    #[must_use]
    pub fn new(config: LshConfig) -> Self {
        Self {
            config,
            #[cfg(not(kani))]
            buckets: BTreeMap::new(),
            #[cfg(kani)]
            inserted_fragments: [None, None, None, None],
            #[cfg(kani)]
            inserted_len: 0,
        }
    }

    /// Inserts a fragment signature into every configured LSH band.
    pub fn insert(&mut self, id: &FragmentId, signature: &MinHashSignature) {
        #[cfg(not(kani))]
        {
            for (band_index, values) in signature.bands(self.config.rows()).enumerate() {
                let key = BandBucketKey::new(band_index, values);
                self.buckets.entry(key).or_default().insert(id.clone());
            }
        }

        #[cfg(kani)]
        {
            let mut inserted_bands = [None, None];
            for (band_index, values) in signature.bands(self.config.rows()).enumerate() {
                if band_index < KANI_MAX_RECORDED_BANDS {
                    inserted_bands[band_index] = values
                        .first()
                        .map(|first_value| BandBucketKeyForKani::new(band_index, *first_value));
                }
            }
            if self.inserted_len < KANI_MAX_INSERTED_FRAGMENTS {
                self.inserted_fragments[self.inserted_len] = Some(InsertedFragmentForKani {
                    id: id.clone(),
                    bands: inserted_bands,
                });
                self.inserted_len += 1;
            }
        }
    }

    /// Returns canonical, deduplicated candidate pairs in lexical order.
    #[cfg(not(kani))]
    #[must_use]
    pub fn candidate_pairs(&self) -> Vec<CandidatePair> {
        let mut pairs = BTreeSet::new();
        for members in self.buckets.values() {
            if members.len() < 2 {
                continue;
            }
            add_bucket_pairs(&mut pairs, members);
        }
        pairs.into_iter().collect()
    }

    #[cfg(kani)]
    pub(super) fn candidate_pair_summary_for_kani(&self) -> CandidatePairSummaryForKani {
        let mut summary = CandidatePairSummaryForKani::default();
        for left_index in 0..self.inserted_len {
            let left = self.inserted_fragments[left_index]
                .as_ref()
                .expect("recorded insertion slot must be populated");
            for right_index in (left_index + 1)..self.inserted_len {
                let right = self.inserted_fragments[right_index]
                    .as_ref()
                    .expect("recorded insertion slot must be populated");
                if fragments_share_band_for_kani(left, right) {
                    summary.add(left, right);
                }
            }
        }
        summary
    }
}

#[cfg(not(kani))]
fn add_bucket_pairs(pairs: &mut BTreeSet<CandidatePair>, members: &BTreeSet<FragmentId>) {
    for (index, left) in members.iter().enumerate() {
        for right in members.iter().skip(index + 1) {
            if let Some(pair) = CandidatePair::new(left.clone(), right.clone()) {
                pairs.insert(pair);
            }
        }
    }
}

#[cfg(kani)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct CandidatePairSummaryForKani {
    pub(super) first_pair: Option<CandidatePair>,
    pub(super) unique_pair_count: usize,
}

#[cfg(kani)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InsertedFragmentForKani {
    id: FragmentId,
    bands: [Option<BandBucketKeyForKani>; KANI_MAX_RECORDED_BANDS],
}

#[cfg(kani)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BandBucketKeyForKani {
    band_index: usize,
    first_value: u64,
}

#[cfg(kani)]
const KANI_MAX_INSERTED_FRAGMENTS: usize = 4;

#[cfg(kani)]
const KANI_MAX_RECORDED_BANDS: usize = 2;

#[cfg(kani)]
impl BandBucketKeyForKani {
    const fn new(band_index: usize, first_value: u64) -> Self {
        Self {
            band_index,
            first_value,
        }
    }
}

#[cfg(kani)]
fn fragments_share_band_for_kani(
    left: &InsertedFragmentForKani,
    right: &InsertedFragmentForKani,
) -> bool {
    for left_band in left.bands.iter().flatten() {
        for right_band in right.bands.iter().flatten() {
            if left_band == right_band {
                return true;
            }
        }
    }
    false
}

#[cfg(kani)]
impl CandidatePairSummaryForKani {
    fn add(&mut self, left: &InsertedFragmentForKani, right: &InsertedFragmentForKani) {
        if let Some(pair) = CandidatePair::new(left.id.clone(), right.id.clone()) {
            self.add_unique_pair(pair);
        }
    }
}

#[cfg(kani)]
impl CandidatePairSummaryForKani {
    fn add_unique_pair(&mut self, pair: CandidatePair) {
        if self.first_pair.as_ref().is_some_and(|first| first == &pair) {
            return;
        }
        if self.first_pair.is_none() {
            self.first_pair = Some(pair);
        }
        self.unique_pair_count += 1;
    }
}
