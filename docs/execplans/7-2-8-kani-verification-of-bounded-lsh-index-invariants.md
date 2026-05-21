# Verify bounded LSH index invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 7.2.8 adds bounded Kani verification for the token-pass locality
sensitive hashing index. Locality-sensitive hashing, or LSH, groups fixed-width
MinHash signatures into bands and emits candidate fragment pairs when two
fragments collide in a band. After this work, maintainers can run the
clone-detector Kani proof target and see that small bounded `LshIndex` states
preserve the candidate-pair invariants already promised by the design: no
self-pairs, canonical pair ordering, repeated-band deduplication, and
insertion-order independence.

This plan is pre-implementation only. Implementation must not begin until this
plan is explicitly approved.

## Constraints

This task covers roadmap item 7.2.8 only. It must not expand into later clone
detector phases, statistical quality proofs, AST clone detection, or SARIF
workflow changes except where regression tests already exercise the existing
externally observable behaviour.

The implementation must use the existing Rust production code paths in
`crates/whitaker_clones_core/src/index/lsh.rs` wherever possible. Kani
harnesses may add private `#[cfg(kani)]` proof seams only when direct bounded
verification of the real code is otherwise impractical. Such seams must stay
adjacent to the index code and must not become public runtime API.

The existing domain boundary must remain intact. `LshIndex`, `CandidatePair`,
`FragmentId`, `LshConfig`, and `MinHashSignature` are clone-detector domain
types in `whitaker_clones_core`; proof tooling and wrapper scripts are
infrastructure around that domain. The domain must not learn about command-line
wrappers, filesystem paths, CI layout, or adapter details.

No new external crate dependency may be added without escalation. Kani is
already integrated through `scripts/install-kani.sh`, `scripts/run-kani.sh`,
`make kani`, and `make kani-clone-detector`.

Normal Cargo builds, tests, and Clippy runs must stay free of `cfg(kani)`
unknown-configuration warnings. Any new Kani-only code must be guarded by the
existing `#[cfg(kani)]` pattern.

The implementation must preserve the public API of `whitaker_clones_core`
unless a public API change is explicitly approved. If a public signature must
change to make the proof tractable, stop and escalate.

All validation commands must be run sequentially. Long commands must use `tee`
and write logs under `/tmp`; do not use `/tmp` as a build target.

## Tolerances

Stop and ask for direction if the implementation requires more than six source
or script files to change, excluding documentation and generated lockfile
metadata.

Stop and ask for direction if the net production-code change exceeds 250 lines
or if `crates/whitaker_clones_core/src/index/lsh.rs` would exceed the
repository's 400-line code-file limit.

Stop and ask for direction before changing any public API signature, adding a
new dependency, changing the meaning of `CandidatePair::new`, or changing the
documented lexical ordering of `FragmentId` candidate pairs.

Stop and ask for direction if a Kani harness cannot complete under
`make kani-clone-detector` after two focused attempts to reduce the bounded
state space.

Stop and ask for direction if `make check-fmt`, `make lint`, or `make test`
fails for reasons not clearly caused by this branch.

Stop and ask for direction if `coderabbit review --agent` reports a concern
that cannot be resolved without widening scope beyond this plan.

## Risks

Risk: `candidate_pairs` uses `BTreeMap`, `BTreeSet`, nested iteration, and
`Vec<u64>` band keys, which can create a large symbolic state space. Severity:
high. Likelihood: medium. Mitigation: keep harnesses small, use concrete
bounded signatures where they still exercise the invariant, prefer fixed two-
or three-fragment scenarios, and add narrowly scoped `#[cfg(kani)]` helpers
only if they preserve the real `insert` and `candidate_pairs` path.

Risk: A harness could prove a tautology by constructing already-deduplicated
candidate pairs instead of exercising `LshIndex`. Severity: high. Likelihood:
medium. Mitigation: each proof harness must create an `LshIndex`, call
`LshIndex::insert`, and assert against `LshIndex::candidate_pairs`.

Risk: Insertion-order independence can become too expensive if fully symbolic
fragment identities or signatures are used. Severity: medium. Likelihood:
medium. Mitigation: prove the bounded case over a small fixed set of fragment
IDs and symbolically vary only the minimum input needed to cover the order
choice. Keep broader insertion-order coverage in ordinary `rstest` tests.

Risk: The proof target may drift from the design if scripts enumerate harnesses
manually and a new harness is omitted. Severity: medium. Likelihood: low.
Mitigation: update `scripts/run-kani.sh` in the same commit as the harnesses
and include `make kani-clone-detector` in validation.

Risk: Behaviour-driven tests may duplicate existing coverage without adding
value. Severity: low. Likelihood: medium. Mitigation: inspect
`crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs` before adding
scenarios. Add BDD coverage only for user-observable candidate generation
contracts that are not already covered.

## Progress

- [x] (2026-05-21T22:20:13Z) Loaded the requested `$leta`, `$kani`, `$verus`,
  and `$hexagonal-architecture` skills, plus the required planning, Rust, PR,
  commit, and Firecrawl workflows for this task.
- [x] (2026-05-21T22:20:13Z) Created a `leta` workspace for this worktree.
- [x] (2026-05-21T22:20:13Z) Renamed the local branch to
  `7-2-8-kani-verification-of-bounded-lsh-index-invariants`.
- [x] (2026-05-21T22:20:13Z) Used a Wyvern agent team to inspect roadmap,
  design, code, validation, and adjacent execplan conventions.
- [x] (2026-05-21T22:20:13Z) Used Firecrawl to verify current Kani
  documentation on proof harnesses, `#[kani::proof]`, and unwind bounds.
- [x] (2026-05-21T22:20:13Z) Drafted this pre-implementation ExecPlan.
- [ ] Submit the plan for review in a draft pull request.
- [ ] Await explicit approval before beginning implementation.
- [ ] After approval, implement bounded Kani `LshIndex` harnesses and
  supporting regression coverage.
- [ ] After implementation and validation, mark roadmap item 7.2.8 done.

## Surprises & discoveries

- Observation: The existing unit tests already cover the four requested
  `LshIndex` behaviours at runtime. Evidence:
  `crates/whitaker_clones_core/src/index/tests.rs` contains
  `insertion_order_does_not_change_candidate_output`,
  `canonical_ordering_across_multiple_pairs_and_bands`,
  `duplicate_band_collisions_emit_one_pair`, and `self_pairs_are_not_emitted`.
  Impact: The implementation should preserve and, if needed, sharpen these
  tests rather than duplicate them wholesale.

- Observation: Clone-detector Kani harnesses are manually allowlisted.
  Evidence: `scripts/run-kani.sh` enumerates the clone-detector harness names in
   `run_clone_detector_harnesses`. Impact: Any new 7.2.8 harness must be added
  to that list or `make kani-clone-detector` will not exercise it.

- Observation: Firecrawl showed the current Kani documentation still describes
  `#[kani::proof]` as the proof-harness marker and says an explicit unwind
  bound must be one greater than the maximum loop iterations when specified.
  Evidence: Kani attributes documentation at
  <https://model-checking.github.io/kani/reference/attributes.html>. Impact:
  The plan should require tight, documented unwind bounds.

## Decision log

- Decision: Keep 7.2.8 as Kani-focused implementation work, not Verus work.
  Rationale: ADR 003 assigns stateful, bounded `LshIndex` candidate-pair
  invariants to Kani and reserves Verus for pure constructor and ordering
  proofs. Date/Author: 2026-05-21T22:20:13Z / Codex.

- Decision: Treat ordinary tests as the first regression net and Kani as the
  exhaustive bounded invariant check. Rationale: ADR 003 explicitly says formal
  proofs do not replace unit, behaviour, or integration tests. Date/Author:
  2026-05-21T22:20:13Z / Codex.

- Decision: Keep harnesses adjacent to the index module in
  `crates/whitaker_clones_core/src/index/kani.rs`. Rationale: The crate already
  uses `#[cfg(kani)] mod kani;` in
  `crates/whitaker_clones_core/src/index/mod.rs`, and the clone-detector design
  records this as the chosen proof layout. Date/Author: 2026-05-21T22:20:13Z /
  Codex.

- Decision: Do not mark roadmap item 7.2.8 done in the plan-only pull request.
  Rationale: The user requested approval before implementation; the roadmap
  item is complete only after harnesses, tests, documentation, and gates land.
  Date/Author: 2026-05-21T22:20:13Z / Codex.

## Outcomes & retrospective

This draft plan captures the intended implementation path and approval gate. No
production code, test code, proof harness, or roadmap completion marker has
been changed yet.

## Context and orientation

The relevant crate is `crates/whitaker_clones_core`, which contains the
clone-detector token-pass domain. The `index` module is responsible for MinHash
sketching and LSH candidate generation:

- `crates/whitaker_clones_core/src/index/lsh.rs` defines `LshIndex`.
- `crates/whitaker_clones_core/src/index/types.rs` defines
  `CandidatePair`, `LshConfig`, `MinHashSignature`, and `MINHASH_SIZE`.
- `crates/whitaker_clones_core/src/index/fragment_id.rs` defines
  `FragmentId`.
- `crates/whitaker_clones_core/src/index/kani.rs` holds clone-detector Kani
  harnesses under `#[cfg(kani)]`.
- `crates/whitaker_clones_core/src/index/tests.rs` holds unit tests using
  `rstest`.
- `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs` holds
  behaviour-driven `rstest-bdd` coverage for candidate generation.
- `scripts/run-kani.sh` and the `kani-clone-detector` target in `Makefile`
  run the bounded proof harnesses.

`LshIndex` stores bucket members in a
`BTreeMap<BandBucketKey, BTreeSet<FragmentId>>`. `LshIndex::insert` splits a
`MinHashSignature` into configured bands and inserts the fragment ID into each
bucket. `LshIndex` then emits candidates through `candidate_pairs`, which
iterates over bucket member sets, calls the private `add_bucket_pairs` helper,
deduplicates pairs with `BTreeSet<CandidatePair>`, and returns a stable vector.

`CandidatePair::new` is the constructor that suppresses equal fragment IDs and
canonicalizes distinct IDs into lexical `FragmentId` order. Roadmap item 7.2.6
already added Verus proofs and direct tests for that constructor. Roadmap item
7.2.7 already added Kani checks for bounded `MinHasher::sketch` invariants.
This plan builds on those prerequisites by verifying the stateful LSH index
that consumes `MinHashSignature` values.

Relevant documentation to keep open while implementing:

- `docs/roadmap.md`, item 7.2.8.
- `docs/adr-003-formal-proof-strategy-for-clone-detector-pipeline.md`.
- `docs/whitaker-clone-detector-design.md`, especially "Implementation
  decisions" for 7.2.2 through 7.2.7 and "Proof and verification scope".
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/whitaker-dylint-suite-design.md` for repository-wide validation and
  architecture context.
- The `$leta`, `$kani`, `$verus`, `$rust-router`, and
  `$hexagonal-architecture` skills.

## Plan of work

Stage A is the approval gate. Review this plan, revise it if requested, and do
not begin code implementation until the user explicitly approves it. The draft
pull request for this stage should contain only this execplan and any review
fixes to the plan itself.

Stage B establishes the red/green regression baseline. Inspect the existing
unit tests in `crates/whitaker_clones_core/src/index/tests.rs` and BDD
scenarios in `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`. If
the four target behaviours are already covered, keep them and add only missing
edge cases, using `rstest` parameterisation rather than duplicated test
functions. If any externally observable candidate-generation behaviour is
missing from BDD, add the smallest feature scenario and step code needed. Run
the targeted crate tests before and after the change so the implementation can
demonstrate that test coverage existed or was strengthened.

Stage C adds bounded Kani harnesses in
`crates/whitaker_clones_core/src/index/kani.rs`. Add helper functions only when
they reduce noise without reimplementing the invariant. Candidate helper names
should describe the proved property, for example:

```rust
fn assert_no_self_pairs(candidates: &[CandidatePair]) { /* ... */ }
fn assert_pairs_are_canonical(candidates: &[CandidatePair]) { /* ... */ }
```

Harnesses must build real `LshIndex` values with `LshIndex::new`, populate them
via `LshIndex::insert`, and assert over `LshIndex::candidate_pairs`. Prefer
small fixed signatures such as one-band identical signatures, multi-band
identical signatures, and a distinct non-colliding signature. If symbolic
variation is used, constrain it to the smallest meaningful domain and state the
production precondition represented by each `kani::assume`.

At minimum, add harness coverage equivalent to:

- two insertions of the same `FragmentId` with the same signature produce no
  self-pair;
- two distinct fragment IDs inserted in reverse lexical order still produce
  one canonical `CandidatePair`;
- two fragments that collide in several bands still produce one candidate
  pair;
- inserting the same bounded fragment/signature set in two different orders
  produces the same candidate vector.

Use tight `#[kani::unwind(...)]` annotations. Per current Kani documentation,
the explicit unwind bound must be one greater than the maximum loop iteration
count being verified. If one shared unwind bound is too costly, split harnesses
by invariant and set each bound independently.

Stage D wires the harnesses into the proof runner. Add every new harness name
to the clone-detector list in `scripts/run-kani.sh`. Do not add Kani to the
default `make test` path. Run `make kani-clone-detector` and confirm the new
harnesses are listed in the command output.

Stage E updates documentation. Update `docs/whitaker-clone-detector-design.md`
with a new implementation-decision entry for 7.2.8 explaining what is proved,
what remains bounded, and what is left to ordinary tests. Update
`docs/developers-guide.md` if the command list or maintainer workflow changes.
Update `docs/users-guide.md` only if user visible behaviour, command-line
output, configuration, or diagnostics change; otherwise record in the PR notes
that no user-guide update was needed. Update `docs/roadmap.md` to mark 7.2.8
done only after implementation and all gates pass.

Stage F performs review and quality gates. Run `coderabbit review --agent`
after the Kani harness milestone and again before the final implementation PR
is ready. Resolve all actionable concerns. Then run the repository gates
sequentially with `tee` logs: `make check-fmt`, `make lint`, and `make test`.
Also run `make markdownlint` and `make nixie` after documentation changes.

## Concrete steps

Work from the repository root:

```bash
pwd
```

Expected output includes:

```plaintext
/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/f726e04f-c7e3-4930-9737-459a534bbe74
```

Confirm the branch and workspace:

```bash
git branch --show-current
leta files crates/whitaker_clones_core/src/index
```

Expected branch:

```plaintext
7-2-8-kani-verification-of-bounded-lsh-index-invariants
```

Before implementation, run targeted existing tests to establish the baseline:

```bash
set -o pipefail
make test TEST_ARGS='-p whitaker_clones_core index::tests::' \
  2>&1 | tee /tmp/test-whitaker-7-2-8-index-baseline.out
```

If the project `Makefile` does not support `TEST_ARGS`, use the nearest
documented target from the `Makefile` and record the substitution in the
Decision Log before proceeding.

After adding or adjusting unit and BDD tests, run the targeted crate tests:

```bash
set -o pipefail
cargo nextest run -p whitaker_clones_core --all-targets --all-features \
  2>&1 | tee /tmp/test-whitaker-7-2-8-clones-core.out
```

After adding Kani harnesses and script wiring, run:

```bash
set -o pipefail
make kani-clone-detector \
  2>&1 | tee /tmp/kani-whitaker-7-2-8-clone-detector.out
```

A successful run should show each new harness name and end each harness with
Kani verification success.

After documentation updates, run:

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/markdownlint-whitaker-7-2-8.out
set -o pipefail
make nixie 2>&1 | tee /tmp/nixie-whitaker-7-2-8.out
```

Before any implementation commit is considered complete, run:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-7-2-8.out
set -o pipefail
make lint 2>&1 | tee /tmp/lint-whitaker-7-2-8.out
set -o pipefail
make test 2>&1 | tee /tmp/test-whitaker-7-2-8.out
```

After each major milestone, run:

```bash
coderabbit review --agent
```

Resolve all concerns before moving to the next milestone.

## Validation and acceptance

The plan-only pull request is accepted when this document exists at
`docs/execplans/7-2-8-kani-verification-of-bounded-lsh-index-invariants.md`,
the branch is pushed, the pull request is a draft, and reviewers can approve or
request changes before implementation begins.

The eventual implementation is accepted when:

- `crates/whitaker_clones_core/src/index/kani.rs` contains bounded Kani
  harnesses for no self-pairs, canonical pair ordering, repeated-band
  deduplication, and insertion-order independence.
- `scripts/run-kani.sh` runs those harnesses through `make kani-clone-detector`.
- Unit tests using `rstest` cover happy paths, unhappy paths, and relevant
  edge cases for `LshIndex` candidate generation.
- `rstest-bdd` coverage is added or explicitly judged already sufficient for
  externally observable candidate-generation behaviour.
- `docs/whitaker-clone-detector-design.md` records the 7.2.8 proof decision.
- `docs/developers-guide.md` records any changed maintainer proof workflow.
- `docs/users-guide.md` is updated if, and only if, user-visible behaviour
  changes.
- `docs/roadmap.md` marks item 7.2.8 done after implementation gates pass.
- `coderabbit review --agent` has no unresolved actionable concerns.
- `make kani-clone-detector`, `make check-fmt`, `make lint`, and `make test`
  all pass.

## Idempotence and recovery

All planned edits are ordinary source, script, and documentation changes. If a
Kani harness becomes too slow, revert only the newest harness or helper in the
current branch and record the reason in the Decision Log. Do not change public
runtime semantics to satisfy the verifier without approval.

If `scripts/run-kani.sh` is edited and a harness name is misspelled, rerun
`make kani-clone-detector`; the wrapper should fail on the missing harness. Fix
the spelling and rerun the same command.

If a quality gate fails because of unrelated main-branch drift or another
agent's changes, stop, record the evidence in this plan, and ask for direction.
Do not revert unrelated work.

## Artifacts and notes

Wyvern planning agents reported three useful facts:

- The roadmap and ADR assign exactly these four 7.2.8 invariants to Kani.
- The current `LshIndex` API is `new`, `insert`, and `candidate_pairs`, with
  private pair construction through `add_bucket_pairs`.
- Existing validation practice requires explicit Kani harness enumeration,
  ordinary tests first, sequential gates, and CodeRabbit review after major
  milestones.

Firecrawl was used to resolve current Kani tooling guidance. The Kani
attributes reference confirms that `#[kani::proof]` marks proof harnesses, that
`#[kani::unwind(n)]` controls loop unwinding, and that an explicit unwind bound
may fail with unwinding assertions if it is too low.

## Interfaces and dependencies

The implementation should keep these interfaces stable:

```rust
pub struct LshIndex {
    config: LshConfig,
    buckets: BTreeMap<BandBucketKey, BTreeSet<FragmentId>>,
}

impl LshIndex {
    pub fn new(config: LshConfig) -> Self;
    pub fn insert(&mut self, id: &FragmentId, signature: &MinHashSignature);
    pub fn candidate_pairs(&self) -> Vec<CandidatePair>;
}

impl CandidatePair {
    pub fn new(left: FragmentId, right: FragmentId) -> Option<Self>;
    pub fn left(&self) -> &FragmentId;
    pub fn right(&self) -> &FragmentId;
}
```

The proof implementation may add private helper functions in
`crates/whitaker_clones_core/src/index/kani.rs`. It may add private
`#[cfg(kani)]` constructors or fixtures only if they reduce verifier load
without changing production semantics. It must not add a new port or adapter
layer for formal verification; the existing boundary is sufficient because the
proof tooling is an external validation adapter around the domain crate.

Revision note: Initial draft created from the roadmap, ADR 003, clone-detector
design, existing `LshIndex` code, nearby proof execplans, Wyvern planning
reports, and current Kani documentation. This establishes the approval gate and
implementation route; it does not authorize code implementation yet.
