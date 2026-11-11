# Score Type-3 similarity and update SARIF Run 1 (roadmap 7.3.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`. Do not begin
implementation until the user has explicitly approved this plan.

## Purpose / big picture

Whitaker's clone detector runs in two passes. Pass A, the token pass, already
finds Type-1 clones (identical after whitespace and comments are removed) and
Type-2 clones (identical after identifiers and literals are renamed), and
writes them as Static Analysis Results Interchange Format (SARIF) "Run 0".
Roadmap item 7.3.1 added the parser adapter for Pass B, the abstract syntax
tree (AST) pass. It lowers a candidate byte span to a parser-independent tree
and extracts feature vectors and a canonical subtree hash from it. Nothing
consumes those vectors yet.

This item closes Pass B. After this change, a library caller can take the token
pass's fragments, its candidate pairs and its accepted pairs, and receive a
SARIF "Run 1" that:

1. Reports Type-3 (near-miss) clones under rule `WHK003` whenever the cosine
   similarity of the two fragments' depth-weighted node-kind histograms is at
   least the configured `type3` threshold.
2. Reclassifies a token-pass `WHK002` result as `WHK003` with
   `baselineState = "updated"` when the AST evidence shows that the pair is a
   near-miss rather than a pure rename.
3. Carries every other token-pass result forward with
   `baselineState = "unchanged"`, so Run 1 is a complete refined view that the
   future `clone_detected` lint can read alone.
4. Records `properties.whitaker.cosine` and an `astHash` partial fingerprint on
   every result whose fragments were lowered successfully.

Observable success: running
`cargo nextest run -p whitaker_clones_core --test run1_sarif_behaviour` passes
new behavioural scenarios that build Run 0 and Run 1 from real Rust source
snippets. A snapshot test pins the complete Run 1 JSON for a fixture that
exercises every classification. `make verus-clone-detector` proves that the
integer acceptance test is exactly "cosine is at least the threshold". It also
proves that the six-decimal cosine printed in SARIF never contradicts the
accept or reject decision.

The command-line interface (`cargo whitaker clones refine`) is roadmap item
7.4.1 and is out of scope. This item delivers the library behaviour that the
command will call.

## Constraints

- Scope is roadmap item 7.3.2 only. Do not implement the CLI (7.4.1), HTML
  reports (7.4.2), the `clone_detected` lint (7.5.x), caching (7.6.1), or
  grouping beyond the pairwise `groupId = 0`, `classSize = 2` convention that
  Run 0 already uses.
- `crates/whitaker_clones_core/src/ast/lowering.rs` remains the only file that
  may import `ra_ap_syntax`. New Run 1 code consumes `NormalizedTree`,
  `KindHistogram` and `AstHash` only. `tests/ast_boundary.rs` must keep passing.
- `lower_span` remains the only emitter of lowering latency, outcome and
  recovery metrics (design §AST operational metrics). Run 1 adds only
  feature-vector consumption and pair-classification metrics, with bounded
  labels.
- Acceptance decisions must be exact and platform-independent. No `f32` or
  `f64` value may influence whether a pair is accepted or how it is classified.
  The only floating-point value is the SARIF `cosine` property. It is produced
  by parsing the stable six-decimal string in one quarantined helper, following
  the 7.2.3 precedent (`score_to_f64` in
  `crates/whitaker_clones_core/src/run0/emit.rs`).
- Run 0 behaviour stays the same apart from the `cosine` property, which
  changes from `0.0` to absent (decision D3). Run 0 regions, fingerprints,
  ordering and rule selection must not change.
- No file may exceed 400 lines (`AGENTS.md`). `run0/emit.rs` is already 391
  lines, so Run 1 code lives in a new `run1/` module subtree and shared result
  helpers move out of `run0/emit.rs` rather than growing it.
- Every module starts with a `//!` comment. Every function carries a `///`
  comment. Public items carry examples that follow
  `docs/rust-doctest-dry-guide.md`.
- No `.expect()` or `.unwrap()` in production code or shared fixtures. Tests
  use `.expect(...)` with a message.
- Verus proofs must not use `assume`, `admit`, `#[verifier::external_body]`
  on proof functions, or any other trusted shortcut to discharge a stated
  lemma. Unproved lemma statements may remain in this plan until their
  milestone.
- Proof tooling stays sidecar-only (ADR 0003). Verus and Kani never become
  Cargo dependencies.
- Dependencies use caret requirements, declared once in the root
  `Cargo.toml` `[workspace.dependencies]` table.
- Documentation uses en-GB Oxford spelling and wraps prose at 80 columns.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 34 changed non-ExecPlan files
  or more than 3,200 net lines of code and other documentation, stop and
  escalate. This ExecPlan file is excluded from both counts.
- Interface: the public API changes listed in decisions D3, D4 and D6 are
  pre-approved by approving this plan. Any other change to an existing public
  signature in `whitaker_sarif` or `whitaker_clones_core` requires escalation.
- Dependencies: `num-bigint` (decision D2) is the only new dependency approved
  by this plan. Any other new crate requires escalation.
- Semantics: if any row of the classification table (decision D5) proves
  unworkable, stop and escalate. Do not patch around it.
- Proofs: if the Cauchy–Schwarz lemma (obligation P4) is not discharged
  after two focused working sessions, stop and present the options. These are
  narrowing the claim, a bounded alternative, or accepting a documented
  residual gap. Do not weaken any other obligation.
- Iterations: if a gate still fails after three fix attempts for the same
  root cause, stop and escalate.
- Time: if a milestone takes more than one working day of agent time, record
  the state in `Progress` and escalate.
- Ambiguity: if the design document and this plan disagree in a way not
  already recorded in `Decision log`, stop and present the options.

## Risks

- Risk: exact cosine arithmetic exceeds native integer widths. Histogram
  weights are `u128` values up to `10_000 * 2^63` (below 2^77). Squared norms
  reach about 2^154, and the squared comparison reaches about 2^330. Severity:
  high. Likelihood: certain. Mitigation: arbitrary-precision unsigned integers
  from `num-bigint` (decision D2). The proof model uses unbounded mathematical
  integers, so the runtime and the model agree without overflow side conditions.
- Risk: the covering-node expansion in `lower_span` makes two fragments
  compare enclosing context rather than the fragment itself, so hashes differ
  for code that is really rename-equivalent. Severity: medium. Likelihood:
  medium. Mitigation: the positive-evidence rule in decision D5. The AST pass
  never weakens or removes token evidence; it reclassifies only when cosine
  meets the threshold.
- Risk: the Verus Cauchy–Schwarz proof (P4) over sequences is substantial
  proof engineering. Severity: medium. Likelihood: medium. Mitigation: it has
  its own milestone step and tolerance. Property tests cover the same invariant
  from the start of Milestone 2, so runtime confidence does not wait on the
  proof.
- Risk: `num-bigint` 0.5 has no `sqrt` method with the behaviour this plan
  assumes, or a different signature. Severity: low. Likelihood: low.
  Mitigation: docs.rs for 0.5.1 lists `pub fn sqrt(&self) -> Self` on
  `BigUint`. A red unit test pins the floor behaviour (`m^2 <= x < (m+1)^2`)
  before any use. If the method is missing, implement an integer Newton square
  root over `BigUint` with the same test.
- Risk: ADR number collision. ADR 0013 already exists on the unmerged remote
  branch `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`.
  Severity: low. Likelihood: medium. Mitigation: this plan allocates ADR 0014.
  Re-fetch all remote branches and rescan `docs/adrs/` before creating the file.
- Risk: `make test` passes but `make lint` fails on Clippy lints that rustc
  does not report, such as `shadow_reuse`, `too_many_arguments` or float
  arithmetic restrictions. Severity: low. Likelihood: medium. Mitigation: run
  `make lint` at every milestone, not only at the end. Keep public functions at
  four or fewer parameters by bundling inputs into `RefinementInput`.
- Risk: the shared-helper extraction from `run0/emit.rs` changes Run 0 output
  by accident. Severity: medium. Likelihood: low. Mitigation: the extraction is
  a separate refactor commit with no behaviour change. The existing Run 0 unit,
  behaviour and SARIF tests must pass unchanged before and after it.

## Progress

- [x] (2026-11-11) Renamed the branch to
  `7-3-2-score-type-3-similarity-and-update-sarif-run-1`.
- [x] (2026-11-11) Reconnaissance: AST surface, SARIF crate, verification
  tooling, documentation, and prior execplans (Wyvern team).
- [x] (2026-11-11) External research: SARIF 2.1.0 `baselineState`,
  `partialFingerprints` and property-bag rules; `num-bigint` 0.5.1 API; DECKARD
  prior art for AST characteristic vectors.
- [x] (2026-11-11) Drafted this ExecPlan.
- [ ] Expert-panel design review and revision.
- [ ] User approval.
- [ ] Milestone 0: baseline and preconditions.
- [ ] Milestone 1: SARIF model support (`BaselineState`, optional cosine,
  fingerprint key constants).
- [ ] Milestone 2: exact cosine kernel.
- [ ] Milestone 3: Verus proofs for the cosine kernel model.
- [ ] Milestone 4: fragment spans, shared result helpers, refinement and
  classification.
- [ ] Milestone 5: Run 1 emission, behavioural, snapshot and end-to-end
  tests.
- [ ] Milestone 6: documentation, ADR, roadmap, and final gates.

## Surprises & discoveries

- Observation: `KindWeight` is a `u128` fixed-point value at scale 2^63, and
  `MAX_AST_NODES` is 10,000. Evidence:
  `crates/whitaker_clones_core/src/ast/features.rs:55-60` and
  `src/ast/lowering_tests.rs`. Impact: native `u128` arithmetic cannot hold the
  dot product, let alone the squared comparison. This drives decision D2.
- Observation: `whitaker_sarif::deduplicate_results` keeps the first result
  for each key, and `merge_runs` concatenates runs in argument order. Evidence:
  `crates/whitaker_sarif/src/merge.rs:105-146`. Impact: Run 1 is emitted as a
  self-contained refined run rather than as an in-place edit of Run 0, and
  reclassified results keep their Run 0 identity (decision D7).
- Observation: Run 0 regions come from the first retained fingerprint range,
  not from the whole fragment, and `TokenFragment` does not record its byte
  extent. Evidence: design §Implementation decisions (7.2.3) item 6 and
  `crates/whitaker_clones_core/src/run0/types.rs`. Impact: Run 1 cannot recover
  the fragment span from Run 0 data. This drives decision D4.
- Observation: a Verus proof for a fixed cosine threshold (`1/5`) already
  exists. Evidence: `verus/decomposition_cosine_threshold.rs` (roadmap 6.4.3).
  Impact: the lemma pattern can be generalized to a rational threshold `n/d`.
  The runtime helper `cosine_threshold_met` in `common` cannot be reused: it is
  `pub(crate)` in another crate, specialized to `u64` weights, and limited to
  `u128` products.

## Decision log

Decisions D1 to D9 are proposals for approval. Each one names its affected
design-document section.

- Decision D1: put Type-3 scoring and Run 1 emission in a new
  `crates/whitaker_clones_core/src/run1/` module subtree that mirrors `run0/`.
  It has an accept-style function (`refine_candidate_pairs`) and an emit
  function (`emit_run1`). Rationale: this keeps the established 7.2.3 split
  between scoring and SARIF emission, respects the 400-line limit, and keeps
  `whitaker_sarif` free of clone-detector policy. Affects: design §Crate
  responsibilities (`score` module). Date/Author: 2026-11-11, planning agent.
- Decision D2: compute cosine acceptance exactly over arbitrary-precision
  unsigned integers (`num_bigint::BigUint`, caret requirement `"0.5.1"`). The
  test for `cos(a, b) >= n/d` is `d^2 * dot^2 >= n^2 * |a|^2 * |b|^2`,
  evaluated only when both squared norms are positive. Rationale: the operands
  exceed `u128` (see Surprises). Floats would make borderline decisions depend
  on rounding. An in-house fixed-width 512-bit integer would add about 200
  lines of arithmetic that would need its own verification. `num-bigint` is a
  widely used rust-num crate under MIT OR Apache-2.0, with MSRV 1.60 and no
  unsafe code in the arithmetic paths used here. The `BigUint` type stays
  private to the kernel and is never exposed in a public signature.
  Alternatives rejected: `f64` (not exact, and not provable in Verus); `ruint`
  or `crypto-bigint` (fixed widths still need a bound argument); changing the
  depth weights to fit `u128` (alters the 7.3.1 design contract). Date/Author:
  2026-11-11, planning agent. Requires approval: yes (new dependency).
- Decision D3: change `whitaker_sarif::WhitakerProperties::cosine` from
  `f64` to `Option<f64>`, serialized with
  `skip_serializing_if = "Option::is_none"`. The builder method becomes
  `with_cosine(f64)`, setting `Some`. Run 0 stops writing the `0.0`
  placeholder. Run 1 omits `cosine` only when a token-accepted pair could not
  be lowered. Rationale: `0.0` is a valid cosine, so a placeholder of `0.0` is
  indistinguishable from "structurally unrelated". Both crates are pre-1.0 and
  no consumer reads `cosine` yet (the lint is 7.5.x), so no compatibility layer
  is needed. Affects: design §Result mapping, users' guide SARIF description.
  Date/Author: 2026-11-11, planning agent. Requires approval: yes (public API
  and wire change).
- Decision D4: `TokenFragment` gains an explicit fragment byte span. Its
  constructor becomes
  `TokenFragment::new(id, profile, location: FragmentLocation, source_text)`.
  Here `FragmentLocation { file_uri: String, span: Range<usize> }` keeps the
  constructor at four parameters. Run 1 lowers exactly this span. Rationale:
  the design defines `FragmentId` over `file | start | end | profile`, so the
  span is part of a fragment's identity. The alternative, deriving a span from
  the hull of retained fingerprints, loses up to `window - 1` tokens at each
  edge. That can change which syntax node covers the fragment, and the hull
  differs between T1 and T2 fragments of the same code. All callers (Run 0
  tests, doctests and behavioural tests) are updated in the same commit, with
  no shim. Run 0 regions are not changed by this decision. Date/Author:
  2026-11-11, planning agent. Requires approval: yes (public API change).
- Decision D5: Run 1 classification follows a "positive-evidence" rule. The
  AST pass adds Type-3 results and refines `WHK002` to `WHK003` only when the
  AST evidence affirmatively supports a near-miss. It never removes or weakens
  a token-pass result on AST evidence alone. The complete table, implemented as
  one pure function `classify(TokenVerdict, AstVerdict) -> Option<Verdict>`, is:

  ```plaintext
  token verdict      AST verdict                              Run 1 result
  -----------------  ---------------------------------------  -------------------
  accepted T1        any                                      WHK001  unchanged
  accepted T2        lowered, hashes equal                    WHK002  unchanged
  accepted T2        lowered, hashes differ, cosine >= type3  WHK003  updated
  accepted T2        lowered, hashes differ, cosine <  type3  WHK002  unchanged
  accepted T1 or T2  lowering failed (per pair)               same rule unchanged,
                                                              no cosine, no astHash
  not accepted       lowered, cosine >= type3                 WHK003  new
  not accepted       lowered, cosine <  type3                 none
  not accepted       lowering failed (per pair)               none
  ```

  Rationale: equal canonical hashes confirm rename-equivalence, so the `WHK002`
  claim stands. Differing hashes with a cosine at or above the threshold are
  positive evidence of a near-miss, which is what the design means by "upgrade
  `WHK002` to `WHK003`". Differing hashes with a low cosine are inconclusive,
  because the covering node may include enclosing context (see Risks). Keeping
  the token verdict there avoids a false negative. `WHK001` is never
  reclassified: the design names only the `WHK002` upgrade, and a Type-1 token
  match is the strongest evidence the detector has. Alternatives rejected:
  dropping token results whose cosine is low (turns AST noise into false
  negatives, and SARIF `absent` results would complicate the lint);
  reclassifying whenever hashes differ (labels a pair `WHK003` while its cosine
  is below the Type-3 threshold, contradicting design §Scoring). Affects:
  design §Scoring and acceptance (Type-3), §SARIF update (Run 1). Recorded in
  ADR 0014. Date/Author: 2026-11-11, planning agent. Requires approval: yes
  (semantic contract consumed by 7.5.x).
- Decision D6: add a typed `whitaker_sarif::BaselineState` enum (`New`,
  `Unchanged`, `Updated`, `Absent`, serialized in lower camel case) and change
  `SarifResult::baseline_state` to `Option<BaselineState>`.
  `ResultBuilder::with_baseline_state` takes the enum. Rationale: SARIF 2.1.0
  §3.27.24 defines a closed value set, and a typo in a free string would
  silently corrupt baseline semantics. The crate is pre-1.0. Date/Author:
  2026-11-11, planning agent. Requires approval: yes (public API change).
- Decision D7: every Run 1 result carries a `baselineState`, and Run 1 uses
  Run 0 as its implicit baseline. A carried or reclassified result keeps its
  Run 0 identity: the same `whitakerFragment` partial fingerprint, the same
  primary and related locations, and the same `tokenHash`. Only `ruleId`,
  `message`, `baselineState`, `properties` and the added `astHash` change. A
  new Type-3 result's `whitakerFragment` is the SHA-256 of
  `left_id NUL right_id NUL "T3"`, matching the Run 0 construction. Its regions
  follow the Run 0 region contract (first retained fingerprint range), so all
  Run 1 locations are produced by one rule. `run.baselineGuid` is not added:
  SARIF requires `baselineState` when `baselineGuid` is present, but not the
  converse, and the run model has no GUID support. Rationale: SARIF result
  matching relies on fingerprints, and an `updated` result must be the same
  logical result as its baseline counterpart. Date/Author: 2026-11-11, planning
  agent.
- Decision D8: the `astHash` partial fingerprint is the lowercase SHA-256 hex
  digest of `left_ast_hash_hex NUL right_ast_hash_hex`, in canonical
  `CandidatePair` order. The key is the literal `astHash`, matching the design
  document and the existing unversioned `whitakerFragment` and `tokenHash`
  keys. Version drift is already handled because `AstHash` is seeded with
  `PARSER_SCHEMA_VERSION`. Rationale: one value per result, symmetric with
  `tokenHash`, and opaque as the design requires. Date/Author: 2026-11-11,
  planning agent.
- Decision D9: the optional bounded tree-edit-distance refinement in design
  §Scoring is out of scope. Rationale: the roadmap text for 7.3.2 names cosine
  and AST-hash metadata only. Tree-edit distance needs per-node spans that
  7.3.1 deliberately omitted (its open question OQ6), and it would roughly
  double this item. Proposal: during Milestone 6, add roadmap item 7.3.3, "Add
  optional bounded tree-edit-distance refinement for Type-3 pairs", requiring
  7.3.2, using the `mapsplice` skill. Date/Author: 2026-11-11, planning agent.
  Requires approval: yes (scope boundary).
- Decision D10: thresholds are validated as strictly ordered,
  `type1 > type2 > type3`, at the start of `refine_candidate_pairs`, by integer
  cross-multiplication. A violation returns `Run1Error::ThresholdOrder`. The
  default `type3` threshold is `17/20` (0.85). Rationale: design §Configuration
  states the ordering constraint, and Run 1 is the first place where all three
  thresholds meet. Date/Author: 2026-11-11, planning agent.
- Decision D11: lowering failures are classified by blast radius.
  `AstError::ParserUnavailable` aborts the whole pass with an error, because
  every pair would otherwise silently fall back to its token verdict while
  `executionSuccessful = true` claims a completed refinement. An invalid
  fragment span (`ByteSpan::new` failure) is a caller error and also aborts.
  `UnparsableSpan`, `TreeTooLarge` and `TreeTooDeep` are per-pair outcomes:
  they take the "lowering failed" rows of D5 and are counted in metrics. Each
  fragment is lowered at most once per pass, through a memo keyed by
  `FragmentId`. Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. To be completed at each milestone and at the end.

## Context and orientation

The repository is a Rust workspace. Paths below are relative to its root.

`crates/whitaker_sarif/` models SARIF 2.1.0. `SarifLog` holds `runs`. Each
`Run` holds a `tool`, `invocations`, `results` and `artefacts`. A `SarifResult`
has `rule_id`, `level`, `message`, `locations`, `related_locations`,
`partial_fingerprints` (a string map), `properties` (a JSON value) and
`baseline_state`. `WhitakerProperties` (`src/whitaker_properties.rs`) is the
typed form of the `properties.whitaker` object: `profile`, `k`, `window`,
`jaccard`, `cosine`, `groupId` and `classSize`. `src/merge.rs` defines
`WHITAKER_FRAGMENT_KEY = "whitakerFragment"`, `deduplicate_results` and
`merge_runs`. `src/rules.rs` already defines `WHK001`, `WHK002` and `WHK003`.
`src/paths.rs` names `clones.token.sarif`, `clones.ast.sarif` and
`clones.refined.sarif`.

`crates/whitaker_clones_core/` holds the detector engines:

- `src/token/` normalizes Rust tokens under profile `T1` or `T2` and produces
  `Fingerprint { hash, range }` values.
- `src/index/` builds MinHash sketches and locality-sensitive hashing (LSH)
  buckets. It emits `CandidatePair { left, right }` in canonical `FragmentId`
  order with self-pairs removed.
- `src/run0/` turns candidate pairs into `AcceptedPair` values by Jaccard
  similarity (`accept_candidate_pairs`) and emits Run 0 (`emit_run0`). Jaccard
  thresholds are rational (`SimilarityThreshold`) and are compared by integer
  cross-multiplication. `SimilarityRatio` prints six decimals using integer
  arithmetic.
- `src/ast/` is the 7.3.1 AST engine. `lower_span(file_text, ByteSpan)`
  returns a `NormalizedTree`. `kind_histogram(&tree)` returns a
  `KindHistogram`, a sorted map from `KindId` to a `KindWeight(u128)` whose
  value is `sum over depths of count * 2^63 >> depth`. `canonical_hash(&tree)`
  returns an opaque `AstHash` with `to_hex()`. `src/ast/metrics.rs` owns the
  `whitaker_clones.ast.*` metric names.

Terms used in this plan:

- Cosine similarity of two non-negative vectors `a` and `b` is
  `dot(a, b) / (|a| * |b|)`, where `dot` is the sum of element-wise products and
  `|a|` is the square root of `dot(a, a)`. For non-negative vectors it lies in
  `[0, 1]`.
- Squared norm `A = dot(a, a)`. The exact acceptance test avoids square roots
  by comparing squares.
- A "carried" result is a Run 0 result reproduced in Run 1 with
  `baselineState = "unchanged"`. A "reclassified" result is a Run 0 `WHK002`
  result reproduced as `WHK003` with `baselineState = "updated"`. A "new"
  result is a Type-3 pair that Run 0 did not accept.
- "Micros" is the cosine floored to six decimal places and expressed as an
  integer in `0..=1_000_000`.

Relevant documentation and skills to read before starting:

- `docs/whitaker-clone-detector-design.md`: §SARIF schema and mapping, §Pass B,
  §Scoring and acceptance (Type-3), §SARIF update (Run 1), and §Implementation
  decisions (7.2.3) and (7.3.1).
- `docs/whitaker-dylint-suite-design.md`, for suite-wide conventions.
- `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`,
  for sidecar proofs and the Verus versus Kani split.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
  and `docs/rust-doctest-dry-guide.md`, for test and doctest conventions.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`, before writing
  the classification and emission code.
- `docs/developers-guide.md` §Proof workflows, §Clone-detector AST structure,
  §Verus scope and trust boundary, and §Shared SARIF test JSON helpers.
- `docs/execplans/7-2-3-emit-sarif-run-0-for-accepted-type-1-and-type-2-pairs.md`
  and
  `docs/execplans/7-3-1-map-candidate-spans-and-extract-ast-feature-vectors.md`.
- `docs/execplans/6-4-3-verus-proofs-for-cosine-threshold-met-invariants.md`
  and `verus/decomposition_cosine_threshold.rs`, the cosine proof precedent.
- Skills: `rust-router` first, then `rust-types-and-apis` (public surface),
  `rust-errors` (`Run1Error`), `rust-unit-testing` (rstest, googletest,
  pretty_assertions, insta), `proptest`, `verus`, `rust-verification`,
  `arch-decision-records` (ADR 0014), `mapsplice` (roadmap edit),
  `codegraph-mcp` (navigation and impact), `en-gb-oxendict-style`
  (documentation), and `execplans` (maintaining this file).

## Conformance basis

Upstream artefacts:

- Roadmap: `docs/roadmap.md` item 7.3.2 (`RM-7.3.2`) at the commit this plan
  is based on (`d9eb8a09`).
- Technical design: `docs/whitaker-clone-detector-design.md` at `d9eb8a09`.
  The traced elements are:
  - `TDD-SCORE-T3`: §Scoring and acceptance (Type-3), "compute cosine
    similarity between histograms; if cosine is at least `type3_threshold`,
    mark as a Type-3 candidate".
  - `TDD-RUN1-UPDATE`: §SARIF update (Run 1), "add or merge a result under
    `WHK003`, or upgrade `WHK002` to `WHK003` with `baselineState =
    "updated"`; record `properties.whitaker.cosine` and the `astHash` partial
    fingerprint".
  - `TDD-RUNS`: §Runs, "Run 1: AST pass (producer `whitaker_clones_cli@ast`,
    `invocations[0].executionSuccessful = true` if completed)".
  - `TDD-METRICS-FV`: §AST operational metrics, "a vector is counted only when
    [the 7.3.2] boundary consumes it".
  - `TDD-CONFIG-ORDER`: §Configuration, "must satisfy: type1 > type2 > type3".
  - `TDD-ACCEPT-B`: §Acceptance criteria, "Pass B SARIF upgrades or adds T3
    results; cosine and Jaccard values are present".
- ADR 0003 (formal proof strategy), accepted.
- There are no Terms of Reference for the clone detector. The roadmap and the
  design document are the governing artefacts.
- Standards: SARIF 2.1.0 Errata 01 (OASIS), §3.27.17 `partialFingerprints`,
  §3.27.24 `baselineState`, §3.8 property bags, §3.20.14 `executionSuccessful`.

Trace links:

```plaintext
RM-7.3.2 -> TDD-SCORE-T3      -> EP-M2, EP-M3, EP-M4 -> cosine kernel tests, P1-P4, classify table
RM-7.3.2 -> TDD-RUN1-UPDATE   -> EP-M1, EP-M4, EP-M5 -> run1_sarif.feature, run1 snapshot
RM-7.3.2 -> TDD-RUNS          -> EP-M5               -> run1 invocation test, end-to-end pipeline test
RM-7.3.2 -> TDD-METRICS-FV    -> EP-M4               -> run1 metrics tests
RM-7.3.2 -> TDD-CONFIG-ORDER  -> EP-M4               -> threshold-order tests
RM-7.3.2 -> TDD-ACCEPT-B      -> EP-M5               -> end-to-end pipeline test
```

Deviations from the design text that this plan proposes, each recorded in
`Decision log` and to be written back into the design document in Milestone 6:
D3 (`cosine` optional), D5 (low-cosine `WHK002` pairs stay `WHK002`; `WHK001`
never reclassified), and D9 (tree-edit refinement deferred).

## Verification plan

The work introduces one piece of contractual arithmetic (the exact cosine
acceptance test and its printed score) and one finite decision table, together
with integration plumbing. Verus is warranted for the arithmetic. It is not
warranted for the decision table, SARIF assembly or metrics, where exhaustive
tables, property tests, snapshots and behavioural tests give the right level of
assurance.

### Abstract model

Let `a` and `b` be finite maps from kind identifiers to non-negative integers,
standing for the two histograms. Absent keys mean zero. Define:

```plaintext
dot(a, b) = sum over k of a[k] * b[k]
A = dot(a, a),  B = dot(b, b)
cos(a, b) = dot(a, b) / (sqrt(A) * sqrt(B))        defined when A > 0 and B > 0
accept(a, b, n, d)  <=>  A > 0  and  B > 0  and  d^2 * dot^2 >= n^2 * A * B
micros(a, b) = isqrt( floor( 10^12 * dot^2 / (A * B) ) )
```

Here `n/d` is a rational threshold with `0 < n <= d`, and `isqrt(x)` is the
greatest integer `m` with `m^2 <= x`. The runtime kernel
`run1::cosine::CosineScore` computes `dot`, `A` and `B` over `BigUint` and
evaluates `accept` and `micros` with exactly these expressions.

Representation relation: the runtime `KindHistogram` is a sparse `BTreeMap`.
The runtime `dot` sums products over the intersection of the two key sets. This
equals the model's sum over the union, because absent keys contribute zero.

Preconditions: weights are non-negative, which `u128` guarantees. Both squared
norms are positive, which every lowered tree satisfies because its root
contributes weight `2^63`. The kernel still returns `None` for a zero norm
rather than relying on that.

### Obligations

- Obligation P1 (`lemma_threshold_cross_multiplication`): for integers
  `dot >= 0`, `A > 0`, `B > 0`, `n >= 0`, `d > 0` and reals `la > 0`, `lb > 0`
  with `la^2 = A` and `lb^2 = B`: `d^2 * dot^2 >= n^2 * A * B` holds exactly
  when `dot / (la * lb) >= n / d`. This generalizes the 6.4.3 lemma from `1/5`
  to any non-negative rational. Sketch: both sides of `dot / (la lb) >= n/d`
  are non-negative, squaring is monotone on non-negative reals, and multiplying
  through by the positive `d^2 la^2 lb^2` gives the integer form. Supporting
  hypothesis: `lemma_square_monotone_nonneg(x, y)`, stating `x >= y` holds
  exactly when `x^2 >= y^2` for `x, y >= 0`. Method: Verus. Artefact:
  `verus/clone_detector_type3_cosine.rs`. Evidence: `make verus-clone-detector`
  reports the lemma verified. Non-vacuity: the file contains a witness lemma
  showing that `dot = 17`, `A = B = 20^2`, `n = 17`, `d = 20` satisfies every
  precondition and is accepted at the exact boundary, and that `dot = 16` with
  the same norms is rejected. A seeded fault (changing `>=` to `>` in the spec
  function) must make the boundary witness fail to verify. Record that run in
  `Artefacts and notes`.
- Obligation P2 (`lemma_micros_brackets_cosine`): with `x = 10^12 * dot^2`,
  `y = A * B > 0`, `q = floor(x / y)` and `m = isqrt(q)`, both `m^2 * y <= x`
  and `x < (m + 1)^2 * y` hold. In words, `m / 10^6` is the six-decimal floor
  of the true cosine. Sketch: `m^2 <= q` gives `m^2 y <= q y <= x`.
  `(m+1)^2 > q` gives `(m+1)^2 >= q + 1`, so `(m+1)^2 y >= (q+1) y > x`. The
  specification of `isqrt` (`m^2 <= q` and `q < (m+1)^2`) is a hypothesis about
  the runtime `BigUint::sqrt`; Verus receives it as a precondition, not an
  axiom. Method: Verus, integer-only nonlinear arithmetic. Artefact: the same
  file. Evidence: lemma verified. Non-vacuity: a witness with `dot = 3`,
  `A = 9`, `B = 2`. The cosine is `1/sqrt(2)`, so `m = 707106`, and the lemma
  instantiates with that `m`. Runtime tests assert the same witness.
- Obligation P3 (`lemma_printed_score_agrees_with_acceptance`): if the
  threshold is a six-decimal value, `n/d = t / 10^6` for an integer `t`, then
  `accept(a, b, n, d)` holds exactly when `micros(a, b) >= t`. Sketch: P1
  rewrites acceptance as `10^12 dot^2 >= t^2 A B`. If `m >= t`, then
  `t^2 y <= m^2 y <= x` by P2's lower bound. If acceptance holds and `m < t`,
  then `(m+1)^2 y <= t^2 y <= x`, contradicting P2's upper bound. For
  thresholds that are not six-decimal values, only the forward direction holds:
  acceptance implies `micros >= floor(10^6 n / d)`. The lemma states that
  direction separately. Method: Verus. Non-vacuity: witnesses at `t = 850000`
  on both sides of the boundary (`micros = 850000` accepted, `micros = 849999`
  rejected). A negative control drops the six-decimal hypothesis and shows a
  counterexample with `n/d = 1/3`. In that case `micros` can equal
  `floor(10^6 / 3) = 333333` while acceptance fails. The control is a runtime
  unit test, because Verus cannot "expect failure".
- Obligation P4 (`lemma_cauchy_schwarz_nonneg`): for sequences `a` and `b` of
  natural numbers of equal length, `dot(a, b)^2 <= dot(a, a) * dot(b, b)`.
  Consequence: `micros <= 10^6`, so the printed cosine is in
  `[0.000000, 1.000000]` and the `f64` parsed from it is finite and at most
  `1.0`. Sketch (a hypothesis to be refined during implementation): prove the
  Lagrange identity `A B - dot^2 = sum over i < j of (a_i b_j - a_j b_i)^2` by
  induction on the sequence length, then conclude non-negativity. Alternative
  decomposition if the double sum is awkward: an inductive step that uses
  `2 a_n b_n S_ab <= a_n^2 S_bb + b_n^2 S_aa`, itself derived from the
  induction hypothesis and AM-GM. Method: Verus. Escalation: see Tolerances.
  Non-vacuity: a witness with `a = b` (equality case) and one with orthogonal
  supports (`dot = 0`). A runtime property test checks the same inequality over
  generated histograms.
- Obligation R1 (representation): the sparse runtime `dot` and norms equal the
  dense model over the union of keys. Method: `proptest` against an independent
  dense oracle that builds `Vec<u128>` over the sorted key union and computes in
  `BigUint`. Domain: up to 24 kinds per side, weights drawn from `0..=2^77`
  including the exact extremes, and overlapping, disjoint and identical key
  sets classified by a `prop_oneof!` tag. Artefact:
  `crates/whitaker_clones_core/src/run1/cosine_tests.rs`. Non-vacuity: a seeded
  mutation (summing over the union with a missing key treated as `1`) must fail
  the property. Record the run in `Artefacts and notes`.
- Obligation R2 (runtime agreement with P1 to P4): `CosineScore::meets` and
  `CosineScore::micros` satisfy the model on generated inputs. Specifically:
  `meets` agrees with the exact `BigUint` cross-multiplication; `micros`
  satisfies the P2 brackets; `micros <= 10^6` (P4); symmetry
  `cos(a, b) = cos(b, a)`; and scale invariance `cos(k a, b) = cos(a, b)` for
  integer `k >= 1`. Method: `proptest`, plus `rstest` boundary cases (exact
  boundary accepted, one below rejected, identical vectors give `1.000000`,
  disjoint supports give `0.000000`, zero norm gives `None`). Non-vacuity: the
  boundary rows are constructed so that `d^2 dot^2 = n^2 A B` exactly.
- Obligation C1 (classification table): `classify` returns exactly the table
  in decision D5 for every input. Method: an exhaustive `rstest` table over all
  3 token verdicts times 5 AST verdicts (lowered with hashes equal or
  differing, times cosine meeting or not, plus lowering failed). That is 15
  rows, with each case named. Exhaustive enumeration is a complete proof for a
  finite domain, so neither Kani nor Verus adds assurance. Non-vacuity: the
  verdict enums are matched without wildcards, so a new variant fails to
  compile until the table grows.
- Obligation C2 (threshold order): `validate_threshold_order` accepts exactly
  the strictly decreasing triples. Method: `proptest` against an oracle using
  `u128` cross-multiplication over numerators and denominators in `1..=10^6`,
  plus `rstest` rows for equal neighbours (rejected) and the defaults
  (accepted).
- Obligation E1 (Run 1 covers Run 0): for every Run 0 result there is exactly
  one Run 1 result with the same `whitakerFragment`, primary location and
  related locations, whose `ruleId` is the same or is `WHK003` with
  `baselineState = "updated"`. Method: `proptest` over generated fragment sets
  built from a small corpus of Rust snippets, and the end-to-end test.
- Obligation E2 (determinism): `emit_run1` output is byte-identical under
  permutation and duplication of the candidate and accepted-pair inputs. Method:
  `proptest` with shuffled and duplicated inputs, comparing serialized JSON.
- Obligation E3 (wire shape): Run 1 has tool name `whitaker_clones_cli@ast`
  (caller-supplied), one invocation with `executionSuccessful = true`, all
  three rules, and results ordered by `(ruleId, whitakerFragment, message)`.
  Every result carries `baselineState`. Method: `insta` JSON snapshot of a
  fixture with one result of each classification, plus targeted unit assertions.

### Axioms

- `num_bigint::BigUint` implements exact unsigned arithmetic (`+`, `*`, `/`,
  `Ord`). `BigUint::sqrt` returns the floor square root. The second axiom is
  exercised by a runtime unit test and a property test against the bracket
  `m^2 <= x < (m+1)^2`.
- `KindHistogram` weights are as specified by 7.3.1. Sums cannot overflow
  `u128` because counts are at most `u32::MAX` and at most 64 depths carry
  non-zero weight, so every weight is below `2^101`. This is existing,
  separately tested behaviour.
- Rust `str::parse::<f64>` returns the correctly rounded value for a
  six-decimal string. This is the existing 7.2.3 quarantine axiom.
- The `metrics` facade delivers counters to the installed recorder. Tests use
  `metrics_util::debugging::DebuggingRecorder`, as 7.3.1 does.

### Trust boundary

Verus proves properties of the integer model, not of the compiled Rust. The
runtime ties itself to the model in two ways. First, the kernel's two
expressions are written exactly as the model's spec functions, and a comment
cross-references the proof file. Second, runtime unit tests assert the same
witnesses that the Verus file proves. This follows ADR 0003 and the precedent in
`verus/decomposition_cosine_threshold.rs`. Kani is not used: the kernel's
`BigUint` arithmetic is intractable for bounded model checking, and the
classification table is finite and exhaustively tested.

## Plan of work

### Stage A: understand and propose (this document)

No code changes. Ends when the user approves this plan, including decisions D2
to D6 and D9.

### Stage B: red tests and failing proof obligations

Each milestone starts with its failing tests. Milestone 3 starts with lemma
statements whose bodies are empty, so Verus reports them unproved.

### Stage C: implementation and proofs together

The cosine kernel and its Verus model are written in adjacent milestones so
that the kernel's expressions copy the spec functions directly.

### Stage D: refactor, documentation and gates

Documentation, the ADR, the roadmap update and the final gates.

## Milestones and plateaus

### EP-M0: baseline and preconditions

Confirm that `make check-fmt`, `make lint` and `make test` pass on the
unmodified branch and record their timings. Re-fetch remote branches and
confirm that ADR 0014 is free. Confirm that `num-bigint` 0.5.1 resolves and that
`BigUint::sqrt` exists, using a scratch crate under `/tmp`, not the workspace.
No commit is needed unless the branch requires a fix.

- Requirements and gaps: none; establishes the baseline.
- Acceptance evidence: three green gate logs under `/tmp`.
- Conformance check: baseline is green; no upstream assumption falsified.
- Recovery: if a baseline gate fails, stop and report. Do not fix unrelated
  failures on this branch.
- Remaining gaps: everything.
- Compatibility decision: none.

### EP-M1: SARIF model support

In `crates/whitaker_sarif`:

1. Add `src/baseline_state.rs` with
   `pub enum BaselineState { New, Unchanged, Updated, Absent }`. It derives
   `Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize` with
   `#[serde(rename_all = "camelCase")]`. Re-export it from `lib.rs`.
2. Change `SarifResult::baseline_state` to `Option<BaselineState>` and
   `ResultBuilder::with_baseline_state(state: BaselineState)`.
3. Change `WhitakerProperties::cosine` to `Option<f64>` with
   `#[serde(default, skip_serializing_if = "Option::is_none")]`.
   `WhitakerPropertiesBuilder::with_cosine(f64)` sets `Some`, and `build` still
   rejects non-finite values.
4. Add `pub const TOKEN_HASH_KEY: &str = "tokenHash";` and
   `pub const AST_HASH_KEY: &str = "astHash";` beside `WHITAKER_FRAGMENT_KEY` in
   `src/merge.rs`, re-exported from `lib.rs`.

In `crates/whitaker_clones_core/src/run0/emit.rs`, remove `.with_cosine(0.0)`
and use `TOKEN_HASH_KEY`.

Red: new tests in `crates/whitaker_sarif` for the round trip of all four
baseline states (`"new"`, `"unchanged"`, `"updated"`, `"absent"`), rejection of
an unknown baseline string, and the absence of `cosine` when unset. A Run 0
test asserts that `properties.whitaker` has no `cosine` key. Green: the model
changes. Then update every existing test that constructed a string baseline
state or read `cosine` as `f64`.

- Requirements and gaps: `TDD-RUN1-UPDATE` (typed `updated`), D3, D6.
- Acceptance evidence: `cargo nextest run -p whitaker_sarif` and
  `cargo nextest run -p whitaker_clones_core run0` pass; all three gates pass.
- Conformance check:
  - D3 and D6 implemented exactly; no other public signature changed.
  - Run 0 output differs from before only by the absent `cosine` key.
  - No new dependency.
- Recovery: revert the single milestone commit.
- Remaining gaps: Run 1 itself.
- Compatibility decision: none. Both crates are pre-1.0 and no consumer of
  `cosine` or `baselineState` exists yet.

### EP-M2: exact cosine kernel

Add `num-bigint = "0.5.1"` to `[workspace.dependencies]` in the root
`Cargo.toml`, and `num-bigint = { workspace = true }` to
`crates/whitaker_clones_core/Cargo.toml`. Add `googletest` and
`pretty_assertions` (already workspace dependencies) to the crate's
dev-dependencies.

Create `crates/whitaker_clones_core/src/run1/mod.rs`, which declares the
submodules and re-exports the public surface, and
`crates/whitaker_clones_core/src/run1/cosine.rs`, which contains:

```rust
/// Exact cosine similarity between two depth-weighted kind histograms.
pub struct CosineScore {
    dot: BigUint,
    left_norm_squared: BigUint,
    right_norm_squared: BigUint,
}

impl CosineScore {
    /// Returns `None` when either histogram has a zero norm.
    pub fn between(left: &KindHistogram, right: &KindHistogram) -> Option<Self>;
    /// Returns whether cosine is at least `threshold`, by exact integer test.
    pub fn meets(&self, threshold: SimilarityThreshold) -> bool;
    /// Returns floor(cosine * 10^6), always within 0..=1_000_000.
    pub fn micros(&self) -> u32;
    /// Returns the six-decimal rendering, for example `"0.707106"`.
    pub fn as_decimal_string(&self) -> String;
}
```

`micros` converts with `u32::try_from` and returns `1_000_000` when the
conversion fails. P4 makes that branch unreachable, and a property test asserts
it is never taken. This avoids a panic path without hiding a violation, because
the test would fail first. The kernel lives under `run1/` rather than `ast/`, so
`ast/` stays parser-agnostic and policy-free, and `tests/ast_boundary.rs` is
unaffected.

Red: `crates/whitaker_clones_core/src/run1/cosine_tests.rs` with the R1 and R2
cases, failing because `CosineScore` does not exist yet. Then green, then
refactor.

- Requirements and gaps: `TDD-SCORE-T3` (scoring), D2.
- Acceptance evidence: the R1 and R2 tests pass; the seeded mutation for R1 is
  observed failing; gates pass.
- Conformance check: `BigUint` does not appear in any public signature
  (`cargo doc` shows `CosineScore` with private fields only); no float in the
  kernel.
- Recovery: revert the milestone commit; the dependency addition is in the
  same commit.
- Remaining gaps: proofs, classification and emission.
- Compatibility decision: none.

### EP-M3: Verus proofs for the cosine model

Create `verus/clone_detector_type3_cosine.rs` with spec functions `spec_dot`
(over `Seq<nat>`), `spec_accept(dot, a, b, n, d)` and
`spec_micros_brackets(m, dot, a, b)`, and lemmas P1 to P4 with their witness
lemmas. Register the file in the `clone-detector` and `all` groups of
`scripts/run-verus.sh`. Add a cross-reference comment above
`CosineScore::meets` and `CosineScore::micros`.

Red: lemma statements with empty bodies. `make verus-clone-detector` reports
them unproved. Green: supply the proofs. Run the seeded-fault check (P1) and
record it. P4 is attempted last and is subject to its tolerance.

- Requirements and gaps: `TDD-SCORE-T3` (exactness of acceptance), and the
  printed `cosine` consistency.
- Acceptance evidence: `make verus-clone-detector` exits 0 with every lemma
  verified.
  `rg -n 'assume|admit|external_body' verus/clone_detector_type3_cosine.rs`
  prints nothing. All three gates pass.
- Conformance check: proofs follow ADR 0003 (sidecar, explicit trust
  boundary); runtime expressions match the spec functions.
- Recovery: the proof file is independent of the crate build; revert it alone.
- Remaining gaps: classification and emission.
- Compatibility decision: none.

### EP-M4: fragment spans, shared helpers, refinement and classification

This milestone has three commits.

Commit 1 (functional API change, decision D4). Add
`pub struct FragmentLocation { file_uri: String, span: Range<usize> }` with
`new(file_uri, span)` and accessors, and change
`TokenFragment::new(id, profile, location, source_text)`. Add
`TokenFragment::span()`. Update every caller: `run0/test_helpers.rs`, the
`run0` tests, doctests, and `tests/run0_sarif_behaviour.rs`. Run 0 behaviour is
unchanged; its tests pass unmodified apart from construction.

Commit 2 (refactor, no behaviour change). Move the pair-result helpers out of
`run0/emit.rs` into `crates/whitaker_clones_core/src/pair_result.rs`. These are
`pair_fingerprint`, `token_hash`, location construction, message formatting and
`score_to_f64`. All are `pub(crate)`. Run 0 output must be byte-identical; the
Run 0 tests, behaviour suite and snapshots pass unchanged. Document the
module's re-use policy in `docs/developers-guide.md`: it is owned by the
clone-detector emitters, its only permitted callers are `run0` and `run1`, and
it composes `whitaker_sarif` builders without adding policy. The repository
sweep for an existing equivalent is already done; `rg` found only the `run0`
private helpers.

Commit 3 (functional). Add, under `run1/`:

- `types.rs`: `AstPassConfig`, `RefinementInput`, `CloneRule`,
  `Refinement`, `AstEvidence`, `RefinedPair` (see
  `Interfaces and dependencies`).
- `classify.rs`: the verdict enums and `classify` (decision D5), plus
  `validate_threshold_order` (decision D10).
- `refine.rs`: `refine_candidate_pairs` and the `pub(crate)` seam
  `refine_with_lowering`, which takes the lowering function as a parameter so
  that tests can inject `UnparsableSpan`, `TreeTooLarge` and
  `ParserUnavailable`. It includes the per-pass lowering memo (decision D11).
- `metrics.rs`: `whitaker_clones.ast.feature_vector.total` (incremented once
  per fragment whose histogram and hash are consumed) and
  `whitaker_clones.run1.pair.total` with the bounded label `classification` in
  `{unchanged, updated, new, rejected, ast_unavailable}`.
- `error.rs`: `Run1Error` (see `Interfaces and dependencies`).

Red: the C1 table, the C2 property, refinement unit tests (memo lowers each
fragment once, `ParserUnavailable` aborts, an invalid span aborts, a per-pair
failure carries the token verdict), and metrics tests with `DebuggingRecorder`.

- Requirements and gaps: `TDD-SCORE-T3`, `TDD-METRICS-FV`,
  `TDD-CONFIG-ORDER`, D4, D5, D10, D11.
- Acceptance evidence: tests listed above pass; gates pass after each commit.
- Conformance check:
  - The D5 table matches the code exactly (C1).
  - No `ra_ap_syntax` import outside `lowering.rs` (`ast_boundary` passes).
  - `lower_span` metrics are not duplicated.
- Recovery: each commit reverts independently, from the last commit back.
- Remaining gaps: SARIF emission.
- Compatibility decision: none (`TokenFragment` is pre-1.0 and has no
  external consumer).

### EP-M5: Run 1 emission and system-level tests

Add `run1/emit.rs` with `emit_run1`. It builds one `SarifResult` per
`RefinedPair` using `pair_result` helpers and applies decision D7 for identity
and D8 for `astHash`. It sets `baselineState`, `properties.whitaker` with
`profile` (`T1`, `T2` or `T3`), `k`, `window`, `jaccard` and `cosine`, then
sorts, deduplicates and wraps everything in a `Run` with all three rules and
one invocation with `executionSuccessful = true`. Re-export the public surface
from `lib.rs`.

Add tests:

- `crates/whitaker_clones_core/tests/features/run1_sarif.feature` and
  `crates/whitaker_clones_core/tests/run1_sarif_behaviour.rs` (see
  `Validation and acceptance` for the feature text).
- `crates/whitaker_clones_core/src/run1/emit_tests.rs`, with the E3 snapshot
  (`insta::assert_json_snapshot!`) and the E1 and E2 properties.
- `crates/whitaker_clones_core/tests/run1_pipeline.rs`, the end-to-end test.
  It starts from three Rust source snippets on disk in a `tempfile` directory,
  read through `cap_std`. It runs normalization, fingerprinting, MinHash and
  LSH, `accept_candidate_pairs`, `emit_run0`, `refine_candidate_pairs` and
  `emit_run1`. It builds a `SarifLog` with `runs = [run0, run1]`, serializes
  it, parses it back, and asserts that `runs[1]` contains a `WHK003` result with
  `cosine` and `astHash` present and `jaccard` present.

- Requirements and gaps: `TDD-RUN1-UPDATE`, `TDD-RUNS`, `TDD-ACCEPT-B`, D7,
  D8.
- Acceptance evidence: the behaviour suite, snapshot, properties and
  end-to-end test pass; gates pass.
- Conformance check: Run 1 identity rules (D7) are verified by E1; the wire
  shape is verified by the snapshot; no new dependency.
- Recovery: revert the milestone commit.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation, ADR, roadmap and final gates

- `docs/adrs/adr-0014-type-3-refinement-semantics-for-sarif-run-1.md`
  records decisions D2, D5 and D7 in the style guide's ADR format, with status
  Accepted and the date of approval. Index it in `docs/contents.md`.
- `docs/whitaker-clone-detector-design.md`: add "Implementation decisions
  (7.3.2)" covering D1 to D11. Update §Result mapping (`cosine` optional and
  `baselineState` present in Run 1), §Scoring and acceptance (exact test and
  the D5 table, with a link to ADR 0014), and §SARIF update (Run 1). Record the
  proof scope (P1 to P4) and its trust boundary.
- `docs/users-guide.md`: replace the "Not yet emitted" paragraph in the
  clone-detection section with a description of Run 1: what `WHK003` means, how
  `baselineState` and `cosine` read, and the default `type3` threshold.
- `docs/developers-guide.md`: the `run1` module structure, the `pair_result`
  re-use policy, the new metric names, and the Verus file under §Verus scope
  and trust boundary.
- `docs/roadmap.md`: mark 7.3.2 done with an evidence note. Add 7.3.3 per
  decision D9 using `mapsplice`, if approved.
- Run `make fmt`, `make markdownlint`, `make nixie` (if diagrams were
  touched), `make spelling`, `make check-fmt`, `make lint`, `make test` and
  `make verus-clone-detector`.

- Acceptance evidence: all gates green; the documentation reflects the
  shipped behaviour.
- Conformance check: every deviation in `Conformance basis` is written back
  to the design document and the ADR.
- Recovery: documentation commits revert independently.
- Remaining gaps: 7.3.3 (if added), 7.4.1 and later items.
- Compatibility decision: none.

## Concrete steps

Run every command from the repository root. Capture long output with `tee`,
using the naming convention below.

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-${BRANCH}.out
make lint      2>&1 | tee /tmp/lint-whitaker-${BRANCH}.out
make test      2>&1 | tee /tmp/test-whitaker-${BRANCH}.out
make verus-clone-detector 2>&1 | tee /tmp/verus-whitaker-${BRANCH}.out
```

Focused loops during red-green-refactor:

```bash
cargo nextest run -p whitaker_sarif
cargo nextest run -p whitaker_clones_core run1
cargo nextest run -p whitaker_clones_core --test run1_sarif_behaviour
cargo nextest run -p whitaker_clones_core --test run1_pipeline
cargo insta test -p whitaker_clones_core --review   # only when a snapshot is intentionally new
```

Expected red output at the start of EP-M2 (abridged):

```plaintext
error[E0433]: failed to resolve: could not find `cosine` in `run1`
```

Expected green output at the end of EP-M5 (abridged):

```plaintext
     Summary [ ... ] N tests run: N passed, 0 skipped
```

Expected Verus output at the end of EP-M3 (abridged):

```plaintext
verification results:: K verified, 0 errors
```

Before committing at each milestone, run the three gates in sequence, and
`make verus-clone-detector` for EP-M3 onwards. Delegate full gate runs to the
`scrutineer` agent where available.

## Validation and acceptance

Behaviour-driven development (BDD) specification, file
`crates/whitaker_clones_core/tests/features/run1_sarif.feature`:

```gherkin
Feature: SARIF Run 1 Type-3 refinement
  The AST pass refines token-pass results and adds near-miss clones.

  Scenario: A near-miss pair missed by the token pass is reported as Type-3
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

  Scenario: Structurally unrelated candidates are not reported
    Given the Rust fragments unrelated_left and unrelated_right
    And the token pass does not accept the pair
    When the AST pass refines the candidates
    Then Run 1 contains no result for the pair

  Scenario: An unparsable fragment keeps its token verdict
    Given the Rust fragments renamed_left and broken_right
    And the token pass accepts the pair as WHK002
    When the AST pass refines the candidates
    Then Run 1 contains a WHK002 result with baseline state unchanged
    And the result has no cosine and no astHash

  Scenario: Misordered thresholds are rejected
    Given a type3 threshold of 0.95 and a type2 threshold of 0.90
    When the AST pass refines the candidates
    Then refinement fails with a threshold order error
```

Step definitions return `Result` from `given` and `when` steps and assert only
in `then` steps, matching the repository's rstest-bdd conventions. Each step
parses at most three values. The "reshaped" fixture must be chosen so that
token Jaccard reaches `9/10` while the canonical hashes differ and the cosine
is at least `0.85`. Selecting it is part of EP-M5's red step. If no natural
snippet achieves this, the scenario lowers the token thresholds through the
test configuration, and that choice is recorded in `Decision log`.

Red-Green-Refactor evidence to record per milestone, in `Artefacts and notes`:

- Red: the focused command and its failure (compile error or assertion).
- Green: the same command passing.
- Refactor: the three gates passing.

Quality criteria:

- Tests: `make test` passes, including all new unit, property, snapshot,
  behavioural and end-to-end tests.
- Verification: P1 to P4 verified by `make verus-clone-detector` with no
  trusted shortcuts, or P4 escalated per tolerance; R1, R2, C1, C2 and E1 to E3
  discharged by the named tests, with non-vacuity evidence recorded.
- Lint and type checks: `make check-fmt` and `make lint` pass.
- Performance: no benchmark is required. The lowering memo guarantees at most
  one lowering per fragment per pass.
- Security: no new trust boundary. `num-bigint` is reviewed under the
  repository's normal dependency policy.

## Idempotence and recovery

All steps are re-runnable. Tests write only to `tempfile` directories. Snapshot
files change only through deliberate `cargo insta` review. Each milestone ends
in one or more commits that pass every gate, so `git revert` of a milestone's
commits restores the previous plateau. The Verus file is outside the Cargo
build and can be reverted alone.

## Artefacts and notes

To be filled during implementation: red and green transcripts, seeded-fault
runs for P1 and R1, the Verus summary line, and the final gate summaries.

## Interfaces and dependencies

New dependency: `num-bigint = "0.5.1"` (workspace), used only inside
`run1/cosine.rs`.

Public surface added to `whitaker_clones_core`, re-exported from the crate root:

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
    pub fn span(&self) -> Range<usize>;
}

pub struct AstPassConfig { /* tool_name, tool_version, type3_threshold */ }
impl AstPassConfig {
    pub fn new(tool_name: impl Into<String>, tool_version: impl Into<String>) -> Self; // type3 = 17/20
    pub const fn with_type3_threshold(self, threshold: SimilarityThreshold) -> Self;
    pub fn tool_name(&self) -> &str;
    pub fn tool_version(&self) -> &str;
    pub const fn type3_threshold(&self) -> SimilarityThreshold;
}

pub struct RefinementInput<'a> { /* fragments, candidates, accepted, token_config */ }
impl<'a> RefinementInput<'a> {
    pub const fn new(fragments: &'a [TokenFragment], candidates: &'a [CandidatePair],
                     accepted: &'a [AcceptedPair], token_config: &'a TokenPassConfig) -> Self;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CloneRule { Type1, Type2, Type3 }      // rule_id() -> WHK001..WHK003

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refinement { Unchanged, Updated, New } // baseline_state() -> BaselineState

pub struct AstEvidence { /* left_hash: AstHash, right_hash: AstHash, cosine: CosineScore */ }
pub struct RefinedPair { /* pair, rule, refinement, token_profile, jaccard, ast: Option<AstEvidence> */ }

pub fn refine_candidate_pairs(input: &RefinementInput<'_>, config: &AstPassConfig)
    -> Run1Result<Vec<RefinedPair>>;
pub fn emit_run1(fragments: &[TokenFragment], refined: &[RefinedPair],
                 token_config: &TokenPassConfig, config: &AstPassConfig)
    -> Run1Result<whitaker_sarif::Run>;

#[derive(Debug, thiserror::Error)]
pub enum Run1Error {
    MissingFragment(FragmentId),
    EmptyFingerprintSet(FragmentId),
    InvalidFragmentSpan { fragment: FragmentId, source: AstError },
    ThresholdOrder { higher: &'static str, lower: &'static str },
    ParserUnavailable,
    Run0(#[from] Run0Error),
    Sarif(#[from] whitaker_sarif::SarifError),
    InvalidScore(String),
}
pub type Run1Result<T> = Result<T, Run1Error>;
```

Public surface added to `whitaker_sarif`: `BaselineState`, `TOKEN_HASH_KEY` and
`AST_HASH_KEY`. Changed: `SarifResult::baseline_state`,
`ResultBuilder::with_baseline_state`, and `WhitakerProperties::cosine`.

Crate-private additions: `pair_result` (shared helpers),
`run1::classify::{TokenVerdict, AstVerdict, Verdict, classify,
validate_threshold_order}`,
`run1::refine::refine_with_lowering`, and `run1::metrics`.

## Revision note

2026-11-11: initial draft from Wyvern reconnaissance and external research.
Awaiting expert-panel review.
