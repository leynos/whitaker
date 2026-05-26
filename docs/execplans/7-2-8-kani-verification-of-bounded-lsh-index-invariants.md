# Verify bounded LSH index invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

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
- [x] (2026-05-21T22:20:13Z) Submitted the plan for review in draft pull
  request #232.
- [x] (2026-05-26T19:06:20Z) Received explicit implementation approval from
  the user and moved this plan to `IN PROGRESS`.
- [x] (2026-05-26T19:06:20Z) Established the existing index-test baseline with
  `cargo nextest run -p whitaker_clones_core --all-targets --all-features
  index::`; 31 tests passed and 60 were skipped by the filter.
- [x] (2026-05-26T19:06:20Z) Added bounded `LshIndex` Kani harnesses for no
  self-pairs, canonical pair ordering, repeated-band deduplication, and
  insertion-order independence.
- [x] (2026-05-26T19:06:20Z) Wired the new Kani harnesses into
  `scripts/run-kani.sh`.
- [x] (2026-05-26T19:32:45Z) Stopped the first direct
  `make kani-clone-detector` attempt after it spent over twenty minutes in
  `BTreeSet<CandidatePair>` and `FragmentId` comparison internals for the first
  new harness without reaching a result.
- [x] (2026-05-26T19:32:45Z) Added a private `#[cfg(kani)]`
  `candidate_pair_summary_for_kani` seam beside `LshIndex` so the new
  harnesses still exercise `LshIndex::insert` and bucket pair construction, but
  avoid model-checking production `BTreeSet<CandidatePair>` insertion.
- [x] (2026-05-26T19:48:41Z) Stopped the second
  `make kani-clone-detector` attempt after it reached the first new LSH harness
  and again spent verifier time inside `BTreeSet<FragmentId>` traversal.
- [x] (2026-05-26T19:48:41Z) Tightened the Kani seam to maintain a private
  insertion log during `LshIndex::insert` and summarize bounded collisions from
  that log, avoiding both production B-tree traversal and production candidate
  pair B-tree insertion in proof builds.
- [x] (2026-05-26T20:04:22Z) Stopped the insertion-log
  `make kani-clone-detector` attempt after the first new harness still spent
  time in production B-tree insertion and drop paths populated by
  `LshIndex::insert`.
- [x] (2026-05-26T20:04:22Z) Converted `LshIndex::insert` to use proof-only
  insertion-log storage in `#[cfg(kani)]` builds and production B-tree storage
  in normal builds.
- [x] (2026-05-26T20:08:10Z) Moved production B-tree storage itself behind
  `#[cfg(not(kani))]` so Kani builds do not model B-tree drop paths for empty
  proof states.
- [x] (2026-05-26T20:25:16Z) Replaced heap-backed proof storage with fixed
  arrays and compact Kani band keys, and shortened harness fragment IDs to
  one-character values.
- [x] (2026-05-26T20:42:03Z) Observed the fixed-array proof build reach the
  new LSH harness and fail on the fixed insertion-array drop loop because the
  explicit unwind bound was too low.
- [x] (2026-05-26T20:42:03Z) Raised the new LSH harness unwind bound to five
  and gated production-only `BandBucketKey` definitions out of Kani builds.
- [x] (2026-05-26T20:59:55Z) Re-ran `make kani-clone-detector`; all existing
  clone-detector harnesses and the four new bounded `LshIndex` harnesses
  verified successfully.
- [x] (2026-05-26T21:04:38Z) Ran the deterministic milestone gates:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`; all passed.
- [x] (2026-05-26T21:09:18Z) Ran `coderabbit review --agent` for the
  validated Kani harness milestone; CodeRabbit reported zero findings.
- [x] (2026-05-26T21:17:02Z) Updated the clone-detector design and developer
  guide with the bounded `LshIndex` Kani proof shape. `docs/users-guide.md`
  required no change because there is no user-visible behaviour or interface
  change.
- [x] (2026-05-26T21:17:02Z) Marked roadmap item 7.2.8 done after the Kani
  harness milestone and deterministic gates passed.
- [x] (2026-05-26T21:19:41Z) Validated the documentation milestone with
  `make check-fmt`, `make markdownlint`, and `make nixie`; all passed.
- [x] (2026-05-26T21:26:31Z) Ran `coderabbit review --agent` for the
  validated documentation milestone; CodeRabbit reported zero findings.
- [x] (2026-05-26T21:33:45Z) Re-ran the final branch-tip gates:
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie`; all passed.

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

- Discovery: Directly calling `LshIndex::candidate_pairs` from the new Kani
  harnesses made the solver spend its budget inside standard-library
  `BTreeSet<CandidatePair>` insertion, B-tree navigation, allocator paths, and
  `FragmentId` string comparison before reaching the LSH invariant. Evidence:
  `/tmp/kani-whitaker-7-2-8-clone-detector.out` shows repeated unwinding in
  `alloc::collections::btree` and `memcmp` while checking
  `verify_lsh_index_rejects_self_pairs`; the attempt was interrupted with exit
  code 130 after more than twenty minutes. Impact: A narrow Kani-only summary
  helper is justified by the plan's proof-seam constraint.

- Discovery: The first summary helper still traversed production
  `BTreeSet<FragmentId>` members and hit the same standard-library modelling
  problem at a different point. Evidence:
  `/tmp/kani-whitaker-7-2-8-clone-detector-seam.out` shows the run reached
  `verify_lsh_index_rejects_self_pairs` and then repeatedly unwound
  `alloc::collections::btree::navigate` for `FragmentId` keys before the run
  was interrupted. Impact: The proof seam must avoid reading the production
  B-tree storage altogether while still being populated by the real
  `LshIndex::insert` transition.

- Discovery: Recording an insertion log was not enough while `#[cfg(kani)]`
  builds still populated the production `BTreeMap<BandBucketKey,
  BTreeSet<FragmentId>>`. Evidence:
  `/tmp/kani-whitaker-7-2-8-clone-detector-log.out` reached
  `verify_lsh_index_rejects_self_pairs` and then unwound B-tree insertion and
  deallocation paths for `FragmentId` keys. Impact: `LshIndex::insert` must use
  proof-only storage in Kani builds; otherwise the proof remains about
  standard-library tree allocation rather than the bounded LSH invariant.

- Discovery: Even an empty production `BTreeMap` field in the Kani build can
  pull B-tree deallocation paths into the proof. Evidence: the same log showed
  `Dying` B-tree node traversal during harness teardown. Impact: the production
  B-tree field and production `candidate_pairs()` implementation must be
  absent from `#[cfg(kani)]` builds, not merely unused.

- Discovery: After removing production B-tree storage, the first new harness
  failed on Kani unwinding through proof-only `Vec` allocation/drop and `memcmp`
  for long heap-backed keys. Evidence:
  `/tmp/kani-whitaker-7-2-8-clone-detector-no-btree.out` reports an unwinding
  failure in `memcmp` plus undetermined checks in `Vec<BandBucketKey>` and
  proof-summary iteration. Impact: proof-only storage must be fixed-size and
  use compact comparable values.

- Discovery: Fixed-size proof storage removed the heap-backed key work, but
  the first new harness failed because `#[kani::unwind(4)]` was too low for
  dropping the four-slot insertion array. Evidence:
  `/tmp/kani-whitaker-7-2-8-clone-detector-fixed-storage.out` reports
  `std::ptr::drop_in_place::<[Option<InsertedFragmentForKani>; 4]>` as the
  failed unwinding assertion. Impact: the LSH harnesses need
  `#[kani::unwind(5)]`, matching Kani's documented "one greater than the
  maximum loop iterations" rule for a four-slot proof array.

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

- Decision: Begin implementation under the approved ExecPlan.
  Rationale: The user explicitly requested implementation on 2026-05-26, so
  the approval gate is satisfied and the plan status can move from `DRAFT` to
  `IN PROGRESS`.
  Date/Author: 2026-05-26T19:06:20Z / Codex.

- Decision: Use `cargo nextest run -p whitaker_clones_core --all-targets
  --all-features index::` for the targeted baseline instead of the draft
  `make test TEST_ARGS=...` command.
  Rationale: The repository Makefile does not consume `TEST_ARGS`; direct
  `cargo nextest` is the nearest supported command for the intended filtered
  baseline.
  Date/Author: 2026-05-26T19:06:20Z / Codex.

- Decision: Keep the new LSH index harnesses concrete rather than symbolic.
  Rationale: The invariant under test is the `LshIndex` state transition and
  candidate emission path, while symbolic `BTreeMap`, `BTreeSet`, `Vec`, and
  `String` states would add solver cost without improving this bounded
  contract. Concrete one-band and two-band signatures still exercise real
  `LshIndex::insert` and `LshIndex::candidate_pairs` behaviour.
  Date/Author: 2026-05-26T19:06:20Z / Codex.

- Decision: Replace direct Kani assertions over `candidate_pairs()` with a
  private `#[cfg(kani)]` summary helper that uses the same inserted buckets and
  `CandidatePair::new` policy, but deduplicates the bounded proof result
  without constructing a production `BTreeSet<CandidatePair>`.
  Rationale: Runtime `rstest` and `rstest-bdd` coverage already exercises the
  public `candidate_pairs()` path. Kani should prove the bounded domain
  invariant over `LshIndex` state, not exhaust allocator and tree-balancing
  internals of the standard library.
  Date/Author: 2026-05-26T19:32:45Z / Codex.

- Decision: Back the Kani summary helper with a private insertion log recorded
  by `LshIndex::insert` in `#[cfg(kani)]` builds.
  Rationale: This keeps the proof tied to the production insertion transition
  and the `CandidatePair::new` canonicalization policy, while avoiding symbolic
  traversal of `BTreeMap` and `BTreeSet` internals that are not the subject of
  roadmap item 7.2.8.
  Date/Author: 2026-05-26T19:48:41Z / Codex.

- Decision: In `#[cfg(kani)]` builds, make `LshIndex::insert` record only the
  proof insertion log and skip production B-tree storage.
  Rationale: Three focused proof attempts showed that any contact with the
  production tree storage makes the bounded proof impractical. The runtime API
  and production implementation remain unchanged, while the Kani build still
  verifies band computation, repeated insertion, pair canonicalization, and
  deduplication policy over bounded inserted states.
  Date/Author: 2026-05-26T20:04:22Z / Codex.

- Decision: Compile production B-tree storage and `candidate_pairs()` only for
  non-Kani builds.
  Rationale: `candidate_pairs()` remains the normal public runtime path and is
  covered by existing unit and behavioural tests. Kani builds need a domain
  proof representation without standard-library B-tree allocation and drop
  machinery.
  Date/Author: 2026-05-26T20:08:10Z / Codex.

- Decision: Represent Kani inserted fragments with fixed arrays and compact
  first-lane band keys.
  Rationale: The bounded harnesses only use one-band and two-band repeated
  signatures, so a compact band key preserves the collision cases being proved
  while removing heap allocation, `Vec` drop, and long slice comparison from
  the proof obligation.
  Date/Author: 2026-05-26T20:25:16Z / Codex.

- Decision: Set the four new LSH harnesses to `#[kani::unwind(5)]` and compile
  `BandBucketKey` only for non-Kani builds.
  Rationale: The Kani proof representation has a four-slot insertion array,
  and Kani requires an unwind bound one greater than the maximum loop
  iterations. `BandBucketKey` is production-only after the proof-storage split,
  so leaving it in Kani builds only creates dead-code warnings.
  Date/Author: 2026-05-26T20:42:03Z / Codex.

## Outcomes & retrospective

This plan has moved from drafting into implementation. The first
implementation step is to establish the existing regression baseline before
adding Kani harnesses or modifying documentation.

The targeted index baseline has passed. The first code milestone adds four
bounded Kani harnesses in `crates/whitaker_clones_core/src/index/kani.rs` and
adds them to the `clone-detector` harness list in `scripts/run-kani.sh`.

The first direct Kani attempt showed that proving through public
`candidate_pairs()` is impractical for the bounded proof target because CBMC
spends its time in `BTreeSet<CandidatePair>` implementation details. The
implementation now uses a Kani-only summary seam next to `LshIndex`; this keeps
the proof adjacent to the domain code and preserves the public runtime API.

The first summary seam still traversed production `BTreeSet<FragmentId>` state,
which remained impractical. The seam now records the bounded insertion facts as
they pass through `LshIndex::insert` and proves collision pairing from that
log.

The insertion log also needs to be the only storage populated in `#[cfg(kani)]`
builds. Otherwise the model checker still verifies B-tree insertion and drop
internals before reaching the LSH assertions.

The proof representation now uses fixed arrays and compact Kani-only band
keys. The latest validation adjustment raises the LSH harness unwind bound to
cover the four-slot proof array and removes production-only bucket-key code
from Kani builds.

The Kani milestone now passes. The wrapper verified the pre-existing
`LshConfig` and `MinHasher` harnesses plus the four new bounded `LshIndex`
harnesses for no self-pairs, canonical ordering, repeated-band deduplication,
and insertion-order independence.

The deterministic milestone gates also pass. CodeRabbit review can now be
requested for this milestone without using it as a substitute for local checks.

The documentation milestone records the bounded proof shape in both the clone
detector design and the developer guide. The users guide remains unchanged
because the work adds verification coverage only; it does not alter CLI
behaviour, output format, configuration, persistence, or other user-facing
semantics.

The documentation milestone's local gates pass. CodeRabbit review can now be
requested for this documentation slice.

CodeRabbit also reported zero findings for the documentation milestone.

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

Revision note: On 2026-05-26, the user explicitly approved implementation.
The plan status changed to `IN PROGRESS`, progress now records the approval,
and the remaining work begins with the baseline validation and Kani harness
implementation stages.

Revision note: The first implementation update records the targeted baseline
command substitution, the concrete proof-shape decision, and the addition of
four bounded `LshIndex` Kani harnesses plus runner wiring. Remaining work is
to validate the harness milestone, run CodeRabbit, update documentation, and
complete the final gates.
