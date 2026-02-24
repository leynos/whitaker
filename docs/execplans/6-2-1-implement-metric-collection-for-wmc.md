# Implement metric collection for WMC, brain method detection, LCOM4, and foreign reach (roadmap 6.2.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

This document must be maintained in accordance with `AGENTS.md`. On approval,
copy this plan to `docs/execplans/6-2-1-implement-metric-collection-for-wmc.md`.

## Purpose / big picture

Roadmap item 6.2.1 delivers the metric collection layer that the `brain_type`
lint driver (6.2.2) will consume to evaluate brain type thresholds. This task
provides pure library types and functions in `common/` for:

1. **WMC (Weighted Methods Count)**: sum cognitive complexity (CC) across all
   methods in a type.
2. **Brain method detection**: identify methods where CC >= threshold AND
   LOC >= threshold.
3. **LCOM4 integration**: reuse the existing `cohesion_components()` from
   `common/src/lcom4/mod.rs` — this task provides the aggregate struct that
   carries the LCOM4 value alongside other metrics.
4. **Foreign reach**: count distinct external modules or types referenced by a
   type's methods (ATFD analogue).

After this change:

1. `common::brain_type_metrics::MethodMetrics` stores per-method CC and LOC.
2. `common::brain_type_metrics::weighted_methods_count()` computes WMC.
3. `common::brain_type_metrics::brain_methods()` identifies brain methods.
4. `common::brain_type_metrics::ForeignReferenceSet` accumulates and
   deduplicates external references with macro-expansion filtering.
5. `common::brain_type_metrics::TypeMetrics` aggregates all four signals.
6. `common::brain_type_metrics::TypeMetricsBuilder` provides incremental
   construction for lint drivers.
7. Unit tests (`#[rstest]`) cover happy, unhappy, and edge cases.
8. BDD tests (`rstest-bdd` v0.5.0) in Gherkin scenarios cover the metric
   collection contract.
9. `docs/brain-trust-lints-design.md` records implementation decisions for
   6.2.1.
10. `docs/roadmap.md` marks 6.2.1 as done.
11. `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- All new code lives in `common/src/brain_type_metrics/` — a new directory
  module alongside the existing `lcom4/` directory.
- No `rustc_private` dependencies in `common/`. All types use `String`,
  `usize`, `BTreeSet<String>`.
- Keep every file under 400 lines.
- No new external dependencies (only `std` and existing `common` types).
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros` at
  v0.5.0 for tests.
- Use `BTreeSet<String>` for deterministic iteration in `ForeignReferenceSet`.
- Use `#[must_use]` on all pure functions and constructors.
- Comments and documentation in en-GB-oxendict spelling.
- Markdown wrapped at 80 columns, code blocks at 120.
- Observe `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"`
  in non-test code.
- On completion, update `docs/roadmap.md` entry 6.2.1 to `[x]`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 12 files or 1200
  net lines, stop and escalate.
- Dependencies: if any new external dependency is needed, stop and escalate.
- Interface: if the existing `MethodInfo` or `cohesion_components` public API
  must change, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts, stop and escalate with logs.

## Risks

- Risk: `TypeMetricsBuilder` API may prove insufficient when the lint driver
  (6.2.2) implements HIR walking (e.g., needing per-method `MethodInfo` and
  `MethodMetrics` in the same pass).
  Severity: medium. Likelihood: low (builder is additive).
  Mitigation: keep builder API additive; new setters can be added later.

- Risk: `ForeignReferenceSet` path representation may not match what the HIR
  walker produces (e.g., `DefPath` vs module path strings).
  Severity: low. Likelihood: medium.
  Mitigation: use plain `String` and document that the HIR walker converts to
  a string representation before recording.

- Risk: The tests file exceeds 400 lines.
  Severity: low. Likelihood: medium.
  Mitigation: split into `tests.rs` and `tests/foreign_reach_tests.rs` or
  move BDD coverage to the integration test harness.

## Progress

- [x] Stage A: Create `MethodMetrics`, `weighted_methods_count`,
  `brain_methods` in `common/src/brain_type_metrics/mod.rs`.
- [x] Stage B: Create `ForeignReferenceSet` and `foreign_reach_count` in
  `common/src/brain_type_metrics/foreign_reach.rs`.
- [x] Stage C: Create `TypeMetrics` and `TypeMetricsBuilder` in
  `common/src/brain_type_metrics/mod.rs`.
- [x] Stage D: Wire module into `common/src/lib.rs` with re-exports.
- [x] Stage E: Add unit tests in `common/src/brain_type_metrics/tests.rs`.
- [x] Stage F: Add BDD feature file and behaviour harness.
- [x] Stage G: Record design decisions in `docs/brain-trust-lints-design.md`.
- [x] Stage H: Run quality gates.
- [x] Stage I: Mark roadmap item 6.2.1 as done.

## Surprises & discoveries

- Clippy's `too_many_arguments` lint (limit 4 in this workspace) fired on the
  initially planned `TypeMetrics::new()` constructor which had 6 parameters.
  Resolved by removing the public constructor entirely and making
  `TypeMetricsBuilder` the sole public construction path. This is
  architecturally cleaner — the builder already existed and is the natural
  entry point for lint drivers.

- The `tests.rs` file stayed comfortably under 400 lines (~300 lines for
  32 unit tests), so the risk of needing to split tests did not materialise.

## Decision log

- Decision: create a new `brain_type_metrics` module rather than extending
  `complexity_signal`.
  Rationale: `complexity_signal` provides per-line signal rasterisation and
  smoothing for the bumpy road lint. Brain type metrics operate at the
  per-method aggregate level — a fundamentally different abstraction. Keeping
  them separate maintains single responsibility.
  Date/Author: 2026-02-24 / DevBoxer.

- Decision: `MethodMetrics` stores pre-computed `cognitive_complexity: usize`
  rather than computing CC from source.
  Rationale: the `common` crate has no `rustc_private` dependencies. The actual
  CC computation from HIR happens in the lint driver (6.2.2), which passes the
  pre-computed value into `MethodMetrics`. This follows the same pattern as
  `MethodInfoBuilder` (pure library stores and aggregates; HIR walker produces).
  Date/Author: 2026-02-24 / DevBoxer.

- Decision: use a builder pattern (`TypeMetricsBuilder`) for aggregate metrics.
  Rationale: follows the `MethodInfoBuilder` pattern from 6.1.2. The lint
  driver discovers methods incrementally during HIR traversal and can call
  `add_method()` for each. Brain method thresholds are provided at construction
  time so the builder identifies brain methods during `build()`.
  Date/Author: 2026-02-24 / DevBoxer.

- Decision: `ForeignReferenceSet` uses `is_from_expansion: bool` parameter
  for macro filtering.
  Rationale: mirrors the pattern in `MethodInfoBuilder.record_field_access()`
  and `MethodInfoBuilder.record_method_call()`. The HIR walker calls
  `record_reference(&path_string, span.from_expansion())`.
  Date/Author: 2026-02-24 / DevBoxer.

- Decision: remove `TypeMetrics::new()` public constructor; use
  `TypeMetricsBuilder` as the sole construction path.
  Rationale: Clippy's `too_many_arguments` lint (workspace limit 4) rejected
  `TypeMetrics::new()` with 6 parameters. The builder already existed and is
  the natural entry point for lint drivers that discover methods incrementally.
  Removing the direct constructor enforces correct usage patterns.
  Date/Author: 2026-02-24 / DevBoxer.

## Outcomes & retrospective

### Deliverables

All 11 acceptance criteria met:

1. `MethodMetrics` struct with `new()`, `name()`, `cognitive_complexity()`,
   `lines_of_code()`, `is_brain_method()`.
2. `weighted_methods_count()` computes WMC as sum of CC.
3. `brain_methods()` filters methods by dual CC/LOC thresholds.
4. `ForeignReferenceSet` with `record_reference()`, macro-expansion filtering,
   and `count()`.
5. `TypeMetrics` aggregate struct with all four signal accessors.
6. `TypeMetricsBuilder` for incremental construction (sole construction path
   after removing `TypeMetrics::new()`).
7. 32 rstest unit tests covering happy, unhappy, and edge cases.
8. 9 BDD scenarios in Gherkin with rstest-bdd v0.5.0 harness.
9. Design decisions recorded in `docs/brain-trust-lints-design.md`.
10. `docs/roadmap.md` marks 6.2.1 as `[x]`.
11. `make check-fmt`, `make lint`, and `make test` all pass.

### Metrics

- Files created: 5 (mod.rs, foreign_reach.rs, tests.rs, feature file,
  behaviour harness).
- Files modified: 3 (lib.rs, brain-trust-lints-design.md, roadmap.md).
- New test count: 41 (32 unit + 9 BDD).
- All files under 400 lines.
- No new external dependencies.
- Quality gate iterations: 2 (first for formatting, second for Clippy
  `too_many_arguments`).

### Lessons

- The workspace enforces Clippy's `too_many_arguments` at a limit of 4.
  Future structs with more than 4 fields should use builders as the sole
  public construction path from the start.
- The `MethodInfoBuilder` pattern from 6.1.2 translated cleanly to
  `TypeMetricsBuilder`. This confirms the builder-per-aggregate pattern
  works well for incremental construction during HIR traversal.

## Context and orientation

### Repository state

The `common` crate (`common/src/lib.rs`) exports modules: `attributes`,
`complexity_signal`, `context`, `diagnostics`, `expr`, `i18n`, `lcom4`, `path`,
`span`, `test_support`. The `lcom4` module uses a directory layout:

- `common/src/lcom4/mod.rs` (311 lines) — `MethodInfo`, `UnionFind`,
  `cohesion_components`, helper functions.
- `common/src/lcom4/extract.rs` (363 lines) — `MethodInfoBuilder`,
  `collect_method_infos`, inline unit tests.
- `common/src/lcom4/tests.rs` — rstest unit tests for `cohesion_components`.

Re-exports in `common/src/lib.rs` line 31:

    pub use lcom4::{MethodInfo, MethodInfoBuilder, cohesion_components, collect_method_infos};

BDD tests for the LCOM4 module live at:

- `common/tests/lcom4_behaviour.rs` — `LcomWorld` struct with `RefCell`/`Cell`
  fields, step definitions, scenario bindings using
  `#[scenario(path = "tests/features/lcom4.feature", index = N)]`.
- `common/tests/features/lcom4.feature` — 8 Gherkin scenarios.

### Existing helpers to reuse

- `common::lcom4::cohesion_components(&[MethodInfo]) -> usize` — LCOM4
  computation. The lint driver in 6.2.2 will call this and pass the result
  into `TypeMetricsBuilder::set_lcom4()`.
- `common::span::span_line_count(SourceSpan) -> usize` — LOC from span.
  The lint driver will use rustc's `SourceMap` to get line counts and pass
  them into `MethodMetrics`.

### Design specification

From `docs/brain-trust-lints-design.md` §`brain_type` signals (lines 75–89):

- **WMC**: sum CC of all methods in the type. Default warn threshold 60.
- **Brain method**: CC >= 25 AND LOC >= 80 (both configurable).
- **LCOM4**: connected components >= 2 indicates low cohesion.
- **Foreign reach**: count distinct external modules/types. Default warn 10.

From §`brain_type` rule set (lines 91–98), for 6.2.2 context:

- Warn when WMC >= 60 AND at least one brain method AND LCOM4 >= 2.
- Escalate when WMC >= 100 OR multiple brain methods OR LCOM4 >= 3.

### Analogous pattern to follow

The closest analogue is the LCOM4 extraction layer (6.1.2):

- **Pure library**: `common/src/lcom4/extract.rs` provides `MethodInfoBuilder`
  with `is_from_expansion` filtering, `build()`, and `collect_method_infos()`.
- **Integration test**: `common/tests/lcom4_behaviour.rs` uses a `LcomWorld`
  struct with `RefCell<Vec<MethodInfo>>` and `Cell<Option<usize>>`, step macros,
  and indexed scenario bindings.
- **Feature file**: `common/tests/features/lcom4.feature` uses natural-language
  step text without quoted values.

## Plan of work

### Stage A: `MethodMetrics` and WMC/brain method functions

Create `common/src/brain_type_metrics/mod.rs` containing:

**`MethodMetrics` struct** — per-method metric record:

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MethodMetrics {
        name: String,
        cognitive_complexity: usize,
        lines_of_code: usize,
    }

Methods: `new(name, cognitive_complexity, lines_of_code)`, `name()`,
`cognitive_complexity()`, `lines_of_code()`,
`is_brain_method(cc_threshold, loc_threshold) -> bool`.

All public items have `///` Rustdoc with `# Examples` sections using the
`common::brain_type_metrics::MethodMetrics` path.

**`weighted_methods_count` function**:

    #[must_use]
    pub fn weighted_methods_count(methods: &[MethodMetrics]) -> usize

Returns `methods.iter().map(MethodMetrics::cognitive_complexity).sum()`.

**`brain_methods` function**:

    #[must_use]
    pub fn brain_methods(
        methods: &[MethodMetrics],
        cc_threshold: usize,
        loc_threshold: usize,
    ) -> Vec<&MethodMetrics>

Returns methods where `is_brain_method(cc_threshold, loc_threshold)` is true,
preserving input order.

Add placeholder submodules:

    pub mod foreign_reach;
    #[cfg(test)]
    mod tests;

Create empty `common/src/brain_type_metrics/foreign_reach.rs` (module doc
comment only) and `common/src/brain_type_metrics/tests.rs` (module doc comment
only).

Acceptance: `cargo check -p common` succeeds.

### Stage B: `ForeignReferenceSet` and `foreign_reach_count`

In `common/src/brain_type_metrics/foreign_reach.rs`:

**`ForeignReferenceSet` struct**:

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct ForeignReferenceSet {
        references: BTreeSet<String>,
    }

Methods:

- `new() -> Self` — empty set.
- `record_reference(&mut self, path: &str, is_from_expansion: bool)` — inserts
  path unless `is_from_expansion` is true.
- `count(&self) -> usize` — distinct reference count.
- `is_empty(&self) -> bool`.
- `references(&self) -> &BTreeSet<String>` — for diagnostic display.

**`foreign_reach_count` convenience function**:

    #[must_use]
    pub fn foreign_reach_count(
        references: impl IntoIterator<Item = (String, bool)>,
    ) -> usize

Builds a `ForeignReferenceSet`, records all references, returns count.

Add re-export in `mod.rs`:

    pub use foreign_reach::{ForeignReferenceSet, foreign_reach_count};

Acceptance: `cargo check -p common` succeeds.

### Stage C: `TypeMetrics` and `TypeMetricsBuilder`

In `common/src/brain_type_metrics/mod.rs`, add:

**`TypeMetrics` struct** — aggregate type-level metrics:

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TypeMetrics {
        type_name: String,
        wmc: usize,
        brain_method_names: Vec<String>,
        lcom4: usize,
        foreign_reach: usize,
        method_count: usize,
    }

Accessors: `type_name()`, `wmc()`, `brain_method_names()`,
`brain_method_count()`, `lcom4()`, `foreign_reach()`, `method_count()`.

**`TypeMetricsBuilder` struct** — incremental builder:

    #[derive(Clone, Debug)]
    pub struct TypeMetricsBuilder {
        type_name: String,
        method_metrics: Vec<MethodMetrics>,
        lcom4: Option<usize>,
        foreign_reach: Option<usize>,
        cc_threshold: usize,
        loc_threshold: usize,
    }

Methods:

- `new(type_name, cc_threshold, loc_threshold) -> Self`
- `add_method(&mut self, name, cognitive_complexity, lines_of_code)` — pushes
  a `MethodMetrics`.
- `set_lcom4(&mut self, lcom4: usize)`
- `set_foreign_reach(&mut self, count: usize)`
- `build(self) -> TypeMetrics` — computes WMC via `weighted_methods_count()`,
  identifies brain methods via `brain_methods()`, extracts their names, and
  assembles `TypeMetrics`. LCOM4 and foreign reach default to 0 if not set.

Acceptance: `cargo check -p common` succeeds.

### Stage D: Wire module into `common/src/lib.rs`

Add to `common/src/lib.rs`:

    pub mod brain_type_metrics;

And re-exports:

    pub use brain_type_metrics::{
        ForeignReferenceSet, MethodMetrics, TypeMetrics, TypeMetricsBuilder,
        brain_methods, foreign_reach_count, weighted_methods_count,
    };

Acceptance: `cargo check -p common` succeeds.

### Stage E: Unit tests

In `common/src/brain_type_metrics/tests.rs`, add `#[rstest]` tests:

**MethodMetrics tests:**

- `construction_and_accessors` — verify name, CC, LOC.
- `is_brain_method_both_above_thresholds` — CC=30, LOC=100, thresholds
  25/80 → true.
- `is_brain_method_at_exact_thresholds` — CC=25, LOC=80 → true (>=).
- `is_brain_method_cc_below_threshold` — CC=20, LOC=100 → false.
- `is_brain_method_loc_below_threshold` — CC=30, LOC=40 → false.
- `is_brain_method_both_below_thresholds` — CC=5, LOC=20 → false.

**`weighted_methods_count` tests:**

- `empty_slice_returns_zero`
- `single_method_returns_its_cc`
- `multiple_methods_returns_sum`
- `methods_with_zero_cc_contribute_nothing`

**`brain_methods` tests:**

- `empty_slice_returns_empty`
- `no_qualifying_methods`
- `one_qualifying_method`
- `multiple_qualifying_methods_in_order`
- `method_meeting_only_cc_threshold_excluded`
- `method_meeting_only_loc_threshold_excluded`

**ForeignReferenceSet tests:**

- `empty_set_has_zero_count`
- `distinct_references_counted`
- `duplicate_references_deduplicated`
- `macro_expanded_references_filtered`
- `mixed_expanded_and_regular`
- `foreign_reach_count_convenience`

**TypeMetricsBuilder tests:**

- `empty_builder_produces_zero_metrics`
- `builder_computes_wmc`
- `builder_identifies_brain_methods`
- `builder_defaults_lcom4_and_foreign_reach`
- `builder_preserves_set_values`
- `method_count_is_correct`

Acceptance: `cargo test -p common` passes all new tests.

### Stage F: BDD tests

**Create `common/tests/features/brain_type_metrics.feature`** with scenarios:

1. WMC is the sum of all method complexities
2. A method qualifies as a brain method when both thresholds exceeded
3. A method below both thresholds is not a brain method
4. A method meeting only the CC threshold is not a brain method
5. A method meeting only the LOC threshold is not a brain method
6. Empty type has zero WMC
7. Type metrics aggregate all signals
8. Foreign references are deduplicated
9. Macro-expanded foreign references are filtered

Step text uses unquoted natural-language style (no quoted values, no commas
in placeholders) following the pattern in `lcom4.feature`:

- `Given a method called {name} with CC {cc} and LOC {loc}`
- `And the brain method CC threshold is {threshold}`
- `And the brain method LOC threshold is {threshold}`
- `And the LCOM4 value is {value}`
- `And the foreign reach is {count}`
- `And a foreign reference to {path}`
- `And a foreign reference to {path} from expansion`
- `When I compute WMC`
- `When I identify brain methods`
- `When I build type metrics for {name}`
- `When I compute foreign reach`
- `Then the WMC is {value}`
- `Then {name} is a brain method`
- `Then there are no brain methods`
- `Then the type has {n} brain method(s)`
- `Then the type WMC is {value}`
- `Then the type LCOM4 is {value}`
- `Then the type foreign reach is {count}`
- `Then the foreign reach count is {count}`

**Create `common/tests/brain_type_metrics_behaviour.rs`** following the
`lcom4_behaviour.rs` pattern:

- `MetricsWorld` struct with `RefCell<Vec<MethodMetrics>>`,
  `Cell<Option<usize>>` for WMC, `RefCell<Vec<String>>` for brain method
  names, `Cell<usize>` for CC/LOC thresholds, `Cell<Option<usize>>` for
  LCOM4 and foreign reach, `RefCell<ForeignReferenceSet>` for foreign
  refs.
- `#[fixture] fn world() -> MetricsWorld`
- Step definitions for each step text.
- `#[scenario(path = "tests/features/brain_type_metrics.feature", index = N)]`
  bindings for each scenario (indices 0..8).

Acceptance: `cargo test -p common` passes all BDD scenarios.

### Stage G: Design document update

Append a `### Implementation decisions (6.2.1)` section to
`docs/brain-trust-lints-design.md` after the existing 6.1.2 decisions. Cover:

1. New `brain_type_metrics` module — separate from `complexity_signal` because
   per-method aggregate metrics are a different abstraction from per-line
   signal rasterisation.
2. `MethodMetrics` stores pre-computed CC and LOC — actual CC computation from
   HIR happens in the lint driver (6.2.2).
3. `TypeMetricsBuilder` follows the `MethodInfoBuilder` pattern for incremental
   construction during HIR traversal.
4. `ForeignReferenceSet` uses `is_from_expansion: bool` parameter for macro
   filtering, mirroring `MethodInfoBuilder`.

### Stage H: Quality gates

Run with `tee` and `set -o pipefail`:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-2-1-check-fmt.log
    set -o pipefail; make lint      2>&1 | tee /tmp/6-2-1-lint.log
    set -o pipefail; make test      2>&1 | tee /tmp/6-2-1-test.log

Fix any failures and rerun. Tolerance: 3 attempts before escalation.

Acceptance: all three commands exit 0.

### Stage I: Mark roadmap item 6.2.1 as done

In `docs/roadmap.md`, change `- [ ] 6.2.1.` to `- [x] 6.2.1.`.

Acceptance: roadmap reflects shipped state.

## Concrete steps

Working directory: `/home/user/project`

Stage A:

    mkdir -p common/src/brain_type_metrics

Then create `common/src/brain_type_metrics/mod.rs`,
`common/src/brain_type_metrics/foreign_reach.rs` (placeholder), and
`common/src/brain_type_metrics/tests.rs` (placeholder). Verify:

    cargo check -p common

Stage B: Implement `ForeignReferenceSet` in `foreign_reach.rs`. Verify:

    cargo check -p common

Stage C: Add `TypeMetrics` and `TypeMetricsBuilder` to `mod.rs`. Verify:

    cargo check -p common

Stage D: Update `common/src/lib.rs`. Verify:

    cargo check -p common

Stage E: Populate `tests.rs`. Verify:

    cargo test -p common

Stage F: Create feature file and behaviour harness. Verify:

    cargo test -p common

Stage G: Update `docs/brain-trust-lints-design.md`. Verify markdown if
tooling available.

Stage H:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-2-1-check-fmt.log
    set -o pipefail; make lint      2>&1 | tee /tmp/6-2-1-lint.log
    set -o pipefail; make test      2>&1 | tee /tmp/6-2-1-test.log

Stage I: Update `docs/roadmap.md`.

## Validation and acceptance

The feature is complete only when all are true:

- `common/src/brain_type_metrics/mod.rs` exports `MethodMetrics`,
  `TypeMetrics`, `TypeMetricsBuilder`, `weighted_methods_count`, and
  `brain_methods`.
- `common/src/brain_type_metrics/foreign_reach.rs` exports
  `ForeignReferenceSet` and `foreign_reach_count`.
- Unit tests cover happy/unhappy/edge cases for all four metric areas.
- BDD scenarios use `rstest-bdd` v0.5.0 and cover the metric collection
  contract.
- `docs/brain-trust-lints-design.md` records 6.2.1 design decisions.
- `docs/roadmap.md` marks 6.2.1 as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

Quality method:

    make check-fmt && make lint && make test

Expected: all pass, test count increases by approximately 30–40 (unit + BDD).

## Idempotence and recovery

- All stages are additive and safe to rerun.
- If a test fails, fix the implementation or test, then rerun from the
  failing stage.
- If tolerance thresholds are exceeded, stop, document the issue in
  `Surprises & Discoveries`, and escalate.

## Artifacts and notes

No external artifacts. All code is contained within the `common/` crate.

## Interfaces and dependencies

No new external dependencies. The module depends only on:

- `std::collections::BTreeSet`
- Existing `common` types (`MethodMetrics` is self-contained; `TypeMetrics`
  stores values computed by existing helpers)

**Public API surface added:**

In `common/src/brain_type_metrics/mod.rs`:

    pub struct MethodMetrics { /* private fields */ }

    impl MethodMetrics {
        pub fn new(name: impl Into<String>, cognitive_complexity: usize,
                   lines_of_code: usize) -> Self;
        pub fn name(&self) -> &str;
        pub fn cognitive_complexity(&self) -> usize;
        pub fn lines_of_code(&self) -> usize;
        pub fn is_brain_method(&self, cc_threshold: usize,
                               loc_threshold: usize) -> bool;
    }

    pub fn weighted_methods_count(methods: &[MethodMetrics]) -> usize;
    pub fn brain_methods(methods: &[MethodMetrics], cc_threshold: usize,
                         loc_threshold: usize) -> Vec<&MethodMetrics>;

    pub struct TypeMetrics { /* private fields */ }

    // No public constructor — use TypeMetricsBuilder::build().
    impl TypeMetrics {
        pub fn type_name(&self) -> &str;
        pub fn wmc(&self) -> usize;
        pub fn brain_method_names(&self) -> &[String];
        pub fn brain_method_count(&self) -> usize;
        pub fn lcom4(&self) -> usize;
        pub fn foreign_reach(&self) -> usize;
        pub fn method_count(&self) -> usize;
    }

    pub struct TypeMetricsBuilder { /* private fields */ }

    impl TypeMetricsBuilder {
        pub fn new(type_name: impl Into<String>, cc_threshold: usize,
                   loc_threshold: usize) -> Self;
        pub fn add_method(&mut self, name: impl Into<String>,
                          cognitive_complexity: usize, lines_of_code: usize);
        pub fn set_lcom4(&mut self, lcom4: usize);
        pub fn set_foreign_reach(&mut self, count: usize);
        pub fn build(self) -> TypeMetrics;
    }

In `common/src/brain_type_metrics/foreign_reach.rs`:

    pub struct ForeignReferenceSet { /* private fields */ }

    impl ForeignReferenceSet {
        pub fn new() -> Self;
        pub fn record_reference(&mut self, path: &str,
                                is_from_expansion: bool);
        pub fn count(&self) -> usize;
        pub fn is_empty(&self) -> bool;
        pub fn references(&self) -> &BTreeSet<String>;
    }

    pub fn foreign_reach_count(
        references: impl IntoIterator<Item = (String, bool)>,
    ) -> usize;

**Downstream consumers** (future, not part of this task):

The lint driver for `brain_type` (6.2.2) will:

1. Walk all inherent `impl` blocks and trait `impl` blocks for each type.
2. For each method, compute CC (cognitive complexity) and LOC (line count).
3. Feed CC and LOC into `TypeMetricsBuilder::add_method()`.
4. Build `MethodInfo` via `MethodInfoBuilder` and pass to
   `cohesion_components()` for LCOM4.
5. Pass LCOM4 into `TypeMetricsBuilder::set_lcom4()`.
6. Walk method bodies for foreign type/module references, feeding them into
   `ForeignReferenceSet::record_reference()`.
7. Pass `ForeignReferenceSet::count()` into
   `TypeMetricsBuilder::set_foreign_reach()`.
8. Call `TypeMetricsBuilder::build()` to get `TypeMetrics`.
9. Evaluate thresholds against `TypeMetrics` and emit diagnostics.

## Files summary

**Files to create (5):**

| File | Purpose | Est. lines |
|------|---------|-----------|
| `common/src/brain_type_metrics/mod.rs` | `MethodMetrics`, `TypeMetrics`, `TypeMetricsBuilder`, WMC, brain method detection | ~250 |
| `common/src/brain_type_metrics/foreign_reach.rs` | `ForeignReferenceSet`, `foreign_reach_count` | ~120 |
| `common/src/brain_type_metrics/tests.rs` | rstest unit tests for all functions | ~300 |
| `common/tests/features/brain_type_metrics.feature` | Gherkin BDD scenarios | ~80 |
| `common/tests/brain_type_metrics_behaviour.rs` | BDD step definitions and scenario bindings | ~200 |

**Files to modify (3):**

| File | Change |
|------|--------|
| `common/src/lib.rs` | Add `pub mod brain_type_metrics;` and re-exports |
| `docs/brain-trust-lints-design.md` | Append §Implementation decisions (6.2.1) |
| `docs/roadmap.md` | Mark 6.2.1 as `[x]` |
