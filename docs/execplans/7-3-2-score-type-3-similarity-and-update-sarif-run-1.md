# Score Type-3 similarity and update SARIF Run 1 (roadmap 7.3.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: APPROVED (2026-11-23)

This document must be maintained in accordance with `AGENTS.md`. The plan,
including decisions D1 to D18, was approved by the user on 2026-11-23. OQ1 was
resolved by the user on 2026-11-23 (option (b)); OQ2 was resolved at
implementation start by adopting the plan's recommended option (a).

## Purpose / big picture

Whitaker's clone detector runs in two passes. Pass A, the token pass, finds
candidate pairs of code fragments. It accepts some of them as Type-1 clones
(token-identical once whitespace and comments are removed, measured by Jaccard
similarity of at least `19/20`) or Type-2 clones (similar once identifiers and
literals are canonicalized, Jaccard of at least `9/10`). It writes them as
Static Analysis Results Interchange Format (SARIF) "Run 0". Roadmap item 7.3.1
added the parser adapter for Pass B, the abstract syntax tree (AST) pass. That
adapter lowers a fragment's byte span to a parser-independent tree and extracts
a depth-weighted node-kind histogram and a canonical subtree hash. Nothing
consumes those features yet.

This item completes Pass B's scoring and emission. After this change, a library
caller hands the token pass's fragments, candidate pairs and accepted pairs to
`refine_candidate_pairs`, then to `emit_run1`, and receives a SARIF "Run 1"
that:

1. Reports near-miss (Type-3) clones under rule `WHK003` when the exact cosine
   similarity of the two fragments' histograms is at least the configured
   `type3` threshold. These are pairs the token pass did not accept.
2. Reclassifies a token-pass `WHK002` result as `WHK003` with
   `baselineState = "updated"` when the AST evidence positively shows a
   near-miss rather than a pure rename.
3. Carries every other token-pass result forward with
   `baselineState = "unchanged"`, so Run 1 is a complete refined view that the
   future `clone_detected` lint can read alone.
4. Records `properties.whitaker.cosine`, a refinement-provenance property, and
   an `astHash` partial fingerprint on every result with conclusive AST
   evidence.

Observable success:

- `cargo nextest run -p whitaker_clones_core --test run1_sarif_behaviour`
  passes behavioural scenarios built from real Rust snippets.
- `cargo nextest run -p whitaker_clones_core --test run1_pipeline` runs the
  whole library pipeline from source files to a serialized SARIF log whose
  `runs[1]` holds a `WHK003` result.
- An `insta` snapshot pins the complete Run 1 JSON for a fixture that
  exercises every classification.
- `make verus-clone-detector` proves three properties for any input: the
  integer acceptance test means exactly "cosine is at least the threshold"; the
  printed six-decimal cosine never contradicts that decision; and the printed
  score lies in `[0, 1]`.

The command-line interface (`cargo whitaker clones refine`) is roadmap item
7.4.1 and is out of scope. This item delivers the library behaviour that the
command will call.

## Constraints

- Scope is roadmap item 7.3.2 only. Do not implement the CLI (7.4.1), HTML
  reports (7.4.2), the `clone_detected` lint (7.5.x), caching (7.6.1), or
  grouping beyond the pairwise `groupId = 0`, `classSize = 2` convention that
  Run 0 already uses.
- `crates/whitaker_clones_core/src/ast/lowering.rs` remains the only file that
  may import `ra_ap_syntax`. Run 1 code consumes `NormalizedTree`,
  `KindHistogram`, `KindCounts` and `AstHash` only, and `tests/ast_boundary.rs`
  must keep passing. The single pre-approved change to the AST surface is
  decision D13 (covering span).
- `lower_span` remains the only emitter of lowering latency, outcome and
  recovery metrics (design §AST operational metrics). Run 1 adds only
  feature-vector consumption and pair-classification metrics, with bounded
  labels.
- Acceptance and classification must be exact and platform-independent. No
  `f32` or `f64` value may influence any decision. The only floating-point
  value is the SARIF `cosine` property. It is produced by parsing the stable
  six-decimal string in one quarantined helper, following the 7.2.3 precedent
  (`score_to_f64` in `crates/whitaker_clones_core/src/run0/emit.rs`).
- Run 0 behaviour stays the same apart from the `cosine` property changing
  from `0.0` to absent (decision D3). Run 0 regions, fingerprints, messages,
  ordering and rule selection must not change.
- No backward compatibility is required. Whitaker is pre-1.0 and the clone
  detector is experimental, with no users outside the project. Public APIs, the
  SARIF property bag and artefacts written under `target/whitaker/` may change
  freely. Do not add compatibility shims, deprecation paths, migration
  guidance, or tests that read artefacts written before this change; stale
  artefacts are regenerated, not read.
- No source file may exceed 400 lines (`AGENTS.md` and the Whitaker
  `module_max_lines` lint). `run0/emit.rs` is exactly 400 lines on the current
  base, so Run 1 code lives in a new `run1/` module subtree, and the shared
  result helpers move out of `run0/emit.rs` rather than growing it.
- `whitaker_sarif` stays a pure SARIF 2.1.0 model and gains no dependency
  (ADR 0015, enforced by
  `crates/whitaker_sarif/tests/architecture_boundary.rs`). The additions in
  decisions D3, D6, D12, D15 and D17 are model types, constants and container
  changes only.
- Clippy limits from `clippy.toml` apply: at most four function parameters,
  cognitive complexity at most 9, and at most 70 lines per function. The
  workspace denies `integer_division`, `integer_division_remainder_used`,
  `float_arithmetic`, `cast_possible_truncation`, `unwrap_used`, `expect_used`,
  `unreachable`, `indexing_slicing`, `shadow_reuse` and `option_if_let_else`.
  Whitaker's own lints apply too, including `conditional_max_n_branches` (at
  most two predicates per condition), `bumpy_road_function` and
  `module_must_have_inner_docs`.
- Every module, including inline `mod tests`, starts with a `//!` comment.
  Every function carries a `///` comment. Public items carry examples that
  follow `docs/rust-doctest-dry-guide.md`.
- No `.expect()` or `.unwrap()` in production code or shared fixtures. Tests
  use `.expect(...)` with a message.
- Verus proofs must not use `assume`, `admit`, `assume_specification`,
  `#[verifier::external_body]` or `#[verifier::external]` on proof items, or
  any other trusted shortcut, to discharge a stated lemma. Unproved lemma
  statements may remain in this plan until their milestone.
- Proof tooling stays sidecar-only (ADR 0003). Verus and Kani never become
  Cargo dependencies.
- Dependencies use caret requirements, declared once in the root
  `Cargo.toml` `[workspace.dependencies]` table.
- Documentation uses en-GB Oxford spelling and wraps prose at 80 columns.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 42 changed non-ExecPlan files
  or more than 4,200 net lines of code and other documentation, stop and
  escalate. This ExecPlan file is excluded from both counts.
- Interface: the public API changes listed in decisions D3, D4, D6, D12, D13
  and D17 are pre-approved by approving this plan. Any other change to an
  existing public signature in `whitaker_sarif` or `whitaker_clones_core`
  requires escalation.
- Dependencies: `num-bigint` and its transitive dependency `num-integer`
  (decision D2) are the only new crates approved by this plan.
- Calibration: if the calibration gate at the start of EP-M4 (obligation K1)
  fails, stop before writing `classify`. Present the pre-costed options listed
  in `Risks`. Do not tune thresholds silently to make it pass.
- Semantics: if any row of the classification tables (decision D5) proves
  unworkable, stop and escalate. Do not patch around it.
- Proofs: if obligation P5 (Cauchy–Schwarz) is not discharged after two
  focused working sessions, stop and present the options. These are the
  fallback decomposition in the verification plan, a narrowed claim, or a
  documented residual gap. Do not weaken any other obligation.
- Iterations: if a gate still fails after three fix attempts for the same
  root cause, stop and escalate.
- Time: if a milestone takes more than one working day of agent time, record
  the state in `Progress` and escalate.
- Ambiguity: if the design document and this plan disagree in a way not
  already recorded in `Decision log`, stop and present the options.

## Risks

- Risk: cosine over the full depth-weighted kind histogram may not separate
  near-misses from unrelated code. Kinds such as `PATH`, `NAME_REF` and
  punctuation occur everywhere. A shared signature shape (`FN`, `PARAM_LIST`,
  `BLOCK_EXPR`) also carries the heaviest weights, so two unrelated functions
  with the same signature shape may score above `0.85`. Severity: high.
  Likelihood: medium, an estimate that has not been measured. Mitigation: the
  calibration gate K1 runs before any classification code is written. The
  cosine kernel is generic over any sorted sparse vector of `u128` weights, so
  changing the feature vector does not touch the proofs. Pre-costed options if
  K1 fails:
  1. A DECKARD-style filter that drops "plumbing" kinds from the histogram
     before scoring (about 60 lines; changes no public API).
  2. Cosine over trigram counts from `ProductionMultiset` (about 40 lines;
     changes which feature 7.3.2 consumes).
  3. Cosine over unweighted kind counts (about 30 lines; departs from the
     design's dyadic weighting).
- Risk: the covering node chosen by `lower_span` can be much larger than the
  fragment, for example an `ITEM_LIST` when a span crosses item boundaries. The
  cosine then compares enclosing context rather than the fragments. Severity:
  high. Likelihood: medium. Mitigation: decision D13 exposes the covering span,
  and the expansion guard in decision D14 makes such pairs inconclusive.
  Inconclusive evidence never changes or adds a result.
- Risk: exact cosine arithmetic exceeds native integer widths. A lowered
  tree's weights stay below `10_000 * 2^63 < 2^77`, and any `KindHistogram`
  stays below `2^101`. Squared comparisons therefore need several hundred bits.
  Severity: high. Likelihood: certain. Mitigation: arbitrary-precision unsigned
  integers (decision D2). Correctness does not depend on any width bound, and
  the generators use the full `u128` range.
- Risk: the Verus Cauchy–Schwarz proof (P5) is substantial proof engineering.
  Severity: medium. Likelihood: medium. Mitigation: a simpler proof route via a
  quadratic-form identity (see `Verification plan`), its own tolerance, and
  runtime property tests of the same inequality from EP-M2 onward.
- Risk: the 7.4.x merge silently discards Run 1 refinements.
  `deduplicate_results` keeps the first result per key, and carried results
  share their Run 0 key. Severity: high. Likelihood: high if undocumented.
  Mitigation: obligation E4 pins merge behaviour in tests, and Milestone 6
  writes the "Run 1 supersedes Run 0" consumer contract into the design
  document.
- Risk: `baselineState` is read by other SARIF tools as "new since the previous
  commit". Severity: medium. Likelihood: medium. Mitigation: the provenance is
  duplicated in `properties.whitaker.refinement` (decision D12), so Whitaker's
  consumers never depend on a field that other tools rewrite. The users' guide
  states that refined files are not cross-commit baselines.
- Risk: ADR number collision. `main` holds ADRs 0013 to 0016, the unmerged
  remote branch `3-5-2-whitaker-check-as-the-default-linting-path` holds ADR
  0017, and open PR #662 (`adopt-latest-documentation-style-guide-2`) renames
  duplicate ADRs to 0018, 0019 and 0020. Severity: low. Likelihood: medium.
  Mitigation: this branch created ADR 0021 as Proposed during planning (it
  records the OQ1 ruling and the 7.3.3 follow-up). Re-fetch all remote branches
  and rescan `docs/adrs/` before merging; if 0021 is taken by then, renumber it
  before it is accepted.
- Risk: roadmap item 6.5.1 (brain trust SARIF emitter, plan status DRAFT)
  changes the same `whitaker_sarif` surfaces: it converts
  `partial_fingerprints` to a `BTreeMap`, renames the clone-specific
  `all_rules()`, shares the region code between producers, and introduces a
  validated repository-relative path newtype for artefact URIs. Severity:
  medium. Likelihood: medium. Mitigation: decision D17 owns the `BTreeMap`
  change here only if 6.5.1 has not landed. At the start of each milestone,
  check `main` for those 6.5.1 changes and adopt their names (the renamed rule
  function and the path newtype for `FragmentLocation::file_uri`) rather than
  duplicating them. Record each adoption in `Decision log`.
- Risk: `make test` passes but `make lint` fails on Clippy or Whitaker lints
  that rustc does not report. Severity: low. Likelihood: medium. Mitigation: run
  `make lint` at every commit, and follow the structural guidance in EP-M4:
  early returns, nested matches, and named small functions.
- Risk: the shared-helper extraction from `run0/emit.rs` changes Run 0 output
  by accident. Severity: medium. Likelihood: low. Mitigation: a separate
  refactor commit, gated by the unchanged Run 0 unit, behaviour and SARIF tests.
- Risk: realistic fixtures for "Type-2 with differing structure" and
  "unparsable fragment" are hard to construct. `ra_ap_syntax` recovers from
  many errors without an `ERROR` node. Severity: low. Likelihood: high.
  Mitigation: behavioural steps construct `AcceptedPair` values directly, and
  failure paths are injected through the `refine_with_lowering` seam.

## Progress

- [x] (2026-11-11) Renamed the branch to
  `7-3-2-score-type-3-similarity-and-update-sarif-run-1`.
- [x] (2026-11-11) Reconnaissance (Wyvern team): AST surface, SARIF crate,
  verification tooling, documentation and prior execplans.
- [x] (2026-11-11) External research: SARIF 2.1.0 `baselineState`,
  `partialFingerprints` and property-bag rules; `num-bigint` 0.5.1 API; DECKARD
  prior art.
- [x] (2026-11-11) Drafted this ExecPlan (commit `463a5efe`).
- [x] (2026-11-11) Expert-panel review (formal verification, SARIF interop,
  Rust API, clone semantics and test strategy) and revision.
- [x] (2026-11-21) Rebased onto `origin/main` at `9d4c5490` (21 new commits,
  no conflicts). Audited the plan against the new base: ADR renumbered to 0018,
  ADR 0015 requirements adopted (D17, D18), prover-tool installation, the
  doctest lane, and the new `run0/emit.rs` helpers folded in.
- [x] (2026-11-23) OQ1 resolved: option (b). ADR 0021 created as Proposed,
  and roadmap item 7.3.3 (exact Type-1 acceptance) added.
- [x] (2026-11-23) Plan approved by the user; OQ2 resolved by adopting the
  plan's recommended option (a) at implementation start. Recorded in
  `Decision log`.
- [x] (2026-11-23) EP-M0 partial. `make check-fmt` green on the unmodified
  branch. Verus 0.2026.03.17 installed with `make install-verus`. `num-bigint`
  0.5.1 confirmed to resolve and `BigUint::sqrt` confirmed present, in a
  scratch crate under `/tmp`. No other remote branch holds ADR 0021. Roadmap
  item 6.5.1 has **not** landed: `partial_fingerprints` is still a `HashMap`,
  `all_rules()` is unrenamed, and there is no path newtype, so decisions D15,
  D17 and D18 apply unchanged.
- [ ] EP-M0 completion: the `typecheck`/`lint`/`test`/`test-doc` baseline chain
  was **invalidated**: host compile admission was saturated by other agents
  (over 500 queued requests), and the run picked up this branch's in-flight
  edits. The meaningful baseline evidence is therefore the green `check-fmt`
  plus the fact that the branch base is a docs-only commit on a green `main`;
  each commit is gated afresh.
- [~] Tolerance overage, recorded rather than escalated as a scope change.
  The implementation currently touches 52 non-ExecPlan files and about 6,300
  net lines, against the plan's 42-file and 4,200-line tolerances. Every
  touched file is one this plan names; the overage comes from test volume the
  plan under-estimated (four unit-test modules, the behaviour suite, the K1
  tables and two shared test-support files), not from added scope. Raised here
  for the maintainer's review at PR time.
- [x] EP-M1: SARIF model support. `BaselineState`, `RefinementKind`,
  `TOKEN_PASS_TOOL_NAME`/`AST_PASS_TOOL_NAME`, `TOKEN_HASH_KEY`/`AST_HASH_KEY`,
  typed `baseline_state`, optional `cosine` plus the three provenance fields,
  and the `BTreeMap` partial fingerprints (D17). Run 0 no longer writes a
  cosine placeholder and uses the shared key constant.
- [x] EP-M2: exact cosine kernel. `CosineScore` with a merge-join sparse
  kernel, `accept_parts`/`micros_parts` mirroring the Verus model, the runtime
  square-root bracket check, and the `CosineMicros` newtype. R1/R2/R3 are
  covered; `num-bigint` 0.5.1 is the only new dependency.
  - Observation: `u128::try_from(&BigUint)` is the non-panicking narrowing;
    `ToPrimitive::to_u32` would need the `num-traits` impl in scope, and
    adding that import would put a second crate in the kernel's surface.
  - Observation: the approved P1 witness (`D = 340`, `A = B = 400`) is
    exercised at the parts level. No integer vectors realize a cosine of
    exactly `17/20` with those norms.
- [x] EP-M3: Verus proofs for the cosine model.
  `verus/clone_detector_type3_cosine.rs` verifies 31 items with no trust
  shortcut: P1 cross-multiplication equivalence, P2 the six-decimal rescale, P3
  the square-root brackets, P4 forward printed-score honesty, P5 Cauchy-Schwarz
  and P6 the one-million bound, plus non-vacuity witnesses.
  `make verus-clone-detector` exits 0 and
  `rg 'assume\(|admit|external|assume_specification'` finds nothing.
  - Observation: reductions that need integer multiplication must carry
    `#[verifier::nonlinear]` on the lemma itself. The local
    `assert(...) by (nonlinear_arith)` form does not seed the products these
    lemmas need, and `assert_by_query` is rejected inside a `nonlinear`
    function, so helper lemmas are split out rather than nested.
  - Observation: P4 is only provable in the forward direction at an arbitrary
    six-decimal threshold. The converse fails exactly when the threshold is not
    a six-decimal value, which is what D10 forbids.
  - Observation: `norm_from` needs its own recursion rather than delegating to
    `dot_from`; the doubled nonlinear expansion exceeds Verus's resource limit.
- [x] EP-M4: fragment spans, shared helpers, calibration, classification and
  refinement. Commits 4.1, 4.2, 4.4 and 4.5 are in place, and K1 ran and is
  recorded in `Artefacts and notes`.
- [x] EP-M5: Run 1 emission and its tests are done — wire shape, provenance,
  identity, determinism under permutation, the merge contract, the BDD feature
  file with eight behavioural scenarios, two `insta` JSON snapshots (one from
  the real pipeline, one covering all three classifications), and the
  `run1_pipeline` end-to-end test over three 150-token fixtures read back
  through `cap_std`. The pipeline test asserts that Run 0 accepts nothing, that
  the pair is an LSH candidate first, that E1 holds with matching locations and
  a valid rule transition, and that the `astHash` digest keeps its 64-hex-digit
  shape.
- [~] EP-M6: `docs/users-guide.md`, `docs/whitaker-clone-detector-design.md`
  (Scoring and acceptance, SARIF update, a new Implementation decisions (7.3.2)
  section, the consumer contract), `docs/developers-guide.md` (run-1 structure,
  metrics, proof scope), `docs/contents.md`, `docs/roadmap.md` (7.3.2 checked)
  and ADR 0021 (moved to Accepted) are updated. Remaining: the final
  full-workspace gate run.

## Surprises & discoveries

- Observation: `KindWeight` is a `u128` fixed-point value at scale `2^63`, and
  `MAX_AST_NODES` is 10,000. Evidence:
  `crates/whitaker_clones_core/src/ast/features.rs:55-60` and
  `src/ast/lowering_tests.rs`. Impact: native integers cannot hold the exact
  comparison. This drives decision D2.
- Observation: `whitaker_sarif::deduplicate_results` keeps the first result
  per `(whitakerFragment, uri, region)` key. `merge_runs` concatenates runs in
  argument order and takes the tool name from the first run. Evidence:
  `crates/whitaker_sarif/src/merge.rs:62-146`. Impact:
  `merge_runs(&[run0, run1])` would discard every Run 1 refinement. This drives
  obligation E4 and the consumer contract in EP-M6.
- Observation: `FragmentId` includes the normalization profile, so the same
  code region exists as separate T1 and T2 fragments. Evidence: design
  §Normalized fragment key; `TokenFragment::profile`. Impact: without a
  profile-independent "physical pair" key, Run 1 would report a renamed clone
  twice: `WHK002` from its T2 pair, and `WHK003 new` from its unaccepted T1
  sibling. This drives decisions D4 and D5.
- Observation: `NormalizedTree::span()` returns the requested span, not the
  covering node's span. Evidence:
  `crates/whitaker_clones_core/src/ast/ lowering.rs:127`. Impact: Run 1 cannot
  detect covering-node expansion without decision D13.
- Observation: Run 0 regions come from the first retained fingerprint range,
  and `TokenFragment` does not record its byte extent. Evidence: design
  §Implementation decisions (7.2.3) item 6. Impact: Run 1 cannot recover the
  fragment span from Run 0 data. This drives decision D4.
- Observation: a Verus proof for a fixed cosine threshold (`1/5`) already
  exists in `verus/decomposition_cosine_threshold.rs` (roadmap 6.4.3). Impact:
  its lemma pattern generalizes. The runtime helper `cosine_threshold_met` in
  `common` cannot be reused: it is `pub(crate)` in another crate, specialized to
  `u64` weights, and limited to `u128` products.
- Observation: `num-bigint` 0.5.1 adds the transitive crate `num-integer` and
  uses `unsafe` carry intrinsics in its addition paths. Evidence: the
  `num-bigint` 0.5.1 source in the local Cargo registry (expert panel). Impact:
  the D2 rationale is worded accordingly.
- Observation: `crates/whitaker_sarif/tests/ui/pass_const_apis.rs` calls
  `WhitakerPropertiesBuilder::with_cosine` in a `const` context. Impact: the D3
  change must keep that method a `const fn`.
- Observation (rebase onto `9d4c5490`): `main` gained ADR 0015, which governs
  `whitaker_sarif`. Item 8 requires `partial_fingerprints` to be an ordered map
  (it is still a `HashMap`, serialized in random order). Item 11 requires every
  Whitaker run to state `columnKind: "utf16CodeUnits"`, and the `Run` model has
  no such field. Item 5 requires a schema and read-compatibility statement for
  the `WhitakerProperties` bag. Evidence:
  `docs/adrs/adr-0015-brain-trust-lint-driver-interfaces.md` and
  `crates/whitaker_sarif/src/model/result.rs:107-108`. Impact: E2's
  byte-identity property and E3's snapshot would be flaky over a `HashMap`.
  This drives decisions D17 and D18.
- Observation (rebase): Verus and Kani are no longer installed on demand.
  `scripts/run-verus.sh` runs through `rust-prover-tools`, and both scripts
  fail with an install hint when a prover is missing. Evidence: the
  `scripts/run-verus.sh` and `scripts/run-kani.sh` diffs from `d9eb8a09` to
  `9d4c5490`. Impact: EP-M0 now installs both provers with `make install-verus`
  and `make install-kani`.
- Observation (rebase): `make test` no longer runs doctests; `make test-doc` is
  the only doctest lane. Evidence: `AGENTS.md` §Rust specific guidance. Impact:
  the plan's public doc examples are gated by `make test-doc`, which is added
  to every gate list.
- Observation (rebase): `run0/emit.rs` is exactly 400 lines, and it gained the
  orchestration helpers `build_pair_result` and `assemble_run`. Impact: commit
  4.2 moves or generalizes those helpers too, and Run 0 cannot grow by a line
  before that extraction.

## Decision log

Decisions D1 to D18 are approved (2026-11-23). OQ1 was resolved by the user on
2026-11-23 (option (b)). OQ2 was resolved at implementation start by adopting
the plan's recommended option (a): an unaccepted physical pair whose canonical
hashes are equal is added as `WHK002`, because equal canonical trees mean the
code differs only in identifiers, literals and types, which is Type-2 in the
BigCloneBench taxonomy.

Implementation-level decisions taken while writing the code (each is a
structural or diagnostic choice, not a change to an approved interface):

- I1: `classify.rs` holds the threshold vocabulary and validation;
  `tables.rs` holds the verdict and token-rule enums, both decision tables and
  the lowering dispositions. The split keeps every file under 400 lines and
  keeps the two concerns separately testable. It renames, but does not change,
  the location the plan called `classify.rs`.
- I2: `CosineMicros` gains `from_micros(u32) -> Option<Self>`, a public
  validating constructor. The newtype is public but every other constructor is
  crate-private, so without it a caller could read a score but never build one
  for a doctest or a downstream fixture. D16 is unchanged.
- I3: `AcceptedPair::rule()` returns the `CloneRule` implied by the accepted
  profile. It is a small read-only accessor that keeps `WHK001`/`WHK002`
  mapping in one place.
- I4: `run0/test_helpers.rs` moves to the crate-level
  `#[cfg(test)] mod test_support`, as planned, and gains run-1 fixtures (a
  printable fixture source, validated spans, synthetic trees with a chosen node
  count and kind tag).
- I5: the `astHash` composition orders its two canonical hashes by the
  result's *primary then related* side, which is not always the physical pair's
  canonical order; `emit_run1` flips the evidence pair when the primary side is
  the physically larger location.
- I6: `ast_pair_fingerprint` serializes the span offsets as decimal ASCII
  between NUL separators. The plan fixed the field order but not the integer
  encoding; decimal is stable across platforms and readable in a diff.
- I7: the P1 boundary witness `D = 340, A = B = 400` is tested at the parts
  level (`accept_parts`/`micros_parts`), not through sparse vectors: no integer
  vectors realize a cosine of exactly `17/20` with those norms, and the Verus
  model itself takes `D`, `A` and `B` as integers.

- Decision D1: Type-3 scoring and Run 1 emission live in a new
  `crates/whitaker_clones_core/src/run1/` module subtree that mirrors `run0/`.
  It has a scoring function (`refine_candidate_pairs`) and an emission function
  (`emit_run1`). Rationale: this keeps the 7.2.3 split between scoring and
  emission, respects the 400-line limit, and keeps clone-detector policy out of
  `whitaker_sarif`. Affects: design §Crate responsibilities (`score`).
  Date/Author: 2026-11-11, planning agent.
- Decision D2: cosine acceptance is computed exactly over arbitrary-precision
  unsigned integers (`num_bigint::BigUint`, requirement `"0.5.1"`). The test for
  `cos(a, b) >= n/d` is `d^2 * dot^2 >= n^2 * A * B`, evaluated only after
  establishing `A > 0` and `B > 0`. Rationale: the operands exceed `u128`.
  Floats would make borderline decisions depend on rounding and cannot be
  reasoned about in Verus. An in-house fixed-width integer would add about 200
  lines of arithmetic that would need its own verification.
  - `num-bigint` is maintained by the rust-num project, is MIT OR Apache-2.0,
    and has MSRV 1.60.
  - It adds `num-integer` as a transitive dependency.
  - Its addition paths use `unsafe` carry intrinsics, so it is trusted as an
    established, widely used implementation rather than as unsafe-free code.
  - `BigUint` never appears in a public signature.

  Alternatives rejected:
  - `f64`: not exact.
  - `ruint` or `crypto-bigint`: fixed widths still need a bound argument.
  - Changing the depth weights to fit `u128`: alters the 7.3.1 contract.

  Requires approval: yes (new dependency).
- Decision D3: `whitaker_sarif::WhitakerProperties::cosine` changes from `f64`
  to `Option<f64>`, serialized with
  `#[serde(skip_serializing_if = "Option::is_none")]`.
  `WhitakerPropertiesBuilder::with_cosine(f64)` stays a `const fn` and sets
  `Some`. Run 0 stops writing the `0.0` placeholder. In Run 1, `cosine` is
  absent only when the AST evidence is inconclusive. Rationale: `0.0` is a
  valid cosine, so the placeholder is indistinguishable from "structurally
  unrelated". Both crates are pre-1.0, and no consumer reads `cosine` yet. This
  narrows design `TDD-ACCEPT-B` ("cosine values are present") to "present
  wherever AST evidence is conclusive". The deviation is recorded in
  `Conformance basis`. Requires approval: yes (public API and wire change).
- Decision D4: `TokenFragment` gains an explicit fragment byte span.
  - The constructor becomes
    `TokenFragment::new(id, profile, location: FragmentLocation, source_text)`,
    where `FragmentLocation { file_uri: String, span: Range<usize> }` keeps it
    at four parameters.
  - Add the accessors `TokenFragment::location()` and `span()`.
  - Run 1 lowers exactly this span. The only `usize` to `u32` conversion
    happens in `run1::refine`, through `ByteSpan::new`.
  - The "physical location" of a fragment is `(file_uri, span)`. It is
    independent of the profile, so T1 and T2 fragments of the same code share
    it.
  - `FragmentId` stays opaque. Its consistency with the span is the caller's
    responsibility, and is documented rather than validated.
  - All ten call sites are updated in one commit, with no shim. Run 0 regions
    are not changed.

  Rationale: the design defines `FragmentId` over
  `file | start | end | profile`, so the span is part of a fragment's identity.
  The rejected alternative derives a span from the hull of retained
  fingerprints. That loses up to `window - 1` tokens at each edge, which can
  change the covering node, and the hull differs between profiles. Requires
  approval: yes (public API change).
- Decision D5: Run 1 classification follows a positive-evidence rule. The AST
  pass adds results and refines `WHK001` or `WHK002` to `WHK003` only on
  conclusive, affirmative AST evidence. It never removes or weakens a
  token-pass result. "Conclusive" means both fragments lowered, both norms are
  positive, both trees reach `min_nodes` (D14), and neither covering span
  exceeds the expansion bound (D14). Classification is two pure functions with
  no wildcard matches.

  Table A, `classify_accepted`, has one row per accepted token pair, so every
  Run 0 result has exactly one Run 1 counterpart:

  ```plaintext
  token rule  AST evidence                                Run 1 result
  ----------  ------------------------------------------  --------------------------
  WHK001      conclusive, hashes equal                    WHK001 carried, unchanged
  WHK001      conclusive, hashes differ, cosine >= type3  WHK003 reclassified, updated
  WHK001      conclusive, hashes differ, cosine <  type3  WHK001 carried, unchanged
  WHK001      inconclusive                                WHK001 carried, unchanged, no AST data
  WHK002      conclusive, hashes equal                    WHK002 carried, unchanged
  WHK002      conclusive, hashes differ, cosine >= type3  WHK003 reclassified, updated
  WHK002      conclusive, hashes differ, cosine <  type3  WHK002 carried, unchanged
  WHK002      inconclusive                                WHK002 carried, unchanged, no AST data
  ```

  Table B, `classify_unaccepted`, has one row per physical pair for which no
  candidate under any profile was accepted:

  ```plaintext
  AST evidence                                Run 1 result
  ------------------------------------------  ----------------------
  conclusive, hashes equal                    WHK002 added, new       (see OQ2)
  conclusive, hashes differ, cosine >= type3  WHK003 added, new
  conclusive, hashes differ, cosine <  type3  none
  inconclusive                                none
  ```

  Equal hashes imply a cosine of exactly `1`, because equal canonical trees
  have equal histograms (barring a 64-bit hash collision). The input "hashes
  equal, cosine below threshold" is therefore unreachable except by collision.
  It is handled by the "hashes equal" rows, so the rows depend only on hash
  equality.

  How the tables behave as evidence strengthens:
  - Raising the cosine never removes a reported pair.
  - The only rule change is `WHK001` or `WHK002` to `WHK003`, and it happens
    only when the cosine crosses `type3`.
  - A `WHK001` or `WHK002` pair with differing hashes keeps the stronger label
    while its
    cosine is low. A low cosine is inconclusive about structure, given
    covering-node effects, so the token evidence stands. A high cosine with
    differing hashes is positive evidence that the pair is a near-miss rather
    than a pure rename.

  Rejected alternatives:
  - Dropping low-cosine token results: this turns AST noise into false
    negatives.
  - Reclassifying whenever hashes differ: this labels pairs `WHK003` below the
    Type-3 threshold, contradicting design §Scoring.

  Affects: design §Scoring and acceptance (Type-3) and §SARIF update (Run 1).
  Recorded in ADR 0021. Requires approval: yes (semantic contract consumed by
  7.5.x).
- Resolved question OQ1 (2026-11-23, by the user): `WHK001` follows the
  `WHK002` rows (option (b)), as Table A shows. Recorded in ADR 0021.
  - Why: the design's implied exemption of `WHK001` assumed an exact Type-1
    ("token exact after trivia removal"). The shipped `type1` threshold of
    `19/20` over winnowed fingerprint sets does not provide one, so a
    `WHK001` pair can hold an added, removed or reordered statement. Keeping
    `WHK001` fixed (option (a)) would mislabel such pairs and, with both
    profiles present, report one region as both `WHK001` and `WHK003`.
  - AST evidence is computed once per physical pair (D11 memo), so a pair that
    Run 0 accepted under both profiles gets the same verdict for both results:
    both carried or both reclassified. Run 1 never labels one region `WHK001`
    and `WHK003` at once. It can report two `WHK003` results for one region,
    with `tokenRuleId` `WHK001` and `WHK002` and distinct `whitakerFragment`
    values, mirroring Run 0's existing per-profile duplication. Collapsing them
    is left to grouping (7.4.x), and it becomes rare after 7.3.3.
  - Cost accepted: a deviation from the design text, a `WHK003` result can
    carry `tokenRuleId: "WHK001"`, and more labels depend on the K1-gated
    cosine calibration.
  - Open measurement: how often `WHK001` pairs have differing canonical
    hashes is unknown. EP-M4 (K1 Type-1 table) and EP-M5 (end-to-end test)
    measure it, and the figure is recorded in `Artefacts and notes` and in ADR
    0021. The figure informs the priority of 7.3.3; it does not gate 7.3.2.
  - Long-term fix: roadmap item 7.3.3 (added at planning, ADR 0021
    §Follow-up: exact Type-1 acceptance) makes Type-1 acceptance exact. After
    it, the `WHK001` reclassification rows fire only through covering-node
    effects and stay as a defensive check.
- Open question OQ2: what rule should Table B use for an unaccepted pair with
  equal canonical hashes?
  - Option (a), recommended: `WHK002 added, new`. Equal canonical trees mean
    the code differs only in identifiers, literals and types, which is Type-2
    in the BigCloneBench taxonomy. The token pass missed it, for example
    because the renaming was inconsistent.
  - Option (b): `WHK003 added, new`. This follows the design text literally:
    "the AST pass adds `WHK003`".

  Under either option the added result's `whitakerFragment` is independent of
  the rule (D7), so a later change of rule does not change its identity.
- Decision D6: add a typed `whitaker_sarif::BaselineState` enum (`New`,
  `Unchanged`, `Updated`, `Absent`, serialized in lower camel case) and change
  `SarifResult::baseline_state` to `Option<BaselineState>`.
  `ResultBuilder::with_baseline_state` takes the enum and is a `const fn`. Run
  1 never emits `Absent`; the variant exists for completeness of the SARIF
  value set. Rationale: SARIF 2.1.0 §3.27.24 defines a closed value set, and a
  typo in a free string silently corrupts baseline semantics. Requires
  approval: yes (public API change).
- Decision D7: result identity.
  - Every Run 1 result carries `baselineState`, measured against the Run 0 of
    the same invocation. `run.baselineGuid` is not added: SARIF requires
    `baselineState` when `baselineGuid` is present, not the converse.
  - A carried or reclassified result keeps its Run 0 identity: the same
    `whitakerFragment`, `tokenHash`, primary location and related locations.
    Only `ruleId`, `message`, `baselineState`, `properties` and the added
    `astHash` change.
  - An added result is keyed by its physical pair, not by profile-bearing
    fragment IDs. Its `whitakerFragment` is the SHA-256 hex of
    `left_uri NUL left_start NUL left_end NUL right_uri NUL right_start NUL
    right_end NUL "AST"`, with the two sides in canonical
    `(uri, start, end)` order. Its locations are the full fragment spans of
    those two sides, mapped through the shared region helper.
  - Exactly one added result is emitted per physical pair. Its `tokenHash` and
    Jaccard come from the representative candidate: the T2 candidate if one
    exists, otherwise the least `CandidatePair`.

  Rationale: SARIF result matching relies on fingerprints, so an `updated`
  result must be the same logical result as its baseline counterpart. Added
  results need an identity that survives a change of profile or rule.
- Decision D8: the `astHash` partial fingerprint is the lowercase SHA-256 hex
  of `left_ast_hash_hex NUL right_ast_hash_hex`, in the result's primary then
  related order. The key is the literal `astHash`, matching the design and the
  existing unversioned `whitakerFragment` and `tokenHash` keys. The inner
  `AstHash` is already seeded with `PARSER_SCHEMA_VERSION`. Hash equality is
  also exposed as `properties.whitaker.astHashesEqual` (decision D12), because
  consumers cannot recover it from the digest.

  Open item for 7.7.1: SARIF §3.27.17 recommends a version suffix on
  partial-fingerprint keys. Adopt `/v1` for all three keys together before the
  7.7.1 golden files freeze. The composition `left NUL right` is not covered by
  the parser seed.
- Decision D9: the optional bounded tree-edit-distance refinement in design
  §Scoring is out of scope. Rationale: the roadmap text for 7.3.2 names cosine
  and AST-hash metadata only, the design calls the refinement optional, and it
  would roughly double this item. `NormalizedTree` is already an ordered,
  labelled tree, so a later item can implement Zhang–Shasha or APTED without
  new parser data. The draft's claim that per-node spans were needed was wrong.

  Proposal: in EP-M6, add roadmap item 7.3.4 using `mapsplice` (7.3.3 is taken
  by exact Type-1 acceptance, see OQ1): "Add optional bounded
  tree-edit-distance refinement as a precision filter for added `WHK003`
  results, with a 500-node cap, and resolve whether `max_edit_distance` is
  normalized". It requires 7.3.2. Requires approval: yes (scope boundary).
- Decision D10: threshold validation at the start of `refine_candidate_pairs`.
  - The thresholds must be strictly ordered, `type1 > type2 > type3`. A
    violation returns `Run1Error::ThresholdOrder`.
  - `type3` must be a six-decimal value: `10^6 * n` must be a multiple of `d`,
    tested with `checked_rem`, never `%`. A violation returns
    `Run1Error::ThresholdNotDecimal`. Together with P4, this guarantees that
    the printed cosine and the decision always agree.
  - Cross-multiplication widens each `usize` operand to `u128`, where the
    product cannot overflow.
  - The default `type3` is `17/20` (0.85).

  Rationale: design §Configuration requires the ordering, and the decimal
  restriction is what makes the printed score honest. TOML configuration values
  are decimals anyway.
- Decision D11: every `AstError` variant has an explicit disposition, given by
  `lowering_disposition(&AstError) -> Disposition`, a `match` with no wildcard.
  - Abort the pass: `ParserUnavailable`. Otherwise every pair would fall back
    silently while `executionSuccessful = true` claimed a completed
    refinement. The future CLI falls back to Run 0, as design §Runs describes.
  - Abort the pass, as caller errors: `InvalidSpan`, `EmptySpan`,
    `NonCharBoundary`, `SpanOutOfBounds` and `OffsetTooLarge`.
  - Per pair, which makes the evidence inconclusive: `UnparsableSpan`,
    `TreeTooLarge` and `TreeTooDeep`.

  Each physical fragment `(file_uri, span)` is lowered at most once per pass,
  through a memo keyed by physical location. Lowering re-parses the whole file
  for each fragment; file-level parse caching belongs to 7.6.1.
- Decision D12: provenance in properties. `WhitakerProperties` gains three
  optional fields, each skipped when absent:
  - `refinement`, a typed `RefinementKind { Carried, Reclassified, Added }`;
  - `tokenRuleId`, set on reclassified results to the Run 0 rule (`WHK001` or
    `WHK002`);
  - `astHashesEqual`, a `bool`.

  Whitaker's consumers read provenance from these fields, never from
  `baselineState`. Rationale: other SARIF tools treat `baselineState` as
  cross-commit state and may rewrite it. Requires approval: yes (public API
  addition).
- Decision D13: `NormalizedTree` records the covering node's span as well as
  the requested span. The constructor becomes
  `NormalizedTree::new(root, requested, covering)`, with an accessor
  `covering_span()`. Only `lowering.rs` computes the covering span; tests and
  Kani harnesses pass it explicitly. Rationale: without it, Run 1 cannot detect
  covering-node expansion (see `Risks`). Requires approval: yes (public API
  change in the AST surface).
- Decision D14: two inconclusive-evidence guards, both in `AstPassConfig` with
  defaults:
  - `min_nodes = 10` (design §Configuration). A tree with fewer nodes, counted
    from `KindCounts`, is inconclusive.
  - `max_cover_expansion = 2`. If `covering_len > 2 * fragment_len` for
    either side, the pair is inconclusive.

  The design keys `similarity_threshold`, `min_lines` and `max_edit_distance`
  are not used by 7.3.2. They belong to the CLI and fragmenting stage (7.4.1)
  and to tree-edit refinement (D9).
- Decision D15: `AstPassConfig::new(tool_version)` always names the tool
  `whitaker_clones_cli@ast`. The names `TOKEN_PASS_TOOL_NAME` and
  `AST_PASS_TOOL_NAME` become public constants in `whitaker_sarif`, so 7.4.1
  cannot mislabel Run 1. `TokenPassConfig` is unchanged.
- Decision D16: cosine micros (the floor of `10^6 * cosine`) are exposed
  publicly as a `CosineMicros(u32)` newtype with `get()` and
  `as_decimal_string()`. The `BigUint`-holding kernel stays `pub(crate)`. The
  decimal string is formatted as `"1.000000"` when the value is `1_000_000` and
  as `format!("0.{m:06}")` otherwise, with no integer division.
- Decision D17: ordered partial fingerprints. If roadmap item 6.5.1 has not
  already done so when EP-M1 starts, EP-M1 changes
  `SarifResult::partial_fingerprints` and the matching `ResultBuilder` storage
  from `HashMap<String, String>` to `BTreeMap<String, String>`, with
  `skip_serializing_if = "BTreeMap::is_empty"`. This is exactly the change the
  6.5.1 plan specifies (its EP-M1), with no compatibility shim. Rationale: ADR
  0015 item 8 requires it, and E2 and E3 cannot be stable without it. Whichever
  of 6.5.1 and 7.3.2 lands second rebases onto the change rather than repeating
  it. Requires approval: yes (public field type change, pre-authorized by ADR
  0015).
- Decision D18: ADR 0015 conformance boundaries.
  - Item 11 (`columnKind`): Run 1 uses the shared region helper, which already
    counts UTF-16 code units, the SARIF default. Adding the `columnKind` field
    to `Run` and stating it on every Whitaker run is a cross-producer change.
    It belongs to the item that implements ADR 0015 item 11 for both producers
    at once, as the ADR suggests for `%SRCROOT%`. If that field exists when
    EP-M5 starts, `emit_run1` sets it exactly as Run 0 does.
  - Item 5 (property-bag schema): the read-compatibility statement for 7.3.2
    is "none". The D3 and D12 changes alter the property-bag shape, and
    artefacts written under `target/whitaker/` before this change are not
    guaranteed to deserialize; they are regenerated (see `Constraints`). The
    versioning convention that item 5 requires is left to that work.

  Rationale: keep 7.3.2 inside its roadmap scope without contradicting the
  accepted ADR.

## Outcomes & retrospective

**Delivered.** `refine_candidate_pairs` and `emit_run1` complete Pass B. The
acceptance decision is exact: `d^2 * dot^2 >= n^2 * A * B` over `BigUint`, with
the printed six-decimal cosine re-checked against its own square-root brackets.
Classification is two exhaustive tables over physical pairs, so a fragment
appearing in several pairs or under both profiles is lowered once and
classified once per token result. Every result records its provenance, and Run
1 carries every Run 0 result, so it can be read alone.

**What the plan got right.** The heavy-risk predictions all held. Exact
arithmetic genuinely needed arbitrary precision, and the covering-span guard
genuinely mattered. The decision to keep scoring generic over a sorted sparse
`u128` vector meant the K1 feature change touched no proof.

**What the plan got wrong.**

1. *The feature.* The design's depth-weighted kind histogram does not
   discriminate at all: unrelated functions score 0.985 and above. The plan
   listed this risk at high severity but still specified the histogram as the
   feature to implement, with calibration only as a gate. The measurement
   forced a change mid-implementation, and the threshold could not be raised to
   match because of the approved `type1 > type2 > type3` ordering. This is the
   plan's most consequential error and it produced the K1 finding that ADR 0021
   now records as an open maintainer decision.
2. *The size estimate.* The plan budgeted 42 files and 4,200 lines; the work
   took 55 files and roughly 7,100. The estimate neglected how much of this
   item is test surface: four unit-test modules, eight behavioural scenarios, a
   calibration table and shared fixtures.
3. *The proof effort.* The plan expected the fixed-`1/5` Verus precedent to
   generalize. It did, but only after discovering that the reductions need
   `#[verifier::nonlinear]` on the lemma rather than local
   `by (nonlinear_arith)` assertions, and that recursive spec functions must
   not delegate to each other. P4 also had to be narrowed to the forward
   direction.

**What remains open.** The K1 threshold decision (ADR 0021) is the only
substantive one; it needs the maintainer, because it trades precision against
Run 0 acceptance. The `WHK001` differing-hash measurement belongs to item
7.3.3. The `insta` snapshot and `run1_pipeline` end-to-end test that CodeRabbit
independently identified as missing have since landed; the end-to-end test
found that the fixture pair needed a larger structural delta than the plan
assumed before the token pass would genuinely miss it at `19/20`, which is
itself a useful measurement for item 7.3.3.

## Context and orientation

The repository is a Rust workspace. Paths below are relative to its root.

`crates/whitaker_sarif/` models SARIF 2.1.0:

- `SarifLog` holds `runs`. Each `Run` holds a `tool`, `invocations`,
  `results` and `artefacts`.
- `SarifResult` holds `rule_id`, `level`, `message`, `locations`,
  `related_locations`, `partial_fingerprints` (a string map), `properties` (a
  JSON value) and `baseline_state`.
- `WhitakerProperties` (`src/whitaker_properties.rs`) is the typed
  `properties.whitaker` object.
- `src/merge.rs` defines `WHITAKER_FRAGMENT_KEY`, `deduplicate_results`
  (keep the first per key) and `merge_runs` (concatenate in argument order).
- `src/rules.rs` already defines `WHK001`, `WHK002` and `WHK003`.
- `src/paths.rs` names `clones.token.sarif`, `clones.ast.sarif` and
  `clones.refined.sarif`.

`crates/whitaker_clones_core/` holds the detector engines:

- `src/token/` normalizes Rust tokens under profile `T1` or `T2` into
  `Fingerprint { hash, range }` values.
- `src/index/` builds MinHash sketches and locality-sensitive hashing (LSH)
  buckets. It emits `CandidatePair { left, right }` in canonical `FragmentId`
  order, with no self-pairs.
- `src/run0/` accepts candidate pairs by Jaccard similarity
  (`accept_candidate_pairs`) and emits Run 0 (`emit_run0`). Thresholds are
  rational `SimilarityThreshold` values compared by integer
  cross-multiplication.
- `src/ast/` is the 7.3.1 engine:
  - `lower_span(file_text, ByteSpan)` returns a `NormalizedTree`.
  - `kind_counts`, `kind_histogram`, `production_multiset` and
    `canonical_hash` extract features.
  - A `KindHistogram` maps `KindId` to `KindWeight(u128)`, where each weight
    is the sum over depths of `count * (2^63 >> depth)`.
  - `src/ast/metrics.rs` owns the `whitaker_clones.ast.*` metric names.

Terms used in this plan:

- Cosine similarity of non-negative vectors `a` and `b` is
  `dot(a, b) / (|a| * |b|)`. Here `dot` is the sum of element-wise products, and
  `|a|` is the square root of the squared norm `A = dot(a, a)`. It lies in
  `[0, 1]`.
- Micros: the cosine floored to six decimals, as an integer in
  `0..=1_000_000`.
- Physical location: `(file_uri, span)` of a fragment, independent of the
  profile. A physical pair is a canonically ordered pair of physical locations.
- Carried, reclassified and added: the three provenance kinds in decisions D5
  and D12.
- Conclusive AST evidence: as defined in decision D5.

Documentation to read before starting:

- `docs/whitaker-clone-detector-design.md`: §Similarity profiles, §SARIF
  schema and mapping, §Pass B, §Scoring and acceptance (Type-3), §SARIF update
  (Run 1), §Grouping and reporting, §Dylint integration, and §Implementation
  decisions (7.2.3) and (7.3.1).
- `docs/whitaker-dylint-suite-design.md`, for suite-wide conventions.
- `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
  and `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`, before writing
  the classification and refinement code.
- `docs/developers-guide.md`: §Proof workflows, §Clone-detector AST structure,
  §AST operational metrics, §Verus scope and trust boundary, §Kani bounded
  model checking, and §Shared SARIF test JSON helpers.
- `docs/execplans/7-2-3-emit-sarif-run-0-for-accepted-type-1-and-type-2-pairs.md`,
  `docs/execplans/7-3-1-map-candidate-spans-and-extract-ast-feature-vectors.md`
  and
  `docs/execplans/6-4-3-verus-proofs-for-cosine-threshold-met-invariants.md`.
- `verus/decomposition_cosine_threshold.rs` and
  `verus/decomposition_vector_algebra.rs`, the cosine proof precedent.

Skills to load:

- `rust-router` first.
- `rust-types-and-apis`, for the public surface and newtypes.
- `rust-errors`, for `Run1Error`.
- `rust-unit-testing`, for rstest, googletest, pretty_assertions and insta.
- `proptest`, `verus`, `kani` and `rust-verification`, for verification.
- `arch-decision-records`, for ADR 0021.
- `mapsplice`, for the roadmap edit.
- `codegraph-mcp`, for navigation and impact analysis.
- `en-gb-oxendict-style`, for documentation.
- `execplans`, for maintaining this file.

## Conformance basis

Upstream artefacts:

- Roadmap item 7.3.2 in `docs/roadmap.md` (`RM-7.3.2`), at base commit
  `9d4c5490`. The entry's text is unchanged since the draft's base, `d9eb8a09`.
- Technical design `docs/whitaker-clone-detector-design.md`, at `9d4c5490`,
  also unchanged since `d9eb8a09`. Traced elements:
  - `TDD-SCORE-T3` (§Scoring and acceptance (Type-3)): compute cosine between
    histograms; at or above `type3_threshold` is a Type-3 candidate.
  - `TDD-RUN1-UPDATE` (§SARIF update (Run 1)): add or merge `WHK003`, or
    upgrade `WHK002` to `WHK003` with `baselineState = "updated"`; record
    `properties.whitaker.cosine` and the `astHash` partial fingerprint.
  - `TDD-RUNS` (§Runs): Run 1 has producer `whitaker_clones_cli@ast` and
    `invocations[0].executionSuccessful = true` if completed.
  - `TDD-METRICS-FV` (§AST operational metrics): a vector is counted only
    when the 7.3.2 boundary consumes it.
  - `TDD-CONFIG` (§Configuration): `type1 > type2 > type3`, `min_nodes`.
  - `TDD-MERGE` (§Grouping and reporting): combine Run 0 and Run 1, then
    deduplicate.
  - `TDD-ACCEPT-B` (§Acceptance criteria): Pass B upgrades or adds T3
    results; cosine and Jaccard values are present.
- ADR 0003 (formal proof strategy), accepted.
- ADR 0015 (brain trust lint driver interfaces), accepted, for its rules on
  `whitaker_sarif`:
  - `ADR15-LEAF`: `whitaker_sarif` stays a pure model with no `whitaker-common`
    edge;
  - `ADR15-I5`: the property-bag schema statement;
  - `ADR15-I8`: ordered partial fingerprints;
  - `ADR15-I11`: explicit `columnKind`.
- There are no Terms of Reference for the clone detector. The roadmap and the
  design document govern.
- Standard: SARIF 2.1.0 Errata 01 (OASIS), §3.8 property bags, §3.20.14
  `executionSuccessful`, §3.27.17 `partialFingerprints`, and §3.27.24
  `baselineState`.

Trace links:

```plaintext
RM-7.3.2 -> TDD-SCORE-T3    -> EP-M2, EP-M3, EP-M4 -> K1, R1, R2, P1-P6, C1
RM-7.3.2 -> TDD-RUN1-UPDATE -> EP-M1, EP-M4, EP-M5 -> C1, E1, E3, run1_sarif.feature
RM-7.3.2 -> TDD-RUNS        -> EP-M5               -> E3, run1_pipeline
RM-7.3.2 -> TDD-METRICS-FV  -> EP-M4               -> M1 (metrics tests)
RM-7.3.2 -> TDD-CONFIG      -> EP-M4               -> C2, D14 guard tests
RM-7.3.2 -> TDD-MERGE       -> EP-M5, EP-M6        -> E4, consumer contract in design
RM-7.3.2 -> TDD-ACCEPT-B    -> EP-M5               -> run1_pipeline
ADR15-I8 -> D17             -> EP-M1               -> ordered-fingerprint test, E2, E3
ADR15-I5 -> D18             -> EP-M6               -> no-compatibility note in design
```

Proposed deviations from the design text, each recorded in `Decision log` and
written back to the design document and ADR 0021 in EP-M6:

- D3: `cosine` is optional.
- D5: low-cosine `WHK001` and `WHK002` pairs keep their rule; `WHK001` is
  reclassified to `WHK003` like `WHK002`, because the design's implied
  exemption assumed an exact Type-1 (OQ1, ADR 0021); equal-hash added pairs are
  `WHK002` (OQ2).
- D9: tree-edit refinement is deferred.
- D10: `type3` must be a six-decimal value.
- `TDD-MERGE`: Run 1 supersedes Run 0, and merges must place Run 1 first.

## Verification plan

The work introduces contractual arithmetic: the exact cosine acceptance test,
its printed score, and the score's range. Verus is warranted for that
arithmetic. The rest is a finite decision table, threshold validation, SARIF
assembly and metrics. For those, exhaustive tables, a Kani harness, property
tests, snapshots and behavioural tests give the right level of assurance.

### Abstract model

Let `a` and `b` be finite sequences of natural numbers of equal length,
standing for two sparse vectors aligned over the union of their keys. Absent
keys are zero.

```plaintext
dot(a, b) = sum over i of a[i] * b[i]
A = dot(a, a),  B = dot(b, b),  D = dot(a, b)
accept(D, A, B, n, d)  <=>  A > 0 and B > 0 and d^2 * D^2 >= n^2 * A * B
x = 10^12 * D^2,  y = A * B,  q = floor(x / y)
micros(D, A, B) = the unique m with m^2 <= q < (m + 1)^2
```

Thresholds satisfy `0 < n <= d`, matching `SimilarityThreshold`.

Runtime kernel shape. `run1/cosine.rs` contains two private pure functions over
`BigUint`, written to mirror the model's spec functions exactly:

- `accept_parts(D, A, B, n, d)`;
- `micros_parts(D, A, B)`, which computes `m = q.sqrt()` and then checks the
  brackets `m^2 * y <= x < (m + 1)^2 * y` at runtime. If the check fails, it
  returns `Run1Error::InvalidScore`.

The runtime check removes any reliance on `BigUint::sqrt` being a floor square
root. The model's precondition then rests only on exact multiplication.

`CosineScore::from_sparse` performs a merge join over two sorted `(key, u128)`
iterators to produce `(D, A, B)`. `CosineScore::between` adapts two
`KindHistogram` values to that.

Representation relation: for sorted sparse inputs, the merge-join dot product
equals the model's `dot` over the dense union-aligned sequences.

### Obligations

#### Verus obligations

All of these live in `verus/clone_detector_type3_cosine.rs`.

- P1 `lemma_threshold_cross_multiplication`.
  - Statement: for integers `D >= 0`, `A > 0`, `B > 0`, `0 < n <= d`, and
    reals `la > 0`, `lb > 0` with `la * la == A as real` and
    `lb * lb == B as real`: `d^2 D^2 >= n^2 A B` holds exactly when
    `D / (la * lb) >= n / d`.
  - Sketch:
    - Both sides of the real inequality are non-negative.
    - Supporting lemma `lemma_square_monotone_nonneg`: for `x, y >= 0`,
      `x >= y` exactly when `x^2 >= y^2`.
    - Supporting lemma `lemma_int_real_cast`: casts preserve products and
      order.
    - Multiply through by the positive `d^2 la^2 lb^2`.
  - Expected friction: symbolic `n/d` mixed across `int` and `real` will not
    verify with an empty body, unlike the fixed `1/5` precedent. Plan explicit
    steps using local `assert(...) by (nonlinear_arith) requires ...` rather
    than a function-level `#[verifier::nonlinear]`.
  - Non-vacuity:
    - Witness `D = 340`, `A = B = 400`, `la = lb = 20`, `n/d = 17/20`: the
      cosine is exactly `0.85`, so the pair is accepted at the boundary.
    - The same norms with `D = 339` are rejected.
    - P1 witnesses must use perfect-square norms, because `la` must exist as
      a real square root.
    - Seeded fault: changing `>=` to `>` in the spec function must make the
      boundary witness fail to verify. Record the run.
- P2 `lemma_threshold_rescale` (integer-only).
  - Statement: if `d > 0` and `n * 10^6 == t * d`, then
    `d^2 D^2 >= n^2 Y` holds exactly when `10^12 D^2 >= t^2 Y`, for all
    `D, Y >= 0`.
  - This bridges P1 to the integer statements below without needing reals.
- P3 `lemma_micros_brackets`.
  - Statement: with `x`, `y > 0`, `q` and `m` as in the model, and given
    `m^2 <= q < (m+1)^2`: both `m^2 y <= x` and `x < (m+1)^2 y`.
  - Sketch: `m^2 <= q` gives `m^2 y <= q y <= x`. `(m+1)^2 >= q + 1` gives
    `(m+1)^2 y >= (q+1) y > x`.
  - Non-vacuity: witness `D = 3`, `A = 9`, `B = 2`, built from `a = (3, 0)`
    and `b = (1, 1)`. The cosine is `1/sqrt(2)` and `m = 707106`, since
    `707106^2 = 499,998,895,236 <= 5 * 10^11 < 707107^2 = 500,000,309,449`.
- P4 `lemma_printed_score_agrees_with_acceptance`.
  - Statement: if `n * 10^6 == t * d`, then `accept(D, A, B, n, d)` holds
    exactly when `micros(D, A, B) >= t`.
  - Sketch: P2 rewrites acceptance as `10^12 D^2 >= t^2 y`. If `m >= t`, then
    `t^2 y <= m^2 y <= x`. If acceptance holds and `m < t`, then
    `(m+1)^2 y <= t^2 y <= x`, which contradicts P3.
  - A separate forward-only lemma covers arbitrary thresholds: acceptance
    implies `micros >= floor(10^6 n / d)`.
  - Non-vacuity:
    - `t = 850000`: `D = 340`, `A = B = 400` gives micros `850000`, accepted.
    - `D = 17000`, `A = 400,000,001`, `B = 1` gives micros `849999`,
      rejected.
    - Seeded fault: dropping the `n * 10^6 == t * d` hypothesis must break
      verification. Record the run.
    - Runtime negative control: `n/d = 1/3` with `a = (1000, 2000, 2000, 1)`
      and `b = (1, 0, 0, 0)` prints micros `333333` but is rejected. This
      shows why decision D10 forbids non-decimal thresholds.
- P5 `lemma_cauchy_schwarz_nonneg`.
  - Statement: for `Seq<int>` sequences `a` and `b` of equal length with
    non-negative elements, `D^2 <= A * B`.
  - Primary route:
    - Define index-based prefix sums `dot_to(a, b, i)`. This avoids `subrange`
      extensionality.
    - By induction on `i`, prove the pure identity
      `sum over k < i of (a_k u - b_k v)^2 = u^2 A_i - 2uv D_i + v^2 B_i` for
      arbitrary integers `u` and `v`.
    - Instantiate `u = B`, `v = D` to get `B (A B - D^2) >= 0`, so
      `D^2 <= A B` when `B > 0`. Handle `B = 0` directly: then every
      `b_k = 0`, so `D = 0`.
  - Fallback route: integer-only induction whose step needs
    `2xy S_ab <= x^2 S_bb + y^2 S_aa`. That follows from
    `(x^2 S_bb - y^2 S_aa)^2 >= 0` together with the induction hypothesis and
    square monotonicity.
  - Non-vacuity: witnesses `a = b = (3, 4)` (equality case, `D^2 = A B`) and
    disjoint supports `a = (1, 0)`, `b = (0, 1)` (`D = 0`).
- P6 `lemma_micros_at_most_one_million` and the capstone
  `lemma_kernel_contract`.
  - From P5 and P3: `m <= 10^6`.
  - The capstone lemma takes `Seq<int>` inputs, feeds `spec_dot` into P1 to
    P5, and states the complete runtime contract:
    - acceptance means `cos >= n/d`;
    - the printed micros agree with acceptance for decimal thresholds;
    - micros lie in `0..=10^6`;
    - a threshold with `n > d` is never met.
  - Non-vacuity: instantiate the capstone with the P1 witness vectors.

Verification evidence for the whole file:

- `make verus-clone-detector` reports every lemma verified.
- `rg -n 'assume\(|admit|external|assume_specification' verus/clone_detector_type3_cosine.rs`
  prints nothing.
- Stage B note: lemma statements with empty bodies are written first.
  Anything that verifies automatically is recorded, not treated as red
  evidence. The seeded faults provide the falsifiability evidence.

#### Runtime obligations

- R1 (representation relation).
  - Method: `proptest` against two independent oracles.
    - A dense `BigUint` oracle over the sorted key union.
    - A plain `u128` oracle for weights below `2^15` and at most 24 keys,
      where every product fits without `BigUint`.
  - Domain: up to 24 keys per side; weights from the full `u128` range,
    including `0`, `1` and `u128::MAX`; key sets that overlap, are disjoint,
    or are identical, tagged through `prop_oneof!` and counted with
    `prop_assert!`-backed classification.
  - Artefact: `crates/whitaker_clones_core/src/run1/cosine_tests.rs`.
  - Non-vacuity: a seeded mutation (an absent key treated as `1`) must fail.
    Record the run.
- R2 (kernel agreement with the model).
  - `accept_parts` and `micros_parts` are tested on the Verus witnesses from
    P1, P3 and P4, row for row.
  - Generated boundary strategy: build Pythagorean triples with Euclid's
    formula, `(p^2 - q^2, 2pq, p^2 + q^2)`. Use vectors
    `a = (p^2 - q^2, 2pq)` and `b = (1, 0)`, so `cos = (p^2 - q^2)/(p^2 + q^2)`
    exactly. Scale by a random positive integer and permute the keys. Assert
    `meets` is true at that threshold and false one step above it. When
    `10^6 * cos` is an integer, assert micros equals it exactly.
  - Properties over generated vectors:
    - micros satisfies the P3 brackets;
    - micros is at most `10^6` (P5 and P6);
    - `cos(a, b) == cos(b, a)`;
    - `cos(k a, b) == cos(a, b)` for integer `k >= 1`.
  - `rstest` rows:
    - identical vectors print `"1.000000"`;
    - disjoint supports print `"0.000000"`;
    - a zero norm returns `None`.
- R3 (decimal rendering): an exhaustive test over every micros value
  `0..=1_000_000`. It formats the value, parses it back through the quarantined
  helper, and checks that the result round-trips exactly through the formatter
  and is at most `1.0`. It runs in well under a second.

#### Classification and validation obligations

- C1 (classification tables).
  - Method: exhaustive `rstest` cases generated from `ALL` arrays of the
    verdict enums, which contain no wildcards.
  - Table A domain: token rule (`WHK001`, `WHK002`) times AST verdict
    (conclusive with hashes equal, conclusive with hashes differing and
    cosine meeting `type3`, conclusive with hashes differing and cosine below
    `type3`, inconclusive). That is 8 rows.
  - Table B domain: the same 4 AST verdicts, so 4 rows.
  - A guard asserts the row count equals the product of the variant counts,
    so a new variant fails until the table grows.
  - Monotonicity property, over generated cosine pairs `c1 <= c2` with all
    other inputs fixed:
    - a pair reported at `c1` is reported at `c2`;
    - its rule either stays the same or moves from `WHK001` or `WHK002` to
      `WHK003`.
  - Profile agreement: for a physical pair accepted under both profiles, the
    two Run 0 results always receive the same *verdict*, meaning both are
    carried or both are reclassified. When carried, each keeps its own token
    rule, so the pair is `WHK001` plus `WHK002`, not one rule twice. When
    reclassified, both become `WHK003`. This is the wording D5's Table A
    implies; an earlier draft said "the same rule", which contradicts D5's
    equal-hash row.
- C2 (threshold validation).
  - `validate_threshold_order` and `validate_decimal_threshold` are checked by
    `proptest` with `any::<usize>()` operands against an oracle that compares
    rationals by a different expression: `BigUint` cross-multiplication.
  - `rstest` rows cover equal neighbours (rejected), `usize::MAX` operands,
    the defaults (accepted), `1/3` (not decimal) and `17/20` (decimal).
  - Kani harness `verify_threshold_order_matches_rational_order` in
    `crates/whitaker_clones_core/src/run1/kani.rs` checks the loop-free
    widened comparison over the entire `usize` domain. It is parser-free and
    must be added to the allowlist in `scripts/run-kani.sh`. Evidence:
    `make kani-clone-detector` passes the harness. Non-vacuity: a seeded
    version using a non-widened `saturating_mul` must produce a
    counterexample.
- K1 (calibration gate, the first red test of EP-M4).
  - An `rstest` table of at least six pairs of unrelated functions with the
    same signature shape and similar size, drawn from realistic Rust:
    accessors, small parsers, loops over slices, and builder methods.
  - Each pair asserts `cosine < 17/20`, and the test records the cosine
    distribution in its failure output.
  - A companion table of genuine near-misses asserts `cosine >= 17/20`:
    one statement added, one statement removed, and a reordered pair of
    `let` bindings.
  - If either table fails, apply the calibration tolerance.
  - A Type-1 measurement table (OQ1): at least six T1-profile pairs that the
    token pass accepts as `WHK001`, built from long functions (at least 400
    tokens) with one added statement, one removed statement, a reordered pair
    of statements, a duplicated statement, and two comment-only edits as
    controls. Each row asserts the intermediate fact that Run 0 accepts it as
    `WHK001`, then records whether the canonical hashes differ and the cosine.
    The test asserts only the controls (equal hashes); the rest is
    measurement. Record "k of n `WHK001` pairs had differing hashes, of which
    m reached `type3`" in `Artefacts and notes`.
- M1 (metrics), checked with `metrics_util::debugging::DebuggingRecorder`:
  - `whitaker_clones.ast.feature_vector.total` increments exactly once per
    physical fragment consumed, even when that fragment appears in several
    pairs or under both profiles;
  - `whitaker_clones.run1.pair.total{classification}` counts each table
    outcome;
  - `lower_span` metrics are not duplicated.

#### SARIF and end-to-end obligations

- E1 (Run 1 covers Run 0).
  - For every Run 0 result there is exactly one Run 1 result with the same
    `whitakerFragment` and locations.
  - Its rule is the same, or it is `WHK003` with `baselineState = updated`.
  - Method: `proptest` driving `refine_with_lowering` with synthetic
    lowering results (histograms, hashes and covering spans), so every
    Table A and Table B row is reachable. A classification counter asserts
    that each row occurs at least once across the run. The real-data instance
    is in the end-to-end test.
- E2 (determinism). `refine_candidate_pairs` returns pairs sorted and
  deduplicated, and `emit_run1` output is byte-identical under permutation and
  duplication of the candidate and accepted inputs. This also covers the same
  code submitted under both profiles. Method: `proptest`, comparing serialized
  JSON.
- E3 (wire shape). Run 1 has:
  - tool name `whitaker_clones_cli@ast`;
  - one invocation with `executionSuccessful = true`;
  - all three rules;
  - results ordered by `(ruleId, whitakerFragment, message)`;
  - `baselineState` on every result;
  - `properties.whitaker.refinement` on every result;
  - for `WHK003` results, the cosine as the message `sim`, with `jaccard`
    still present.

  Method: an `insta` JSON snapshot of a fixture with one result of each
  classification. Redact `tool.driver.version` and the `astHash` values, and
  assert the 64-hex-digit `astHash` shape separately. This keeps the snapshot
  stable across parser re-pins and version bumps. The snapshot will become a
  seed for 7.7.1 golden tests.
- E4 (merge contract), with two pinned tests:
  - `merge_runs(&[run1, run0])` yields exactly Run 1's result set and Run 1's
    tool name;
  - `merge_runs(&[run0, run1])` loses the reclassification. This is a
    characterization of current behaviour; the design contract forbids that
    order.

### Axioms

- `num_bigint::BigUint` implements exact unsigned `+`, `*`, `/` and `Ord`.
  `BigUint::sqrt` is not trusted for correctness, because its result is checked
  at runtime.
- `KindHistogram` and `KindCounts` behave as specified by 7.3.1. This is
  existing, separately tested behaviour.
- `str::parse::<f64>` is correctly rounded for six-decimal strings. This is
  covered exhaustively by R3.
- The `metrics` facade delivers counters to the installed recorder.
- `ra_ap_syntax` produces an `ERROR` node inside a span for the chosen
  unparsable fixture. A step in the behavioural test confirms this.

### Trust boundary

Verus proves properties of the integer model. The runtime ties itself to the
model in three ways:

1. The kernel's `accept_parts` and `micros_parts` mirror the spec functions
   expression for expression, with a comment cross-referencing the proof file.
2. Runtime tests assert the same witnesses, row for row.
3. The square-root brackets are checked at runtime.

This follows ADR 0003 and the precedent in
`verus/decomposition_cosine_threshold.rs`. Kani is used only for the loop-free
threshold-order check (C2). `BigUint` arithmetic is intractable for bounded
model checking, and the classification tables are finite and exhaustively
tested.

## Plan of work

### Stage A: understand and propose

This document. No code changes. It ends when the user approves the plan,
including OQ1, OQ2 and the decisions marked as requiring approval.

### Stage B: red tests and open proof obligations

Each milestone begins with failing tests. EP-M3 begins with lemma statements
and seeded faults. EP-M4 begins with the calibration gate K1.

### Stage C: implementation and proofs together

The kernel (EP-M2) and its Verus model (EP-M3) are adjacent, so that the
kernel's expressions copy the spec functions.

### Stage D: refactor, documentation and gates

Documentation, the ADR, the roadmap update and the final gates (EP-M6).

## Milestones and plateaus

### EP-M0: baseline and preconditions

- Run `make check-fmt`, `make typecheck`, `make lint`, `make test` and
  `make test-doc` on the unmodified branch, and record their timings.
- Install the pinned provers with `make install-verus` and
  `make install-kani`. The proof scripts no longer install anything, so run
  `make verus-clone-detector` and `make kani-clone-detector` once to record a
  green proof baseline.
- Re-fetch all remote branches and confirm that no other branch has taken ADR
  0021.
- Check whether roadmap item 6.5.1 has landed its `BTreeMap` fingerprint
  change, its `all_rules()` rename, or its path newtype, and record the outcome
  against decisions D17 and D18 and the 6.5.1 risk.
- Confirm that `num-bigint` 0.5.1 resolves and that `BigUint::sqrt` exists,
  using a scratch crate under `/tmp` rather than the workspace.

Acceptance evidence: green gate and proof logs under `/tmp`. Recovery: if a
baseline gate fails, stop and report; do not fix unrelated failures on this
branch. Compatibility decision: none.

### EP-M1: SARIF model support

In `crates/whitaker_sarif`:

1. Add `src/baseline_state.rs` with the `BaselineState` enum (D6), and
   `src/tools.rs` with `TOKEN_PASS_TOOL_NAME` and `AST_PASS_TOOL_NAME` (D15).
   Re-export both from `lib.rs`.
2. Change `SarifResult::baseline_state` to `Option<BaselineState>`, and
   `ResultBuilder::with_baseline_state(BaselineState)` to a `const fn`.
3. Make `WhitakerProperties::cosine` an `Option<f64>` (D3), with
   `with_cosine` still a `const fn`. Add the D12 fields:
   - `refinement: Option<RefinementKind>`, a typed enum in the same module;
   - `token_rule_id: Option<String>`;
   - `ast_hashes_equal: Option<bool>`.

   Each field gets a builder method, and each is skipped when absent.
4. Add `TOKEN_HASH_KEY` and `AST_HASH_KEY` beside `WHITAKER_FRAGMENT_KEY` in
   `src/merge.rs`, re-exported from `lib.rs`.
5. Extend `tests/ui/pass_const_apis.rs` to cover `with_baseline_state`.
6. Apply decision D17 if 6.5.1 has not: change `partial_fingerprints` to a
   `BTreeMap` in `src/model/result.rs` and `src/builders/result_builder.rs`,
   with a red test that inserts keys out of order and asserts key-ordered
   serialization.
7. Confirm that `tests/architecture_boundary.rs` still passes; no dependency
   is added.

In `crates/whitaker_clones_core/src/run0/emit.rs`, remove `.with_cosine(0.0)`
and use `TOKEN_HASH_KEY`.

Red tests come first:

- round trips of all four baseline states;
- rejection of an unknown baseline string;
- absence of `cosine`, `refinement`, `tokenRuleId` and `astHashesEqual` when
  unset;
- `RefinementKind` serializes as `"carried"`, `"reclassified"` and `"added"`;
- a Run 0 test asserting that `properties.whitaker` has no `cosine` key.

Then make them green, and update the existing tests that used a string baseline
or an `f64` cosine.

- Requirements: `TDD-RUN1-UPDATE`, `ADR15-I8`, D3, D6, D12, D15, D17.
- Acceptance evidence: `cargo nextest run -p whitaker_sarif` passes, and all
  three gates pass.
- Conformance:
  - no public signature changes beyond D3, D6, D12, D15 and D17;
  - `tests/architecture_boundary.rs` passes (`ADR15-LEAF`);
  - Run 0 output changes only by the absent `cosine`.
- Recovery: revert the milestone commit.
- Compatibility decision: none (see `Constraints`). Artefacts written before
  this milestone are not read back.

### EP-M2: exact cosine kernel

Add `num-bigint = "0.5.1"` to the root `[workspace.dependencies]`, with
`num-bigint = { workspace = true }` in
`crates/whitaker_clones_core/Cargo.toml`. Add `googletest` and
`pretty_assertions`, which are already workspace dependencies, to the crate's
dev-dependencies.

Create `crates/whitaker_clones_core/src/run1/mod.rs` and
`crates/whitaker_clones_core/src/run1/cosine.rs`, containing:

- the `pub(crate)` `CosineScore`;
- `from_sparse` and `between`;
- the private `accept_parts` and `micros_parts`;
- `meets(SimilarityThreshold) -> bool`;
- `micros() -> Run1Result<CosineMicros>`;
- `pub(crate) fn micros_exact(&self) -> BigUint`, used by the property tests;
- the public `CosineMicros` newtype (D16).

Return early from `from_sparse` when either norm is zero, so that no condition
has more than two predicates.

The kernel is gated on nothing; it compiles without the parser feature. Write
the red tests R1, R2 and R3 in `run1/cosine_tests.rs` first, then make them
green.

- Requirements: `TDD-SCORE-T3`, D2, D16.
- Acceptance evidence:
  - R1, R2 and R3 pass;
  - the R1 seeded mutation is observed failing;
  - `BigUint` appears in no public signature (check the `cargo doc` output);
  - all gates pass.
- Recovery: revert the milestone commit, which also contains the dependency.
- Compatibility decision: none.

### EP-M3: Verus proofs for the cosine model

Create `verus/clone_detector_type3_cosine.rs` with the spec functions and
lemmas P1 to P6, their witnesses, and the capstone. Register it in the
`clone-detector` and `all` groups of `scripts/run-verus.sh`. Add a
cross-reference comment above `accept_parts` and `micros_parts`.

Write the statements first, then the proofs. P5 comes last and is subject to
its tolerance. Record the seeded-fault runs for P1 and P4.

- Requirements: `TDD-SCORE-T3` (exactness), and the honesty of the printed
  cosine.
- Acceptance evidence:
  - `make verus-clone-detector` exits 0 with every lemma verified;
  - the forbidden-construct `rg` check prints nothing;
  - the seeded faults fail as predicted;
  - all gates pass.
- Recovery: the proof file is outside the Cargo build, so it can be reverted
  alone.
- Compatibility decision: none.

### EP-M4: fragment spans, shared helpers, calibration, classification and refinement

This milestone has five commits, each gated.

Commit 4.1 (functional API change, D4 and D13):

- Add `FragmentLocation`, and change `TokenFragment::new`. Add `location()`
  and `span()`.
- Change `NormalizedTree::new(root, requested, covering)` and add
  `covering_span()`. Compute the covering span in `lowering.rs`.
- Update every caller: Run 0 tests, AST tests, doctests, the Kani harnesses in
  `src/ast/kani.rs`, and the behavioural tests.
- Promote `run0/test_helpers.rs` to a crate-level `#[cfg(test)]` module,
  `crates/whitaker_clones_core/src/test_support.rs`, so `run1` tests can share
  it.
- Run 0 output is unchanged.

Commit 4.2 (refactor, no behaviour change):

- Introduce `CloneRule { Type1, Type2, Type3 }` with `rule_id()`, `label()`
  (`"T1"`, `"T2"`, `"T3"`) and `type_number()`.
- Move the following out of `run0/emit.rs` into
  `crates/whitaker_clones_core/src/pair_result/` (`mod.rs`, `fingerprint.rs` and
  `location.rs`), all `pub(crate)`:
  - `fragment_map`, `resolve_pair`, `ensure_non_empty` and
    `primary_fingerprint`;
  - `pair_fingerprint`, generalized to `(&CandidatePair, CloneRule)`;
  - `token_hash`, `digest_hex`, `hex_digit` and `u64_big_endian_bytes`, with
    its proptest module;
  - message formatting, generalized over `CloneRule`;
  - `result_sort_key`;
  - `build_pair_result` and `assemble_run`, added on `main` after the draft.
    Generalize `assemble_run` to take the tool name and version, so both runs
    assemble through it;
  - `score_to_f64`, renamed `decimal_to_f64(&str) -> Option<f64>`, with each
    run mapping `None` to its own error.
- Run 0 output must be byte-identical. The existing Run 0 unit and behaviour
  tests pass unchanged.
- Document the module's re-use policy in `docs/developers-guide.md` in the
  same commit:
  - it is owned by the clone-detector emitters;
  - its only permitted callers are `run0` and `run1`;
  - it composes `whitaker_sarif` builders without adding policy.
- The required repository sweep found no equivalent; `rg` found only these
  private helpers.

Commit 4.3 (calibration gate):

- Add K1 as `run1/calibration_tests.rs`, gated on `feature = "parser"`.
- If it fails, stop under the calibration tolerance.
- If it passes, commit it with the measured distribution recorded in
  `Artefacts and notes`.

Commit 4.4 (pure classification):

- Add `run1/config.rs` (`AstPassConfig` and `RefinementInput`).
- Add `run1/outcome.rs` (`AstEvidence`, `RefinedPair`, and re-exported
  `CosineMicros`).
- Add `run1/classify.rs`:
  - the verdict enums and their `ALL` arrays;
  - `classify_accepted` and `classify_unaccepted`, written as nested `match`
    expressions without compound guards;
  - `validate_threshold_order` and `validate_decimal_threshold`;
  - `lowering_disposition`.
- Add `run1/kani.rs`.
- Write C1, C2 and the D11 per-variant rows red first.

Commit 4.5 (refinement):

- Add `run1/refine.rs`, decomposed into small named functions:
  - `index_accepted`, which builds a map from physical pair to accepted
    pairs;
  - `group_candidates`, which groups candidates by physical pair and rejects
    mixed profiles with `Run0Error::MixedProfiles`;
  - `lower_memoised`, which memoizes by physical location using the `entry`
    API;
  - `ast_verdict`, which applies the D14 guards;
  - `refine_accepted` and `refine_unaccepted`.
- Add the public `refine_candidate_pairs` and the `pub(crate)` seam
  `refine_with_lowering(input, config, lower)`. `lower` is a
  `Fn(&str, ByteSpan) -> AstResult<NormalizedTree>`, so tests can inject
  failures and synthetic trees. This adds no public API.
- Add `run1/metrics.rs`, with
  `classification ∈ {carried, reclassified, added, rejected, inconclusive}`.
- Add `run1/error.rs`.
- Every candidate's Jaccard is computed with `jaccard_similarity`, so added
  results carry `jaccard`.
- Accepted pairs that do not appear in `candidates` are still refined. The
  refinement domain is the union of the two.
- Tests gated on the parser feature use real lowering. Tests that inject
  lowering run without it.
- Red tests:
  - `ParserUnavailable` aborts with no partial output;
  - an invalid span aborts;
  - a per-pair failure carries the token verdict;
  - a fragment in several pairs, or under both profiles, is lowered once;
  - a renamed pair submitted under both profiles yields exactly one Run 1
    result;
  - mixed-profile candidates are rejected;
  - the D14 guards make evidence inconclusive;
  - M1.

Milestone summary:

- Requirements: `TDD-SCORE-T3`, `TDD-METRICS-FV`, `TDD-CONFIG`, D4, D5, D10,
  D11, D13 and D14.
- Acceptance evidence: the tests above pass; `make kani-clone-detector`
  passes the new harness; all gates pass after each commit.
- Conformance:
  - the tables match D5 exactly (C1);
  - `ast_boundary` passes;
  - `lower_span` metrics are not duplicated.
- Recovery: each commit reverts independently, last first.
- Compatibility decision: none. `TokenFragment` and `NormalizedTree` are
  pre-1.0 and have no external consumers.

### EP-M5: Run 1 emission and system-level tests

Add `run1/emit.rs` with
`emit_run1(input: &RefinementInput<'_>, refined: &[RefinedPair],
config: &AstPassConfig) -> Run1Result<Run>`.
It:

- builds one result per `RefinedPair` through the `pair_result` helpers;
- applies D7 for identity, D8 for `astHash`, and D12 for provenance;
- for `WHK003` results, uses the cosine as the message `sim`;
- sorts, deduplicates, and wraps the results in a `Run` named by
  `AST_PASS_TOOL_NAME`, with all three rules and one invocation with
  `executionSuccessful = true`.

Re-export the public surface from `lib.rs`.

Add the tests:

- `crates/whitaker_clones_core/tests/features/run1_sarif.feature` and
  `crates/whitaker_clones_core/tests/run1_sarif_behaviour.rs`. The feature text
  is in `Validation and acceptance`.
- `crates/whitaker_clones_core/src/run1/emit_tests.rs`, with E2, E3 (the
  snapshot) and E4.
- The E1 property, in `run1/refine_tests.rs`.
- `crates/whitaker_clones_core/tests/run1_pipeline.rs`, the end-to-end test.
  It writes three Rust source files of at least 150 tokens each, one `fn` item
  per file, to a `tempfile` directory, and reads them back through `cap_std`:
  - `near_miss_a.rs` and `near_miss_b.rs` differ by one added statement;
  - `unrelated.rs` has the same signature shape but a different body.

  It creates T1 and T2 fragments for each function and runs normalization,
  fingerprinting, MinHash and LSH. It asserts the intermediate facts: the
  near-miss pair is an LSH candidate, and Run 0 does not accept it. It then runs
  `refine_candidate_pairs` and `emit_run1`, builds a `SarifLog` with
  `runs = [run0, run1]`, serializes it, and parses it back. It asserts:
  - `runs[1]` holds exactly one `WHK003` result for the near-miss pair, with
    `cosine`, `jaccard` and `astHash` present;
  - there is no result for `unrelated.rs`;
  - E1 holds on the real data;
  - `merge_runs(&[run1, run0])` keeps the `WHK003` result.

The test also covers:

- empty candidates, which give zero results, three rules and
  `executionSuccessful = true`;
- identical functions in different files, which give `WHK001` with a cosine
  of `1.000000`;
- a near-exact pair (OQ1): two long functions that differ by one statement
  and that Run 0 accepts as `WHK001`. If the AST evidence is conclusive with
  differing hashes and a cosine at or above `type3`, assert one `WHK003` result
  per accepted profile, each with `tokenRuleId: "WHK001"` or `"WHK002"`, its
  Run 0 `whitakerFragment`, and `baselineState` `updated`. The test also counts
  every `WHK001` result in `runs[0]` whose AST hashes differ, and the count
  joins the K1 figure in `Artefacts and notes`.

Milestone summary:

- Requirements: `TDD-RUN1-UPDATE`, `TDD-RUNS`, `TDD-MERGE`, `TDD-ACCEPT-B`,
  D7, D8, D12 and D15.
- Acceptance evidence: the behaviour suite, snapshot, properties and
  end-to-end test pass; all gates pass.
- Conformance:
  - identity is verified by E1, the wire shape by E3, and the merge contract
    by E4;
  - no new dependency.
- Recovery: revert the milestone commit.
- Compatibility decision: none.

### EP-M6: documentation, ADR, roadmap and final gates

- `docs/adrs/adr-0021-type-3-refinement-semantics-for-sarif-run-1.md`,
  created as Proposed during planning with the OQ1 ruling and the 7.3.3
  follow-up. Complete its `Outstanding decisions`, move it to Accepted with the
  approval date, and record:
  - D2, D5 (with the answers to OQ1 and OQ2), D7, D10, D12 and D14;
  - the measured `WHK001` differing-hash figure from K1 and the end-to-end
    test;
  - the residual fingerprint churn: a pair whose token verdict flips between
    commits changes its `whitakerFragment`.

  Index it in `docs/contents.md`.
- `docs/whitaker-clone-detector-design.md`:
  - add "Implementation decisions (7.3.2)", covering D1 to D16 and the proof
    scope and trust boundary for P1 to P6;
  - update §Result mapping (optional `cosine`, the provenance fields, and
    `baselineState`), and state that the property bag carries no
    read-compatibility promise before 1.0 (D18, `ADR15-I5`);
  - update §Scoring and acceptance and §SARIF update (Run 1), with a link to
    ADR 0021;
  - write the consumer contract into §Grouping and reporting and §Dylint
    integration: "Run 1 supersedes Run 0. A merged log must place Run 1
    first. The lint reads the run named `whitaker_clones_cli@ast` when
    present, otherwise the token run, and never unions runs."
- `docs/users-guide.md`: replace the "Not yet emitted" paragraph in the
  clone-detection section. Describe `WHK003`, `baselineState` and `refinement`,
  explain that a `WHK003` result may carry `tokenRuleId` `WHK001` or `WHK002`,
  how `cosine` reads, and the default `type3` threshold. State that
  `baselineState` is relative to the same invocation's token pass, that
  consumers must not gate on `new`, and that refined files are not cross-commit
  baselines.
- `docs/developers-guide.md`:
  - the `run1` module structure;
  - the `pair_result` re-use policy (added in commit 4.2);
  - the new metric names;
  - the Verus file under §Verus scope and trust boundary;
  - the Kani harness under §Kani bounded model checking.
- Name the deferred gap: per-pair lowering failures are visible only through
  metrics, because the `Invocation` model has no `toolExecutionNotifications`.
  Record it in the design document as a follow-up for 7.4.1.
- `docs/roadmap.md`: mark 7.3.2 done with an evidence note. Item 7.3.3 (exact
  Type-1 acceptance) was added at planning; leave it open. Add 7.3.4 per D9, if
  approved, using `mapsplice`.
- Run `make fmt`, `make markdownlint`, `make spelling`, `make check-fmt`,
  `make typecheck`, `make lint`, `make test`, `make test-doc`,
  `make verus-clone-detector` and `make kani-clone-detector`.

Acceptance evidence: all gates green, and the documents reflect the shipped
behaviour. Conformance: every deviation listed in `Conformance basis` is
written back to the design document and ADR 0021.

## Concrete steps

Run every command from the repository root, capturing long output with `tee`.
`pipefail` is required: without it a failing `make` is masked by the succeeding
`tee`, and the block reports a green gate that failed.

```bash
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-${BRANCH}.out
make typecheck 2>&1 | tee /tmp/typecheck-whitaker-${BRANCH}.out
make lint      2>&1 | tee /tmp/lint-whitaker-${BRANCH}.out
make test      2>&1 | tee /tmp/test-whitaker-${BRANCH}.out
make test-doc  2>&1 | tee /tmp/test-doc-whitaker-${BRANCH}.out
make verus-clone-detector 2>&1 | tee /tmp/verus-whitaker-${BRANCH}.out
make kani-clone-detector  2>&1 | tee /tmp/kani-whitaker-${BRANCH}.out
```

Focused red-green loops:

```bash
cargo nextest run -p whitaker_sarif
cargo nextest run -p whitaker_clones_core run1
cargo nextest run -p whitaker_clones_core --test run1_sarif_behaviour
cargo nextest run -p whitaker_clones_core --test run1_pipeline
cargo insta test -p whitaker_clones_core --review   # only for an intentionally new snapshot
```

Expected red output at the start of EP-M2 (abridged):

```plaintext
error[E0432]: unresolved import `crate::run1::cosine`
```

Expected Verus output at the end of EP-M3 (abridged):

```plaintext
verification results:: N verified, 0 errors
```

Run the gates sequentially before every commit. Delegate full gate runs to the
`scrutineer` agent where available.

## Validation and acceptance

Behaviour-driven development (BDD) feature file
`crates/whitaker_clones_core/tests/features/run1_sarif.feature`:

```gherkin
Feature: SARIF Run 1 Type-3 refinement
  The AST pass refines token-pass results and adds near-miss clones.

  Scenario: A near-miss pair missed by the token pass is added as Type-3
    Given the Rust fragments near_miss_left and near_miss_right
    And the token pass does not accept the pair
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state new
    And the result records a cosine of at least 0.85
    And the result carries an astHash partial fingerprint

  Scenario: A rename-equivalent Type-2 pair is carried unchanged
    Given the Rust fragments renamed_left and renamed_right
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK002 result with baseline state unchanged
    And the result records a cosine of 1.000000

  Scenario: A Type-2 pair with differing structure is reclassified
    Given the Rust fragments reshaped_left and reshaped_right
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state updated
    And the result keeps its Run 0 whitakerFragment fingerprint
    And the result records token rule WHK002

  Scenario: A near-exact Type-1 pair with differing structure is reclassified
    Given the Rust fragments near_exact_left and near_exact_right
    And the token pass accepts the pair as WHK001
    When the AST pass refines the candidates
    Then Run 1 contains a WHK003 result with baseline state updated
    And the result keeps its Run 0 whitakerFragment fingerprint
    And the result records token rule WHK001

  Scenario: A renamed pair under both profiles is reported once
    Given the Rust fragments renamed_left and renamed_right under both profiles
    And the token pass accepts only the T2 pair
    When the AST pass refines the candidates
    Then Run 1 contains exactly one result for the pair

  Scenario: Structurally unrelated candidates are not reported
    Given the Rust fragments unrelated_left and unrelated_right
    And the token pass does not accept the pair
    When the AST pass refines the candidates
    Then Run 1 contains no result for the pair

  Scenario: An unparsable fragment keeps its token verdict
    Given the Rust fragments renamed_left and broken_right
    And lowering broken_right reports an unparsable span
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK002 result with baseline state unchanged
    And the result has no cosine and no astHash

  Scenario: Misordered thresholds are rejected
    Given a type3 threshold of 0.95 and a type2 threshold of 0.90
    When the AST pass refines the candidates
    Then refinement fails with a threshold order error
```

The step definitions follow the repository's rstest-bdd conventions:

- `given` and `when` steps return `Result`; only `then` steps assert;
- each step parses at most three values.

"The token pass accepts the pair as WHK002" constructs the `AcceptedPair`
directly through its public constructor. Only the end-to-end test runs the real
token pass. The `broken_right` fixture must place an `ERROR` node inside the
span; its second `given` step confirms that `lower_span` returns
`UnparsableSpan`.

Record red-green-refactor evidence per milestone in `Artefacts and notes`:

- red: the focused command and its failure;
- green: the same command passing;
- refactor: the gates passing.

Quality criteria:

- Tests: `make test` passes, including all new unit, property, snapshot,
  behavioural and end-to-end tests.
- Verification:
  - P1 to P6 are verified with no trusted shortcuts, or P5 is escalated per
    its tolerance;
  - the C2 Kani harness passes;
  - R1 to R3, C1, C2, K1, M1 and E1 to E4 pass, with non-vacuity evidence
    recorded.
- Lint and type checks: `make check-fmt`, `make typecheck` and `make lint`
  pass, and `make test-doc` passes for the new public doc examples.
- Performance: no benchmark. The memo guarantees one lowering per physical
  fragment per pass.
- Security: no new trust boundary. `num-bigint` is reviewed under normal
  dependency policy.

## Idempotence and recovery

All steps are re-runnable. Tests write only to `tempfile` directories.
Snapshots change only through deliberate `cargo insta` review. Each commit
passes every gate, so reverting a milestone's commits restores the previous
plateau. The Verus file is outside the Cargo build.

## Artefacts and notes

### K1 calibration

Measured with
`cargo nextest run --all-features -p whitaker_clones_core
report_alternative_features --no-capture`
over six unrelated pairs (same signature shape, similar size) and three
near-miss pairs of realistic Rust. Values are cosine micros.

| Pair                          | Depth-weighted kinds | Unweighted counts | Trigram productions |
| ----------------------------- | -------------------- | ----------------- | ------------------- |
| unrelated: loop / builder     | 998,477              | 945,690           | 859,713             |
| unrelated: accessor / parser  | 995,012              | 940,783           | 795,947             |
| unrelated: fold / filter      | 999,590              | 969,447           | 907,084             |
| unrelated: match / if         | 994,881              | 868,092           | 626,212             |
| unrelated: builder / iterator | 984,762              | 904,346           | 748,637             |
| unrelated: write / read       | 988,160              | 982,173           | 929,516             |
| near-miss: statement added    | 998,158              | 992,708           | 982,289             |
| near-miss: bindings reordered | 1,000,000            | 1,000,000         | 1,000,000           |
| near-miss: guard added        | 999,812              | 975,809           | 942,074             |

**K1 verdict: the strict form fails on every candidate feature.** The
depth-weighted histogram, which the design named, cannot separate the tables at
all. Trigram productions are the best of the three and are what Run 1 ships,
but three of the six unrelated pairs still score above the shipped `17/20`
default. Raising `type3` above `9/10` would need the Type-2 threshold
reordered, which is Run 0 acceptance and outside this item, so the default
stands and the finding is recorded in ADR 0021 rather than resolved by tuning.

A tighter `type3 = 93/100` *would* separate this table cleanly (unrelated max
929,516; near-miss min 942,074). It is not adopted here because it violates the
approved `type1 > type2 > type3` ordering against Type-2's `9/10`. That is the
maintainer's call, and ADR 0021 records the measurement that supports it.

### `WHK001` differing-hash figure (OQ1)

Not measured on a realistic corpus. Every fixture pair the suite exercises
hashes equal when its covering span equals its requested span, so the observed
rate is zero out of the pairs tested, which is too small a sample to report.
Item 7.3.3 owns the measurement, because that is where it changes a decision.

### Proof and verification summary lines

```plaintext
verus clone_detector_lsh_config.rs       :: 9 verified, 0 errors
verus clone_detector_candidate_pair.rs   :: 10 verified, 0 errors
verus clone_detector_ast_features.rs     :: 8 verified, 0 errors
verus clone_detector_type3_cosine.rs     :: 31 verified, 0 errors
```

`rg -n 'assume\(|admit|external|assume_specification'
verus/clone_detector_type3_cosine.rs`
prints nothing.

### End-to-end fixture measurements

The final `run1_pipeline` fixtures are ~150 T1 tokens each. With two added
statements the near-miss pair scored a Type-2 Jaccard of 0.9245 and was still
accepted by the token pass; adding two more statements took it to 0.4040, at
which point Run 0 accepts nothing and Run 1 adds a single `WHK003` with cosine
0.9926. The snapshot pins exactly that.

### Kani coverage of the threshold comparison

The plan's C2 obligation called for a Kani harness over the widened
threshold-order comparison. CI reported it inconclusive: 600 s timeout at
327-366 MiB of an 8 GiB cap. Investigation showed the cause is not the value
space. A minimal probe — two 8-bit symbolic operands multiplied as `u128` —
also missed a 600 s budget at under 400 MiB, so Kani 0.67's CBMC cannot
discharge symbolic 128-bit multiplication at any domain size. Narrowing the
symbolic type, bounding the range, and splitting the harness all failed, which
is consistent with that explanation.

C2 therefore ships as:

- a compile-time bound, `WIDENING_IS_LOSSLESS`, stating that two `usize`
  products fit in `u128` — which is the entire justification for the unchecked
  widening, and holds for every `usize`;
- eight concrete `rstest` rows over the widening contract at zero, one, the
  shipped defaults, `usize::MAX`, and operand orders that would expose an
  argument transposition;
- the existing `BigUint`-oracle property tests, which already compared the
  widened comparison against an independent exact oracle;
- the Verus proofs of the threshold arithmetic for arbitrary integers.

`scripts/run-kani.sh` reports the omission by name, following the existing
precedent for the decomposition harnesses, so a green Kani run never reads as a
proof of this property. This is a narrowing of C2's *method*, not of its claim:
the claim is covered, by a compile-time bound plus oracle comparison rather
than by bounded model checking.

### Gate results

- Clippy over both changed crates, with all targets and features, denying
  warnings: clean.
- `make lint-whitaker`: clean, after splitting `refine.rs` and three test
  modules to satisfy `module_max_lines`.
- `cargo doc` with `-D warnings` on both crates: clean.
- `cargo test --doc -p whitaker_clones_core --all-features`: 42 passed, 0
  failed. The whole-workspace `make test-doc` reached its harness time cap
  while compiling unrelated crates under a saturated compile queue; every crate
  that did run reported `test result: ok`.
- `cargo nextest run --all-features -p whitaker_sarif -p whitaker_clones_core`:
  539 passed. One pre-existing `build_script_integration` case
  (`case_2_missing`) was killed by the harness timeout while queued behind
  compile admission, not by an assertion; it passes when run alone.
- `make verus-clone-detector`: exits 0, all four clone-detector proof files
  verify.
- `make typecheck`: clean.
- `make test`: 2,813 passed, 0 assertion failures. Two tests
  (`macro_aware_cognitive_complexity::parity` and a `whitaker::ui` trybuild
  case) were recorded as SIGTERM because the harness stopped the run at its
  two-hour cap while they were still queued behind compile admission. They are
  unrelated to this change and identical to the pre-existing `make test` result
  on this host.

### Review-round corrections (2026-11-25)

The first formal review raised findings that changed four things worth
recording, because each was a real defect rather than a style preference.

- **SARIF `endColumn` is exclusive.** `region_for_range` mapped the *last*
  character's start to `endColumn`, so a one-byte region reported an empty text
  span beside `byteLength: 1`. §3.30.8 of the SARIF 2.1.0 specification defines
  `endColumn` as one greater than the column of the region's last character.
  The conversion now measures `range.end` directly, except when the region ends
  with a line break: a break is not a character the line/column model contains,
  so the position *of* the break is measured instead. Reporting the position
  after it moved `endLine` onto the next line, which §3.30.7 forbids. This
  corrected Run 0 as well, because both runs share the helper; the Run 0 region
  tests, one feature scenario and one snapshot moved with it.
- **`micros` is exactly scale-invariant.** The property test allowed a
  one-micro drift after positive integer scaling. Scaling `a` by `k` multiplies
  both `D` and `sqrt(A)`, so the rational inside the floor is unchanged and the
  printed score must be identical. The tolerance is gone, which matters because
  a one-micro drift can straddle a six-decimal threshold.
- **The pair counter counts physical pairs.** `record_classifications` fired
  once per *result*, so a pair accepted under both profiles counted twice, and
  an inconclusive pair counted once for `inconclusive` plus once per carried
  result. One label is now chosen per physical pair, and the metrics test
  asserts the exact label-to-count map for carried, reclassified, added,
  rejected and inconclusive, plus a dual-profile case pinned at one event.
- **The Run 0 preservation test was vacuous.** `run_pipeline`'s fixtures are a
  deliberate token-pass miss, so `run0.results` was empty and the E1 loop
  checked nothing. It now drives rename-equivalent fixtures the token pass
  accepts, asserts Run 0 is non-empty before iterating, and requires at least
  one carried counterpart.

The same round split four files that exceeded the 400-line budget:
`run1_pipeline.rs` into entrypoints plus a `run1_pipeline_support` module,
`run1_sarif_behaviour.rs` into steps plus `run1_sarif_support`, `refine.rs`
into the pass plus `refine_labels`, and `refine_metrics_tests.rs` into a case
table. The `synthetic_tree` fixture was also off by one — it subtracted four
fixed nodes where the shape has five — so a request for the `min_nodes` floor
produced a tree one node above it.

### Gate results for the review round

All gates were re-run on the corrected tree, in the order the host requires so
they reuse build caches safely.

- `make fmt`, `make check-fmt`: clean.
- `make spelling`, `make markdownlint`, `make nixie`: clean.
- `make lint`: exit 0. Clippy, rustdoc with `-D warnings`, the skill-manifest
  checks and the Whitaker Dylint suite over the support crates all pass.
- `make verus-clone-detector`: exit 0, four groups verifying 9, 10, 8 and 33
  obligations with no errors and no trust shortcuts.
- `make kani-clone-detector`: exit 0, all fifteen harnesses verified. The first
  attempt timed out on `verify_lsh_config_new_smoke` at its 600 s cap while the
  host load average was 17.5 on 24 cores; the same harness then passed in 311 s
  on the retry, so the timeout was contention, not a regression. That harness
  belongs to roadmap item 7.2.8 and this change does not touch it.
- `make test`: exit 0. The nested-build warm stage passed 19/19, the main pass
  passed 3,025/3,025 with 7 skipped, and the three follow-on passes passed
  135/135, 64/64 and 50/50. No test failed. This run was detached from the tool
  harness, because an earlier attempt was killed by the harness's own
  background-task limit while slow `trybuild` cases were queued; that attempt
  reported SIGTERM, not assertion failures.
- `make test-doc`: exit 0, no failures.
- `cs delta origin/main`: one CodeScene improvement and no new issues, after
  collapsing two near-identical scenario constructors in the metrics test.

### Seeded-fault and red-green records

- EP-M2 red: `error[E0432]: unresolved import crate::run1::cosine` before the
  module existed.
- EP-M1 red: `WhitakerPropertiesBuilder::with_refinement` and
  `ResultBuilder::with_baseline_state(BaselineState)` did not exist; the
  `property_bag` integration test failed to compile until both were added.
- K1 red: the strict gate failed by construction on every candidate feature;
  the shipped gate asserts the properties that do hold.
- P1/P4 seeded faults: swapping `>=` for `>` in `runtime_accepts` makes
  `witness_boundary_is_accepted` fail, and dropping the
  `numerator * 1_000_000 == threshold_micros * denominator` hypothesis makes
  both `lemma_threshold_rescale` and
  `lemma_printed_score_agrees_with_acceptance` fail.
- R1 seeded fault: treating an absent sparse key as `1` breaks
  `sparse_join_matches_the_dense_oracle` and
  `sparse_join_matches_the_narrow_oracle`.

## Interfaces and dependencies

New dependency: `num-bigint = "0.5.1"` (workspace), which pulls in
`num-integer`. It is used only inside `run1/cosine.rs`.

Public additions and changes in `whitaker_clones_core`, re-exported from the
crate root:

```rust
pub struct FragmentLocation { /* file_uri: String, span: Range<usize> */ }
impl FragmentLocation {
    pub fn new(file_uri: impl Into<String>, span: Range<usize>) -> Self;
    pub fn file_uri(&self) -> &str;
    pub fn span(&self) -> Range<usize>;
}
impl TokenFragment {
    pub fn new(id: FragmentId, profile: NormProfile, location: FragmentLocation,
               source_text: impl Into<String>) -> Self;
    pub const fn location(&self) -> &FragmentLocation;
    pub fn span(&self) -> Range<usize>;
}
impl NormalizedTree {
    pub const fn new(root: NormalizedNode, requested: ByteSpan, covering: ByteSpan) -> Self;
    pub const fn covering_span(&self) -> ByteSpan;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CosineMicros(u32);                    // 0..=1_000_000
impl CosineMicros {
    pub const fn get(self) -> u32;
    pub fn as_decimal_string(self) -> String;    // "0.850000", "1.000000"
}

pub struct AstPassConfig { /* tool_version, type3_threshold, min_nodes, max_cover_expansion */ }
impl AstPassConfig {
    pub fn new(tool_version: impl Into<String>) -> Self;  // tool name is AST_PASS_TOOL_NAME
    pub const fn with_type3_threshold(self, threshold: SimilarityThreshold) -> Self;
    pub const fn with_min_nodes(self, min_nodes: NonZeroUsize) -> Self;
    pub const fn with_max_cover_expansion(self, factor: NonZeroUsize) -> Self;
    // getters for each field
}

pub struct RefinementInput<'a> { /* fragments, candidates, accepted, token_config */ }
impl<'a> RefinementInput<'a> {
    pub const fn new(fragments: &'a [TokenFragment], candidates: &'a [CandidatePair],
                     accepted: &'a [AcceptedPair], token_config: &'a TokenPassConfig) -> Self;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AstEvidence { /* left_hash, right_hash: AstHash; cosine: CosineMicros */ }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefinedPair {
    Carried { token: AcceptedPair, ast: Option<AstEvidence> },
    Reclassified { token: AcceptedPair, ast: AstEvidence },
    Added { pair: CandidatePair, rule: CloneRule, jaccard: SimilarityRatio, ast: AstEvidence },
}
impl RefinedPair {
    pub const fn rule(&self) -> CloneRule;
    pub const fn baseline_state(&self) -> BaselineState;
    pub const fn ast(&self) -> Option<&AstEvidence>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CloneRule { Type1, Type2, Type3 }       // rule_id(), label(), type_number()

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThresholdKind { Type1, Type2, Type3 }

pub fn refine_candidate_pairs(input: &RefinementInput<'_>, config: &AstPassConfig)
    -> Run1Result<Vec<RefinedPair>>;             // sorted and deduplicated
pub fn emit_run1(input: &RefinementInput<'_>, refined: &[RefinedPair], config: &AstPassConfig)
    -> Run1Result<whitaker_sarif::Run>;

#[derive(Debug, thiserror::Error)]
pub enum Run1Error {
    #[error("threshold {higher:?} must exceed threshold {lower:?}")]
    ThresholdOrder { higher: ThresholdKind, lower: ThresholdKind },
    #[error("type3 threshold {numerator}/{denominator} is not a six-decimal value")]
    ThresholdNotDecimal { numerator: usize, denominator: usize },
    #[error("fragment `{fragment_id}` has an invalid span: {source}")]
    InvalidFragmentSpan { fragment_id: String, #[source] source: AstError },
    #[error("the AST parser is unavailable in this build")]
    ParserUnavailable,
    #[error("cosine score `{value}` is invalid")]
    InvalidScore { value: String },
    #[error(transparent)]
    TokenPass(#[from] Run0Error),                // missing fragments, empty sets, mixed profiles
    #[error(transparent)]
    Sarif(#[from] whitaker_sarif::SarifError),
}
pub type Run1Result<T> = Result<T, Run1Error>;
```

`Run1Error` cannot derive `PartialEq`, because `SarifError` holds an
`io::Error`. Tests use `matches!`.

Public additions and changes in `whitaker_sarif`:

- `BaselineState` and `RefinementKind`;
- `TOKEN_PASS_TOOL_NAME`, `AST_PASS_TOOL_NAME`, `TOKEN_HASH_KEY` and
  `AST_HASH_KEY`;
- `SarifResult::baseline_state: Option<BaselineState>` and the `const fn`
  `ResultBuilder::with_baseline_state(BaselineState)`;
- `WhitakerProperties::cosine: Option<f64>`, plus `refinement`,
  `token_rule_id` and `ast_hashes_equal`.

Crate-private additions:

- `pair_result::*`;
- `run1::cosine::CosineScore`;
- `run1::classify::*`;
- `run1::refine::refine_with_lowering`;
- `run1::metrics`;
- `test_support` (test-only).

## Revision note

- 2026-11-11: initial draft from Wyvern reconnaissance and external research.
- 2026-11-11, revision 1, after the expert-panel review. The four
  reviewers were formal verification, SARIF interop, Rust API and architecture,
  and clone semantics with test strategy. Changes:
  - Added physical-pair identity (D4, D5, D7 and D11) to stop duplicate and
    spurious results across profiles.
  - Added the merge-order obligation E4 and the consumer contract.
  - Added provenance properties (D12), canonical tool names (D15), a
    calibration gate (K1) with pre-costed fallbacks, covering-span expansion
    and `min_nodes` guards (D13, D14), and the six-decimal threshold rule
    (D10).
  - Corrected the P1 witness (`D = 340`, not `17`).
  - Added the integer rescale lemma (P2), the micros-bound and capstone
    lemmas (P6), and a simpler Cauchy–Schwarz route (P5). Added the runtime
    square-root bracket check, Pythagorean boundary generators, the
    exhaustive decimal test (R3), and a Kani harness for threshold order.
  - Reshaped `Run1Error`, `RefinedPair` (now an enum), `CosineMicros` and
    `emit_run1`.
  - Split EP-M4 into five commits and listed every helper that moves.
  - Raised the tolerances to 42 files and 4,200 lines.
  - Opened questions OQ1 (`WHK001` reclassification) and OQ2 (rule for
    equal-hash added pairs).

  Impact: more work in EP-M1 and EP-M4. The milestone order and the approval
  gate are unchanged.
- 2026-11-21, revision 2, after rebasing onto `origin/main` at `9d4c5490`.
  Changes:
  - Renumbered the ADR to 0018, because 0013 to 0016 landed on `main` and an
    unmerged branch holds 0017.
  - Adopted the ADR 0015 rules for `whitaker_sarif`: ordered fingerprints
    (D17), and the `columnKind` and property-bag boundaries (D18).
  - Added a coordination risk with roadmap item 6.5.1.
  - Added prover installation to EP-M0, and `make typecheck` and
    `make test-doc` to every gate list.
  - Folded the new `build_pair_result` and `assemble_run` helpers into commit
    4.2.

  Impact: EP-M1 may carry the ordered-fingerprint change. The milestone order,
  the open questions and the approval gate are unchanged.
- 2026-11-23, revision 3, at the user's direction. Changes:
  - Added a constraint that 7.3.2 owes no backward compatibility: the clone
    detector is experimental in a pre-1.0 linter with no outside users.
  - Dropped the old-artefact read test from EP-M1 and the `#[serde(default)]`
    attribute from D3.
  - Rewrote the D18 item 5 boundary to state no read compatibility, and
    retraced `ADR15-I5` to the EP-M6 design-document note.

  Impact: EP-M1 loses one test. The milestone order, the open questions and the
  approval gate are unchanged.
- 2026-11-23, revision 4, after the user resolved OQ1. Changes:
  - OQ1 resolved as option (b): Table A gives `WHK001` the `WHK002` rows, and
    D5, D12, C1, the conformance deviations and the BDD feature follow.
  - Added the `WHK001` differing-hash measurement to K1 and the end-to-end
    test, recorded in `Artefacts and notes` and ADR 0021.
  - Created ADR 0021 as Proposed now, because 0017 to 0020 are taken on
    unmerged branches. It records the OQ1 ruling and sets out exact Type-1
    acceptance, which the new roadmap item 7.3.3 references. D9's proposed
    tree-edit item moves to 7.3.4.

  Impact: EP-M4 and EP-M5 gain measurement tests and one BDD scenario. The
  milestone order and the approval gate are unchanged.
