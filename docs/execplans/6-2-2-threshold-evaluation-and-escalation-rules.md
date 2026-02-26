# Implement threshold evaluation and escalation rules for brain_type (roadmap 6.2.2)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 6.2.2 delivers the threshold evaluation and escalation rules that
determine whether a type qualifies as a "brain type", plus diagnostic
formatting that surfaces measured values to the developer. After this change:

1. A pure evaluation function accepts a `TypeMetrics` value (produced by the
   metric collection layer from 6.2.1) and a threshold configuration, and
   returns a three-level disposition: pass, warn, or deny.
2. The warn rule fires when all warn conditions hold simultaneously (AND-based):
   Weighted Methods Count (WMC) >= 60 AND at least one brain method AND Lack of
   Cohesion in Methods (LCOM4) >= 2.
3. The deny rule fires when any single deny condition holds (OR-based):
   WMC >= 100 OR brain method count >= 2 OR LCOM4 >= 3. Deny supersedes warn.
4. Diagnostic formatting functions produce primary message, note, and help text
   that include the measured values (WMC, brain method names with CC and LOC,
   LCOM4, foreign reach), matching the format specified in the design document.
5. Unit tests (`rstest`) and behaviour-driven tests (`rstest-bdd` v0.5.0) cover
   happy paths, unhappy paths, boundary conditions, and diagnostic formatting.
6. The design document records implementation decisions for 6.2.2.
7. The roadmap marks 6.2.2 as done.
8. `make check-fmt`, `make lint`, and `make test` all pass.

Observable outcome: running `cargo test -p common` shows new tests passing for
evaluation logic and diagnostic formatting. The evaluation function correctly
classifies types into pass, warn, and deny dispositions based on the threshold
rules from the design document.

## Constraints

- All new code lives in `common/src/brain_type_metrics/` — no `rustc_private`
  dependencies. The `common` crate must remain free of compiler types.
- Every file must stay under 400 lines. The existing `mod.rs` is at 352 lines;
  new logic must go in separate submodules.
- No new external dependencies. Only `std` and existing `common` types.
- `serde` is not a dependency of `common`. Threshold configuration structs must
  not derive `Deserialize`. The lint driver crate (future work) will
  deserialize from TOML and convert into the common threshold type.
- Workspace Clippy `too_many_arguments` limit is 4. Use builder pattern for
  any struct with more than 4 construction parameters.
- Use `#[must_use]` on all pure functions and constructors.
- Comments and documentation use en-GB-oxendict spelling ("-ize"/"-ise"/"-our").
- Markdown wrapped at 80 columns; code blocks at 120.
- `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"` in
  non-test code.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros` at
  v0.5.0 for tests.
- On completion, update `docs/roadmap.md` entry 6.2.2 to `[x]`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 12 files or 1200 net
  lines, stop and escalate.
- Dependencies: if any new external dependency is needed, stop and escalate.
- Interface: if the existing `TypeMetrics` public API change
  (`brain_method_names()` return type) breaks more than 10 call sites, stop and
  escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts per gate, stop and escalate with logs.

## Risks

- Risk: changing `brain_method_names()` return type from `&[String]` to
  `Vec<&str>` breaks existing tests. Severity: low. Likelihood: medium.
  Mitigation: there are approximately 5 call sites, all in the same workspace.
  The `Vec<&str>` return type supports `PartialEq<&[&str]>` comparisons, so
  most test assertions should continue to work. Update any that do not.

- Risk: `evaluation.rs` exceeds 400 lines with types, evaluation, diagnostics,
  and formatting all in one file. Severity: medium. Likelihood: medium.
  Mitigation: split diagnostic formatting into a separate `diagnostic.rs`
  submodule if the file approaches 350 lines. Budget: ~120 lines for types and
  evaluation, ~120 lines for diagnostic struct and formatting, ~10 lines for
  module wiring.

- Risk: default threshold values from the design document may not match what
  the user expects for "escalate" (deny) thresholds, since the design doc says
  "LCOM4 >= 3" for escalation but does not give an explicit numeric threshold
  for all deny conditions. Severity: low. Likelihood: low. Mitigation: use
  conservative values derived from the design doc and the 6.2.1 exec plan
  context: `wmc_deny=100`, `lcom4_deny=3`, `brain_method_deny_count=2`. These
  are configurable and documented.

## Progress

- [x] Stage A: Write ExecPlan.
- [x] Stage B: Modify `TypeMetrics` to store full brain method metrics.
- [x] Stage C: Create `evaluation.rs` with threshold types and evaluation
  function.
- [x] Stage D: Add diagnostic detail struct and formatting functions.
- [x] Stage E: Wire new types into `common/src/lib.rs`.
- [x] Stage F: Add unit tests for evaluation and diagnostics.
- [x] Stage G: Add BDD feature file and behaviour harness.
- [x] Stage H: Record design decisions in `docs/brain-trust-lints-design.md`.
- [x] Stage I: Run quality gates.
- [x] Stage J: Mark roadmap item 6.2.2 as done.

## Surprises & discoveries

- The `brain_method_names()` return type change from `&[String]` to
  `Vec<&str>` required no test assertion changes. `Vec<&str>` implements
  `PartialEq<&[&str]>`, and existing assertions like
  `assert_eq!(metrics.brain_method_names(), &["parse", "transform"])` compiled
  and passed without modification.

- The BDD step for configuring a type with specific WMC, LCOM4, and brain
  method count required synthesizing `TypeMetrics` values. The builder API
  makes this straightforward: add placeholder methods with appropriate CC
  values to achieve the desired WMC, and adjust CC/LOC to control brain method
  count.

- Quality gate iteration: `make lint` failed with 3 Clippy errors in the
  BDD harness. (1) `clippy::too_many_arguments` on the Given step function that
  parsed 4 values from the feature text (5 args total with `world`). Fix: split
  into two Given steps — one for type name, WMC, and brain count, another for
  LCOM4. (2/3) `clippy::expect_used` on two functions using `.expect()` in the
  When/Then steps. Fix: added `#[expect(clippy::expect_used, reason = "...")]`
  following the pattern in `complexity_signal_behaviour.rs`.

## Decision log

- Decision: place evaluation in `common/src/brain_type_metrics/evaluation.rs`
  rather than in a lint crate. Rationale: the evaluation logic is pure (no
  `rustc_private`), independently testable, and reusable by `brain_trait`.
  Keeping it in `common` follows the same pattern as `cohesion_components()`
  and `weighted_methods_count()`. Date/Author: 2026-02-25 / DevBoxer.

- Decision: change `TypeMetrics` field from `brain_method_names: Vec<String>`
  to `brain_methods: Vec<MethodMetrics>`. Rationale: the design doc diagnostic
  format requires per-method CC and LOC (e.g.,
  `` `parse_all` (CC=31, LOC=140) ``). Storing full `MethodMetrics` enables
  this without requiring the diagnostic builder to receive the data separately.
  Date/Author: 2026-02-25 / DevBoxer.

- Decision: use `BrainTypeThresholdsBuilder` with consuming-self chainable
  setters. Rationale: the struct has 5 threshold fields, exceeding the
  workspace Clippy `too_many_arguments` limit of 4. A builder provides the
  construction path. This follows the `TypeMetricsBuilder` pattern from 6.2.1.
  Date/Author: 2026-02-25 / DevBoxer.

- Decision: threshold struct does not derive `Deserialize`.
  Rationale: `serde` is not a dependency of `common`. The lint driver crate
  will deserialize from TOML config and convert, following the
  `bumpy_road_function` Config → Settings pattern. Date/Author: 2026-02-25 /
  DevBoxer.

- Decision: warn is AND-based, deny is OR-based.
  Rationale: directly from the design document §`brain_type` rule set: "Warn
  when WMC >= 60 AND at least one brain method AND LCOM4 >= 2. Escalate when
  WMC >= 100 OR multiple brain methods OR cohesion extremely low." Deny
  supersedes warn. Date/Author: 2026-02-25 / DevBoxer.

- Decision: use "Deny" rather than "Escalate" for the enum variant.
  Rationale: Rust lint levels are Allow, Warn, Deny, Forbid. The design doc's
  "escalate" maps to a stricter lint level. Using `Deny` aligns with Rust
  ecosystem terminology. Date/Author: 2026-02-25 / DevBoxer.

## Outcomes & retrospective

### Deliverables

All 8 acceptance criteria met:

1. `evaluate_brain_type()` pure evaluation function with AND-based warn and
   OR-based deny.
2. `BrainTypeThresholds` with builder and design-doc defaults.
3. `BrainTypeDiagnostic` carrying measured values for rendering.
4. `format_primary_message()`, `format_note()`, `format_help()` formatting
   functions matching the design doc diagnostic format.
5. `TypeMetrics` extended with full brain method metrics.
6. Unit tests covering pass/warn/deny evaluation, custom thresholds, and
   diagnostic formatting.
7. BDD scenarios covering the threshold evaluation contract.
8. Design decisions recorded in `docs/brain-trust-lints-design.md`.

### Lessons

- The workspace `too_many_arguments` limit of 4 continues to be a consistent
  design pressure. All structs with >4 fields should use builders from the
  start.
- The `TypeMetricsBuilder` pattern translates cleanly to test fixture
  construction in BDD scenarios.
- BDD step functions parsing multiple values from the feature text are
  subject to the workspace `too_many_arguments` limit of 4, since the world
  reference counts as an argument. Steps with >3 parsed values must be split
  into separate Given/And lines.
- Quality gate iterations: 3 (formatting, Clippy `too_many_arguments`
  and `expect_used`, final pass). All gates pass.

## Context and orientation

### Repository state

The `common` crate (`common/src/lib.rs`) is a shared library for all Whitaker
lints. It exports modules including `brain_type_metrics`, `lcom4`, `i18n`,
`diagnostics`, and others. The crate has no `rustc_private` dependencies — all
types use plain Rust standard library types.

The brain type metric collection layer (6.2.1) lives in
`common/src/brain_type_metrics/` and provides:

- `common/src/brain_type_metrics/mod.rs` (352 lines): `MethodMetrics` (per-
  method CC and LOC), `TypeMetrics` (aggregate type-level metrics),
  `TypeMetricsBuilder` (incremental builder), `weighted_methods_count()`, and
  `brain_methods()`.
- `common/src/brain_type_metrics/foreign_reach.rs` (156 lines):
  `ForeignReferenceSet` and `foreign_reach_count()`.
- `common/src/brain_type_metrics/tests.rs` (289 lines): 32 rstest unit tests.
- `common/tests/brain_type_metrics_behaviour.rs` (282 lines): 10 BDD scenarios
  with step definitions.
- `common/tests/features/brain_type_metrics.feature` (77 lines): Gherkin
  feature file.

Key types in the existing API:

```rust
// common/src/brain_type_metrics/mod.rs

pub struct MethodMetrics {
    name: String,
    cognitive_complexity: usize,
    lines_of_code: usize,
}

pub struct TypeMetrics {
    type_name: String,
    wmc: usize,
    brain_method_names: Vec<String>,  // will change to Vec<MethodMetrics>
    lcom4: usize,
    foreign_reach: usize,
    method_count: usize,
}
```

`TypeMetrics` currently stores only brain method names (`Vec<String>`). The
design document's diagnostic format requires per-method CC and LOC values
(e.g., `` `parse_all` (CC=31, LOC=140) ``), so this field must change to
`Vec<MethodMetrics>`.

### Design specification

From `docs/brain-trust-lints-design.md` §`brain_type` rule set (lines 91–96):

- **Warn** when WMC >= 60 AND at least one brain method is present AND
  LCOM4 >= 2 (or TCC <= 0.33).
- **Escalate** (configurable) when WMC >= 100 OR multiple brain methods exist
  OR cohesion is extremely low.

From §Diagnostic output (lines 234–242):

```plaintext
brain_type: `Foo` has WMC=118, LCOM4=3, and a brain method `parse_all`
(CC=31, LOC=140).
```

### Analogous patterns

The closest analogue is `conditional_max_n_branches` in
`crates/conditional_max_n_branches/src/driver.rs`, which uses:

- A `ConditionDisposition` enum with `WithinLimit` and `ExceedsLimit` variants.
- A pure `evaluate_condition(branches, limit)` function.
- rstest unit tests with `#[case]` parameterization.
- BDD tests with a `PredicateWorld` struct using `Cell` fields.

## Plan of work

### Stage A: Write ExecPlan

Write this document to
`docs/execplans/6-2-2-threshold-evaluation-and-escalation-rules.md`.

### Stage B: Modify `TypeMetrics` to store full brain method metrics

In `common/src/brain_type_metrics/mod.rs`:

1. Change field `brain_method_names: Vec<String>` to
   `brain_methods: Vec<MethodMetrics>`.
2. Update `brain_method_names()` to return `Vec<&str>`.
3. Add `brain_methods() -> &[MethodMetrics]` accessor.
4. Update `brain_method_count()` to use `self.brain_methods.len()`.
5. Update `TypeMetricsBuilder::build()` to store cloned `MethodMetrics`.
6. Update tests if needed.

Acceptance: `cargo check -p common && cargo test -p common` succeed.

### Stage C: Create evaluation types and evaluation function

Create `common/src/brain_type_metrics/evaluation.rs` with:

- `BrainTypeDisposition` enum (Pass, Warn, Deny).
- `BrainTypeThresholds` struct (5 private fields).
- `BrainTypeThresholdsBuilder` with consuming-self setters and defaults.
- `evaluate_brain_type(metrics, thresholds) -> BrainTypeDisposition`.

Wire into `mod.rs`: add `pub mod evaluation;`.

Acceptance: `cargo check -p common` succeeds.

### Stage D: Add diagnostic detail struct and formatting functions

In `evaluation.rs`, add:

- `BrainTypeDiagnostic` struct.
- Constructor: `new(metrics, disposition)`.
- `format_primary_message()`, `format_note()`, `format_help()`.

Acceptance: `cargo check -p common` succeeds.

### Stage E: Wire re-exports into `common/src/lib.rs`

Add re-exports for all new public types.

Acceptance: `cargo check -p common` succeeds.

### Stage F: Unit tests

Create `evaluation_tests.rs` with rstest tests covering defaults, builder,
pass/warn/deny evaluation, custom thresholds, and diagnostic formatting.

Acceptance: `cargo test -p common` passes.

### Stage G: BDD tests

Create feature file and behaviour harness with 9 scenarios covering the
threshold evaluation contract.

Acceptance: `cargo test -p common` passes.

### Stage H: Design document update

Append §Implementation decisions (6.2.2) to `docs/brain-trust-lints-design.md`.

### Stage I: Quality gates

Run `make check-fmt`, `make lint`, `make test`.

### Stage J: Mark roadmap 6.2.2 as done

## Concrete steps

Working directory: `/home/user/project`

Stage B:

```sh
cargo check -p common && cargo test -p common
```

Stage C–E:

```sh
cargo check -p common
```

Stage F–G:

```sh
cargo test -p common
```

Stage I:

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-2-2-check-fmt.log
set -o pipefail; make lint      2>&1 | tee /tmp/6-2-2-lint.log
set -o pipefail; make test      2>&1 | tee /tmp/6-2-2-test.log
```

## Validation and acceptance

The feature is complete only when all of the following are true:

- `evaluation.rs` exports `BrainTypeDisposition`, `BrainTypeThresholds`,
  `BrainTypeThresholdsBuilder`, `evaluate_brain_type()`, `BrainTypeDiagnostic`,
  `format_primary_message()`, `format_note()`, and `format_help()`.
- `TypeMetrics` provides both `brain_method_names()` and `brain_methods()`.
- Unit tests cover pass, warn, and deny evaluation paths, boundary conditions,
  custom thresholds, and diagnostic formatting.
- BDD scenarios cover the threshold evaluation contract.
- Design decisions recorded. Roadmap updated. All quality gates pass.

Quality method:

```sh
make check-fmt && make lint && make test
```

## Idempotence and recovery

All stages are additive and safe to rerun. If a test fails, fix the
implementation or test, then rerun from the failing stage.

## Artifacts and notes

No external artifacts. All code is contained within the `common/` crate.

## Interfaces and dependencies

No new external dependencies. The module depends only on:

- `std::fmt::Write` (for string formatting).
- Existing `common::brain_type_metrics` types.

**Public API surface added:**

In `common/src/brain_type_metrics/evaluation.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrainTypeDisposition {
    Pass,
    Warn,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrainTypeThresholds { /* private fields */ }

impl BrainTypeThresholds {
    pub fn wmc_warn(&self) -> usize;
    pub fn wmc_deny(&self) -> usize;
    pub fn lcom4_warn(&self) -> usize;
    pub fn lcom4_deny(&self) -> usize;
    pub fn brain_method_deny_count(&self) -> usize;
}

#[derive(Clone, Copy, Debug)]
pub struct BrainTypeThresholdsBuilder { /* private fields */ }

impl BrainTypeThresholdsBuilder {
    pub fn new() -> Self;
    pub fn wmc_warn(self, value: usize) -> Self;
    pub fn wmc_deny(self, value: usize) -> Self;
    pub fn lcom4_warn(self, value: usize) -> Self;
    pub fn lcom4_deny(self, value: usize) -> Self;
    pub fn brain_method_deny_count(self, value: usize) -> Self;
    pub fn build(self) -> BrainTypeThresholds;
}

impl Default for BrainTypeThresholdsBuilder { /* delegates to new() */ }

#[must_use]
pub fn evaluate_brain_type(
    metrics: &TypeMetrics,
    thresholds: &BrainTypeThresholds,
) -> BrainTypeDisposition;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainTypeDiagnostic { /* private fields */ }

impl BrainTypeDiagnostic {
    pub fn new(
        metrics: &TypeMetrics,
        disposition: BrainTypeDisposition,
    ) -> Self;
    pub fn type_name(&self) -> &str;
    pub fn disposition(&self) -> BrainTypeDisposition;
    pub fn wmc(&self) -> usize;
    pub fn lcom4(&self) -> usize;
    pub fn foreign_reach(&self) -> usize;
    pub fn brain_methods(&self) -> &[MethodMetrics];
}

#[must_use]
pub fn format_primary_message(diagnostic: &BrainTypeDiagnostic) -> String;

#[must_use]
pub fn format_note(diagnostic: &BrainTypeDiagnostic) -> String;

#[must_use]
pub fn format_help(diagnostic: &BrainTypeDiagnostic) -> String;
```

**Modified in `TypeMetrics`:**

```rust
// New accessor
pub fn brain_methods(&self) -> &[MethodMetrics];

// Changed return type (was &[String], now Vec<&str>)
pub fn brain_method_names(&self) -> Vec<&str>;
```
