# Emit SARIF Run 0 for accepted Type-1 and Type-2 clone pairs (roadmap 7.2.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.2.3 turns the completed token-pass building blocks from 7.2.1
and 7.2.2 into an observable token-pass output. After this change,
`crates/whitaker_clones_core/` will be able to take stable candidate pairs,
score them for Type-1 or Type-2 acceptance, and emit a SARIF Run 0 payload that
downstream tooling can read immediately. The emitted run must use the existing
`whitaker_sarif` crate, must carry stable partial fingerprints, and must
convert retained token byte ranges into stable 1-based SARIF regions.

Observable outcome:

1. `whitaker_clones_core` exposes a documented API that accepts token-pass
   fragment data plus candidate pairs and returns a SARIF `Run` for the token
   pass.
2. Accepted Type-1 pairs produce `WHK001` results and accepted Type-2 pairs
   produce `WHK002` results.
3. Each emitted result carries deterministic `message`, `locations`,
   `related_locations`, `partial_fingerprints`, and Whitaker `properties`.
4. Unit tests cover happy paths, unhappy paths, threshold boundaries,
   multi-line span conversion, stable ordering, and duplicate suppression.
5. Behaviour-driven development (BDD) tests using `rstest-bdd` v0.5.0 exercise
   end-to-end Run 0 emission for accepted, rejected, and malformed inputs.
6. `docs/whitaker-clone-detector-design.md` records the final 7.2.3
   implementation decisions in a new `## Implementation decisions (7.2.3)`
   section.
7. `docs/roadmap.md` marks 7.2.3 done only after the implementation, tests,
   and quality gates succeed.
8. The implementation turn finishes with `make fmt`, `make markdownlint`,
   `make nixie`, `make check-fmt`, `make lint`, and `make test` all passing.

## Constraints

- Scope only roadmap item 7.2.3. Do not implement filesystem discovery,
  grouping into clone classes, abstract syntax tree refinement, HTML output, or
  CLI wiring in this change.
- Keep the implementation in `crates/whitaker_clones_core/`, reusing
  `crates/whitaker_sarif/` for all SARIF modelling and builder logic.
- Do not bypass `whitaker_sarif` by hand-assembling JSON strings.
- Keep every Rust source file below 400 lines. Add sibling modules rather than
  growing `token/`, `index/`, or `lib.rs` into large files.
- Every new Rust module must begin with a `//!` module-level comment.
- Every public API added for 7.2.3 must carry Rustdoc with examples that pass
  under `make test`.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for unit and behavioural coverage.
- Behaviour tests must stay within the workspace Clippy argument threshold of
  1. A step function may parse at most 3 values in addition to the world
  fixture.
- Integration tests under `tests/` must avoid `unwrap()` and `expect()`.
- Preserve deterministic behaviour. The same accepted inputs must always yield
  identical result ordering, fingerprints, messages, and spans.
- Use integer arithmetic for threshold decisions. Workspace Clippy denies
  float arithmetic and precision-loss casts in implementation crates.
- Record all final 7.2.3 decisions in
  `docs/whitaker-clone-detector-design.md`.
- Do not mark roadmap item 7.2.3 done until the implementation, tests, and all
  quality gates succeed.

## Tolerances

- Scope tolerance: if implementation starts requiring filesystem crawling,
  command-line parsing, or AST refinement to make Run 0 useful, stop and
  escalate because that crosses into roadmap items 7.3.x and later CLI work.
- Crate-boundary tolerance: if `whitaker_clones_core` cannot emit Run 0
  without pushing non-trivial logic back into `whitaker_sarif` or a new crate,
  stop and escalate with the concrete boundary problem.
- Dependency tolerance: if implementation requires a new third-party crate
  beyond workspace dependencies, stop and escalate before adding it. This is
  especially important if the design sketch's aspirational `Blake3` fingerprint
  shape becomes mandatory.
- Size tolerance: if the change exceeds 18 touched files or 1600 net new lines
  of code, stop and escalate before proceeding further.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after 3 targeted fix iterations, stop and escalate with the saved log
  paths.
- SARIF tolerance: if local documentation is insufficient to resolve a SARIF
  field-shape question, use the Firecrawl MCP to fetch the relevant SARIF spec
  details before continuing rather than guessing.

## Risks

- Fingerprint-shape risk: the design document sketches a `Blake3`-based
  fragment key, but 7.2.2 shipped an opaque string `FragmentId` specifically to
  defer that decision. Mitigation: keep 7.2.3 on deterministic string-backed
  fragment IDs and, if needed, derive any extra pair hash from existing
  workspace dependencies rather than adding `blake3`.
- Threshold-arithmetic risk: Jaccard acceptance wants ratios, but the workspace
  denies float arithmetic and precision-loss casts. Mitigation: represent
  similarity as integer numerator and denominator pairs, compare thresholds via
  cross-multiplication, and only format decimals in a tightly scoped helper for
  human-facing text or SARIF properties.
- Span-conversion risk: retained fingerprints currently store byte ranges, but
  SARIF locations need 1-based line and column regions. Mitigation: add a
  dedicated byte-index helper with unit tests for single-line, multi-line,
  trailing-newline, and end-of-file cases.
- Duplicate-emission risk: the same `CandidatePair` can appear more than once
  through repeated inputs or reversed ordering. Mitigation: canonicalize pairs,
  choose one primary location deterministically, and deduplicate emitted
  results before finalizing the run.
- Result-shape risk: `whitaker_sarif::deduplicate_results()` keys on
  `(whitakerFragment, file, region)`, so a poor primary-location choice would
  undermine merge behaviour later. Mitigation: make the lexically smaller
  fragment the primary result location and keep the peer fragment in
  `related_locations`.
- Test-ergonomics risk: BDD step functions can easily exceed the argument
  threshold when modelling two fragments, thresholds, and expected results.
  Mitigation: store fragment data in a world struct and keep individual steps
  small and incremental.

## Progress

- [x] Stage A: Gather repository context and draft this ExecPlan
  (2026-03-29).
- [x] Stage B: Add failing unit tests for threshold acceptance, byte-range to
  SARIF-region conversion, stable fingerprints, and deterministic result
  ordering (completed 2026-03-30).
- [x] Stage C: Add failing `rstest-bdd` feature scenarios for accepted,
  rejected, and malformed Run 0 emission.
- [x] Stage D: Add the new acceptance and Run 0 emission modules in
  `crates/whitaker_clones_core/` and wire their public exports.
- [x] Stage E: Make the targeted tests green and refactor for readability while
  keeping files below 400 lines.
- [x] Stage F: Update `docs/whitaker-clone-detector-design.md` with
  `## Implementation decisions (7.2.3)`.
- [x] Stage G: Mark roadmap item 7.2.3 done in `docs/roadmap.md`.
- [x] Stage H: Run documentation and code quality gates successfully.
- [x] Stage I: Finalize the living sections in this ExecPlan after
  implementation.

## Surprises & Discoveries

- `whitaker_clones_core` already provides the exact 7.2.3 upstream inputs that
  matter: canonical `CandidatePair`, deterministic `FragmentId`, and retained
  `Fingerprint { hash, range }` values.
- `whitaker_sarif` already ships the builders and rule IDs that 7.2.3 needs:
  `RunBuilder`, `ResultBuilder`, `LocationBuilder`, `RegionBuilder`,
  `WhitakerPropertiesBuilder`, `WHK001_ID`, and `WHK002_ID`.
- `whitaker_sarif::merge::deduplicate_results()` deduplicates only on the
  first location plus the `whitakerFragment` fingerprint. This makes the
  primary-versus-related location choice part of 7.2.3's observable contract.
- There is no existing byte-offset to line-and-column mapper in
  `whitaker_clones_core` or `whitaker_sarif`. A dedicated helper is required
  for 7.2.3.
- The current design document says Run 0 is produced by
  `whitaker_clones_cli@token`, but roadmap 7.2.3 is implementable as a pure
  library API in `whitaker_clones_core`; the CLI-specific producer string can
  stay as a small parameter rather than forcing CLI work now.
- Workspace dependencies already include `whitaker_sarif`, `camino`, and
  `sha2`. If a stable hash beyond `FragmentId` is needed, 7.2.3 can likely use
  existing dependencies without widening the dependency surface.
- `whitaker_sarif::WhitakerProperties` still requires numeric `jaccard` and
  `cosine` fields, so 7.2.3 needs one narrowly scoped decimal-string-to-`f64`
  conversion helper even though the acceptance logic itself stays integer-only.
- The workspace Clippy threshold of four arguments shaped the final public API:
  `TokenFragment::new` now sets source identity only, while retained
  fingerprints are supplied via `.with_retained_fingerprints(...)`.

## Decision Log

- Decision: keep 7.2.3 inside `crates/whitaker_clones_core/` and add a
  dependency on `whitaker_sarif` there. Rationale: the design assigns scoring
  and token-pass processing to clone core, while `whitaker_sarif` is already a
  pure modelling crate and should remain that way. Date/Author: 2026-03-29 /
  Codex.
- Decision: emit one SARIF result per accepted pair, not per future clone
  class. Rationale: roadmap 7.2.3 is explicitly pair-based; grouping belongs to
  later work. Date/Author: 2026-03-29 / Codex.
- Decision: choose the lexically smaller fragment ID as the primary result
  location and place the peer fragment in `related_locations`. Rationale: this
  makes result ordering and deduplication stable, and prevents swapped-pair
  duplicates. Date/Author: 2026-03-29 / Codex.
- Decision: compare Jaccard thresholds with integer cross-multiplication rather
  than floating-point arithmetic. Rationale: this matches workspace lint policy
  and keeps acceptance deterministic. Date/Author: 2026-03-29 / Codex.
- Decision: compute SARIF regions from byte ranges using a dedicated UTF-8
  byte-line index helper and record both line-column data and byte
  offset-length data in the `Region`. Rationale: retained fingerprints are
  byte-based today, so the mapping should stay lossless and deterministic.
  Date/Author: 2026-03-29 / Codex.
- Decision: use SHA-256 hex digests for both the pair fingerprint and the
  combined token hash. Rationale: `sha2` is already a workspace dependency,
  while the earlier Blake3 shape was only aspirational and not yet committed
  anywhere else in the codebase. Date/Author: 2026-03-30 / Codex.
- Decision: derive the primary SARIF span from the first retained fingerprint
  of each accepted fragment. Rationale: 7.2.3 only needs stable pair emission,
  not clone-class region aggregation, and the first retained fingerprint is the
  narrowest deterministic span already available from 7.2.1. Date/Author:
  2026-03-30 / Codex.

## Context and orientation

### Repository state

The repository root is `/home/user/project`. Roadmap items 7.2.1 and 7.2.2
already shipped `crates/whitaker_clones_core/` with:

- `src/token/` for normalization, shingling, rolling hash, and winnowing.
- `src/index/` for deterministic MinHash sketches and LSH candidate pairs.
- `tests/token_pass_behaviour.rs` and `tests/min_hash_lsh_behaviour.rs` for
  existing BDD coverage.

The repository also already contains `crates/whitaker_sarif/`, which exports
the SARIF model and builder surface needed for Run 0:

- `RunBuilder`, `ResultBuilder`, `LocationBuilder`, and `RegionBuilder`
- `WHK001_ID`, `WHK002_ID`
- `WhitakerPropertiesBuilder`
- `merge_runs()` and `deduplicate_results()`

### Design requirements from `docs/whitaker-clone-detector-design.md`

The clone-detector design currently states all of the following:

1. The token pass performs candidate pairing first, then Jaccard similarity,
   then SARIF emission.
2. Run 0 holds token-pass results and uses the Type-1 or Type-2 rule IDs.
3. Each result includes `message.text`, `locations[0]`,
   `relatedLocations[*]`, `partialFingerprints`, and `properties`.
4. `partialFingerprints` should include a stable Whitaker fragment key and a
   token hash.
5. `properties.whitaker` should include the profile, `k`, `window`, and
   `jaccard`.

### Existing code that matters

- `crates/whitaker_clones_core/src/index/types.rs`
  `CandidatePair` canonicalizes pair ordering and suppresses self-pairs.
- `crates/whitaker_clones_core/src/token/types.rs`
  `Fingerprint` already carries stable byte ranges.
- `crates/whitaker_sarif/src/merge.rs`
  deduplicates by `(whitakerFragment, file, region)`.
- `crates/whitaker_sarif/src/whitaker_properties.rs`
  currently expects numeric `jaccard` and `cosine` fields in
  `WhitakerProperties`.

### Testing and documentation references

The implementation should follow these local guides while keeping this plan
self-contained:

- `docs/rstest-bdd-users-guide.md` for fixture-backed BDD tests and step
  wiring.
- `docs/rust-testing-with-rstest-fixtures.md` for reusable test-data fixtures.
- `docs/rust-doctest-dry-guide.md` for Rustdoc example style.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping the
  implementation split into small, readable helpers.
- `docs/whitaker-dylint-suite-design.md` for workspace testing and quality-gate
  expectations.

## Outcomes & Retrospective

- 7.2.3 now ships as a dedicated `run0/` module inside
  `crates/whitaker_clones_core/`, exposing acceptance and SARIF-emission APIs
  without pulling in CLI concerns.
- Accepted Type-1 and Type-2 pairs now emit deterministic SARIF Run 0 results
  via `whitaker_sarif`, including stable ordering, primary-versus-related
  locations, SHA-256 partial fingerprints, Whitaker properties, and byte-range
  to region conversion.
- Unit coverage and `rstest-bdd` behavioural coverage now exercise threshold
  boundaries, malformed inputs, multi-line spans, duplicate suppression, and
  deterministic primary-location selection.
- `docs/whitaker-clone-detector-design.md` records the final 7.2.3
  implementation decisions, and `docs/roadmap.md` now marks roadmap item 7.2.3
  complete.
- Final quality gates passed with logs captured at:
  `/tmp/7-2-3-final-fmt.log`, `/tmp/7-2-3-final-markdownlint.log`,
  `/tmp/7-2-3-final-nixie.log`, `/tmp/7-2-3-final-check-fmt.log`,
  `/tmp/7-2-3-final-lint.log`, and `/tmp/7-2-3-final-test.log`.
- Final validation summary: `make test` completed successfully with
  `Summary [ 111.463s] 1188 tests run: 1188 passed, 2 skipped`.

## Proposed implementation shape

Keep 7.2.3 narrow by adding one new module subtree for token-pass acceptance
and Run 0 emission. A concrete shape that fits the current crate structure is:

- `crates/whitaker_clones_core/src/run0/mod.rs`
  public re-exports and module wiring.
- `crates/whitaker_clones_core/src/run0/types.rs`
  input structs such as `TokenFragment`, `AcceptedPair`, `TokenPassConfig`, and
  a small `SimilarityRatio` or equivalent integer-backed score type.
- `crates/whitaker_clones_core/src/run0/error.rs`
  typed errors for missing fragments, empty retained fingerprints, invalid byte
  ranges, and malformed score formatting.
- `crates/whitaker_clones_core/src/run0/score.rs`
  Jaccard intersection, union, threshold predicates, and deterministic decimal
  formatting helpers.
- `crates/whitaker_clones_core/src/run0/span.rs`
  byte-range to SARIF-region conversion.
- `crates/whitaker_clones_core/src/run0/emit.rs`
  conversion from accepted token pairs into a `whitaker_sarif::Run`.
- `crates/whitaker_clones_core/src/run0/tests.rs`
  focused unit tests local to the new module.

Update `crates/whitaker_clones_core/src/lib.rs` to re-export the public Run 0
API alongside the existing token and index APIs.

Update `crates/whitaker_clones_core/Cargo.toml` to add the workspace
dependencies actually needed by the new module:

- `whitaker_sarif`
- `camino` only if path normalization is needed in the final API
- `sha2` only if a stable pair hash is needed beyond deterministic string
  concatenation

The initial public API should stay explicit and builder-friendly. A concrete
shape to implement is:

```rust
//! Example public surface only; the implementation may rename items to fit the
//! crate, but the separation of concerns should stay the same.

use whitaker_sarif::Run;

pub struct TokenFragment {
    pub id: FragmentId,
    pub file_uri: String,
    pub source_text: String,
    pub retained_fingerprints: Vec<Fingerprint>,
}

pub struct TokenPassConfig {
    pub tool_name: String,
    pub tool_version: String,
    pub shingle_size: usize,
    pub winnow_window: usize,
    pub type1_threshold_num: usize,
    pub type1_threshold_den: usize,
    pub type2_threshold_num: usize,
    pub type2_threshold_den: usize,
}

pub struct AcceptedPair {
    pub pair: CandidatePair,
    pub profile: NormProfile,
    pub score: SimilarityRatio,
}

pub fn accept_candidate_pairs(
    fragments: &[TokenFragment],
    candidates: &[CandidatePair],
    config: &TokenPassConfig,
) -> Run0Result<Vec<AcceptedPair>>;

pub fn emit_run0(
    fragments: &[TokenFragment],
    accepted_pairs: &[AcceptedPair],
    config: &TokenPassConfig,
) -> Run0Result<Run>;
```

Use parameter structs rather than long function argument lists so the plan
stays within the workspace's Clippy `too_many_arguments` threshold.

## Detailed plan of work

### Stage B: Add failing unit tests first

Before implementation, add focused unit tests under
`crates/whitaker_clones_core/src/run0/tests.rs` that fail against the current
codebase:

1. Threshold acceptance at exact boundaries for T1 and T2.
2. Rejection just below the threshold.
3. Jaccard set semantics: duplicate retained fingerprint hashes do not inflate
   the score.
4. Byte-range conversion for a single-line fragment.
5. Byte-range conversion for a multi-line fragment with a trailing newline.
6. Canonical pair ordering produces one primary result and one related
   location.
7. Stable result ordering for multiple accepted pairs.
8. Duplicate or reversed accepted pairs produce one emitted result.
9. Missing fragment data or invalid byte ranges produce typed errors.

These tests provide the red stage for the acceptance and emission logic.

### Stage C: Add failing BDD coverage

Add:

- `crates/whitaker_clones_core/tests/features/run0_sarif.feature`
- `crates/whitaker_clones_core/tests/run0_sarif_behaviour.rs`

Use the existing `MinHashLshWorld` and `SarifWorld` patterns as the model: a
fixture-backed world with `RefCell` fields and `match`-based helper accessors.

The feature file should cover at least:

1. Happy path: a Type-1 pair above threshold emits one `WHK001` result with
   one primary location and one related location.
2. Happy path: a Type-2 pair above threshold emits one `WHK002` result with
   `properties.whitaker.profile = "T2"` and the configured `k` and `window`.
3. Unhappy path: a pair below threshold emits no result.
4. Unhappy path: an empty retained-fingerprint set fails before emission.
5. Edge case: a multi-line byte range yields the expected 1-based start and end
   lines and columns.
6. Edge case: reversed pair input still emits one deterministic result.

Keep every step under the argument-count limit by loading fragment details into
the world incrementally rather than parsing both fragments and all expected
fields in one step.

### Stage D: Implement acceptance and span mapping

Create the new `run0/` module subtree.

In `score.rs`:

1. Deduplicate retained fingerprint hashes per fragment to honour set semantics.
2. Compute `intersection` and `union` counts using integer arithmetic only.
3. Compare against thresholds via cross-multiplication:
   `intersection * threshold_den >= union * threshold_num`.
4. Add a small helper that formats the accepted score into a stable decimal
   string for `message.text`.
5. If `WhitakerProperties` still requires `f64`, convert only in one
   quarantined helper so the rest of the logic stays integer-based.

In `span.rs`:

1. Build a line-start index from the fragment's source text once.
2. Convert a half-open byte range into:
   - `start_line`
   - `start_column`
   - `end_line`
   - `end_column`
   - `byte_offset`
   - `byte_length`
3. Reject out-of-bounds or inverted ranges with a typed error.

The span helper must be pure and deterministic so it can be unit-tested without
SARIF builders in the way.

### Stage E: Implement Run 0 emission

In `emit.rs`:

1. Resolve the left and right `TokenFragment` values for each accepted pair.
2. Choose the primary fragment deterministically from the canonical pair order.
3. Convert the primary and peer fingerprint byte ranges into SARIF `Region`
   values.
4. Use `LocationBuilder` for the primary `locations[0]`.
5. Use `RelatedLocation` for the peer occurrence so the result captures both
   fragments without creating a second swapped result.
6. Use `ResultBuilder` with:
   - `rule_id = WHK001_ID` for `NormProfile::T1`
   - `rule_id = WHK002_ID` for `NormProfile::T2`
   - a stable, human-readable message:
     `Type-{N} clone: {fileA}:{spanA} <-> {fileB}:{spanB} (sim = {score})`
7. Populate `partial_fingerprints` with at least:
   - `whitakerFragment`: stable pair fingerprint used for deduplication
   - `tokenHash`: stable fingerprint derived from the accepted pair's retained
     token hashes
8. Populate `properties.whitaker` with:
   - `profile`
   - `k`
   - `window`
   - `jaccard`
   - `cosine = 0.0` for Run 0
   - `groupId = 0`
   - `classSize = 2`
9. Sort results deterministically before finalizing the run.
10. Run the emitted result list through `whitaker_sarif::deduplicate_results()`
    as a final safety net.

Construct the run with `RunBuilder`. Use a producer name compatible with the
design, but keep it configurable through `TokenPassConfig` so 7.2.3 remains a
library API.

### Stage F: Update documentation

Append `## Implementation decisions (7.2.3)` to
`docs/whitaker-clone-detector-design.md`.

Record the final decisions explicitly, including:

1. The stable pair-fingerprint shape used for `whitakerFragment`.
2. The token-hash shape used for `partialFingerprints["tokenHash"]`.
3. The exact threshold representation and comparison strategy.
4. The primary-versus-related location contract.
5. The byte-range to line-column mapping rules.
6. Any pragmatic deviation from the earlier `Blake3` sketch in the design.

Do not mark the roadmap item done until the implementation and all quality
gates pass.

### Stage G: Mark the roadmap item done

After implementation and successful validation, change the 7.2.3 checkbox in
`docs/roadmap.md` from `[ ]` to `[x]`.

### Stage H: Validation and quality gates

Because the implementation touches Rust code and Markdown, the implementation
turn must run all repository-required gates with `tee` and `set -o pipefail`.
Use distinct log files so failures are inspectable after truncated output.

Run:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/7-2-3-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/7-2-3-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/7-2-3-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-3-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-3-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/7-2-3-test.log
```

During implementation, use targeted commands for quick feedback before the full
workspace gates:

```bash
set -o pipefail && cargo test -p whitaker_clones_core 2>&1 | tee /tmp/7-2-3-core-test.log
set -o pipefail && cargo clippy -p whitaker_clones_core --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/7-2-3-core-lint.log
```

Expected end-state signals:

1. The new unit tests and BDD scenarios pass.
2. `make check-fmt`, `make lint`, and `make test` pass.
3. `docs/whitaker-clone-detector-design.md` contains
   `## Implementation decisions (7.2.3)`.
4. `docs/roadmap.md` shows 7.2.3 as done.

## Acceptance criteria

The implementation is complete when a novice can verify all of the following:

1. Calling the new `emit_run0` API with a Type-1 accepted pair returns a
   `whitaker_sarif::Run` containing one `WHK001` result.
2. Calling the same API with a Type-2 accepted pair returns one `WHK002`
   result.
3. A below-threshold candidate pair does not appear in the emitted run.
4. Reversed pair inputs still emit the same stable result once.
5. A multi-line retained fingerprint range is represented with correct 1-based
   SARIF span fields and matching byte offset-length fields.
6. Serializing the emitted run to JSON twice yields identical output for the
   same input.
