# Prove `cosine_threshold_met` invariants with Verus (roadmap 6.4.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

This document must be maintained in accordance with `AGENTS.md`.

Implementation began after explicit user approval in the follow-up task.

## Purpose / big picture

Roadmap item 6.4.3 adds a machine-checked proof for the most delicate part of
the decomposition-advice similarity logic:
`common/src/decomposition_advice/vector.rs::cosine_threshold_met`. After this
change, the repository will not merely rely on unit tests and inspection to
justify the integer cross-multiplication check. It will also contain a
repeatable Verus proof showing that, for non-zero norms, the shipped check is
equivalent to testing whether cosine similarity is at least `0.20`, and that
the implementation never needs to divide by zero because zero-norm inputs are
rejected before any denominator-based reasoning is required.

Observable outcome:

1. The repository gains a reproducible Verus workflow, following the same
   sidecar pattern used by the referenced Chutoro examples: proof files live
   outside the Cargo workspace, wrapper scripts manage the toolchain, and the
   proof run is deterministic.
2. Verus proves the actual shipped threshold interpretation used by Whitaker:
   the runtime compares `25 * dot^2 >= left_norm * right_norm`, which is the
   squared form of `cosine >= 1/5`.
3. Rust unit tests cover exact-boundary, below-threshold, and zero-norm cases
   for the runtime function.
4. Behaviour tests using `rstest-bdd` v0.5.0 cover happy, unhappy, and
   zero-feature edge cases through an observable test-support seam rather than
   by widening the production API.
5. `docs/brain-trust-lints-design.md` records the final proof scope and the
   important threshold interpretation decision that `1 / 25` is the squared
   cosine threshold, not the raw cosine fraction.
6. `docs/roadmap.md` marks 6.4.3 done only after the proof, tests, and all
   required quality gates succeed.
7. The implementation turn ends with successful runs of `make check-fmt`,
   `make lint`, and `make test`, plus the new Verus proof command.

## Constraints

- Scope only roadmap item 6.4.3. Do not take on roadmap items 6.4.4, 6.4.5,
  or 6.4.6 in the same change, except for the smallest proof-local helper
  lemmas needed to state the 6.4.3 theorem.
- Keep the shipped runtime behaviour unchanged unless a small refactor is
  needed to make the proof target and the implementation obviously equivalent.
  Any such refactor must preserve the current threshold semantics exactly.
- Keep `common` free of `rustc_private` and free of a runtime dependency on
  Verus. Proof artefacts must live outside the Cargo workspace build path.
- Treat the current runtime check as the source of truth. The proof must be
  about the real expression used by `cosine_threshold_met`, not about a
  cleaned-up duplicate that could later drift from production code.
- Keep source files under 400 lines. If the proof grows too large, split it
  into small sibling Verus modules rather than one long file.
- Use the workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  `0.5.0` for tests.
- Behaviour tests must respect the workspace Clippy threshold of 4 arguments.
  Each BDD step may parse at most 3 values from feature text in addition to the
  world fixture.
- Public helpers added to `common` or `common::test_support` require Rustdoc
  comments with examples that follow `docs/rust-doctest-dry-guide.md`.
- Update `docs/brain-trust-lints-design.md` with the final implementation
  decisions taken during delivery.
- Do not mark roadmap item 6.4.3 done until the proof command, unit tests,
  behaviour tests, and all requested quality gates succeed.
- Run command-heavy validation through `tee` with `set -o pipefail`, because
  this environment truncates long output.

## Tolerances (exception triggers)

- Scope: if the change grows beyond 11 touched files or 1000 net lines, stop
  and escalate before continuing.
- Semantics: if the proof appears to require changing the threshold from the
  shipped `1 / 25` squared form or changing the early-return behaviour for zero
  norms, stop and escalate.
- Interface: if the only way to make the BDD coverage observable is to expose
  `MethodFeatureVector` or `cosine_threshold_met` publicly from
  `common::decomposition_advice`, stop and escalate before widening that API.
- Tooling: if the Verus workflow cannot be made reproducible with one wrapper
  script iteration and one refinement iteration, stop and escalate with the
  exact toolchain or invocation failure.
- Proof modelling: if Verus can prove only a weakened theorem that does not
  cover exact-threshold equality or non-zero-norm preconditions, stop and
  escalate instead of silently narrowing the guarantee.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix iterations, stop and escalate with captured logs.

## Risks

- Threshold-meaning risk: the runtime parameters
  `min_similarity_numerator` and `min_similarity_denominator` can be read as if
  they encode the raw cosine threshold, but the current implementation uses the
  squared threshold instead. Severity: high. Likelihood: high. Mitigation:
  record this explicitly in docs, clarify nearby naming or comments if needed,
  and make the proof state the squared-form equivalence to `cosine >= 0.20`.
- Drift risk: a proof that restates the logic without sharing constants or the
  same expression shape can silently diverge from production code. Severity:
  high. Likelihood: medium. Mitigation: factor threshold constants or helper
  comments into the runtime code only when it reduces ambiguity, and keep the
  proof file aligned with the shipped names and values.
- Arithmetic-domain risk: Verus reasons over mathematical integers, while the
  runtime uses `u64` and `u128`. Severity: medium. Likelihood: medium.
  Mitigation: state the proof over non-negative mathematical values and add
  runtime unit tests that exercise the same boundary cases through the concrete
  Rust implementation.
- Toolchain risk: the repository currently has no local `verus/` directory,
  no Verus scripts, and no Makefile target. Severity: medium. Likelihood: high.
  Mitigation: mirror the proven Chutoro pattern with top-level `verus/` files
  and small shell wrappers, then document the exact commands in this plan and
  the design doc.
- Testing-seam risk: integration tests cannot currently call the crate-private
  vector helpers directly. Severity: medium. Likelihood: high. Mitigation: add
  a narrow `common::test_support::decomposition` helper that exposes only the
  observable boolean decision needed by BDD tests.

## Progress

- [x] 2026-03-22: Draft this ExecPlan, inspect the current decomposition code,
  and capture the proof/testing/tooling constraints.
- [x] 2026-03-22: Add unit tests covering exact-boundary, below-threshold,
  zero-dot, and zero-norm runtime behaviour in
  `common/src/decomposition_advice/tests.rs`.
- [x] 2026-03-22: Add `rstest-bdd` scenarios for strong overlap,
  below-threshold overlap, and empty-vector safety through
  `common::test_support::decomposition::methods_meet_cosine_threshold()`.
- [x] 2026-03-22: Add the Verus sidecar workflow with
  `scripts/install-verus.sh`, `scripts/run-verus.sh`,
  `verus/decomposition_cosine_threshold.rs`, and `make verus`.
- [x] 2026-03-22: Keep runtime behaviour unchanged while clarifying the shared
  squared-threshold constants in `common/src/decomposition_advice/vector.rs`.
- [x] 2026-03-22: Record 6.4.3 implementation decisions in
  `docs/brain-trust-lints-design.md`.
- [x] 2026-03-22: Mark roadmap item 6.4.3 done in `docs/roadmap.md`.
- [x] 2026-03-22: Run `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, `make test`, and `make verus` successfully.
- [x] 2026-03-22: Finalize the living sections in this ExecPlan.

## Surprises & Discoveries

- `common/src/decomposition_advice/vector.rs` already implements the roadmap
  target with integer-only arithmetic: `dot == 0` short-circuits to `false`,
  zero norms short-circuit to `false`, and the comparison uses `u128`
  cross-multiplication.
- The current call site in `common/src/decomposition_advice/community.rs` uses
  constants `1` and `25`. That means the code is comparing against the squared
  threshold, which is equivalent to `cosine >= 0.20`, but the current constant
  names do not make that interpretation obvious.
- The repository has no local `verus/` directory, no `scripts/install-verus.sh`
  or `scripts/run-verus.sh`, and no Makefile target for proofs yet.
- The referenced Chutoro examples use the pattern this repository should copy:
  top-level `verus/*.rs` proof files plus shell wrappers for install and run.
- Roadmap item 6.4.4 separately owns algebraic proofs for `dot_product` and
  `norm_squared`. This plan must not absorb those larger proofs, though 6.4.3
  may still need tiny local lemmas about non-negativity or squared inequalities.
- Existing integration tests already rely on `common::test_support` for
  decomposition fixtures, which makes that module the safest seam for the new
  BDD coverage.
- The current pinned Verus release `0.2026.03.17.a96bad0` ships binaries in
  its zip archive without executable mode preserved in this environment, so the
  install script must `chmod +x` `verus`, `cargo-verus`, `rust_verify`, and
  `z3` after extraction.
- On 2026-03-22, that pinned Verus release required Rust toolchain
  `1.94.0-x86_64-unknown-linux-gnu` on Linux. The install script can discover
  the requirement by running `verus --version`, parsing the suggested
  `rustup install ...` command, and then retrying.

## Decision Log

- Decision: keep Verus artefacts as a top-level sidecar (`verus/` plus small
  wrapper scripts) instead of adding Verus to Cargo dependencies. Rationale:
  that matches the referenced Chutoro pattern, keeps normal Rust builds free of
  proof tooling, and minimizes blast radius. Date/Author: 2026-03-22 / Codex.
- Decision: interpret the shipped `1 / 25` pair as the squared cosine
  threshold corresponding to `0.20`, and make that explicit in the proof and
  design doc. Rationale: this is the actual mathematics of the current runtime
  comparison and is the key subtlety future maintainers are most likely to
  misread. Date/Author: 2026-03-22 / Codex.
- Decision: keep BDD coverage on an observable test-support helper rather than
  exposing private vector types from the production module. Rationale: the
  roadmap requires behavioural tests, but the production API should stay narrow
  and compiler-independent. Date/Author: 2026-03-22 / Codex.
- Decision: prefer a small reproducible command surface for proofs, ideally via
  `make verus` delegating to `scripts/run-verus.sh`. Rationale: the project
  conventions prefer Makefile targets, and proof execution should be as easy to
  repeat as `make test`. Date/Author: 2026-03-22 / Codex.
- Decision: pin Verus release `0.2026.03.17.a96bad0` in the install script and
  allow override via environment variables. Rationale: this keeps the proof
  workflow reproducible while still permitting later upgrades without editing
  the wrapper shape. Date/Author: 2026-03-22 / Codex.

## Context and orientation

### Repository state

The code under proof lives in `common/src/decomposition_advice/vector.rs`. The
relevant runtime shape today is:

1. Build sparse integer-weighted feature vectors from method profiles.
2. Compute `dot_product(left.weights(), right.weights())`.
3. Reject `dot == 0`.
4. Compute `left.norm_squared()` and `right.norm_squared()`.
5. Reject `left_norm == 0 || right_norm == 0`.
6. Compare `min_similarity_denominator * dot^2` against
   `min_similarity_numerator * left_norm * right_norm` using `u128`.

The actual decomposition graph builder in
`common/src/decomposition_advice/community.rs` calls
`cosine_threshold_met(&vectors[left], &vectors[right], 1, 25)`. The proof and
tests must therefore explain and verify the following equivalence:

```text
25 * dot^2 >= left_norm * right_norm
```

is exactly the squared-denominator form of:

```text
dot / (sqrt(left_norm) * sqrt(right_norm)) >= 1 / 5
```

for `left_norm > 0` and `right_norm > 0`.

### External reference pattern

The user supplied Chutoro Verus examples that establish the preferred repo
shape for this work:

- `scripts/install-verus.sh`
- `scripts/run-verus.sh`
- `verus/edge_harvest_extract.rs`
- `verus/edge_harvest_ordering.rs`
- `verus/edge_harvest_proofs.rs`

Those examples are not this repository's source of truth, but they are the best
template for how Whitaker should host proof files and wrapper scripts.

### Relationship to nearby roadmap items

- 6.4.1 shipped the integer-weighted sparse vectors and the threshold check now
  being proved.
- 6.4.2 shipped note rendering based on those suggestions.
- 6.4.4 will later prove algebraic properties of `dot_product` and
  `norm_squared`.

This means 6.4.3 should stay narrowly focused on the threshold predicate, the
exact `0.20` interpretation, and the zero-norm safety story.

## Proposed implementation shape

The implementation should stay small and explicit. The intended file map is:

- `common/src/decomposition_advice/vector.rs`
- `common/src/decomposition_advice/tests.rs`
- `common/src/test_support/decomposition.rs`
- `common/tests/cosine_threshold_behaviour.rs`
- `common/tests/features/cosine_threshold.feature`
- `scripts/install-verus.sh`
- `scripts/run-verus.sh`
- `verus/decomposition_cosine_threshold.rs`
- `Makefile`
- `docs/brain-trust-lints-design.md`
- `docs/roadmap.md`

If the Verus proof exceeds the 400-line file limit, split it into:

- `verus/decomposition_cosine_spec.rs`
- `verus/decomposition_cosine_proofs.rs`
- `verus/decomposition_cosine_threshold.rs`

where the top-level file contains `mod` declarations and a tiny `main()`.

## Implementation details

### Stage B: Write the failing Rust tests first

Add unit coverage in `common/src/decomposition_advice/tests.rs` for the actual
runtime predicate. Keep these tests close to the private helper so they can use
synthetic vectors or a small local test-only constructor without widening the
public API.

The minimum unit cases are:

1. Exact boundary: a synthetic pair whose cosine is exactly `0.20` returns
   `true`, proving the code uses `>=` and not `>`.
2. Just below boundary: a nearby case returns `false`.
3. Zero dot product: unrelated vectors return `false`.
4. Left zero norm: returns `false` without panicking.
5. Right zero norm: returns `false` without panicking.

The exact-boundary and just-below cases should use tiny synthetic weights so
the expected inequality is obvious to a reader, ideally with a short comment
showing the arithmetic.

### Stage C: Add Behaviour-Driven Development (BDD) coverage through test support

Create a narrow helper in `common/src/test_support/decomposition.rs`, for
example:

```rust
pub fn methods_meet_cosine_threshold(
    left: &MethodProfile,
    right: &MethodProfile,
) -> bool
```

This helper should internally build feature vectors and evaluate the same
threshold path used by production code. It exists only to give integration and
BDD tests an observable seam.

Add `common/tests/features/cosine_threshold.feature` and
`common/tests/cosine_threshold_behaviour.rs` using `rstest-bdd` `0.5.0`.

The behaviour scenarios should cover:

1. Happy path: two non-zero method profiles with strong overlap are considered
   similar.
2. Unhappy path: two method profiles below the threshold are not considered
   similar.
3. Edge case: a method profile whose features collapse to an empty vector does
   not cause a panic and does not satisfy the threshold.

Keep the BDD world small. Use fixture-backed `MethodProfile` values or a small
state struct, and keep each step at or below the 4-argument Clippy limit.

### Stage D: Add the Verus workflow

Mirror the referenced Chutoro pattern:

1. Add `scripts/install-verus.sh` to fetch or prepare the Verus toolchain in a
   deterministic way.
2. Add `scripts/run-verus.sh` to invoke Verus on the proof entrypoint.
3. Add a small `Makefile` target, preferably `verus`, that delegates to
   `scripts/run-verus.sh`.
4. Add a top-level proof file under `verus/`.

The proof should define the smallest mathematical model needed:

1. Non-negative `dot`, `left_norm`, and `right_norm`.
2. Positive norms for the main equivalence theorem.
3. A predicate representing the real-valued cosine threshold
   `dot / (sqrt(left_norm) * sqrt(right_norm)) >= 1/5`.
4. A predicate representing the shipped cross-multiplied runtime check
   `25 * dot^2 >= left_norm * right_norm`.

The main proof obligations are:

1. For `left_norm > 0` and `right_norm > 0`, the two predicates above are
   equivalent.
2. The runtime algorithm does not need any division when either norm is zero,
   because it returns `false` before denominator-based reasoning is needed.
3. Exact equality on the boundary is accepted.

Avoid turning this into a full vector-algebra proof. The inputs to the theorem
can be the scalar quantities `dot`, `left_norm`, and `right_norm`; 6.4.4 will
cover how those quantities arise from `dot_product` and `norm_squared`.

### Stage E: Tighten the production code only where clarity demands it

If the tests and proof expose ambiguity in the runtime names, make the smallest
clarifying change that reduces future drift. Acceptable examples:

- rename local constants or parameters so they say "squared threshold",
- add a comment above `cosine_threshold_met` explaining why `1 / 25`
  corresponds to cosine `0.20`,
- factor the threshold constants into one shared location so the call site and
  proof speak the same values.

Do not rewrite the algorithm or change its public shape unless the proof
absolutely requires it.

### Stage F: Update the design document

Append `## Implementation decisions (6.4.3)` to
`docs/brain-trust-lints-design.md`.

Record at least:

1. Where the Verus sidecar files live and how they are run.
2. That `1 / 25` is the squared threshold corresponding to `cosine >= 0.20`.
3. That zero-norm inputs are handled by runtime early return rather than by any
   denominator-bearing arithmetic.
4. Any small naming or helper refactors taken to keep the proof aligned with
   runtime code.

### Stage G: Mark the roadmap item done

Only after the proof and all quality gates pass, update `docs/roadmap.md` to
mark 6.4.3 as done.

## Validation and evidence

Run the proof and the full repository gates with log capture:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/6-4-3-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/6-4-3-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/6-4-3-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-4-3-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/6-4-3-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/6-4-3-test.log
set -o pipefail; make verus 2>&1 | tee /tmp/6-4-3-verus.log
```

Success criteria:

1. The new unit tests and BDD scenarios pass.
2. The Verus proof command exits successfully.
3. `make check-fmt`, `make lint`, and `make test` all succeed.
4. The roadmap item is marked done only after those commands succeed.

## Acceptance checklist for the implementation turn

The implementation is complete only when all of the following are true:

1. The repo contains a reproducible Verus workflow and proof file for
   `cosine_threshold_met`.
2. The proof explicitly covers the `0.20` interpretation for non-zero norms.
3. Zero-norm safety is explained both in the proof and in runtime tests.
4. Unit tests cover exact-boundary and below-threshold cases.
5. `rstest-bdd` scenarios cover happy, unhappy, and edge cases.
6. `docs/brain-trust-lints-design.md` records the final 6.4.3 decisions.
7. `docs/roadmap.md` marks 6.4.3 done.
8. The captured gate logs show success.

## Outcomes & Retrospective

Completed on 2026-03-22.

- Proved:
  - `25 * dot^2 >= left_norm * right_norm` is equivalent to
    `cosine >= 0.20` for non-zero norms, modelled in Verus via positive real
    vector lengths whose squares are the integer norms.
  - Zero norms and zero dot product short-circuit the runtime algorithm before
    any denominator-bearing reasoning is needed.
  - Exact boundary equality is accepted.
- Runtime clarifications:
  - Shared squared-threshold constants now live in
    `common/src/decomposition_advice/vector.rs` as
    `MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED` and
    `MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED`.
  - A crate-visible helper
    `methods_meet_cosine_threshold(left, right)` keeps behavioural tests on the
    same threshold path without exposing `MethodFeatureVector`.
- Tests added:
  - Unit tests for exact-boundary, below-threshold, zero-dot, and zero-norm
    cases in `common/src/decomposition_advice/tests.rs`.
  - BDD scenarios in `common/tests/cosine_threshold_behaviour.rs` and
    `common/tests/features/cosine_threshold.feature`.
- Proof and gate commands that passed:
  - `set -o pipefail; make fmt 2>&1 | tee /tmp/6-4-3-fmt.log`
  - `set -o pipefail; make markdownlint 2>&1 | tee /tmp/6-4-3-markdownlint.log`
  - `set -o pipefail; make nixie 2>&1 | tee /tmp/6-4-3-nixie.log`
  - `set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-4-3-check-fmt.log`
  - `set -o pipefail; make lint 2>&1 | tee /tmp/6-4-3-lint.log`
  - `set -o pipefail; make test 2>&1 | tee /tmp/6-4-3-test.log`
  - `set -o pipefail; make verus 2>&1 | tee /tmp/6-4-3-verus.log`
- Follow-on work:
  - Roadmap 6.4.4 still needs separate algebraic proofs for `dot_product` and
    `norm_squared`.
