# Implement trait item counting, default method cognitive complexity (CC) aggregation, and implementor burden metrics (roadmap 6.3.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 6.3.1 delivers the metric-collection layer for `brain_trait`. This
is the trait analogue of roadmap 6.2.1 (`brain_type` metrics), but scoped
strictly to the three signals specified by `docs/brain-trust-lints-design.md`
§`brain_trait` signals:

1. Interface size (trait item counting, with focus on required methods).
2. Default method complexity (sum cognitive complexity across default bodies).
3. Implementor burden (required method count each implementor must provide).

After this change, the `common` crate provides a pure, compiler-independent
metrics module for traits that future lint-driver work can populate from
High-level Intermediate Representation (HIR).
No threshold evaluation is included in 6.3.1; that remains roadmap 6.3.2.

Observable outcome:

1. `common` exports new trait metric types and helpers.
2. Unit tests validate happy, unhappy, and edge cases.
3. Behaviour tests using `rstest-bdd` v0.5.0 validate end-to-end metric
   contracts.
4. `docs/brain-trust-lints-design.md` records implementation decisions for
   6.3.1.
5. `docs/roadmap.md` marks 6.3.1 done only after implementation and all
   quality gates pass.
6. `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Scope only roadmap item 6.3.1. Do not implement 6.3.2 threshold rules in this
  change.
- Keep `common` free of `rustc_private` dependencies. Accept plain Rust values
  (`String`, `usize`, enums, slices, vectors) only.
- Follow established module layout and line-count constraints (`< 400` lines per
  source file). Split files if needed.
- Use builder patterns when constructor argument count would exceed workspace
  Clippy limits (`too_many_arguments` threshold is 4).
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`).
- Preserve deterministic behaviour (stable ordering for rendered metric lists).
- Use en-GB-oxendict spelling in comments/docs.
- Update design documentation with any final decisions made during
  implementation.
- Completion must include roadmap checkbox update for 6.3.1.

## Tolerances (exception triggers)

- Scope tolerance: if implementation exceeds 10 touched files or 1000 net lines
  of code (LOC), stop and escalate.
- API tolerance: if implementing 6.3.1 requires changing existing
  `brain_type_metrics` public APIs, stop and escalate.
- Dependency tolerance: if any new dependency appears necessary, stop and
  escalate.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after 3 targeted fix iterations, stop and escalate with captured logs.

## Risks

- Ambiguity risk: design text says “total trait items, with a focus on required
  methods”, but does not prescribe exact item taxonomy. Mitigation: define
  explicit item model (required methods, default methods, associated types,
  associated consts) and record decision.
- Behaviour drift risk: implementing burden metrics as anything other than
  required method count could diverge from design intent. Mitigation: treat
  burden as required-method cardinality in 6.3.1, reserve weighted variants for
  future work.
- Clippy risk in tests: `rstest` and `rstest-bdd` parameter counts can trigger
  `too_many_arguments`. Mitigation: use tuple-style `#[case]` inputs and keep
  BDD step placeholders to at most 3 parsed values (+ `world`).
- Macro-inflation risk: default CC can be overstated when macro-expanded spans
  are included upstream. Mitigation: include an explicit collection API that
  accepts `is_from_expansion: bool` for default methods and ignores
  expansion-derived entries.

## Progress

- [x] Stage A: Draft this ExecPlan.
- [x] Stage B: Add failing unit and behaviour tests (red phase) for trait
  metrics contract.
- [x] Stage C: Implement new `common::brain_trait_metrics` module.
- [x] Stage D: Wire `common/src/lib.rs` exports.
- [x] Stage E: Make tests green and refactor while preserving behaviour.
- [x] Stage F: Record implementation decisions in design doc.
- [x] Stage G: Mark roadmap item 6.3.1 done.
- [x] Stage H: Run `make check-fmt`, `make lint`, and `make test` successfully.
- [x] Stage I: Complete outcomes and retrospective section.

## Surprises & Discoveries

- The repository currently has no dedicated `brain_trait` lint crate.
  Existing brain trust work for 6.1.x and 6.2.x is implemented as reusable
  `common` modules with unit and BDD coverage first.
- Existing patterns for brain-trust metrics are split into small files under
  `common/src/brain_type_metrics/` and companion behaviour tests in
  `common/tests/` with indexed `#[scenario(..., index = N)]` bindings.
- `rstest-bdd` fixture name matching is literal. Step functions must use
  `world` (not `_world`) for the world fixture parameter.
- A dedicated `TraitItemMetrics` + `TraitMetricsBuilder` API works cleanly for
  both unit and BDD tests without introducing extra helper structs.
- `common/src/brain_trait_metrics/mod.rs` landed at 390 lines. This stayed
  under the 400-line limit, but only narrowly; future additions should split
  the module into sibling files early.
- `make test` again showed the expected long-tail user interface (UI) behaviour
  (`bumpy_road` and `conditional_max_n_branches` UI suites), but completed
  successfully.

## Decision Log

- Decision: created a dedicated module `common/src/brain_trait_metrics/`
  rather than extending `common/src/brain_type_metrics/`. Rationale: keeps type
  and trait concerns decoupled while mirroring the repository’s existing
  feature-oriented organization. Date/Author: 2026-03-01 / Codex.
- Decision: modelled implementor burden as required-method count.
  Rationale: this directly matches design language and keeps 6.3.1 strictly a
  metric-collection task. Date/Author: 2026-03-01 / Codex.
- Decision: included macro-expansion filtering in default method ingestion via
  `add_default_method(name, cc, is_from_expansion)`. Rationale: consistent with
  established macro-filtering APIs (`ForeignReferenceSet`, `MethodInfoBuilder`,
  `CognitiveComplexityBuilder`). Date/Author: 2026-03-01 / Codex.
- Decision: implemented `TraitItemKind` as four explicit variants
  (`RequiredMethod`, `DefaultMethod`, `AssociatedType`, `AssociatedConst`) and
  kept all per-item data in a single `TraitItemMetrics` struct. Rationale: this
  keeps counting semantics transparent and avoids duplicating item containers.
  Date/Author: 2026-03-01 / Codex.
- Decision: implementor burden is computed as required-method count and exposed
  on `TraitMetrics`. Rationale: this directly matches design intent and avoids
  introducing speculative weighting rules in 6.3.1. Date/Author: 2026-03-01 /
  Codex.
- Decision: default methods marked `is_from_expansion = true` are discarded by
  `TraitMetricsBuilder::add_default_method`. Rationale: preserves consistency
  with existing macro-filtering semantics used elsewhere in `common`.
  Date/Author: 2026-03-01 / Codex.

## Context and orientation

Relevant current state:

- `common/src/brain_type_metrics/` already implements the equivalent collection
  and evaluation pipeline for type metrics.
- `common/tests/features/brain_type_metrics.feature` and
  `common/tests/brain_type_metrics_behaviour.rs` are the canonical BDD pattern
  for metric collection in this repository.
- `docs/brain-trust-lints-design.md` defines 6.3.1 signals and 6.3.2 thresholds
  separately.
- Workspace already pins `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"` in `[workspace.dependencies]`.

## Plan of work

### Stage B: Write failing tests first (red)

Create tests that encode the expected metric contract before implementation.

Files to add:

- `common/src/brain_trait_metrics/tests.rs`
- `common/tests/features/brain_trait_metrics.feature`
- `common/tests/brain_trait_metrics_behaviour.rs`

Unit test coverage matrix:

1. Trait item counting includes methods, associated types, and associated
   consts.
2. Required method counting excludes default methods.
3. Default method CC aggregation sums only default implementations.
4. Implementor burden equals required method count.
5. Empty trait produces zeroed metrics.
6. Expansion-flagged default methods are excluded when flagged.
7. Boundary and mixed cases (all required, all default, mixed associated items).

Behaviour test coverage (`rstest-bdd` v0.5.0):

1. Happy path with mixed trait items and expected totals.
2. Unhappy path where no default methods exist (default CC sum remains zero).
3. Edge path for empty trait.
4. Expansion-filtered default methods not contributing to CC sum.
5. Implementor burden tracks required method count only.

### Stage C: Implement trait metrics module

Create a new directory module:

- `common/src/brain_trait_metrics/mod.rs`

Planned public API (names may be refined during implementation):

```rust
pub enum TraitItemKind {
    RequiredMethod,
    DefaultMethod,
    AssociatedType,
    AssociatedConst,
}

pub struct TraitItemMetrics {
    name: String,
    kind: TraitItemKind,
    default_method_cc: Option<usize>,
}

pub struct TraitMetrics {
    trait_name: String,
    total_item_count: usize,
    required_method_count: usize,
    default_method_count: usize,
    default_method_cc_sum: usize,
    implementor_burden: usize,
}

pub struct TraitMetricsBuilder { /* incremental collector */ }
```

Implementation notes:

- Keep construction incremental to match lint-driver traversal style.
- Ensure `implementor_burden` is derived from required methods, not separately
  mutable.
- Expose small, testable helpers where useful, for example:
  `trait_item_count`, `required_method_count`, `default_method_cc_sum`.

### Stage D: Wire exports

Update `common/src/lib.rs` to:

- add `pub mod brain_trait_metrics;`
- re-export public trait-metric types/functions for downstream lint-driver use.

### Stage E: Green and refactor

Make failing tests pass, then refactor for readability without changing
behaviour.

Refactoring guardrails:

- Keep functions focused and small.
- Prefer explicit helper functions over dense branching.
- Split files before approaching the 400-line limit.

### Stage F: Record design decisions

Update `docs/brain-trust-lints-design.md` with a new subsection:

- `### Implementation decisions (6.3.1)`

Record at least:

1. Trait item taxonomy used for interface size.
2. Exact implementor burden definition.
3. Expansion filtering semantics for default CC aggregation.
4. Any data-structure choices made for determinism.

### Stage G: Mark roadmap complete

After implementation and validation succeed, update `docs/roadmap.md`:

- change 6.3.1 checkbox from `[ ]` to `[x]`.

### Stage H: Quality gates

Run all required gates with log capture.

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/6-3-1-check-fmt.log
```

```sh
set -o pipefail
make lint 2>&1 | tee /tmp/6-3-1-lint.log
```

```sh
set -o pipefail
make test 2>&1 | tee /tmp/6-3-1-test.log
```

Expected success markers:

```plaintext
make check-fmt: exit code 0
make lint: exit code 0
make test: exit code 0
```

### Stage I: Finalize the living sections

When the implementation completes, update this file:

- set `Status` to `COMPLETE`.
- fill `Surprises & Discoveries` with real observations.
- append final decisions with dates in `Decision Log`.
- complete `Outcomes & Retrospective` with deliverables, test counts, and any
  follow-up work.

## Outcomes & Retrospective

Implemented roadmap 6.3.1 end-to-end.

Delivered:

- Added `common/src/brain_trait_metrics/mod.rs` with:
  - `TraitItemKind` and `TraitItemMetrics`,
  - counting helpers (`trait_item_count`, `required_method_count`,
    `default_method_count`, `default_method_cc_sum`),
  - aggregate `TraitMetrics`,
  - incremental `TraitMetricsBuilder` with macro-expansion filtering for
    default methods.
- Added unit coverage in `common/src/brain_trait_metrics/tests.rs` (16 tests)
  covering happy, unhappy, and edge cases.
- Added behaviour coverage with `rstest-bdd` v0.5.0:
  - `common/tests/features/brain_trait_metrics.feature` (6 scenarios),
  - `common/tests/brain_trait_metrics_behaviour.rs` step bindings.
- Wired public exports through `common/src/lib.rs`.
- Recorded implementation decisions in
  `docs/brain-trust-lints-design.md` (`### Implementation decisions (6.3.1)`).
- Marked roadmap item 6.3.1 done in `docs/roadmap.md`.

Validation:

- `make check-fmt` passed (`/tmp/6-3-1-check-fmt.log`).
- `make lint` passed (`/tmp/6-3-1-lint.log`).
- `make test` passed (`/tmp/6-3-1-test.log`) with summary:
  `882 tests run: 882 passed (2 slow), 2 skipped`.

Scope check:

- Touched 8 files total (within tolerance).
- New code files remain under 400 lines.
- No new dependencies added.
