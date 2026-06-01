//! Locality-sensitive hashing over fixed-width MinHash signatures.

#[cfg(not(kani))]
use std::collections::{BTreeMap, BTreeSet};

#[cfg(kani)]
use super::MINHASH_SIZE;
use super::{CandidatePair, FragmentId, LshConfig, MinHashSignature};

/// Bucketed LSH index that emits canonical candidate fragment pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LshIndex {
    config: LshConfig,
    #[cfg(not(kani))]
    buckets: BTreeMap<BandBucketKey, BTreeSet<FragmentId>>,
    #[cfg(kani)]
    inserted_fragments: InsertedFragmentsForKani,
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
            inserted_fragments: InsertedFragmentsForKani::new(),
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
            let mut inserted_bands = empty_recorded_bands_for_kani();
            for (band_index, values) in signature.bands(self.config.rows()).enumerate() {
                if band_index < KANI_MAX_RECORDED_BANDS {
                    inserted_bands[band_index] =
                        Some(BandBucketKeyForKani::new(band_index, values));
                }
            }
            self.inserted_fragments.push(InsertedFragmentForKani {
                id: id.clone(),
                bands: inserted_bands,
            });
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
        for left_index in 0..self.inserted_fragments.len() {
            let Some(left) = self.inserted_fragments.get(left_index) else {
                kani::assert(false, "inserted_len must reference populated left slots");
                continue;
            };
            for right_index in (left_index + 1)..self.inserted_fragments.len() {
                let Some(right) = self.inserted_fragments.get(right_index) else {
                    kani::assert(false, "inserted_len must reference populated right slots");
                    continue;
                };
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CandidatePairSummaryForKani {
    pub(super) first_pair: Option<CandidatePair>,
    pub(super) unique_pair_count: usize,
    emitted_pairs: [Option<CandidatePair>; KANI_MAX_RECORDED_PAIRS],
}

#[cfg(kani)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InsertedFragmentForKani {
    id: FragmentId,
    bands: [Option<BandBucketKeyForKani>; KANI_MAX_RECORDED_BANDS],
}

#[cfg(kani)]
#[derive(Clone, Debug, PartialEq, Eq)]
struct InsertedFragmentsForKani {
    items: [Option<InsertedFragmentForKani>; KANI_MAX_INSERTED_FRAGMENTS],
    len: usize,
}

#[cfg(kani)]
#[derive(Clone, Copy, Debug)]
struct BandBucketKeyForKani {
    band_index: usize,
    values: [u64; KANI_MAX_ROWS_PER_BAND],
    rows_len: usize,
}

#[cfg(kani)]
macro_rules! rows_equal_for_kani {
    ($left:expr, $right:expr, $rows_len:expr, [$($index:expr),* $(,)?]) => {
        true $(&& ($rows_len <= $index || $left[$index] == $right[$index]))*
    };
}

#[cfg(kani)]
const KANI_MAX_INSERTED_FRAGMENTS: usize = 4;

#[cfg(kani)]
const KANI_MAX_RECORDED_BANDS: usize = 2;

#[cfg(kani)]
const KANI_MAX_RECORDED_PAIRS: usize = 6;

#[cfg(kani)]
const KANI_MAX_ROWS_PER_BAND: usize = MINHASH_SIZE;

#[cfg(kani)]
const fn empty_inserted_fragments_for_kani()
-> [Option<InsertedFragmentForKani>; KANI_MAX_INSERTED_FRAGMENTS] {
    [const { None }; KANI_MAX_INSERTED_FRAGMENTS]
}

#[cfg(kani)]
const fn empty_recorded_pairs_for_kani() -> [Option<CandidatePair>; KANI_MAX_RECORDED_PAIRS] {
    [const { None }; KANI_MAX_RECORDED_PAIRS]
}

#[cfg(kani)]
const fn empty_recorded_bands_for_kani() -> [Option<BandBucketKeyForKani>; KANI_MAX_RECORDED_BANDS]
{
    [const { None }; KANI_MAX_RECORDED_BANDS]
}

#[cfg(kani)]
impl InsertedFragmentsForKani {
    const fn new() -> Self {
        Self {
            items: empty_inserted_fragments_for_kani(),
            len: 0,
        }
    }

    fn push(&mut self, fragment: InsertedFragmentForKani) {
        if self.len < KANI_MAX_INSERTED_FRAGMENTS {
            self.items[self.len] = Some(fragment);
            self.len += 1;
        }
    }

    const fn len(&self) -> usize {
        self.len
    }

    fn get(&self, index: usize) -> Option<&InsertedFragmentForKani> {
        self.items.get(index).and_then(Option::as_ref)
    }
}

#[cfg(kani)]
impl BandBucketKeyForKani {
    fn new(band_index: usize, values: &[u64]) -> Self {
        let mut arr = [0u64; KANI_MAX_ROWS_PER_BAND];
        let rows_len = values.len().min(KANI_MAX_ROWS_PER_BAND);
        arr[..rows_len].copy_from_slice(&values[..rows_len]);
        Self {
            band_index,
            values: arr,
            rows_len,
        }
    }
}

#[cfg(kani)]
impl PartialEq for BandBucketKeyForKani {
    fn eq(&self, other: &Self) -> bool {
        self.band_index == other.band_index
            && self.rows_len == other.rows_len
            && rows_equal_for_kani!(
                self.values,
                other.values,
                self.rows_len,
                [
                    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41,
                    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61,
                    62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81,
                    82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100,
                    101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
                    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127,
                ]
            )
    }
}

#[cfg(kani)]
impl Eq for BandBucketKeyForKani {}

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
    fn contains_pair(&self, pair: &CandidatePair) -> bool {
        self.emitted_pairs
            .iter()
            .flatten()
            .any(|emitted_pair| emitted_pair == pair)
    }

    fn add_unique_pair(&mut self, pair: CandidatePair) {
        if self.contains_pair(&pair) {
            return;
        }
        if self.unique_pair_count < KANI_MAX_RECORDED_PAIRS {
            self.emitted_pairs[self.unique_pair_count] = Some(pair.clone());
        }
        if self.first_pair.is_none() {
            self.first_pair = Some(pair);
        }
        self.unique_pair_count += 1;
    }
}

#[cfg(kani)]
impl Default for CandidatePairSummaryForKani {
    fn default() -> Self {
        Self {
            first_pair: None,
            unique_pair_count: 0,
            emitted_pairs: empty_recorded_pairs_for_kani(),
        }
    }
}
