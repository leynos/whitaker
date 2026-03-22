//! Locality-sensitive hashing over fixed-width MinHash signatures.

use std::collections::{BTreeMap, BTreeSet};

use super::{CandidatePair, FragmentId, LshConfig, MinHashSignature};

/// Bucketed LSH index that emits canonical candidate fragment pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LshIndex {
    config: LshConfig,
    buckets: BTreeMap<BandBucketKey, BTreeSet<FragmentId>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BandBucketKey {
    band_index: usize,
    values: Vec<u64>,
}

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
            buckets: BTreeMap::new(),
        }
    }

    /// Inserts a fragment signature into every configured LSH band.
    pub fn insert(&mut self, id: &FragmentId, signature: &MinHashSignature) {
        for (band_index, values) in signature.bands(self.config.rows()).enumerate() {
            let key = BandBucketKey::new(band_index, values);
            self.buckets.entry(key).or_default().insert(id.clone());
        }
    }

    /// Returns canonical, deduplicated candidate pairs in lexical order.
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
}

fn add_bucket_pairs(pairs: &mut BTreeSet<CandidatePair>, members: &BTreeSet<FragmentId>) {
    for (index, left) in members.iter().enumerate() {
        for right in members.iter().skip(index + 1) {
            if let Some(pair) = CandidatePair::new(left.clone(), right.clone()) {
                pairs.insert(pair);
            }
        }
    }
}
