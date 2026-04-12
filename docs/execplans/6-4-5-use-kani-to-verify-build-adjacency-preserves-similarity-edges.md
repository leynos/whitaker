# Verify `build_adjacency` with Kani (roadmap 6.4.5)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 6.4.5 adds a machine-checked Kani verification pass for
`build_adjacency` in `common/src/decomposition_advice/community.rs`. After this
change, Whitaker will not rely only on local reasoning and example-driven tests
to justify the adjacency builder that feeds label propagation. It will also
ship a repeatable bounded model check showing that, for valid similarity edges
produced by `build_similarity_edges`, adjacency construction preserves every
edge, never emits out-of-bounds neighbour indices, and always produces
symmetric neighbour lists.

Observable success after implementation:

1. `make kani` installs or reuses a pinned Kani toolchain and completes full
   Kani/CBMC verification of the new adjacency harnesses with zero failures.
2. Unit tests exercise the shipped Rust adjacency construction directly,
   covering happy paths, absence-of-edge paths, and edge cases such as isolated
   nodes and sorted neighbour order.
3. Behaviour tests using `rstest-bdd` `0.5.0` cover the same observable
   adjacency outcomes through a narrow `common::test_support::decomposition`
   seam, including an unhappy-path validation case in the test helper.
4. `docs/brain-trust-lints-design.md` records the final 6.4.5 modelling and
   tooling decisions.
5. `docs/roadmap.md` marks 6.4.5 done only after `make check-fmt`,
   `make lint`, `make test`, and `make kani` all succeed.

## Constraints

- Scope only roadmap item 6.4.5. Do not absorb roadmap item 6.4.6
  (`propagate_labels`) into this change.
- Keep the shipped runtime behaviour of
  `common/src/decomposition_advice/community.rs` unchanged unless a small
  refactor is required to colocate Kani harnesses or expose a crate-visible
  report helper for tests.
- Do not widen the public production API of
  `common::decomposition_advice` merely to make Kani or behavioural tests
  easier. Any new observable seam must live in
  `common::test_support::decomposition`.
- Keep Kani tooling outside the normal Cargo dependency graph. Follow the
  sidecar pattern already used for Verus: top-level shell wrappers plus a Make
  target.
- Model the input contract that actually reaches `build_adjacency` in
  production. The harness should assume only valid `SimilarityEdge` values:
  `left < right < node_count`, positive weight, and no duplicate unordered
  pairs. These are the invariants already guaranteed by
  `build_similarity_edges`.
- Prefer colocated Kani harnesses that can call the private
  `build_adjacency` function directly, for example a `#[cfg(kani)]`
  verification submodule under `community.rs`.
- Keep every Rust source file and proof-support file under 400 lines. If the
  harness module grows too large, split it into a child module rather than
  letting `community.rs` or the verification file sprawl.
- Use the workspace-pinned `rstest`, `rstest-bdd`, and
  `rstest-bdd-macros` `0.5.0` for new behavioural coverage.
- Behaviour tests must respect the workspace Clippy `too_many_arguments`
  threshold of 4. Each behaviour-driven development (BDD) step may parse at
  most 3 values in addition to the fixture.
- Public helpers added to `common::test_support::decomposition` require
  Rustdoc comments with examples following `docs/rust-doctest-dry-guide.md`.
- Update `docs/brain-trust-lints-design.md` with the implementation decisions
  taken during delivery.
- Mark roadmap item 6.4.5 done only after Kani, unit tests, behavioural
  tests, and all required quality gates succeed.
- Run long validation commands through `tee` with `set -o pipefail`, because
  this environment truncates long output.

## Tolerances (exception triggers)

- Scope: if the change grows beyond 12 touched files or 1,000 net lines, stop
  and escalate before continuing.
- Tooling: if Kani cannot be installed reproducibly with one pinned install
  script, one run script, and one Make target, stop and present options rather
  than improvising a fragile workflow.
- Modelling: if Kani cannot verify the current `Vec` plus `sort_by`
  implementation within 2 bounded-harness iterations, stop and document the
  failing operations before considering any runtime refactor.
- Semantics: if proving the roadmap properties appears to require accepting
  malformed similarity edges that `build_similarity_edges` never emits, stop
  and clarify the intended preconditions before proceeding.
- Interface: if the only practical behavioural-test path requires exposing
  raw adjacency internals publicly from `common::decomposition_advice`, stop
  and escalate.
- Validation: if `make check-fmt`, `make lint`, `make test`, or `make kani`
  still fail after 3 targeted fix iterations, stop and escalate with the
  captured logs.
- Dependencies: if a new runtime dependency would be required, stop and
  escalate.

## Risks

- Kani state-space risk: `build_adjacency` uses `Vec` growth and per-node
  `sort_by`, which may become expensive for symbolic inputs. Severity: high.
  Likelihood: medium. Mitigation: keep symbolic bounds small, use fixed-size
  harness inputs plus an active-length field, and record the final bounds and
  unwind settings in the design doc.
- Input-contract risk: `build_adjacency` itself does not validate malformed
  indices, so a proof over arbitrary edges would be modelling a broader
  contract than production actually has. Severity: high. Likelihood: high.
  Mitigation: constrain harness inputs to the invariants established by
  `build_similarity_edges` and state that contract explicitly in the design doc.
- API-seam risk: behavioural tests need observable adjacency data without
  widening production APIs. Severity: medium. Likelihood: high. Mitigation: add
  a crate-visible report helper near the runtime code and expose only a narrow
  test-support wrapper to integration tests.
- Toolchain drift risk: there is no existing Kani workflow in the repository.
  Severity: medium. Likelihood: high. Mitigation: pin the Kani version in a
  dedicated install script and keep the runner logic parallel to the existing
  Verus sidecar.
- Configuration drift risk: the workspace currently has no visible Kani
  configuration, and `common/Cargo.toml` still carries a stale comment that
  mentions `rstest-bdd 0.2.x` even though the workspace pin is `0.5.0`.
  Severity: low. Likelihood: high. Mitigation: correct any touched comments and
  add the smallest necessary configuration to keep `cfg(kani)` and the test
  guidance coherent.

## Progress

- [x] 2026-04-02: Review roadmap item 6.4.5, the decomposition design
  document, the current `build_adjacency` implementation, existing Verus-side
  proof workflow, and current `rstest-bdd` guidance.
- [x] 2026-04-02: Draft this ExecPlan with a concrete Kani workflow, bounded
  harness model, runtime test strategy, and documentation closure steps.
- [x] 2026-04-03: Add pinned Kani install and run wrappers plus `make kani`.
- [x] 2026-04-03: Add bounded Kani harnesses for `build_adjacency`.
- [x] 2026-04-03: Add focused unit tests for adjacency construction.
- [x] 2026-04-03: Add `rstest-bdd` behavioural coverage through
  `common::test_support::decomposition`.
- [x] 2026-04-03: Record 6.4.5 implementation decisions in
  `docs/brain-trust-lints-design.md`.
- [x] 2026-04-03: Mark roadmap item 6.4.5 done in `docs/roadmap.md`.
- [x] 2026-04-03: Run `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, and `make test` successfully (1225 tests, 0
  failures). `make kani` completes full Kani/CBMC verification for all 6
  shipped harnesses with zero failures.
- [x] 2026-04-03: Finalize the living sections in this ExecPlan after
  implementation.
- [x] 2026-04-11: Address PR review findings — add exclusion-property
  harness (`verify_build_adjacency_no_spurious_edges`), document sort-coverage
  limitation on `verify_build_adjacency_sorted_neighbours`, document
  `kani::assume(node_count > 0)` intent in harness doc-comments, and update
  design and developer documentation.

## Surprises & Discoveries

- `build_adjacency` is a private helper inside
  `common/src/decomposition_advice/community.rs`. It is currently only reached
  from `detect_communities`, which makes colocated verification harnesses the
  least invasive way to call it.
- `build_similarity_edges` already enforces the key input invariants that make
  6.4.5 tractable: each edge is unique, `left < right`, the endpoints are
  within `vectors.len()`, and `weight > 0`.
- There is no existing Kani script, Make target, or repository-local install
  workflow, so 6.4.5 must establish that sidecar from scratch.
- The repository already has the right pattern for proof sidecars in the Verus
  workflow at `scripts/install-verus.sh`, `scripts/run-verus.sh`, and
  `make verus`. Kani should mirror that shape instead of inventing a second
  style.
- `common/src/test_support/decomposition.rs` already exposes narrow helper
  seams for the 6.4.3 and 6.4.4 proof items, so 6.4.5 can follow the same
  pattern for adjacency reporting.
- `common/Cargo.toml` still comments that behavioural specs use
  `rstest-bdd 0.2.x`, which contradicts the workspace pin and the current user
  requirement to use `0.5.0`. Fixed during implementation.
- `build_adjacency` was private (`fn`), requiring promotion to `pub(crate)` to
  allow unit tests and the `test_support::decomposition` adjacency report
  helper to call it. This is consistent with `build_similarity_edges` and does
  not widen the public crate API.
- The `community` module itself was private (`mod community`), requiring
  promotion to `pub(crate) mod community` so that `test_support` code within
  the same crate could access `SimilarityEdge` and `build_adjacency`.
- Kani 0.67.0 distributes pre-built tarballs per platform, so the install
  script downloads and extracts a `.tar.gz` rather than using the Verus-style
  `.zip` unpack.
- `cfg(kani)` triggers an `unexpected_cfgs` warning under the 2024 edition
  unless registered via `check-cfg` in `[lints.rust]` in `Cargo.toml`.
- Kani 0.67.0 ships `kani-driver` rather than a `cargo-kani` binary.
  The install script creates a `cargo-kani` symlink so the driver switches to
  cargo-plugin mode. The driver also expects a `toolchain/` directory symlinked
  to the matching nightly rustc/cargo installation.
- `kani-compiler` is dynamically linked against the toolchain's
  `libLLVM`, so `LD_LIBRARY_PATH` must include `<install>/toolchain/lib`.
  CBMC's `goto-cc` invokes `gcc` via `execvp`, requiring `CC` and `PATH` to
  include the system C compiler.
- Rust's standard `sort_by` generates deeply nested loop structures that
  cause CBMC state-space explosion. With `MAX_NODES=4` and `MAX_EDGES=6`, a
  single harness did not complete within a 5-minute wall-clock budget. Reducing
  to `MAX_NODES=3` / `MAX_EDGES=3` / `unwind(7)` makes the model tractable at
  the cost of a smaller verification envelope. Full CBMC runs should target CI
  runners with dedicated resources.

## Decision Log

- Decision: establish a sidecar Kani workflow with `scripts/install-kani.sh`,
  `scripts/run-kani.sh`, and `make kani`. Rationale: the repository already
  treats proof tooling as top-level sidecar infrastructure, and Kani should be
  reproducible without polluting normal developer builds. Date/Author:
  2026-04-02 / Codex.
- Decision: place the 6.4.5 harnesses adjacent to `build_adjacency`, ideally
  in a `#[cfg(kani)]` verification child module under `community.rs`.
  Rationale: child modules can call the private helper directly, which avoids
  widening the runtime API or duplicating the implementation in proof code.
  Date/Author: 2026-04-02 / Codex.
- Decision: model symbolic inputs with a fixed-size array of edge specs plus a
  symbolic active-length field instead of a symbolic `Vec`. Rationale: this
  gives Kani a small, explicit search space and avoids requiring
  `kani::Arbitrary` implementations for `Vec<SimilarityEdge>`. Date/Author:
  2026-04-02 / Codex.
- Decision: constrain the Kani harness to the valid-edge contract established
  by `build_similarity_edges`, not to arbitrary malformed pairs. Rationale: the
  roadmap item is about preserving real similarity edges, not inventing a new
  defensive API contract for malformed inputs. Date/Author: 2026-04-02 / Codex.
- Decision: expose behavioural-test observations through a test-support report
  type rather than by making `build_adjacency` or raw adjacency vectors public.
  Rationale: integration tests need observable results, but the production API
  should stay narrow. Date/Author: 2026-04-02 / Codex.
- Decision: include one unhappy-path BDD scenario in the test-support seam by
  rejecting malformed declarative edge input before it reaches the private
  runtime helper. Rationale: the user asked for unhappy-path coverage, and a
  validated test helper can provide that without changing production semantics.
  Date/Author: 2026-04-02 / Codex.

## Context and orientation

The implementation under verification lives in
`common/src/decomposition_advice/community.rs`.

Today, the relevant runtime shape is:

1. `build_similarity_edges(vectors)` computes unique similarity edges with
   positive weights when the cosine threshold is met.
2. `build_adjacency(node_count, edges)` creates a `Vec<Vec<(usize, u64)>>`,
   inserts both directions for every edge, and sorts each neighbour list by
   neighbour index.
3. `propagate_labels(...)` consumes that adjacency list for deterministic
   label propagation. Roadmap item 6.4.6 will verify label propagation
   separately; 6.4.5 must stop at adjacency construction.

The repository areas most likely to change are:

1. `common/src/decomposition_advice/community.rs`
2. A new Kani verification child file adjacent to `community.rs`
3. `common/src/decomposition_advice/tests.rs` plus a new adjacency-focused
   unit-test sibling file
4. `common/src/test_support/decomposition.rs`
5. `common/tests/decomposition_adjacency_behaviour.rs`
6. `common/tests/features/decomposition_adjacency.feature`
7. `scripts/install-kani.sh`
8. `scripts/run-kani.sh`
9. `Makefile`
10. `docs/brain-trust-lints-design.md`
11. `docs/roadmap.md`

The closest existing examples to reuse are:

1. `scripts/install-verus.sh`, `scripts/run-verus.sh`, and `make verus` for
   the proof-tooling sidecar pattern.
2. `common/src/test_support/decomposition.rs` and
   `common/tests/decomposition_vector_algebra_behaviour.rs` for a narrow
   behavioural-test seam.
3. `common/src/decomposition_advice/tests/vector_algebra.rs` for focused
   helper-level unit coverage.

## Proposed implementation shape

The safest implementation is to keep the runtime adjacency code small, prove
bounded invariants with Kani, and observe representative behaviour through unit
and BDD tests.

### Milestone 1: establish the Kani workflow and red phase

Start by making the missing Kani work observable before filling in the final
harnesses.

1. Add `scripts/install-kani.sh` that pins a specific Kani version, installs
   it into a repository-local cache or install root, and runs the equivalent of
   `cargo kani setup` from that pinned binary.
2. Add `scripts/run-kani.sh` that invokes the pinned Kani binary against the
   `common` crate and accepts an optional harness filter.
3. Add `make kani` to the `Makefile`.
4. Add a skeletal verification child module next to `build_adjacency` with the
   intended proof-harness names and placeholder assertions.
5. Run the targeted red-phase command and confirm it fails for the expected
   reason before filling in the final harness logic.

Recommended red-phase command:

```sh
set -o pipefail && make kani | tee /tmp/6-4-5-red-kani.log
```

The command should fail because the adjacency harnesses are not fully
implemented yet, not because the runner cannot find Kani or cannot locate the
crate.

### Milestone 2: add bounded Kani harnesses for adjacency invariants

Implement the proof harnesses adjacent to `build_adjacency`.

The most practical harness shape is:

1. Define a small proof-only edge-spec struct with `left`, `right`, and
   `weight` fields.
2. Use a symbolic fixed-size array of those specs plus a symbolic
   `active_edge_count`.
3. Assume a small `node_count` bound such as `0..=3` and a matching maximum
   active-edge count of `3`, aligning the written plan with the shipped
   `MAX_NODES = 3` / `MAX_EDGES = 3` model envelope.
4. Assume that every active edge satisfies the production preconditions:
   `left < right`, `right < node_count`, `weight > 0`, and no duplicate
   unordered pair.
5. Materialize a concrete `Vec<SimilarityEdge>` from the active prefix, call
   `build_adjacency`, and assert the roadmap properties.

Use separate proof harnesses when that improves failure localization. The
minimum required assertions are:

1. `adjacency.len() == node_count`.
2. For every active input edge `(left, right, weight)`, adjacency `left`
   contains `(right, weight)` and adjacency `right` contains `(left, weight)`.
3. Every neighbour index in every adjacency bucket is `< node_count`.
4. For every adjacency entry `(node -> neighbour, weight)`, the mirrored entry
   `(neighbour -> node, weight)` also exists.
5. Optional but useful: each per-node neighbour list is sorted by neighbour
   index after construction, since the runtime intentionally sorts those
   vectors.

If the sort operation requires a Kani unwind bound, keep that configuration
close to the Kani runner or the harness annotations and record the final value
in the design doc.

### Milestone 3: add focused runtime unit coverage

Add adjacency-specific unit coverage under
`common/src/decomposition_advice/tests/`.

Prefer a new sibling file such as
`common/src/decomposition_advice/tests/adjacency.rs`, wired from
`common/src/decomposition_advice/tests.rs`.

Required unit coverage:

1. Empty edge input yields `node_count` empty neighbour lists.
2. A single edge is inserted in both directions with the original weight.
3. Multiple edges touching one node produce a neighbour list sorted by
   neighbour index.
4. Sparse graphs preserve isolated nodes as empty lists.
5. A small multi-edge example proves symmetry across more than one connected
   component.

These tests should exercise the shipped adjacency builder through a crate-local
helper or report seam rather than through higher-level decomposition
suggestions.

### Milestone 4: add behavioural coverage with `rstest-bdd`

Add an integration-test pair:

1. `common/tests/decomposition_adjacency_behaviour.rs`
2. `common/tests/features/decomposition_adjacency.feature`

Do not expose raw adjacency internals publicly. Instead, add one narrow helper
to `common/src/test_support/decomposition.rs`, for example:

```text
adjacency_report(node_count, edges) -> Result<AdjacencyReport, AdjacencyError>
```

The helper should:

1. Accept declarative edge input suitable for BDD scenarios.
2. Validate that the declared endpoints are within bounds and reject malformed
   input before calling the private runtime helper.
3. Return an easily asserted report, such as normalized per-node neighbour
   lists and convenience predicates for symmetry and in-bounds checks.

Suggested BDD scenarios:

1. Happy path: two or more valid edges produce symmetric neighbour lists for
   every connected node.
2. Unhappy path: malformed declarative input with `left >= right` is rejected
   by the test-support wrapper.
3. Edge case: a graph with isolated nodes yields empty neighbour lists for
   those nodes.
4. Edge case: multiple neighbours for one node appear in sorted order.

Follow existing repository patterns:

1. Use a small world fixture.
2. Return `Result<(), String>` from fallible step functions.
3. Keep step argument counts within the Clippy threshold.
4. Bind scenarios by index, as the existing decomposition behaviour tests do.

### Milestone 5: document the modelling decisions and close the roadmap item

Update `docs/brain-trust-lints-design.md` with a new
`### Implementation decisions (6.4.5)` section. Record at least:

1. The decision to model only valid similarity-edge inputs already guaranteed
   by `build_similarity_edges`.
2. The chosen Kani harness shape, including the final node and edge bounds.
3. Any required unwind configuration for adjacency sorting.
4. The choice to keep the observable behavioural seam in
   `common::test_support::decomposition`.
5. The pinned Kani workflow and why it lives in a sidecar rather than inside
   Cargo dependencies.

After all validation succeeds, mark roadmap item 6.4.5 done in
`docs/roadmap.md`.

## Validation and evidence

After implementation, run the full required gates with log capture:

```sh
set -o pipefail && make fmt | tee /tmp/6-4-5-fmt.log
set -o pipefail && make markdownlint | tee /tmp/6-4-5-markdownlint.log
set -o pipefail && make nixie | tee /tmp/6-4-5-nixie.log
set -o pipefail && make check-fmt | tee /tmp/6-4-5-check-fmt.log
set -o pipefail && make lint | tee /tmp/6-4-5-lint.log
set -o pipefail && make test | tee /tmp/6-4-5-test.log
set -o pipefail && make kani | tee /tmp/6-4-5-kani.log
```

Useful targeted commands during implementation:

```sh
set -o pipefail && cargo test -p common adjacency -- --nocapture | tee /tmp/6-4-5-unit.log
set -o pipefail && cargo test -p common --test decomposition_adjacency_behaviour -- --nocapture | tee /tmp/6-4-5-bdd.log
set -o pipefail && make kani | tee /tmp/6-4-5-kani-targeted.log
```

Expected success signals:

```plaintext
- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0 and reports the new adjacency unit and behaviour tests.
- `make kani` exits 0 after full Kani/CBMC verification and reports zero
  failing harnesses.
```

Before marking the roadmap item done, inspect the captured logs and make sure
the Kani harnesses, the new unit-test module, and the new BDD binary all
actually ran.

## Outcomes & Retrospective

Implementation completed 2026-04-03.

The repository now ships:

- A pinned `make kani` workflow (`scripts/install-kani.sh`,
  `scripts/run-kani.sh`) that downloads Kani 0.67.0, configures the required
  nightly toolchain, and runs bounded model checking against the `common` crate.
- Six bounded Kani harnesses (`MAX_NODES=3`, `MAX_EDGES=3`,
  `unwind(7)`) verifying that `build_adjacency` preserves valid similarity
  edges, keeps neighbour indices in bounds, produces symmetric adjacency lists,
  emits no spurious edges, and sorts neighbours. `make kani` runs the shipped
  full Kani/CBMC verification flow for these harnesses and succeeds with zero
  failing proofs at the documented bounds.
- Five focused unit tests for `build_adjacency` covering empty edges,
  single-edge both-directions, multiple edges sorted, sparse graph isolated
  nodes, and multi-edge symmetry.
- Four `rstest-bdd` BDD scenarios exercising adjacency behaviour through
  the `common::test_support::decomposition` seam, including an unhappy-path
  validation case.
- All standard quality gates pass: 1225 tests, 0 failures; clippy,
  rustdoc, check-fmt, markdownlint, and nixie clean.
- A design-document record of the exact modelling decisions and bounds
  that shipped.

Shell script test exemption: `scripts/install-kani.sh` and
`scripts/run-kani.sh` do not have dedicated BATS or unit-level shell script
tests. This mirrors the existing Verus sidecar pattern
(`scripts/install-verus.sh`, `scripts/run-verus.sh`), which similarly relies on
the `make kani` / `make verus` integration targets as the validation seam
rather than isolated shell script unit tests. The scripts contain
platform-detection logic, cache directory management, and environment variable
setup, but these are exercised end-to-end whenever `make kani` runs. Adding
BATS coverage for the sidecar scripts is a possible future improvement tracked
separately from the 6.4.5 scope.

Remaining adjacent roadmap work after 6.4.5 will still include:

- 6.4.6 for Kani verification of `propagate_labels`.
