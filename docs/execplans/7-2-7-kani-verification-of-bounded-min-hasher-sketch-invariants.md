# Verify bounded MinHasher sketch invariants

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 7.2.7 adds bounded Kani verification for `MinHasher::sketch`, the
clone-detector token-pass function that turns retained token fingerprints into
a fixed-width MinHash signature. After this work is approved and implemented,
maintainers can run `make kani-clone-detector` and observe machine-checked
coverage for three implementation invariants:

- Sketching the same bounded retained fingerprints is deterministic.
- Repeating an already present fingerprint hash does not change the sketch.
- Sketching an empty retained-fingerprint input fails with
  `IndexError::EmptyFingerprintSet`.

This plan deliberately does not claim to prove MinHash statistical quality,
collision probability, or unbounded set behaviour. It proves bounded
implementation behaviour over the real Rust code, in line with
`docs/adr-003-formal-proof-strategy-for-clone-detector-pipeline.md`.

The implementation must not begin until this plan is explicitly approved.

## Constraints

- Work must implement roadmap item 7.2.7 only. Do not start roadmap item 7.2.8
  for `LshIndex`.
- Kani harnesses must call the real `MinHasher::sketch` implementation in
  `crates/whitaker_clones_core/src/index/minhash.rs`, not a clean-room model.
- Kani proof code must stay behind `#[cfg(kani)]` in
  `crates/whitaker_clones_core/src/index/kani.rs` or a child module reached
  from there.
- Do not add a runtime dependency for proof tooling. Reuse
  `scripts/install-kani.sh`, `scripts/run-kani.sh`, `make kani`, and
  `make kani-clone-detector`.
- Do not widen the public API solely for Kani. Prefer colocated harnesses and
  crate-private helpers when a small proof seam is justified.
- Keep ordinary `rstest` unit tests and `rstest-bdd` behaviour tests as the
  first regression net. Kani complements these tests; it does not replace them.
- Use Oxford British English in documentation.
- Run commands through Makefile targets where targets exist.
- Run formatting, linting, and tests sequentially, with `tee` output logs under
  `/tmp`; do not run test, lint, format, or proof commands in parallel.
- Do not use `/tmp` as a build target. Use it only for logs or scratch output.
- Do not revert unrelated working-tree changes.
- Commit atomic changes only after the relevant gates pass.
- Run `coderabbit review --agent` after each major milestone and clear all
  concerns before moving to the next milestone.
- On completion of the feature, mark item 7.2.7 done in `docs/roadmap.md`.

## Tolerances

- Scope: if the implementation requires modifying more than eight repository
  files, stop and ask for approval. Expected files are
  `crates/whitaker_clones_core/src/index/kani.rs`, `scripts/run-kani.sh`,
  `crates/whitaker_clones_core/src/index/tests.rs`,
  `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`,
  `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature`,
  `docs/whitaker-clone-detector-design.md`, `docs/developers-guide.md`, and
  `docs/roadmap.md`.
- API: if a public function signature, public type, or public error variant
  must change, stop and ask for approval.
- Dependencies: if any new Cargo dependency, system package, or verifier
  package is required, stop and ask for approval.
- Proof bounds: if a MinHasher harness needs more than four symbolic retained
  fingerprints, or any `#[kani::unwind(...)]` value above 16, stop and explain
  the state-space risk before proceeding.
- Validation: if `make check-fmt`, `make lint`, `make test`, or
  `make kani-clone-detector` still fails after two focused fix attempts, stop
  and record the blocker.
- Review: if `coderabbit review --agent` reports a concern that cannot be
  resolved without changing the approved scope, stop and ask for direction.
- Ambiguity: if duplicate-hash insensitivity could mean either duplicate
  `Fingerprint.hash` values only or full duplicate `Fingerprint` structs
  including ranges, prefer hash-only set semantics as documented in
  `docs/whitaker-clone-detector-design.md`; stop only if code contradicts that
  contract.

## Risks

- Risk: Kani state-space explosion from `BTreeSet` construction and
  `array::from_fn` over the 128-wide signature. Severity: medium. Likelihood:
  medium. Mitigation: keep symbolic inputs small, prove one property per
  harness, and use fixed-size arrays with explicit active lengths where needed.

- Risk: An over-broad proof proves a helper model rather than production code.
  Severity: high. Likelihood: low. Mitigation: every harness must call
  `MinHasher::new().sketch(...)` and inspect the returned `MinHashSignature` or
  `IndexError`.

- Risk: Empty-input verification is trivial if it only checks `sketch(&[])`
  with no symbolic path. Severity: low. Likelihood: medium. Mitigation: include
  a concrete empty-input Kani harness and retain ordinary tests; the property
  is intentionally a boundary error-path check.

- Risk: Duplicate-hash proofs accidentally depend on byte-range equality.
  Severity: medium. Likelihood: medium. Mitigation: use
  `Fingerprint::new(hash, range)` values with different ranges but repeated
  hash values in unit tests and, where tractable, in Kani input construction.

- Risk: `coderabbit review --agent` may be unavailable in the local
  environment. Severity: medium. Likelihood: medium. Mitigation: run it after
  each milestone; if the command is missing or fails for authentication or
  service reasons, record the exact failure and ask for direction before
  treating the milestone as complete.

## Progress

- [x] 2026-05-18: Read `AGENTS.md` and loaded the required `execplans`,
  `leta`, `rust-router`, `kani`, `commit-message`, `pr-creation`,
  `en-gb-oxendict-style`, and `firecrawl-mcp` skills for the planning task.
- [x] 2026-05-18: Confirmed the current branch is
  `feat/kani-minhasher-plan`, not `main`.
- [x] 2026-05-18: Used a Wyvern agent team to inspect code, documentation, and
  validation conventions for item 7.2.7.
- [x] 2026-05-18: Used Firecrawl to check current Kani guidance on proof
  harnesses, `kani::assume`, and loop unwinding.
- [x] 2026-05-18: Drafted this pre-implementation ExecPlan.
- [ ] Await explicit plan approval before implementation.
- [ ] Rename the working branch to
  `7-2-7-kani-verification-of-bounded-min-hasher-sketch-invariants` and push it
  tracking
  `origin/7-2-7-kani-verification-of-bounded-min-hasher-sketch-invariants`.
- [ ] Create a draft pull request for this plan.
- [ ] After approval, implement the bounded Kani harnesses and tests.
- [ ] After implementation, update documentation and mark roadmap item 7.2.7
  done.

## Surprises & Discoveries

- `MinHasher::sketch` already rejects empty input, deduplicates hashes through
  `BTreeSet<u64>`, and has ordinary unit coverage for determinism, duplicate
  hash insensitivity, empty input, and set-order independence in
  `crates/whitaker_clones_core/src/index/tests.rs`.
- Existing clone-detector Kani harnesses are already colocated in
  `crates/whitaker_clones_core/src/index/kani.rs`, but currently cover only
  `LshConfig::new`.
- `scripts/run-kani.sh` runs clone-detector harnesses by an explicit name list,
  so adding 7.2.7 harnesses also requires updating that script.
- `docs/users-guide.md` does not currently document clone-detector proof
  workflow. This feature is maintainer-facing, so the user guide should remain
  unchanged unless the implementation changes user-visible CLI behaviour.
- Official Kani documentation confirms that bounded proofs require explicit
  finite input bounds, and that unwind bounds often need to be one greater than
  the maximum loop iteration count.

## Decision Log

- Decision: keep item 7.2.7 as a pre-implementation plan until the user
  approves it. Rationale: the user explicitly stated that the plan must be
  approved before implementation. Date/Author: 2026-05-18 / Codex.

- Decision: implement the future-proof work in the existing
  `crates/whitaker_clones_core/src/index/kani.rs` harness location, unless the
  file becomes too large or the helper structure becomes unclear. Rationale:
  ADR 003 and the clone-detector design already establish colocated `cfg(kani)`
  harnesses as the clone-detector proof pattern. Date/Author: 2026-05-18 /
  Codex.

- Decision: use bounded arrays plus active lengths for symbolic fingerprint
  inputs. Rationale: Kani verifies every value in the bounded state space;
  fixed-size arrays keep the proof finite and make duplicate insertion
  explicit. Date/Author: 2026-05-18 / Codex.

- Decision: do not add a new ADR for 7.2.7 unless implementation uncovers a
  substantive strategy change. Rationale: ADR 003 already decides the proof
  split and names `MinHasher::sketch` as a Kani target. Date/Author: 2026-05-18
  / Codex.

## Outcomes & Retrospective

No implementation has been performed yet. The expected outcome is a set
of Kani harnesses, tests, and documentation updates that demonstrate
`MinHasher::sketch` behaves deterministically, ignores duplicate hashes under
set semantics, and rejects empty inputs. This section must be updated after
implementation with validation logs, CodeRabbit results, and any deviations
from the plan.

## Orientation

The relevant implementation lives in the `whitaker_clones_core` crate.
`crates/whitaker_clones_core/src/index/minhash.rs` defines `MinHasher`.
`MinHasher::new` builds a deterministic 128-seed family from fixed SplitMix64
constants. `MinHasher::sketch` accepts a slice of `Fingerprint` values,
collapses the `Fingerprint.hash` values into a `BTreeSet<u64>`, mixes each
unique hash with each seed, and returns a `MinHashSignature`.

The surrounding index module is exposed through
`crates/whitaker_clones_core/src/index/mod.rs` and re-exported from
`crates/whitaker_clones_core/src/lib.rs`. Existing unit tests live in
`crates/whitaker_clones_core/src/index/tests.rs`. Existing behaviour tests live
in `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`, backed by
`crates/whitaker_clones_core/tests/features/min_hash_lsh.feature`.

Formal verification entry points already exist. `make kani-clone-detector`
calls `scripts/run-kani.sh clone-detector`, which invokes the pinned
`cargo-kani` binary against explicit harness names in
`crates/whitaker_clones_core/src/index/kani.rs`.

Relevant documents to keep open during implementation:

- `docs/roadmap.md`, item 7.2.7.
- `docs/adr-003-formal-proof-strategy-for-clone-detector-pipeline.md`.
- `docs/whitaker-clone-detector-design.md`, especially the MinHash and LSH
  sections and the implementation decisions for 7.2.2 through 7.2.5.
- `docs/developers-guide.md`, especially the Kani bounded model checking
  section.
- `docs/whitaker-dylint-suite-design.md`.
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/rstest-bdd-users-guide.md`.

Relevant skills for the implementer:

- `leta`, for code navigation and symbol relationships.
- `rust-router`, followed by `kani` for bounded model checking.
- `rust-errors`, if the implementation needs to inspect or extend
  `IndexError` handling.
- `rust-types-and-apis`, only if a proof seam appears to require type or API
  changes.
- `nextest`, if `make test` failures require focused nextest diagnosis.
- `commit-message`, for every commit.
- `pr-creation`, when opening or updating the pull request.

## External research notes

Firecrawl was used on 2026-05-18 to confirm current public Kani guidance from
the official Kani documentation:

- The Kani first-steps tutorial describes proof harnesses as test-like entry
  points that use `kani::any()` for symbolic values and `kani::assume()` to
  encode real preconditions:
  <https://model-checking.github.io/kani/tutorial-first-steps.html>.
- The Kani loop-unwinding tutorial explains that Kani proofs over loops are
  bounded, must constrain problem size, and often require explicit
  `#[kani::unwind(...)]` values high enough to avoid unwinding assertion
  failures:
  <https://model-checking.github.io/kani/tutorial-loop-unwinding.html>.

These notes reinforce the local repository guidance in
`docs/developers-guide.md`: use small, explicit bounds and treat Kani as
bounded implementation verification, not as unbounded mathematical proof.

## Implementation plan

Do not start this section until the plan is approved.

### Milestone 1: Baseline and branch preparation

Rename the branch and make it track the requested remote branch:

```sh
BRANCH=7-2-7-kani-verification-of-bounded-min-hasher-sketch-invariants
git branch -m "${BRANCH}"
git push -u origin "${BRANCH}"
```

Record the initial state:

```sh
git status --short --branch
```

Run baseline checks before touching implementation files, with logs:

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-7-fmt-base.out
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-7-lint-base.out
set -o pipefail && make test 2>&1 | tee /tmp/7-2-7-test-base.out
set -o pipefail && \
  make kani-clone-detector 2>&1 | tee /tmp/7-2-7-kani-base.out
```

Expected result: all commands exit 0. If a baseline check fails before the
implementation starts, inspect whether the failure is unrelated to existing
state. Do not hide or work around it; record it in this plan and ask for
direction if it blocks the work.

Run:

```sh
coderabbit review --agent
```

Expected result: no concerns that block Milestone 2. Resolve or escalate every
concern before proceeding.

### Milestone 2: Strengthen ordinary regression coverage

Update ordinary tests before adding Kani harnesses, so there is a concrete
red/green regression net. The likely file is
`crates/whitaker_clones_core/src/index/tests.rs`.

Add `rstest`-based coverage for these cases if they are not already explicit
enough:

- Empty fingerprint slices return `Err(IndexError::EmptyFingerprintSet)`.
- Two `MinHasher::new()` instances sketch the same fingerprints identically.
- Duplicate `Fingerprint.hash` values with different ranges do not change the
  signature.
- Reordered unique hash sets produce identical signatures.

The repository already has close coverage for these behaviours, so this
milestone may be a small refactor from plain `#[test]` functions into
parameterized `#[rstest]` cases rather than new behavioural assertions. Avoid
rewriting tests just for style if the existing tests already communicate the
contract clearly.

Extend the `rstest-bdd` feature and harness only if an end-to-end behaviour gap
remains after inspecting the existing scenarios. Candidate generation already
exercises empty-input failure through `MinHasher::sketch`; add a new scenario
only if duplicate-hash insensitivity or deterministic sketching is observable
through candidate generation without making the feature file noisy.

Validate this milestone:

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-7-fmt-tests.out
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-7-lint-tests.out
set -o pipefail && make test 2>&1 | tee /tmp/7-2-7-test-tests.out
coderabbit review --agent
```

Expected result: formatting, linting, and tests pass, and CodeRabbit has no
unresolved concerns. Commit this milestone only after the gates pass.

### Milestone 3: Add bounded Kani harnesses for MinHasher

Extend `crates/whitaker_clones_core/src/index/kani.rs` with focused harnesses.
Keep one property per harness, with names suitable for the explicit list in
`scripts/run-kani.sh`.

Recommended harness names:

- `verify_min_hasher_sketch_rejects_empty_input`.
- `verify_min_hasher_sketch_is_deterministic`.
- `verify_min_hasher_sketch_ignores_duplicate_hashes`.

Use helper functions in the Kani module if they improve clarity. A likely
pattern is:

```rust
const MAX_SYMBOLIC_FINGERPRINTS: usize = 3;

fn bounded_fingerprint_inputs() -> (
    [Fingerprint; MAX_SYMBOLIC_FINGERPRINTS],
    usize,
) {
    // Build a fixed-size symbolic array and an active length constrained with
    // `kani::assume(active_len <= MAX_SYMBOLIC_FINGERPRINTS)`.
}
```

The helper must create valid `Fingerprint` values through the real
`Fingerprint::new(hash, range)` constructor. If symbolic ranges make the state
space too large, use deterministic small ranges and keep the hash values
symbolic; the invariant under proof is hash-set behaviour, not byte-range
ordering.

The deterministic harness should build two sketches from equivalent bounded
inputs and assert that `values()` are equal. The duplicate-insensitivity
harness should build one sketch from a bounded non-empty unique prefix and a
second sketch from the same prefix plus a repeated hash value, then assert the
signatures are equal. The empty-input harness should assert the exact
`IndexError::EmptyFingerprintSet` error.

Use explicit `#[kani::unwind(...)]` values. Start with the smallest practical
bound that covers:

- active-length construction,
- `BTreeSet` insertion for at most three fingerprints,
- 128 signature slots, and
- any helper loops used by the harness.

If the unwind value needs to exceed 16, stop under the proof-bound tolerance.

Update `scripts/run-kani.sh` so `run_clone_detector_harnesses` invokes the new
MinHasher harness names after the existing `LshConfig` harnesses. This keeps
`make kani-clone-detector` as the one supported entry point.

Validate this milestone:

```sh
set -o pipefail && \
  make kani-clone-detector 2>&1 | tee /tmp/7-2-7-kani-minhash.out
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-7-fmt-minhash.out
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-7-lint-minhash.out
set -o pipefail && make test 2>&1 | tee /tmp/7-2-7-test-minhash.out
coderabbit review --agent
```

Expected result: the Kani output reports successful verification for all new
MinHasher harnesses and the existing clone-detector harnesses; normal gates
pass. Commit this milestone only after the gates pass.

### Milestone 4: Update documentation

Update `docs/whitaker-clone-detector-design.md` with an implementation decision
section for 7.2.7. It should state:

- the harnesses exercise `MinHasher::sketch` directly;
- the proof bounds are intentionally small and bounded;
- duplicate-hash insensitivity means duplicate `Fingerprint.hash` values are
  collapsed before sketching, regardless of `Fingerprint` range; and
- ordinary tests and behaviour scenarios remain the regression safety net.

Update `docs/developers-guide.md` so the clone-detector proof workflow names
the new MinHasher harness group or explains that `make kani-clone-detector` now
covers both `LshConfig::new` and `MinHasher::sketch`.

Do not update `docs/users-guide.md` unless implementation changes user-visible
commands, output, configuration, or CLI behaviour. If it remains unchanged,
record that decision in this plan's `Decision Log`.

Validate Markdown:

```sh
set -o pipefail && make fmt 2>&1 | tee /tmp/fmt-whitaker-7-2-7-docs.out
set -o pipefail && \
  make markdownlint 2>&1 | tee /tmp/7-2-7-mdlint-docs.out
set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-whitaker-7-2-7-docs.out
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-7-fmt-docs.out
set -o pipefail && make lint 2>&1 | tee /tmp/lint-whitaker-7-2-7-docs.out
set -o pipefail && make test 2>&1 | tee /tmp/test-whitaker-7-2-7-docs.out
set -o pipefail && \
  make kani-clone-detector 2>&1 | tee /tmp/7-2-7-kani-docs.out
coderabbit review --agent
```

Expected result: documentation and normal gates pass. Commit this milestone
only after the gates pass.

### Milestone 5: Final gates, roadmap completion, and pull request

Once code, tests, proofs, and documentation are complete, mark roadmap item
7.2.7 done in `docs/roadmap.md` by changing its checkbox from `[ ]` to `[x]`.
Do not mark 7.2.8 done.

Run the final required gates:

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/7-2-7-fmt-final.out
set -o pipefail && make lint 2>&1 | tee /tmp/7-2-7-lint-final.out
set -o pipefail && make test 2>&1 | tee /tmp/7-2-7-test-final.out
set -o pipefail && \
  make kani-clone-detector 2>&1 | tee /tmp/7-2-7-kani-final.out
set -o pipefail && make kani 2>&1 | tee /tmp/kani-whitaker-7-2-7-final.out
coderabbit review --agent
```

If documentation changed, also run:

```sh
set -o pipefail && \
  make markdownlint 2>&1 | tee /tmp/7-2-7-mdlint-final.out
set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-whitaker-7-2-7-final.out
```

Commit the roadmap update after gates pass. Push the branch:

```sh
git push
```

Record the Lody session reference:

```sh
echo ${LODY_SESSION_ID}
```

Create or update the draft pull request. The title must include the roadmap
item as `(7.2.7)`, for example:

```text
Verify bounded MinHasher sketch invariants (7.2.7)
```

The pull request description must mention this ExecPlan document and include a
`## References` section ending with:

```text
Lody session: https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
```

## Acceptance criteria

- `crates/whitaker_clones_core/src/index/kani.rs` contains Kani harnesses that
  call the real `MinHasher::sketch` implementation and prove bounded
  deterministic output, duplicate-hash insensitivity, and empty-input failure.
- `scripts/run-kani.sh` includes the new harness names in the clone-detector
  harness list.
- `make kani-clone-detector` exits 0 and reports successful verification for
  the MinHasher harnesses.
- Unit tests using `rstest` cover the same externally meaningful
  `MinHasher::sketch` contracts.
- `rstest-bdd` behaviour coverage is added or explicitly documented as already
  sufficient for externally observable candidate-generation behaviour.
- `docs/whitaker-clone-detector-design.md` records the 7.2.7 proof decision.
- `docs/developers-guide.md` documents the maintainer-facing Kani coverage.
- `docs/users-guide.md` is updated only if user-visible behaviour changes; if
  unchanged, the rationale is recorded in this plan.
- `docs/roadmap.md` marks item 7.2.7 done after implementation is complete.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- `coderabbit review --agent` has been run after each major milestone, and all
  concerns have been cleared or explicitly escalated.

## Rollback plan

Each milestone should be committed separately after passing its gates. If a
later milestone fails, use Git history to inspect or revert only the milestone
commit that introduced the problem. Do not use `git reset --hard` or checkout
away unrelated changes unless explicitly directed.

If a Kani harness becomes intractable, revert only the Kani milestone commit,
keep any independently useful tests or documentation only if they still pass
the gates, and update this plan's `Decision Log` with the failed bound, runtime
symptoms, and proposed alternative.
