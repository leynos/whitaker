//! End-to-end pipeline: source files to a serialized SARIF log with Run 1.
//!
//! This is the one test that runs the *real* token pass. Every other Run 1 test
//! constructs `AcceptedPair` values directly, which is what lets them express
//! "the token pass missed this pair" without reverse-engineering a Jaccard
//! score. Here the verdicts come from normalization, shingling, winnowing,
//! `MinHash` and LSH, so a regression anywhere in that chain shows up as a wrong
//! Run 1 result.

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
use whitaker_sarif::{
    AST_HASH_KEY,
    BaselineState,
    SarifLog,
    WHITAKER_FRAGMENT_KEY,
    WHK003_ID,
    merge_runs,
};

/// Shingle width used for the pipeline fixtures.
const K: usize = 5;

/// Winnow window used for the pipeline fixtures.
const WINDOW: usize = 4;

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

/// A near-miss pair: the same loop with one extra statement.
const NEAR_MISS_A: &str = concat!(
    "fn parse(input: &str) -> usize {\n",
    "    let mut total = 0;\n",
    "    for part in input.split(',') {\n",
    "        total += part.len();\n",
    "    }\n",
    "    total\n",
    "}\n",
);

/// The near-miss twin of [`NEAR_MISS_A`].
const NEAR_MISS_B: &str = concat!(
    "fn parse(input: &str) -> usize {\n",
    "    let mut total = 0;\n",
    "    for part in input.split(',') {\n",
    "        total += part.len();\n",
    "    }\n",
    "    total += 1;\n",
    "    total\n",
    "}\n",
);

/// A function with the same signature shape but an unrelated body.
const UNRELATED: &str = concat!(
    "fn parse(items: &[i32], scale: i32) -> i32 {\n",
    "    let mut best = items.first().copied().unwrap_or_default();\n",
    "    while best < scale {\n",
    "        best = best.saturating_mul(3).saturating_add(7);\n",
    "    }\n",
    "    best\n",
    "}\n",
);

/// One source fixture and the two profile fragments built from it.
struct Fixture {
    name: &'static str,
    text: &'static str,
}

impl Fixture {
    /// The fixture's function span, which is its whole text.
    const fn span(&self) -> std::ops::Range<usize> { 0..self.text.len() }

    /// Build the fragment for one normalization profile.
    fn fragment(&self, profile: NormProfile) -> TokenFragment {
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
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn candidates(fragments: &[TokenFragment]) -> Vec<CandidatePair> {
    let hasher = MinHasher::new();
    let config = LshConfig::new(BANDS, ROWS).expect("bands times rows is the sketch width");
    let mut index = LshIndex::new(config);
    for fragment in fragments {
        let signature = hasher
            .sketch(fragment.retained_fingerprints())
            .expect("the fixtures retain fingerprints");
        index.insert(fragment.id(), &signature);
    }
    index.candidate_pairs()
}

/// The whole fixture set, with both profiles for each source.
fn all_fragments() -> Vec<TokenFragment> {
    let fixtures = [
        Fixture {
            name: "near_miss_a",
            text: NEAR_MISS_A,
        },
        Fixture {
            name: "near_miss_b",
            text: NEAR_MISS_B,
        },
        Fixture {
            name: "unrelated",
            text: UNRELATED,
        },
    ];
    fixtures
        .iter()
        .flat_map(|fixture| [NormProfile::T1, NormProfile::T2].map(|p| fixture.fragment(p)))
        .collect()
}

/// Write the three fixtures to a temporary directory and read them back.
///
/// Reading the sources back through `cap_std` keeps the pipeline honest about
/// where its text comes from; the test never scores a literal it did not first
/// round-trip through the filesystem.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn read_fixtures_back() -> BTreeMap<&'static str, String> {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let root =
        camino::Utf8Path::from_path(temp.path()).expect("the temporary path must be valid UTF-8");
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .expect("the temporary directory must open");
    let fixtures = [
        ("near_miss_a", NEAR_MISS_A),
        ("near_miss_b", NEAR_MISS_B),
        ("unrelated", UNRELATED),
    ];
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

/// Token-pass and AST-pass configurations for the pipeline.
fn configs() -> (TokenPassConfig, AstPassConfig) {
    (
        TokenPassConfig::new("whitaker_clones_cli@token", "0.3.0")
            .with_shingle_size(K)
            .with_winnow_window(WINDOW),
        AstPassConfig::new("0.3.0"),
    )
}

/// Run the full pipeline and return the Run 0 and Run 1 runs.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn run_pipeline() -> (whitaker_sarif::Run, whitaker_sarif::Run, Vec<AcceptedPair>) {
    let sources = read_fixtures_back();
    for (name, text) in &sources {
        let expected = match *name {
            "near_miss_a" => NEAR_MISS_A,
            "near_miss_b" => NEAR_MISS_B,
            _ => UNRELATED,
        };
        assert_eq!(text, expected, "fixture `{name}` must round-trip unchanged");
    }

    let fragments = all_fragments();
    let candidates = candidates(&fragments);
    let (token_config, ast_config) = configs();
    let accepted = accept_candidate_pairs(&fragments, &candidates, &token_config)
        .expect("token-pass acceptance must succeed");
    let run0 = emit_run0(&fragments, &accepted, &token_config).expect("Run 0 must emit");
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, &refined, &ast_config).expect("Run 1 must emit");
    (run0, run1, accepted)
}

/// Report whether the token pass accepted the near-miss pair.
///
/// Whether it does depends on the fixture text and the Jaccard threshold, so
/// the pipeline asserts the *consequence* rather than fixing the verdict: an
/// accepted pair is reclassified and an unaccepted one is added, and both are
/// `WHK003` results in Run 1.
fn near_miss_was_accepted(accepted: &[AcceptedPair]) -> bool {
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
fn assert_single_near_miss(run1: &whitaker_sarif::Run, was_accepted: bool) {
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
fn assert_unrelated_is_not_reported(run1: &whitaker_sarif::Run) {
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

/// Check the whole pipeline from source files to a serialized SARIF log.
#[test]
fn pipeline_produces_run_one_with_an_added_near_miss() {
    let (run0, run1, accepted) = run_pipeline();
    let was_accepted = near_miss_was_accepted(&accepted);

    assert_single_near_miss(&run1, was_accepted);
    assert_unrelated_is_not_reported(&run1);

    let log = SarifLog {
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_owned(),
        version: "2.1.0".to_owned(),
        runs: vec![run0, run1],
    };
    let json = serde_json::to_string(&log).expect("the log must serialize");
    let parsed: SarifLog = serde_json::from_str(&json).expect("the log must parse back");

    let parsed_runs = &parsed.runs;
    assert_eq!(parsed_runs.len(), 2, "the log carries Run 0 and Run 1");
    let second = parsed_runs.get(1).expect("Run 1 is the second run");
    assert_eq!(
        second.tool.driver.name, "whitaker_clones_cli@ast",
        "the refined run names the AST producer"
    );
    assert!(
        second
            .results
            .iter()
            .any(|result| result.rule_id == WHK003_ID),
        "the parsed Run 1 still holds the added near-miss"
    );
}

/// Check that E1 holds on real data: Run 1 covers every Run 0 result.
#[test]
fn run_one_covers_every_run_zero_result() {
    let (run0, run1, _accepted) = run_pipeline();

    for result in &run0.results {
        let key = result
            .partial_fingerprints
            .get(WHITAKER_FRAGMENT_KEY)
            .expect("every Run 0 result carries a fragment key");
        assert!(
            run1.results.iter().any(|refined| {
                refined
                    .partial_fingerprints
                    .get(WHITAKER_FRAGMENT_KEY)
                    .is_some_and(|other| other == key)
            }),
            "Run 1 must carry Run 0's result {key}, got {:?}",
            run1.results.len()
        );
    }
}

/// Check that a merged log keeps Run 1's refinements when Run 1 comes first.
#[test]
fn merged_log_keeps_run_one_first() {
    let (run0, run1, _accepted) = run_pipeline();
    let merged = merge_runs(&[run1, run0]).expect("the runs must merge");

    assert_eq!(
        merged.tool.driver.name, "whitaker_clones_cli@ast",
        "the merged tool name comes from the first run"
    );
    assert!(
        merged
            .results
            .iter()
            .any(|result| result.rule_id == WHK003_ID),
        "the added near-miss must survive the merge"
    );
}

/// Check that an empty candidate set still emits a well-formed Run 1.
#[test]
fn empty_candidates_still_emit_a_run() {
    let fragments = all_fragments();
    let (token_config, ast_config) = configs();
    let input = RefinementInput::new(&fragments, &[], &[], &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, &refined, &ast_config).expect("Run 1 must emit");

    assert!(
        run1.results.is_empty(),
        "no candidates means no results, got {}",
        run1.results.len()
    );
    assert_eq!(
        run1.tool.driver.rules.len(),
        3,
        "Run 1 still declares all three rules"
    );
    assert!(
        run1.invocations
            .first()
            .is_some_and(|invocation| invocation.execution_successful),
        "Run 1 records one successful invocation"
    );
}

/// Check that identical functions in different files report a cosine of one.
#[test]
fn identical_functions_score_a_whole_cosine() {
    let left = Fixture {
        name: "twin_left",
        text: NEAR_MISS_A,
    };
    let right = Fixture {
        name: "twin_right",
        text: NEAR_MISS_A,
    };
    let fragments = vec![
        left.fragment(NormProfile::T1),
        right.fragment(NormProfile::T1),
    ];
    let candidates = candidates(&fragments);
    let (token_config, ast_config) = configs();
    let accepted = accept_candidate_pairs(&fragments, &candidates, &token_config)
        .expect("the identical pair must be accepted");
    assert!(
        !accepted.is_empty(),
        "identical functions must be accepted by the token pass"
    );
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, &refined, &ast_config).expect("Run 1 must emit");

    let [result] = run1.results.as_slice() else {
        panic!(
            "the twin pair must produce one result, got {:?}",
            run1.results
        );
    };
    let properties = result
        .properties
        .as_ref()
        .expect("every result carries Whitaker properties");
    let extracted =
        whitaker_sarif::WhitakerProperties::try_from(properties).expect("properties extract");
    assert_eq!(
        extracted.cosine.map(f64::to_bits),
        Some(1.0_f64.to_bits()),
        "identical function bodies must score a cosine of exactly one"
    );
}
