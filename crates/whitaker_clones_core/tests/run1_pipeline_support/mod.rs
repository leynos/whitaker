//! Shared construction and assertion helpers for the Run 1 pipeline tests.

use std::collections::BTreeMap;

use cap_std::{ambient_authority, fs_utf8::Dir};
use whitaker_clones_core::{
    AcceptedPair,
    AstPassConfig,
    CandidatePair,
    Fingerprint,
    FragmentId,
    FragmentLocation,
    LshConfig,
    LshIndex,
    MINHASH_SIZE,
    MinHasher,
    NormProfile,
    RefinementInput,
    ShingleSize,
    TokenFragment,
    TokenPassConfig,
    WinnowWindow,
    accept_candidate_pairs,
    emit_run0,
    emit_run1,
    hash_shingles,
    normalize,
    refine_candidate_pairs,
    winnow,
};
use whitaker_sarif::{AST_HASH_KEY, BaselineState, WHK003_ID};

/// Shingle width used for the pipeline fixtures.
pub(super) const K: usize = 5;

/// Winnow window used for the pipeline fixtures.
pub(super) const WINDOW: usize = 4;

/// LSH bands. A single band would require all 128 sketch lanes to agree, which
/// almost no realistic pair does; 32 bands of 4 rows lets one shared band
/// surface the pair, which is what the real index configuration does.
const BANDS: usize = 32;

/// Rows per LSH band. Must multiply with `BANDS` to the sketch width.
const ROWS: usize = 4;

const _: () = assert!(
    BANDS * ROWS == MINHASH_SIZE,
    "the LSH bands must span the sketch exactly"
);

/// A near-miss pair: the same function with two extra statements.
pub(super) const NEAR_MISS_A: &str = include_str!("../fixtures/run1_pipeline/near_miss_a.rs");

/// The near-miss twin of [`NEAR_MISS_A`].
pub(super) const NEAR_MISS_B: &str = include_str!("../fixtures/run1_pipeline/near_miss_b.rs");

/// A function with the same signature shape but an unrelated body.
pub(super) const UNRELATED: &str = include_str!("../fixtures/run1_pipeline/unrelated.rs");

/// A rename-equivalent twin of [`CARRIED_B`], which the token pass accepts.
pub(super) const CARRIED_A: &str = include_str!("../fixtures/run1_pipeline/carried_a.rs");

/// A rename-equivalent twin of [`CARRIED_A`], which the token pass accepts.
pub(super) const CARRIED_B: &str = include_str!("../fixtures/run1_pipeline/carried_b.rs");

/// One source fixture and the two profile fragments built from it.
pub(super) struct Fixture<'a> {
    name: &'a str,
    text: &'a str,
}

impl<'a> Fixture<'a> {
    /// Build a fixture from a name and its source text.
    pub(super) const fn new(name: &'a str, text: &'a str) -> Self { Self { name, text } }

    /// The fixture's function span, which is its whole text.
    const fn span(&self) -> std::ops::Range<usize> { 0..self.text.len() }

    /// Build the fragment for one normalization profile.
    pub(super) fn fragment(&self, profile: NormProfile) -> TokenFragment {
        let id = FragmentId::from(format!(
            "{}:{}",
            self.name,
            match profile {
                NormProfile::T1 => "T1",
                NormProfile::T2 => "T2",
            }
        ));
        let retained = retained_fingerprints(self.text, profile);
        TokenFragment::new(
            id,
            profile,
            FragmentLocation::new(format!("src/{}.rs", self.name), self.span()),
            self.text,
        )
        .with_retained_fingerprints(retained)
    }
}

/// Normalize, shingle and winnow one source under a profile.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn retained_fingerprints(text: &str, profile: NormProfile) -> Vec<Fingerprint> {
    let tokens = normalize(text, profile).expect("the fixture must normalize");
    let shingles = hash_shingles(&tokens, ShingleSize::try_from(K).expect("k is non-zero"));
    winnow(
        &shingles,
        WinnowWindow::try_from(WINDOW).expect("window is non-zero"),
    )
}

/// Build the candidate pairs for a fragment set through `MinHash` and LSH.
///
/// There is one index per normalization profile. A single index holding both
/// profiles can pair a `T1` fragment with a `T2` fragment, and
/// `accept_candidate_pairs` rejects such a pair with `MixedProfiles`, which
/// would abort these helpers instead of letting refinement run. Profiles are
/// scored separately everywhere else, so separate indexes also match how the
/// passes are configured.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
pub(super) fn candidates(fragments: &[TokenFragment]) -> Vec<CandidatePair> {
    let hasher = MinHasher::new();
    let config = LshConfig::new(BANDS, ROWS).expect("bands times rows is the sketch width");
    let mut pairs = Vec::new();
    for profile in [NormProfile::T1, NormProfile::T2] {
        let mut index = LshIndex::new(config);
        for fragment in fragments.iter().filter(|f| f.profile() == profile) {
            let signature = hasher
                .sketch(fragment.retained_fingerprints())
                .expect("the fixtures retain fingerprints");
            index.insert(fragment.id(), &signature);
        }
        pairs.extend(index.candidate_pairs());
    }
    pairs
}

/// The whole fixture set, with both profiles for each source.
pub(super) fn fragments_from(sources: &BTreeMap<&'static str, String>) -> Vec<TokenFragment> {
    sources
        .iter()
        .flat_map(|(name, text)| {
            [NormProfile::T1, NormProfile::T2].map(|profile| {
                Fixture {
                    name,
                    text: text.as_str(),
                }
                .fragment(profile)
            })
        })
        .collect()
}

/// Write the three shipped fixtures to a temporary directory and read them back.
///
/// Reading the sources back through `cap_std` keeps the pipeline honest about
/// where its text comes from; the test never scores a literal it did not first
/// round-trip through the filesystem.
pub(super) fn read_fixtures_back() -> BTreeMap<&'static str, String> {
    round_trip_fixtures(&[
        ("near_miss_a", NEAR_MISS_A),
        ("near_miss_b", NEAR_MISS_B),
        ("unrelated", UNRELATED),
    ])
}

/// Token-pass and AST-pass configurations for the pipeline.
pub(super) fn configs() -> (TokenPassConfig, AstPassConfig) {
    (
        TokenPassConfig::new("whitaker_clones_cli@token", "0.3.0")
            .with_shingle_size(K)
            .with_winnow_window(WINDOW),
        AstPassConfig::new("0.3.0"),
    )
}

/// Run the full pipeline and return both runs plus the accepted pairs.
///
/// This fixes the shipped fixtures and default configs; `run_named_pipeline`
/// is the general form. Keeping the fixture round-trip assertions here means
/// every test that calls this one also proves its text came from a file.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
pub(super) fn run_pipeline() -> (whitaker_sarif::Run, whitaker_sarif::Run, Vec<AcceptedPair>) {
    let sources = read_fixtures_back();
    for (name, text) in &sources {
        let expected = match *name {
            "near_miss_a" => NEAR_MISS_A,
            "near_miss_b" => NEAR_MISS_B,
            _ => UNRELATED,
        };
        assert_eq!(text, expected, "fixture `{name}` must round-trip unchanged");
    }

    let fragments = fragments_from(&sources);
    let candidates = candidates(&fragments);
    assert_pair_is_a_candidate(&candidates, "near_miss_a", "near_miss_b");
    let (token_config, ast_config) = configs();
    let (run0, run1) = run_named_pipeline(&sources, &token_config, &ast_config);
    let accepted = accept_candidate_pairs(&fragments, &candidates, &token_config)
        .expect("token-pass acceptance must succeed");
    (run0, run1, accepted)
}

/// Run both passes over a caller-supplied fixture set and configuration.
///
/// `run_pipeline` fixes the three shipped fixtures and the default thresholds.
/// This variant exists so the Run 0 preservation test can use fixtures the
/// token pass actually accepts: with the shipped fixtures Run 0 is empty, so a
/// coverage loop over its results would check nothing.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
pub(super) fn run_named_pipeline(
    sources: &BTreeMap<&'static str, String>,
    token_config: &TokenPassConfig,
    ast_config: &AstPassConfig,
) -> (whitaker_sarif::Run, whitaker_sarif::Run) {
    let fragments = fragments_from(sources);
    let candidates = candidates(&fragments);
    let accepted = accept_candidate_pairs(&fragments, &candidates, token_config)
        .expect("token-pass acceptance must succeed");
    let run0 = emit_run0(&fragments, &accepted, token_config).expect("Run 0 must emit");
    let input = RefinementInput::new(&fragments, &candidates, &accepted, token_config);
    let refined = refine_candidate_pairs(&input, ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, &refined, ast_config).expect("Run 1 must emit");
    (run0, run1)
}

/// Write the given fixtures to a temporary directory and read them back.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
pub(super) fn round_trip_fixtures(
    fixtures: &[(&'static str, &str)],
) -> BTreeMap<&'static str, String> {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let root =
        camino::Utf8Path::from_path(temp.path()).expect("the temporary path must be valid UTF-8");
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .expect("the temporary directory must open");
    for (name, text) in fixtures {
        dir.write(format!("{name}.rs"), text)
            .expect("the fixture must write");
    }
    fixtures
        .iter()
        .map(|(name, _)| {
            let text = dir
                .read_to_string(format!("{name}.rs"))
                .expect("the fixture must read back");
            (*name, text)
        })
        .collect()
}

/// Assert that LSH surfaced the named pair as a candidate.
///
/// A pair that Run 0 never sees cannot be reported by either pass, so the
/// pipeline's "added by the AST pass" claim only means something once the
/// index has proposed the pair.
fn assert_pair_is_a_candidate(candidates: &[CandidatePair], left: &str, right: &str) {
    let proposed = candidates.iter().any(|pair| {
        let sides = [pair.left().as_str(), pair.right().as_str()];
        sides.iter().any(|side| side.starts_with(left))
            && sides.iter().any(|side| side.starts_with(right))
    });
    assert!(
        proposed,
        "LSH must propose the {left}/{right} pair, got {} candidates",
        candidates.len()
    );
}

/// Report whether the token pass accepted the near-miss pair.
///
/// Whether it does depends on the fixture text and the Jaccard threshold, so
/// the pipeline asserts the *consequence* rather than fixing the verdict: an
/// accepted pair is reclassified and an unaccepted one is added, and both are
/// `WHK003` results in Run 1.
pub(super) fn near_miss_was_accepted(accepted: &[AcceptedPair]) -> bool {
    accepted.iter().any(|pair| {
        let left = pair.pair().left().as_str();
        let right = pair.pair().right().as_str();
        left.starts_with("near_miss_") && right.starts_with("near_miss_")
    })
}

/// Assert that Run 1 holds exactly one `WHK003` result for the near-miss pair.
#[expect(
    clippy::expect_used,
    reason = "test-support assertion helper outside a #[test] function; the crate's "
)]
pub(super) fn assert_single_near_miss(run1: &whitaker_sarif::Run, was_accepted: bool) {
    let type3 = run1
        .results
        .iter()
        .filter(|result| result.rule_id == WHK003_ID)
        .collect::<Vec<_>>();
    let [near_miss] = type3.as_slice() else {
        panic!("Run 1 must hold exactly one WHK003 result, got {type3:?}");
    };

    let expected_state = if was_accepted {
        BaselineState::Updated
    } else {
        BaselineState::New
    };
    assert_eq!(
        near_miss.baseline_state,
        Some(expected_state),
        "the near-miss must be {} in Run 1",
        if was_accepted {
            "reclassified (the token pass accepted it)"
        } else {
            "added (the token pass missed it)"
        }
    );
    assert!(
        near_miss.partial_fingerprints.contains_key(AST_HASH_KEY),
        "a WHK003 result records its astHash"
    );
    let properties = near_miss
        .properties
        .as_ref()
        .expect("every result carries Whitaker properties");
    let extracted =
        whitaker_sarif::WhitakerProperties::try_from(properties).expect("properties extract");
    let cosine = extracted
        .cosine
        .expect("conclusive evidence records a cosine");
    assert!(
        cosine >= 0.85,
        "the near-miss must reach the threshold, got {cosine}"
    );
    assert!(
        extracted.refinement.is_some(),
        "a WHK003 result records how it reached Run 1"
    );
}

/// Assert that no result mentions the unrelated fixture.
pub(super) fn assert_unrelated_is_not_reported(run1: &whitaker_sarif::Run) {
    for result in &run1.results {
        let mentions_unrelated = result.message.text.contains("unrelated")
            || result.locations.iter().any(|location| {
                location
                    .physical_location
                    .artefact_location
                    .uri
                    .contains("unrelated")
            })
            || result.related_locations.iter().any(|related| {
                related
                    .physical_location
                    .artefact_location
                    .uri
                    .contains("unrelated")
            });
        assert!(
            !mentions_unrelated,
            "the unrelated fixture must not be reported, got {}",
            result.message.text
        );
    }
}
