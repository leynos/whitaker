# Verify `propagate_labels` with Kani (roadmap 6.4.6)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`.

Implementation must not begin until this plan is explicitly approved.

## Purpose / big picture

Roadmap item 6.4.6 extends the existing decomposition-advice proof work with a
machine-checked Kani verification for
`common/src/decomposition_advice/community.rs:propagate_labels`. After this
change, Whitaker will not rely only on local reasoning, unit tests, and
behaviour tests to justify its deterministic weighted label-propagation step.
It will also ship a repeatable bounded model check showing that label
propagation:

1. preserves valid label indices,
2. returns exactly one label per input vector, and
3. terminates within the caller-supplied iteration bound.

Observable success after implementation:

1. `make kani` completes full Kani/CBMC verification for new
   `propagate_labels` harnesses with zero failures, using the existing pinned
   Kani sidecar introduced for roadmap item 6.4.5.
2. Unit tests exercise `propagate_labels` through crate-visible seams and cover
   happy paths, unhappy paths, and edge cases such as isolated nodes, tied
   neighbour scores, zero iterations, and bounded non-convergence inputs.
3. Behaviour tests using `rstest-bdd` `0.5.0` cover the same observable
   outcomes through a narrow `common::test_support::decomposition` seam.
4. `docs/brain-trust-lints-design.md` records the final 6.4.6 modelling and
   proof-bound decisions.
5. `docs/roadmap.md` marks 6.4.6 done only after `make check-fmt`,
   `make lint`, `make test`, and `make kani` all succeed.

## Constraints

- Scope only roadmap item 6.4.6. Do not reopen or broaden 6.4.5
  (`build_adjacency`) beyond small proof-support refactors that are required to
  keep the Kani module coherent.
- Reuse the existing Kani sidecar from 6.4.5 (`scripts/install-kani.sh`,
  `scripts/run-kani.sh`, and `make kani`). Do not add a second Kani workflow.
- Keep shipped runtime behaviour unchanged unless a small refactor is required
  to expose crate-visible proof and test seams around `propagate_labels` or
  `best_neighbour_label`.
- Do not widen the public production API of
  `common::decomposition_advice`. Any new observable seam for integration tests
  must live in `common::test_support::decomposition`.
- Keep the Kani harnesses adjacent to the implementation. If the current
  `common/src/decomposition_advice/community_kani.rs` would exceed the
  repository's 400-line limit after adding 6.4.6, split it into a small
  directory module such as:

  ```text
  common/src/decomposition_advice/community_kani/
  - mod.rs
  - shared.rs
  - build_adjacency.rs
  - propagate_labels.rs
  ```

- Model only inputs that can occur in production after 6.4.1 and 6.4.5:
  adjacency lists must contain only in-bounds neighbour indices, weights must
  be positive, and labels must originate from the runtime's initial
  `0..vectors.len()` labelling.
- The proof of "terminates within the supplied iteration bound" must stay
  faithful to the actual runtime contract: termination means the function
  returns after at most `max_iterations` passes over the active-node set, not
  that convergence is guaranteed within that bound.
- Keep every source file, proof-support file, and test file under 400 lines.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  `0.5.0` for all new tests.
- Behaviour tests must respect the workspace Clippy
  `too_many_arguments` threshold of 4. Each BDD step may parse at most 3 values
  in addition to the fixture.
- New public helpers in `common::test_support::decomposition` require Rustdoc
  comments with examples that follow `docs/rust-doctest-dry-guide.md`.
- Record the final 6.4.6 implementation decisions in
  `docs/brain-trust-lints-design.md`.
- Mark roadmap item 6.4.6 done only after the implementation, proof harnesses,
  tests, and all required quality gates succeed.
- Run long validation commands through `tee` with `set -o pipefail`, because
  this environment truncates long output.

## Tolerances (exception triggers)

- Scope: if the change grows beyond 12 touched files or 1,000 net lines, stop
  and escalate before continuing.
- Proof structure: if adding the 6.4.6 harnesses would push the existing Kani
  module over 400 lines and a clean file split proves awkward, stop and
  document the options before improvising a large mixed proof file.
- Modelling: if Kani cannot verify the bounded `propagate_labels` properties
  with small fixed-size graphs and unwinds within 2 modelling iterations, stop
  and capture the failing operation and candidate bounds before continuing.
- Semantics: if proving the roadmap properties requires changing the runtime
  algorithm, not just refactoring for visibility or proof shape, stop and
  escalate.
- Interface: if behavioural tests can only observe label propagation by making
  raw runtime helpers public from `common::decomposition_advice`, stop and
  escalate.
- Validation: if `make check-fmt`, `make lint`, `make test`, or `make kani`
  still fail after 3 targeted fix iterations, stop and escalate with the
  captured logs.
- Dependencies: if a new runtime dependency appears necessary, stop and
  escalate.

## Risks

- State-space risk: `propagate_labels` nests iteration over active nodes and
  neighbour buckets, so symbolic adjacency plus symbolic weights can expand
  quickly. Severity: high. Likelihood: medium. Mitigation: keep node/edge
  bounds small, reuse the fixed-size symbolic modelling pattern from 6.4.5, and
  document the final bounds and unwind count in the design document.
- Contract risk: `propagate_labels` assumes adjacency indices are valid and
  aligned with `vectors.len()`. A proof over arbitrary malformed adjacency
  would model a broader contract than production actually has. Severity: high.
  Likelihood: high. Mitigation: constrain harness inputs to valid adjacency
  graphs and place unhappy-path validation in test-support helpers instead of
  changing runtime semantics.
- File-size risk: `community_kani.rs` already holds the completed 6.4.5
  harnesses. Adding 6.4.6 in-place may exceed the 400-line repository limit.
  Severity: medium. Likelihood: high. Mitigation: prefer an early split into a
  `community_kani/` directory with shared symbolic helpers.
- Observation-seam risk: behavioural tests need observable labels or iteration
  reports without widening the production API. Severity: medium. Likelihood:
  high. Mitigation: add a narrow report helper in
  `common::test_support::decomposition` that validates declarative graph input
  and returns a stable label report.
- Misstated termination risk: it is easy to conflate "converges within bound"
  with "returns within bound". Severity: medium. Likelihood: medium.
  Mitigation: keep the proof and tests explicit that bounded return is the
  required property, and add a non-converged example where the function still
  returns a label vector of the correct length.

## Progress

- [x] 2026-04-13: Review roadmap item 6.4.6, the decomposition design
  document, the current `community.rs` implementation, the completed 6.4.5 Kani
  sidecar, and the repository testing/documentation guidance.
- [x] 2026-04-13: Draft this ExecPlan with a concrete proof shape, runtime test
  strategy, documentation closure steps, and an explicit approval gate.
- [ ] After approval: split the Kani verification module if needed so 6.4.5
  and 6.4.6 proofs remain under the file-size limit and share symbolic helpers.
- [ ] After approval: add failing unit tests defining the observable
  `propagate_labels` contract.
- [ ] After approval: add `rstest-bdd` happy-path, unhappy-path, and edge-case
  coverage through `common::test_support::decomposition`.
- [ ] After approval: add bounded Kani harnesses for `propagate_labels`.
- [ ] After approval: record 6.4.6 implementation decisions in
  `docs/brain-trust-lints-design.md`.
- [ ] After approval: mark roadmap item 6.4.6 done in `docs/roadmap.md`.
- [ ] After approval: run `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, `make test`, and `make kani` successfully.
- [ ] After approval: finalize the living sections in this ExecPlan after
  implementation.

## Surprises & Discoveries

- `propagate_labels` is currently private to
  `common/src/decomposition_advice/community.rs`, while `build_adjacency` is
  already `pub(crate)` from roadmap item 6.4.5. The 6.4.6 work will likely need
  a crate-visible seam for direct unit tests and test-support helpers.
- The current runtime already guarantees label indices start in range because
  labels are initialised as `0..vectors.len()`, and new labels come only from
  neighbour labels returned by `best_neighbour_label`.
- The "termination" property is structurally tied to the
  `for _ in 0..max_iterations` loop. The proof work is therefore about bounded
  execution under symbolic inputs, not liveness or eventual convergence.
- The existing 6.4.5 Kani harness file is still small enough to read in one
  pass, but adding another proof family without restructuring would likely make
  it awkward to navigate and easy to push past the file-size limit.
- The repository already has a narrow decomposition test-support seam via
  `common/src/test_support/decomposition.rs`. 6.4.6 should extend that seam
  instead of inventing a second integration-test entry point.
- `docs/rstest-bdd-users-guide.md` still contains examples mentioning
  `rstest-bdd` `0.2.0`, while the repository guidance and current work require
  `0.5.0`. This plan follows the repository-local requirement and does not
  treat the guide's old snippets as authoritative version guidance.

## Decision Log

- Decision: draft 6.4.6 around the existing 6.4.5 Kani sidecar rather than a
  fresh workflow. Rationale: the repository already proved the install/run
  shape for Kani, and roadmap 6.4.6 should extend that workflow instead of
  duplicating it. Date/Author: 2026-04-13 / Codex.
- Decision: treat "terminates within the supplied iteration bound" as a bounded
  return property, not a convergence property. Rationale: the runtime loop is
  explicitly bounded by `max_iterations`; convergence may occur earlier, but
  the roadmap wording does not require proving convergence for all valid
  graphs. Date/Author: 2026-04-13 / Codex.
- Decision: plan for a narrow label-propagation report helper in
  `common::test_support::decomposition` rather than widening the public
  decomposition API. Rationale: behaviour tests need observable labels and
  iteration-bound outcomes, but production APIs should remain tight.
  Date/Author: 2026-04-13 / Codex.
- Decision: plan for at least one unhappy-path behaviour test via validated
  test-support input rejection rather than adding new runtime validation to
  `propagate_labels`. Rationale: the user explicitly asked for unhappy-path
  coverage, and the production helper is intentionally internal and
  preconditioned. Date/Author: 2026-04-13 / Codex.
- Decision: signpost the relevant local documentation and skills directly in
  this plan. Rationale: the next implementer should be able to execute the work
  from this file alone. Date/Author: 2026-04-13 / Codex.

## Context and orientation

The implementation under verification lives in
`common/src/decomposition_advice/community.rs`.

Today, the relevant runtime flow is:

1. `build_similarity_edges(vectors)` builds weighted similarity edges.
2. `build_adjacency(node_count, edges)` turns those edges into sorted undirected
   adjacency buckets.
3. `propagate_labels(vectors, adjacency, max_iterations)` performs
   deterministic weighted label propagation over the active nodes.
4. `detect_communities(vectors)` groups nodes by their final labels and sorts
   the resulting communities deterministically.

Repository areas likely to change during implementation:

1. `common/src/decomposition_advice/community.rs`
2. `common/src/decomposition_advice/community_kani.rs` or a split
   `common/src/decomposition_advice/community_kani/` directory
3. `common/src/decomposition_advice/tests.rs` and a new or expanded
   label-propagation-focused unit-test file
4. `common/src/test_support/decomposition.rs`
5. a new helper module under `common/src/test_support/` if needed to keep files
   small
6. `common/tests/decomposition_label_propagation_behaviour.rs`
7. `common/tests/features/decomposition_label_propagation.feature`
8. `docs/brain-trust-lints-design.md`
9. `docs/roadmap.md`
10. this ExecPlan

Relevant local documentation to consult while implementing:

- `docs/brain-trust-lints-design.md`
- `docs/whitaker-dylint-suite-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/execplans/6-4-5-use-kani-to-verify-build-adjacency-preserves-similarity-edges.md`
- `docs/execplans/6-4-1-method-community-detection.md`

Relevant skills to use during implementation:

- `execplans` for maintaining this living plan.
- `rust-router` to route any Rust-specific design question to the smallest
  useful Rust skill.
- `nextest` when the test scope or profile behaviour needs clarification.
- `rust-types-and-apis`, `rust-errors`, or `rust-async-and-concurrency` only if
  the implementation reveals a concrete need. They are not expected to be
  primary for 6.4.6.

## Proposed implementation shape

### Stage A: Establish the proof and test seams

Confirm the smallest runtime visibility change needed for 6.4.6. The preferred
shape is:

1. keep `propagate_labels` non-public outside the crate,
2. promote it to `pub(crate)` only if direct unit tests or test-support helpers
   need to call it, and
3. keep any report types or input validation in `common::test_support`.

If `community_kani.rs` would exceed 400 lines after the new harnesses are
added, split it before adding new proofs. Move the existing 6.4.5 harnesses
without changing their semantics, then add a new `propagate_labels.rs`
companion file plus a tiny shared helper module for symbolic graph material.

### Stage B: Write failing unit tests first

Add focused unit tests in the decomposition-advice module that define the exact
contract before implementation changes:

1. `propagate_labels_returns_one_label_per_vector`
2. `propagate_labels_keeps_labels_in_range_for_connected_graph`
3. `propagate_labels_leaves_isolated_nodes_with_original_labels`
4. `propagate_labels_respects_zero_iteration_bound`
5. `propagate_labels_uses_lexical_tie_break_for_equal_scores`
6. `propagate_labels_returns_after_bound_even_when_not_converged`

Keep helpers small and explicit. If repeated test input construction would push
against Clippy's argument-count rule, use a small parameter object or fixture
builder rather than many-argument helper functions.

### Stage C: Add a narrow integration-test seam

Extend `common/src/test_support/decomposition.rs` with a small helper such as
`label_propagation_report(...) -> Result<LabelPropagationReport, LabelPropagationError>`.

The helper should:

1. accept declarative graph input that is easy to express in BDD scenarios,
2. validate the graph before calling runtime helpers,
3. return stable output including the final labels and whether the input graph
   had any active nodes, and
4. provide one unhappy path by rejecting malformed edges or out-of-range
   indices.

This keeps unhappy-path coverage out of the production API while still giving
behaviour tests an observable contract.

### Stage D: Add `rstest-bdd` behaviour coverage

Add a dedicated feature file and harness, for example:

- `common/tests/features/decomposition_label_propagation.feature`
- `common/tests/decomposition_label_propagation_behaviour.rs`

Cover at least:

1. a happy path where two related communities settle to shared labels,
2. an edge case with isolated nodes returning self labels,
3. an edge case with `max_iterations = 0`,
4. a deterministic tie-break case, and
5. an unhappy path where invalid declarative graph input is rejected by the
   test-support helper.

Follow repository BDD conventions:

- use a small world fixture rather than many parsed parameters,
- avoid `.expect()` and `.unwrap()` in `tests/`,
- keep assertions in `Then` steps,
- use indexed `#[scenario(..., index = N)]` bindings if multiple scenarios
  share one harness.

### Stage E: Add bounded Kani proofs

Add new Kani harnesses for the roadmap properties using the same small symbolic
graph style as 6.4.5. The preferred harness family is:

1. `verify_propagate_labels_returns_vector_per_input`
2. `verify_propagate_labels_preserves_label_indices`
3. `verify_propagate_labels_zero_iterations_keeps_initial_labels`
4. `verify_propagate_labels_bounded_return_for_any_max_iterations`

The bounded symbolic model should:

1. generate `node_count` in a small range such as `0..=3`,
2. build valid undirected adjacency buckets from fixed-size symbolic edge
   arrays,
3. generate a symbolic `max_iterations` with a small bound compatible with the
   chosen `#[kani::unwind(...)]`, and
4. materialise enough `MethodFeatureVector` values to satisfy the runtime's
   deterministic lexical tie-break logic.

Avoid proving stronger claims than the roadmap asks for. In particular, do not
turn 6.4.6 into a convergence proof unless the existing implementation happens
to make that property trivial within the bounded model.

### Stage F: Update design docs and roadmap

Record the final 6.4.6 decisions in `docs/brain-trust-lints-design.md`. At a
minimum, document:

1. the bounded symbolic graph model,
2. the chosen node/edge/iteration bounds and unwind count,
3. the interpretation of the bounded-termination property,
4. any crate-visible seam added for tests, and
5. any trade-off that keeps the proof tractable.

Only after the code, tests, and proofs are complete, mark roadmap item 6.4.6
done in `docs/roadmap.md`.

## Validation plan

During implementation, keep the loop red/green/refactor:

1. write or update failing unit tests and BDD scenarios first,
2. make them pass with the smallest runtime or test-support change,
3. add or update Kani harnesses,
4. run the full validation set before closing the work.

Use `tee` and `set -o pipefail` for every long-running gate:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/6-4-6-fmt.log
```

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/6-4-6-markdownlint.log
```

```bash
set -o pipefail
make nixie 2>&1 | tee /tmp/6-4-6-nixie.log
```

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/6-4-6-check-fmt.log
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/6-4-6-lint.log
```

```bash
set -o pipefail
make test 2>&1 | tee /tmp/6-4-6-test.log
```

```bash
set -o pipefail
make kani 2>&1 | tee /tmp/6-4-6-kani.log
```

Expected success signals after implementation:

1. Unit and behaviour tests covering label propagation pass.
2. `make check-fmt`, `make lint`, and `make test` succeed.
3. `make kani` reports successful verification for the new 6.4.6 harnesses in
   addition to the existing 6.4.5 harnesses.
4. `docs/brain-trust-lints-design.md` and `docs/roadmap.md` reflect the final
   delivered state.

## Outcomes & Retrospective

This section is intentionally incomplete until implementation is approved and
finished.

Planned completion criteria:

1. roadmap item 6.4.6 is implemented and marked done,
2. the proof, unit-test, and behaviour-test story is documented and
   reproducible, and
3. the final bounded-model trade-offs are recorded clearly enough for a future
   contributor to extend or revisit the proof.
