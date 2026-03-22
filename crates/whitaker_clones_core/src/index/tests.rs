//! Unit tests for MinHash and LSH candidate generation.

use rstest::rstest;

use crate::token::Fingerprint;

use super::{CandidatePair, FragmentId, IndexError, LshConfig, LshIndex, MINHASH_SIZE, MinHasher};

fn fingerprints(values: &[u64]) -> Vec<Fingerprint> {
    values
        .iter()
        .enumerate()
        .map(|(index, hash)| Fingerprint::new(*hash, index..index.saturating_add(1)))
        .collect()
}

fn sketch(values: &[u64]) -> super::MinHashSignature {
    MinHasher::new()
        .sketch(&fingerprints(values))
        .expect("fingerprints should sketch successfully")
}

#[rstest]
#[case(((0, 4), IndexError::ZeroBands))]
#[case(((4, 0), IndexError::ZeroRows))]
#[case(((8, 8), IndexError::invalid_band_row_product(8, 8)))]
fn config_rejects_invalid_inputs(#[case] case: ((usize, usize), IndexError)) {
    let ((bands, rows), expected) = case;

    assert_eq!(LshConfig::new(bands, rows), Err(expected));
}

#[test]
fn sketch_rejects_empty_fingerprints() {
    let hasher = MinHasher::new();

    assert_eq!(hasher.sketch(&[]), Err(IndexError::EmptyFingerprintSet));
}

#[test]
fn duplicate_hashes_do_not_change_the_sketch() {
    let hasher = MinHasher::new();
    let unique = hasher
        .sketch(&fingerprints(&[11, 22, 33]))
        .expect("unique hashes should sketch");
    let duplicated = hasher
        .sketch(&fingerprints(&[11, 22, 33, 22, 11]))
        .expect("duplicate hashes should sketch");

    assert_eq!(unique, duplicated);
}

#[test]
fn identical_sets_yield_identical_signatures() {
    let hasher = MinHasher::new();
    let left = hasher
        .sketch(&fingerprints(&[3, 5, 8, 13]))
        .expect("left sketch should succeed");
    let right = hasher
        .sketch(&fingerprints(&[13, 8, 5, 3]))
        .expect("right sketch should succeed");

    assert_eq!(left, right);
}

#[test]
fn insertion_order_does_not_change_candidate_output() {
    let config = LshConfig::new(1, MINHASH_SIZE).expect("LSH config should validate");
    let alpha = FragmentId::from("alpha");
    let beta = FragmentId::from("beta");
    let gamma = FragmentId::from("gamma");
    let shared = sketch(&[1, 2, 3, 4]);
    let distinct = sketch(&[8, 9, 10, 11]);

    let mut forward = LshIndex::new(config);
    forward.insert(&alpha, &shared);
    forward.insert(&beta, &shared);
    forward.insert(&gamma, &distinct);

    let mut reverse = LshIndex::new(config);
    reverse.insert(&gamma, &distinct);
    reverse.insert(&beta, &shared);
    reverse.insert(&alpha, &shared);

    let expected = CandidatePair::new(alpha, beta).expect("distinct ids should form a pair");
    assert_eq!(forward.candidate_pairs(), vec![expected.clone()]);
    assert_eq!(reverse.candidate_pairs(), vec![expected]);
}

#[test]
fn duplicate_band_collisions_emit_one_pair() {
    let config = LshConfig::new(32, 4).expect("LSH config should validate");
    let alpha = FragmentId::from("alpha");
    let beta = FragmentId::from("beta");
    let identical = sketch(&[5, 7, 11, 13]);
    let mut index = LshIndex::new(config);

    index.insert(&beta, &identical);
    index.insert(&alpha, &identical);

    assert_eq!(
        index.candidate_pairs(),
        vec![CandidatePair::new(alpha, beta).expect("distinct ids should form a pair")]
    );
}

#[test]
fn self_pairs_are_not_emitted() {
    let config = LshConfig::new(1, MINHASH_SIZE).expect("LSH config should validate");
    let alpha = FragmentId::from("alpha");
    let signature = sketch(&[2, 4, 6, 8]);
    let mut index = LshIndex::new(config);

    index.insert(&alpha, &signature);
    index.insert(&alpha, &signature);

    assert!(index.candidate_pairs().is_empty());
}
