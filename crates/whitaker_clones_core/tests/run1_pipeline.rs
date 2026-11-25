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

/// A near-miss pair: the same function with two extra statements.
const NEAR_MISS_A: &str = concat!(
    "fn summarize(records: &[Record], limit: usize) -> Summary {\n",
    "    let mut total = 0;\n",
    "    let mut counted = 0;\n",
    "    let mut longest = String::new();\n",
    "    for record in records {\n",
    "        if record.weight == 0 {\n",
    "            continue;\n",
    "        }\n",
    "        total += record.weight;\n",
    "        counted += 1;\n",
    "        if record.label.len() > longest.len() {\n",
    "            longest = record.label.clone();\n",
    "        }\n",
    "    }\n",
    "    if counted == 0 {\n",
    "        return Summary::empty();\n",
    "    }\n",
    "    let average = total / counted;\n",
    "    let spread = total - average;\n",
    "    let label = longest.trim().to_lowercase();\n",
    "    let flagged = average > limit;\n",
    "    Summary {\n",
    "        total,\n",
    "        counted,\n",
    "        average,\n",
    "        spread,\n",
    "        longest: label,\n",
    "        flagged,\n",
    "        truncated: counted > limit,\n",
    "    }\n",
    "}\n",
);

/// The near-miss twin of [`NEAR_MISS_A`].
const NEAR_MISS_B: &str = concat!(
    "fn summarize(records: &[Record], limit: usize) -> Summary {\n",
    "    let mut total = 0;\n",
    "    let mut counted = 0;\n",
    "    let mut longest = String::new();\n",
    "    for record in records {\n",
    "        if record.weight == 0 {\n",
    "            continue;\n",
    "        }\n",
    "        total += record.weight;\n",
    "        counted += 1;\n",
    "        if record.weight > total {\n",
    "            total += record.weight;\n",
    "        }\n",
    "        if record.label.is_empty() {\n",
    "            longest.push('?');\n",
    "        }\n",
    "        if record.label.len() > longest.len() {\n",
    "            longest = record.label.clone();\n",
    "        }\n",
    "    }\n",
    "    if counted == 0 {\n",
    "        return Summary::empty();\n",
    "    }\n",
    "    let average = total / counted;\n",
    "    let spread = total - average;\n",
    "    let label = longest.trim().to_lowercase();\n",
    "    let flagged = average > limit;\n",
    "    Summary {\n",
    "        total,\n",
    "        counted,\n",
    "        average,\n",
    "        spread,\n",
    "        longest: label,\n",
    "        flagged,\n",
    "        truncated: counted > limit,\n",
    "    }\n",
    "}\n",
);

/// A function with the same signature shape but an unrelated body.
const UNRELATED: &str = concat!(
    "fn schedule(tasks: &[Task], budget: usize) -> Vec<Slot> {\n",
    "    let mut slots = Vec::new();\n",
    "    let mut remaining = budget;\n",
    "    for task in tasks {\n",
    "        if task.cost > remaining {\n",
    "            break;\n",
    "        }\n",
    "        remaining -= task.cost;\n",
    "        slots.push(Slot {\n",
    "            name: task.name.clone(),\n",
    "            cost: task.cost,\n",
    "            queued: true,\n",
    "        });\n",
    "    }\n",
    "    slots.sort_by_key(|slot| slot.cost);\n",
    "    slots.dedup_by_key(|slot| slot.name.clone());\n",
    "    if slots.is_empty() {\n",
    "        return Vec::new();\n",
    "    }\n",
    "    slots.reverse();\n",
    "    slots.truncate(budget);\n",
    "    let chosen = slots.len();\n",
    "    debug_assert!(chosen <= budget);\n",
    "    slots\n",
    "}\n",
);

/// One source fixture and the two profile fragments built from it.
struct Fixture<'a> {
    name: &'a str,
    text: &'a str,
}

impl Fixture<'_> {
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
fn fragments_from(sources: &BTreeMap<&'static str, String>) -> Vec<TokenFragment> {
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

    let fragments = fragments_from(&sources);
    let candidates = candidates(&fragments);
    assert_pair_is_a_candidate(&candidates, "near_miss_a", "near_miss_b");
    let (token_config, ast_config) = configs();
    let accepted = accept_candidate_pairs(&fragments, &candidates, &token_config)
        .expect("token-pass acceptance must succeed");
    let run0 = emit_run0(&fragments, &accepted, &token_config).expect("Run 0 must emit");
    let input = RefinementInput::new(&fragments, &candidates, &accepted, &token_config);
    let refined = refine_candidate_pairs(&input, &ast_config).expect("refinement must succeed");
    let run1 = emit_run1(&input, &refined, &ast_config).expect("Run 1 must emit");
    (run0, run1, accepted)
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

    assert!(
        !was_accepted,
        "the fixtures must make the near-miss a genuine token-pass miss"
    );
    assert!(
        run0.results.is_empty(),
        "Run 0 must report nothing for these fixtures, got {} results",
        run0.results.len()
    );
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
        let counterparts = run1
            .results
            .iter()
            .filter(|refined| {
                refined
                    .partial_fingerprints
                    .get(WHITAKER_FRAGMENT_KEY)
                    .is_some_and(|other| other == key)
            })
            .collect::<Vec<_>>();
        let [counterpart] = counterparts.as_slice() else {
            panic!(
                "Run 1 must hold exactly one counterpart for {key}, got {}",
                counterparts.len()
            );
        };

        assert_eq!(
            counterpart.locations, result.locations,
            "a carried or reclassified result keeps its Run 0 primary location"
        );
        assert_eq!(
            counterpart.related_locations.len(),
            result.related_locations.len(),
            "a carried or reclassified result keeps its related locations"
        );
        // The only permitted rule transition is a token rule becoming WHK003.
        let same_rule = counterpart.rule_id == result.rule_id;
        let strengthened = counterpart.rule_id == WHK003_ID && result.rule_id != WHK003_ID;
        assert!(
            same_rule || strengthened,
            "Run 1 may keep rule {} or strengthen it to WHK003, got {}",
            result.rule_id,
            counterpart.rule_id
        );
    }
}

/// Check the `astHash` fingerprint keeps the 64-hex-digit digest shape.
///
/// The snapshot redacts the digest so it survives a parser re-pin, which would
/// otherwise leave the shape unchecked.
#[test]
fn ast_hash_fingerprint_is_a_sixty_four_digit_digest() {
    let (_run0, run1, _accepted) = run_pipeline();

    let hashes = run1
        .results
        .iter()
        .filter_map(|result| result.partial_fingerprints.get(AST_HASH_KEY))
        .collect::<Vec<_>>();
    assert!(
        !hashes.is_empty(),
        "the fixture must produce at least one conclusive result"
    );
    for hash in hashes {
        assert_eq!(hash.len(), 64, "the astHash digest must be 64 hex digits");
        assert!(
            hash.chars().all(|character| character.is_ascii_hexdigit()),
            "the astHash digest must be hexadecimal, got {hash}"
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

/// Pin the complete Run 1 JSON for the pipeline fixture.
///
/// The snapshot redacts the two fields that legitimately move without a
/// contract change: the producer version, and the AST digests, which change
/// whenever the parser pin moves. Everything else — rules, ordering, baseline
/// states, provenance, regions and scores — is pinned.
#[test]
fn run_one_json_snapshot() {
    let (_run0, run1, _accepted) = run_pipeline();
    let value = serde_json::to_value(&run1).expect("Run 1 must serialize");
    let mut redacted = value;
    redact(&mut redacted);
    insta::assert_json_snapshot!("run1_pipeline", redacted);
}

/// Replace version- and parser-dependent values with stable placeholders.
fn redact(value: &mut serde_json::Value) {
    redact_version(value);
    let Some(results) = value.get_mut("results").and_then(|r| r.as_array_mut()) else {
        return;
    };
    for result in results {
        redact_fingerprints(result);
    }
}

/// Replace the producer version, which moves with every release.
fn redact_version(value: &mut serde_json::Value) {
    let version_slot = value
        .get_mut("tool")
        .and_then(|tool| tool.get_mut("driver"))
        .and_then(|driver| driver.get_mut("version"));
    if let Some(version) = version_slot {
        *version = serde_json::Value::String("<version>".to_owned());
    }
}

/// Replace the digests that change when the parser pin moves.
fn redact_fingerprints(result: &mut serde_json::Value) {
    let Some(fingerprints) = result
        .get_mut("partialFingerprints")
        .and_then(|map| map.as_object_mut())
    else {
        return;
    };
    for key in ["astHash", "tokenHash", "whitakerFragment"] {
        if fingerprints.contains_key(key) {
            fingerprints.insert(
                key.to_owned(),
                serde_json::Value::String(format!("<{key}>")),
            );
        }
    }
}

/// Check the fixtures are large enough for the plan's pipeline requirement.
///
/// The `ExecPlan` asks for at least 150 tokens per fixture. Small functions make
/// the Jaccard and cosine scores unrepresentative, and they are also where the
/// trigram feature is least discriminating.
#[test]
fn fixtures_are_at_least_one_hundred_and_fifty_tokens() {
    let sources = read_fixtures_back();
    for (name, text) in &sources {
        let tokens = normalize(text, NormProfile::T1).expect("the fixture must normalize");
        assert!(
            tokens.len() >= 150,
            "fixture `{name}` has {} tokens, below the plan's 150",
            tokens.len()
        );
    }
}

/// E2: Run 1 is byte-identical under permutation and duplication of its input.
///
/// The emitter sorts and deduplicates, so a caller that feeds candidates in a
/// different order — or the same pair twice, as happens when both profiles
/// propose it — must get the same log.
#[test]
fn run_one_is_deterministic_under_input_permutation() {
    let sources = read_fixtures_back();
    let fragments = fragments_from(&sources);
    let candidates = candidates(&fragments);
    let (token_config, ast_config) = configs();

    let mut doubled = candidates.clone();
    doubled.extend(candidates.iter().cloned());
    doubled.reverse();

    let forward = emit_for(&fragments, &candidates, &token_config, &ast_config);
    let shuffled = emit_for(&fragments, &doubled, &token_config, &ast_config);

    let forward_json = serde_json::to_string(&forward).expect("Run 1 must serialize");
    let shuffled_json = serde_json::to_string(&shuffled).expect("Run 1 must serialize");
    assert_eq!(
        forward_json, shuffled_json,
        "permuting and duplicating the candidate input must not change Run 1"
    );
}

/// Refine and emit Run 1 for a given candidate set.
#[expect(
    clippy::expect_used,
    reason = "test-support helper outside a #[test] function; the crate's "
)]
fn emit_for(
    fragments: &[TokenFragment],
    candidates: &[CandidatePair],
    token_config: &TokenPassConfig,
    ast_config: &AstPassConfig,
) -> whitaker_sarif::Run {
    let accepted = accept_candidate_pairs(fragments, candidates, token_config)
        .expect("token-pass acceptance must succeed");
    let input = RefinementInput::new(fragments, candidates, &accepted, token_config);
    let refined = refine_candidate_pairs(&input, ast_config).expect("refinement must succeed");
    emit_run1(&input, &refined, ast_config).expect("Run 1 must emit")
}

/// Check that an empty candidate set still emits a well-formed Run 1.
#[test]
fn empty_candidates_still_emit_a_run() {
    let sources = read_fixtures_back();
    let fragments = fragments_from(&sources);
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
