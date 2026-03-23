//! Unit tests for MinHash and LSH candidate generation.

use rstest::{fixture, rstest};

use crate::token::Fingerprint;

use super::{
    CandidatePair, FragmentId, IndexError, LshConfig, LshIndex, MINHASH_SIZE, MinHashSignature,
    MinHasher,
};

fn fingerprints(values: &[u64]) -> Vec<Fingerprint> {
    values
        .iter()
        .enumerate()
        .map(|(index, hash)| Fingerprint::new(*hash, index..index.saturating_add(1)))
        .collect()
}

fn sketch(values: &[u64]) -> MinHashSignature {
    MinHasher::new()
        .sketch(&fingerprints(values))
        .expect("fingerprints should sketch successfully")
}

#[fixture]
fn single_band_config() -> LshConfig {
    LshConfig::new(1, MINHASH_SIZE).expect("single-band config should validate")
}

#[fixture]
fn multi_band_config() -> LshConfig {
    LshConfig::new(32, 4).expect("multi-band config should validate")
}

#[fixture]
fn shared_signature() -> MinHashSignature {
    sketch(&[1, 2, 3, 4])
}

#[fixture]
fn distinct_signature() -> MinHashSignature {
    sketch(&[8, 9, 10, 11])
}

#[fixture]
fn identical_signature() -> MinHashSignature {
    sketch(&[5, 7, 11, 13])
}

#[fixture]
fn alpha_id() -> FragmentId {
    FragmentId::from("alpha")
}

#[fixture]
fn beta_id() -> FragmentId {
    FragmentId::from("beta")
}

#[fixture]
fn gamma_id() -> FragmentId {
    FragmentId::from("gamma")
}

#[rstest]
#[case(((0, 4), IndexError::ZeroBands))]
#[case(((4, 0), IndexError::ZeroRows))]
#[case(((8, 8), IndexError::invalid_band_row_product(8, 8)))]
fn config_rejects_invalid_inputs(#[case] case: ((usize, usize), IndexError)) {
    let ((bands, rows), expected) = case;

    assert_eq!(LshConfig::new(bands, rows), Err(expected));
}

#[rstest]
#[case((1, MINHASH_SIZE))]
#[case((2, MINHASH_SIZE / 2))]
#[case((4, MINHASH_SIZE / 4))]
fn config_accepts_valid_inputs(#[case] case: (usize, usize)) {
    let (bands, rows) = case;

    let config =
        LshConfig::new(bands, rows).expect("valid (bands, rows) combinations should be accepted");

    assert_eq!(
        config.bands(),
        bands,
        "config should keep the requested number of bands"
    );
    assert_eq!(
        config.rows(),
        rows,
        "config should keep the requested number of rows"
    );
}

#[test]
fn sketch_rejects_empty_fingerprints() {
    let hasher = MinHasher::new();

    assert_eq!(hasher.sketch(&[]), Err(IndexError::EmptyFingerprintSet));
}

#[test]
fn min_hasher_is_deterministic_across_instances() {
    let fingerprints = fingerprints(&[3, 5, 8, 13]);
    let left = MinHasher::new()
        .sketch(&fingerprints)
        .expect("left instance should sketch");
    let right = MinHasher::new()
        .sketch(&fingerprints)
        .expect("right instance should sketch");

    assert_eq!(left, right);
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

#[rstest]
fn insertion_order_does_not_change_candidate_output(
    single_band_config: LshConfig,
    shared_signature: MinHashSignature,
    distinct_signature: MinHashSignature,
) {
    let alpha = FragmentId::from("alpha");
    let beta = FragmentId::from("beta");
    let gamma = FragmentId::from("gamma");

    let mut forward = LshIndex::new(single_band_config);
    forward.insert(&alpha, &shared_signature);
    forward.insert(&beta, &shared_signature);
    forward.insert(&gamma, &distinct_signature);

    let mut reverse = LshIndex::new(single_band_config);
    reverse.insert(&gamma, &distinct_signature);
    reverse.insert(&beta, &shared_signature);
    reverse.insert(&alpha, &shared_signature);

    let expected = CandidatePair::new(alpha, beta).expect("distinct ids should form a pair");
    assert_eq!(forward.candidate_pairs(), vec![expected.clone()]);
    assert_eq!(reverse.candidate_pairs(), vec![expected]);
}

#[rstest]
fn canonical_ordering_across_multiple_pairs_and_bands(
    shared_signature: MinHashSignature,
    distinct_signature: MinHashSignature,
) {
    let config = LshConfig::new(4, MINHASH_SIZE / 4).expect("LSH config should validate");
    let alpha = FragmentId::from("alpha");
    let beta = FragmentId::from("beta");
    let gamma = FragmentId::from("gamma");
    let delta = FragmentId::from("delta");

    let mut forward = LshIndex::new(config);
    forward.insert(&alpha, &shared_signature);
    forward.insert(&beta, &shared_signature);
    forward.insert(&gamma, &shared_signature);
    forward.insert(&delta, &distinct_signature);

    let mut reverse = LshIndex::new(config);
    reverse.insert(&delta, &distinct_signature);
    reverse.insert(&gamma, &shared_signature);
    reverse.insert(&beta, &shared_signature);
    reverse.insert(&alpha, &shared_signature);

    let expected = vec![
        CandidatePair::new(alpha.clone(), beta.clone()).expect("distinct ids should form a pair"),
        CandidatePair::new(alpha.clone(), gamma.clone()).expect("distinct ids should form a pair"),
        CandidatePair::new(beta.clone(), gamma.clone()).expect("distinct ids should form a pair"),
    ];

    assert_eq!(forward.candidate_pairs(), expected);
    assert_eq!(reverse.candidate_pairs(), expected);
}

#[rstest]
fn duplicate_band_collisions_emit_one_pair(
    multi_band_config: LshConfig,
    alpha_id: FragmentId,
    beta_id: FragmentId,
    identical_signature: MinHashSignature,
) {
    let mut index = LshIndex::new(multi_band_config);

    index.insert(&beta_id, &identical_signature);
    index.insert(&alpha_id, &identical_signature);

    assert_eq!(
        index.candidate_pairs(),
        vec![CandidatePair::new(alpha_id, beta_id).expect("distinct ids should form a pair")]
    );
}

#[rstest]
fn self_pairs_are_not_emitted(single_band_config: LshConfig, alpha_id: FragmentId) {
    let signature = sketch(&[2, 4, 6, 8]);
    let mut index = LshIndex::new(single_band_config);

    index.insert(&alpha_id, &signature);
    index.insert(&alpha_id, &signature);

    assert!(index.candidate_pairs().is_empty());
}
