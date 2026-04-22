# Prove `CandidatePair::new` canonicalization and self-pair suppression with Verus (roadmap 7.2.6)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.2.6 adds a machine-checked Verus proof for
`crates/whitaker_clones_core/src/index/types.rs::CandidatePair::new`. Verus is
the repository's theorem prover for small semantic invariants. This change must
show two things clearly and repeatedly:

1. Distinct fragment identifiers are returned in canonical lexical order,
   regardless of the order passed to `CandidatePair::new`.
2. Identical fragment identifiers are suppressed by returning `None`, so the
   token-pass candidate pipeline never admits self-pairs through this
   constructor seam.

This matters because roadmap item 7.2.2 made `CandidatePair` the stable output
of MinHash plus locality-sensitive hashing (LSH) candidate generation, and
roadmap item 7.2.3 already depends on the left-hand side of that canonical pair
to choose the primary Static Analysis Results Interchange Format (SARIF)
location. If the constructor contract drifts, later stages inherit unstable
ordering, duplicate emissions, or accidental self-matches.

Observable outcome:

1. The clone-detector Verus group contains a dedicated proof for
   `CandidatePair::new`, and `make verus-clone-detector` reports that both the
   existing `LshConfig::new` proof and the new `CandidatePair::new` proof pass.
2. Unit tests in `crates/whitaker_clones_core/src/index/tests.rs` cover happy
   paths, unhappy paths, and edge cases for direct `CandidatePair::new`
   construction, rather than relying only on indirect `LshIndex` coverage.
3. Behaviour-driven development (BDD) coverage using workspace-pinned
   `rstest-bdd` v0.5.0 exercises canonical order preservation, reversal
   canonicalization, self-pair suppression, and at least one lexical-order edge
   case through a dedicated behaviour harness.
4. `docs/whitaker-clone-detector-design.md` records the final 7.2.6 decisions,
   especially the exact ordering contract and the Verus trust boundary.
5. `docs/developers-guide.md` explains how the clone-detector Verus workflow
   now covers both constructor proofs, and why the sidecar proof models the
   constructor instead of proving the compiled Rust body directly.
6. `docs/users-guide.md` is updated only if implementation changes a public
   tooling behaviour or surfaced feature. If no user-visible behaviour changes,
   the implementation notes that explicitly and leaves the guide unchanged.
7. `docs/roadmap.md` marks 7.2.6 done only after the proof, tests, and all
   required gates succeed.
8. The implementation turn ends with successful runs of `make fmt`,
   `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
   `make test`, `make verus-clone-detector`, and `make verus`.

## Relevant docs and skills

Use these repository documents during implementation:

- `docs/roadmap.md` for the exact 7.2.6 scope and dependency on 7.2.4.
- `docs/adr-003-formal-proof-strategy-for-clone-detector-pipeline.md` for the
  Verus-versus-Kani proof split.
- `docs/whitaker-clone-detector-design.md` for the published MinHash, LSH,
  `CandidatePair`, and SARIF contracts.
- `docs/developers-guide.md` for the proof workflow and trust-boundary wording
  already established by 7.2.4 and 7.2.5.
- `docs/rstest-bdd-users-guide.md` and
  `docs/rust-testing-with-rstest-fixtures.md` for fixture-backed BDD structure
  and `rstest` composition.
- `docs/rust-doctest-dry-guide.md` for any Rustdoc examples added or revised in
  public APIs.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` and
  `docs/whitaker-dylint-suite-design.md` for the repository's preference for
  small helpers, files under 400 lines, and BDD coverage with `rstest-bdd`
  v0.5.0.

Relevant skills for the implementation turn:

- `execplans` to keep this document current while work proceeds.
- `rust-router` to route Rust-specific questions to the smallest useful skill.
- `rust-types-and-apis` if constructor seams or ordering helpers need careful
  API-boundary decisions.
- `nextest` when interpreting or debugging repository test runs.
- `en-gb-oxendict-style` for any documentation updates.

## Constraints

- Scope only roadmap item 7.2.6. Do not pull 7.2.7 or 7.2.8 forward, and do
  not widen this task into new Kani work.
- Keep proof tooling in sidecar files and scripts. Normal Cargo builds and the
  default `make test` path must remain independent of Verus.
- Preserve the existing runtime behaviour of `CandidatePair::new` unless a
  tiny refactor is required to improve clarity without changing the contract.
- Do not widen the public API of `whitaker_clones_core` solely for proof
  convenience. A crate-private helper is acceptable only if it reduces drift
  and stays narrower than the current public surface.
- Keep each Rust source file below 400 lines. Add focused sibling modules or a
  dedicated behaviour harness instead of overloading existing files.
- Every new Rust module must begin with a `//!` module-level comment.
- Every new or newly public API must carry Rustdoc comments with examples that
  follow the guidance in `docs/rust-doctest-dry-guide.md`.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for unit and behavioural coverage.
- Behaviour tests must stay within the workspace Clippy argument-count limit:
  `world` plus at most three parsed values per step.
- Integration tests under `crates/whitaker_clones_core/tests/` should avoid
  `unwrap()` and `expect()` in new code, following the repository's preferred
  BDD world-helper style.
- Keep the ordering contract explicit: canonicalization is lexical
  `FragmentId` ordering, not insertion order, numeric ordering, or span order.
- Update `docs/whitaker-clone-detector-design.md` with final 7.2.6 decisions.
- Update `docs/developers-guide.md` with any significant developer-facing proof
  workflow or trust-boundary guidance introduced by this change.
- Only update `docs/users-guide.md` if the implementation changes a public
  feature, interface, or observable tool behaviour.
- Do not mark roadmap item 7.2.6 done until implementation, proof runs, tests,
  and all requested quality gates succeed.
- Run long validation commands through `tee` with `set -o pipefail`, because
  the environment truncates direct output.
- Keep this plan in `DRAFT` status until approval is received.

## Tolerances

- Proof-model tolerance: if the Verus proof cannot stay a small
  implementation-shaped model and starts needing trusted assumptions about the
  compiled `FragmentId` or `String` implementation, stop and escalate before
  weakening the claim silently.
- Scope tolerance: if proving 7.2.6 appears to require new Kani harnesses,
  `LshIndex` redesign, or SARIF emission changes, stop and ask whether the
  roadmap scope should be widened deliberately.
- API tolerance: if the only practical route is to expose a new public helper
  from `index/types.rs`, stop and ask before widening the public API.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after three focused fix iterations, stop and escalate with saved log
  paths.
- File-size tolerance: if the implementation would push a Rust file over 400
  lines or touch more than 10 files, stop and split the work more cleanly
  before continuing.
- Semantics tolerance: if the proof or direct tests reveal ambiguity over what
  "lexical order" means for `FragmentId`, stop and ask whether the project
  wants bytewise string order, locale-aware order, or a different domain rule.
- Documentation tolerance: if `docs/users-guide.md` starts needing substantive
  updates, pause and confirm that the change is truly user-facing rather than
  maintainer-facing.

## Risks

- Drift risk: a sidecar proof can drift from the runtime constructor logic.
  Mitigation: mirror the real branch order exactly and add direct runtime unit
  and BDD tests around the concrete Rust implementation.
- Ordering-semantics risk: `FragmentId` is a string-backed newtype with derived
  `Ord`, so future maintainers may misread the contract as natural-number or
  span-based ordering. Mitigation: add one edge-case test that proves the
  contract is lexical string order and document that decision in the design
  document.
- Indirect-coverage risk: the repository already tests pair behaviour through
  `LshIndex` and SARIF flows, but those tests do not isolate the constructor.
  Mitigation: add direct constructor tests and a dedicated behaviour harness.
- Workflow-wiring risk: `scripts/run-verus.sh` currently lists only
  `verus/clone_detector_lsh_config.rs` for the clone-detector group.
  Mitigation: add the new proof file explicitly to both the `clone-detector`
  and `all` groups, then validate both `make verus-clone-detector` and
  `make verus`.
- Documentation-scope risk: proof-only work often tempts unnecessary user-guide
  edits. Mitigation: check explicitly for user-visible behaviour changes before
  touching `docs/users-guide.md`.

## Progress

- [x] Stage A: Gather repository context, inspect the current proof workflow,
  and draft this ExecPlan (2026-04-21).
- [x] Stage B: Add direct unit tests for `CandidatePair::new` happy, unhappy,
  and edge-case behaviour (2026-04-22).
- [x] Stage C: Add `rstest-bdd` v0.5.0 scenarios for direct
  `CandidatePair::new` behaviour (2026-04-22).
- [x] Stage D: Add a dedicated Verus sidecar proof for `CandidatePair::new`
  and wire it into `scripts/run-verus.sh` (2026-04-22).
- [x] Stage E: Make the targeted tests and proof green without a runtime
  refactor (2026-04-22).
- [x] Stage F: Update `docs/whitaker-clone-detector-design.md` and
  `docs/developers-guide.md`, and confirm `docs/users-guide.md` needs no change
  because this work is not user-visible (2026-04-22).
- [x] Stage G: Mark roadmap item 7.2.6 done in `docs/roadmap.md`
  (2026-04-22).
- [x] Stage H: Run documentation, proof, lint, and test gates successfully
  (2026-04-22).
- [x] Stage I: Finalize the living sections in this ExecPlan after the
  implementation turn (2026-04-22).

## Surprises & Discoveries

- `CandidatePair::new` already exists in
  `crates/whitaker_clones_core/src/index/types.rs` and already has the exact
  branch structure the roadmap item wants proved: equality returns `None`,
  ordered distinct inputs are preserved, and reversed distinct inputs are
  swapped.
- The existing `src/index/tests.rs` file covers self-pair suppression and
  canonical ordering only indirectly through `LshIndex`, not through isolated
  constructor tests.
- The existing clone-detector Verus runner knows about only one proof file,
  `verus/clone_detector_lsh_config.rs`, so 7.2.6 must extend the runner rather
  than only dropping in a new proof file.
- `docs/developers-guide.md` already contains trust-boundary language for the
  `LshConfig::new` proof. Reusing that wording pattern will keep 7.2.6 aligned
  with the established proof model.
- `docs/whitaker-clone-detector-design.md` already states that candidate pairs
  are emitted in lexical `FragmentId` order and that self-pairs are suppressed.
  The 7.2.6 work therefore proves an existing documented invariant rather than
  inventing a new one.
- The implementation did not need any production-code refactor in
  `index/types.rs`; direct tests and the new sidecar proof were sufficient to
  pin the contract.
- The Verus proof is simplest and clearest when it models an ordered
  identifier domain with `nat` values, while runtime tests cover the concrete
  lexical-string behaviour of `FragmentId`.

## Decision Log

- Decision: implement 7.2.6 as a dedicated proof file,
  `verus/clone_detector_candidate_pair.rs`, instead of extending
  `verus/clone_detector_lsh_config.rs`. Rationale: one roadmap item maps
  cleanly to one proof artefact, and `scripts/run-verus.sh` output stays
  explicit. Date/Author: 2026-04-21 / Codex.
- Decision: add direct unit and BDD coverage for `CandidatePair::new` instead
  of treating existing `LshIndex` tests as sufficient. Rationale: roadmap item
  7.2.6 is about the constructor seam itself, and direct tests reduce proof
  drift. Date/Author: 2026-04-21 / Codex.
- Decision: keep the ordering contract lexical and string-based, matching the
  current `FragmentId` derived ordering, and document that explicitly.
  Rationale: later SARIF and deduplication stages already depend on that
  concrete contract. Date/Author: 2026-04-21 / Codex.
- Decision: treat `docs/users-guide.md` as "update only if needed", not as an
  automatic edit. Rationale: this roadmap slice is proof and developer-guidance
  work unless implementation exposes a new user-facing behaviour. Date/Author:
  2026-04-21 / Codex.
- Decision: keep the runtime constructor body unchanged and prove/document the
  existing behaviour instead of extracting a proof helper. Rationale: the
  constructor is already small, direct, and readable, so a helper would add
  surface area without reducing drift. Date/Author: 2026-04-22 / Codex.
- Decision: model proof identifiers as `nat` values in Verus and rely on
  direct runtime tests to pin lexical `FragmentId` semantics. Rationale: this
  proves the constructor's ordering control flow without pretending to verify
  Rust `String` internals. Date/Author: 2026-04-22 / Codex.

## Context and orientation

### Repository state

The code under proof lives in `crates/whitaker_clones_core/src/index/types.rs`.
The current constructor is:

```rust
pub fn new(left: FragmentId, right: FragmentId) -> Option<Self> {
    if left == right {
        return None;
    }
    if left < right {
        return Some(Self { left, right });
    }
    Some(Self {
        left: right,
        right: left,
    })
}
```

The clone-detector proof workflow already exists:

- `make verus-clone-detector` delegates to `./scripts/run-verus.sh
  clone-detector`.
- `scripts/run-verus.sh` currently includes only
  `verus/clone_detector_lsh_config.rs` for the clone-detector group.
- `make verus` delegates to the same runner's `all` group.

The direct upstream and downstream dependencies that matter are:

- `crates/whitaker_clones_core/src/index/lsh.rs`, where `CandidatePair::new` is
  used while emitting deduplicated candidate pairs from band buckets.
- `crates/whitaker_clones_core/src/run0/types.rs` and
  `crates/whitaker_clones_core/tests/run0_sarif_behaviour.rs`, where the
  canonical left fragment already drives downstream observable behaviour.
- `docs/whitaker-clone-detector-design.md`, which already documents lexical
  pair ordering and self-pair suppression as part of the shipped 7.2.2 design.

### Proof shape to preserve

ADR 003 assigns small canonicalization invariants to Verus and bounded
collection-heavy behaviour to Kani. That means 7.2.6 should stay intentionally
small:

- Verus proves the constructor model for an ordered identifier domain.
- Unit tests and BDD scenarios pin the concrete runtime semantics of the
  `String`-backed `FragmentId`.
- No new Kani harness is needed for this roadmap item.

### Testing shape to preserve

The repository already uses:

- `rstest` fixtures for unit-level setup reuse.
- `rstest-bdd` v0.5.0 for behaviour tests under
  `crates/whitaker_clones_core/tests/`.
- Indexed `#[scenario]` bindings plus a small world struct, as documented in
  `docs/whitaker-clone-detector-design.md`.

7.2.6 should follow that existing shape rather than inventing a second testing
style.

## Proposed implementation shape

Implement the feature in four small slices.

1. Add direct constructor tests in
   `crates/whitaker_clones_core/src/index/tests.rs`. These tests should prove:
   - already-canonical distinct inputs are preserved,
   - reversed distinct inputs are swapped,
   - identical inputs return `None`,
   - an edge case such as `"fragment-10"` versus `"fragment-2"` follows lexical
     string ordering rather than natural-number ordering.
2. Add a dedicated behaviour harness,
   `crates/whitaker_clones_core/tests/candidate_pair_behaviour.rs`, plus
   `crates/whitaker_clones_core/tests/features/candidate_pair.feature`. The
   world only needs to store the attempted input IDs and the resulting
   `Option<CandidatePair>`.
3. Add `verus/clone_detector_candidate_pair.rs`.
   The proof should mirror the constructor's three-way branch structure and
   establish:
   - equal inputs are suppressed,
   - distinct inputs always yield a pair,
   - the returned pair is ordered so that `left < right`,
   - if the inputs are already ordered, the constructor preserves that order,
   - if the inputs are reversed, the constructor swaps them exactly once.
4. Update `scripts/run-verus.sh` so the `clone-detector` and `all` groups
   include the new proof file. Then update
   `docs/whitaker-clone-detector-design.md` and `docs/developers-guide.md` to
   record the final proof scope and wording.

Prefer no runtime refactor. If constructor tests become awkward to read, small
test-local helpers are acceptable. A new runtime helper inside `index/types.rs`
should be added only if it materially improves clarity without widening the
public API.

## Detailed work plan

### Stage B: make the constructor contract fail loudly in tests first

Add direct unit tests to `crates/whitaker_clones_core/src/index/tests.rs`
before touching the proof runner. The new tests should fail if:

- `CandidatePair::new` stops returning `None` for equal IDs,
- canonical order stops being preserved,
- reversed inputs stop being normalized,
- lexical string order changes unexpectedly for similar-looking IDs.

These tests isolate the constructor contract so later proof work has a runtime
anchor.

### Stage C: add behaviour-level coverage

Create a dedicated BDD harness for `CandidatePair::new` instead of extending
`min_hash_lsh_behaviour.rs`. The behaviour file should stay tightly focused on
the public constructor API and avoid pulling MinHash or LSH setup into the
world.

Use four scenarios:

1. Already-canonical distinct IDs return the same ordered pair.
2. Reversed distinct IDs are normalized into canonical order.
3. Identical IDs produce no pair.
4. Similar identifiers demonstrate lexical ordering explicitly.

Keep every step to `world` plus at most three parsed values. Use `match`-based
world helpers instead of `expect()` in the integration test.

### Stage D: add the Verus proof

Create `verus/clone_detector_candidate_pair.rs` as an implementation-shaped
model of the constructor. The cleanest proof shape is to model identifiers as
an abstract totally ordered value domain in Verus and prove the constructor
logic over that domain.

That proof does not need to model Rust `String` internals. Instead:

- Verus establishes the control-flow theorem for any ordered IDs.
- Runtime unit and BDD tests establish that `FragmentId` uses lexical string
  ordering concretely.

Document that split clearly in the proof file header and in
`docs/developers-guide.md`, following the existing `LshConfig::new` wording.

### Stage E: wire the proof into the repository workflow

Update `scripts/run-verus.sh` so:

- `clone-detector` runs both `verus/clone_detector_lsh_config.rs` and
  `verus/clone_detector_candidate_pair.rs`,
- `all` runs the new proof as well.

Do not change `Makefile` targets unless the existing targets cannot discover
the new file through the script.

### Stage F: update design and developer documentation

Update `docs/whitaker-clone-detector-design.md` with a new
`## Implementation decisions (7.2.6)` section. Record at least:

- that `CandidatePair::new` is now covered by a Verus sidecar proof,
- that the ordering contract is lexical `FragmentId` order,
- that self-pair suppression remains a constructor-level invariant,
- that the proof stays implementation-shaped and does not directly verify the
  compiled Rust body.

Update `docs/developers-guide.md` so the proof-workflow section explains that
the clone-detector Verus group now covers both constructor proofs.

Inspect `docs/users-guide.md`. If no public tooling behaviour changed, leave it
unchanged and say so in the implementation notes. If a user-visible behaviour
or feature description needs clarification, update it narrowly.

### Stage G: mark the roadmap item done

After all proofs and tests pass, change the 7.2.6 entry in `docs/roadmap.md`
from `[ ]` to `[x]`.

## Validation and evidence

Run every long command with `tee` and `set -o pipefail`. Save logs in `/tmp/`
using stable names so failures are easy to inspect and report.

Recommended command sequence for the implementation turn:

```sh
set -o pipefail && make fmt 2>&1 | tee /tmp/7-2-6-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/7-2-6-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/7-2-6-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-6-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-6-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/7-2-6-test.log
set -o pipefail && make verus-clone-detector 2>&1 | tee /tmp/7-2-6-verus-clone-detector.log
set -o pipefail && make verus 2>&1 | tee /tmp/7-2-6-verus.log
```

Expected success signals:

- `make test` reports the new direct constructor tests and the new BDD
  scenarios passing.
- `make verus-clone-detector` reports both clone-detector proof files passing.
- `make verus` reports the full Verus proof set passing, including the new
  clone-detector proof.
- `docs/roadmap.md` shows 7.2.6 marked done only after the commands above are
  green.

## Outcomes & Retrospective

Delivered roadmap item 7.2.6 without changing the production constructor body.
`CandidatePair::new` remains the same small three-branch runtime function, and
the work landed as proof, test, and documentation hardening around that seam.

Final shipped pieces:

- Direct unit coverage in `crates/whitaker_clones_core/src/index/tests.rs` for
  preserved canonical order, reversed-input canonicalization, self-pair
  suppression, and the lexical-order edge case `"fragment-10" < "fragment-2"`.
- Dedicated BDD coverage in
  `crates/whitaker_clones_core/tests/candidate_pair_behaviour.rs` and
  `tests/features/candidate_pair.feature` with four constructor-level scenarios.
- New Verus sidecar `verus/clone_detector_candidate_pair.rs`.
- Updated `scripts/run-verus.sh` so `clone-detector` and `all` include the new
  proof file.
- Updated `docs/whitaker-clone-detector-design.md`,
  `docs/developers-guide.md`, and `docs/roadmap.md`.
- Left `docs/users-guide.md` unchanged because the work did not alter any
  public feature or user-visible workflow.

Validation evidence:

- `make fmt`
- `make markdownlint`
- `make nixie`
- `make check-fmt`
- `make lint`
- `make test` -> `Summary [ 126.157s] 1348 tests run: 1348 passed, 2 skipped`
- `make verus-clone-detector` -> `9 verified, 0 errors` for
  `clone_detector_lsh_config.rs` and `7 verified, 0 errors` for
  `clone_detector_candidate_pair.rs`
- `make verus` -> `5 verified, 0 errors`, `8 verified, 0 errors`,
  `9 verified, 0 errors`, and `7 verified, 0 errors` across the full Verus
  proof set

Lessons worth carrying into 7.2.7 and 7.2.8:

- Keep constructor proofs separate and small; they are easiest to maintain
  when one proof file maps to one runtime seam.
- For ordering proofs, model the control flow over an ordered identifier
  domain in Verus and let runtime tests pin the concrete string semantics.
- Direct constructor-level tests reduce proof drift more effectively than
  relying on indirect coverage through later pipeline stages.
