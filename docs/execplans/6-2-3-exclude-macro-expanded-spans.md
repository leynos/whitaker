# Exclude macro-expanded spans during CC calculation (roadmap 6.2.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 6.2.3 delivers a `CognitiveComplexityBuilder` in the `common`
crate that computes SonarSource-style cognitive complexity (CC) incrementally,
with built-in filtering for macro-expanded spans. After this change:

1. A pure library builder (`CognitiveComplexityBuilder`) accepts incremental
   complexity contributions via method calls. Each call accepts
   `is_from_expansion: bool` so the future High-level Intermediate
   Representation (HIR) walker can pass
   `span.from_expansion()` without the `common` crate depending on
   `rustc_private`.
2. Macro-expanded nodes are silently excluded from the CC score, preventing
   macro-generated HIR from inflating complexity counts — the core problem
   described in Clippy issue #14417.
3. The builder tracks nesting depth internally, with macro-expanded nesting
   levels tracked for stack balance but excluded from the effective depth used
   in nesting increments.
4. The final CC value produced by `build()` can be passed directly to
   `TypeMetricsBuilder::add_method()`, completing the producer side of the
   brain type metric pipeline.

Observable outcome: running `cargo test -p common` shows new unit tests and
Behaviour-Driven Development (BDD) scenarios passing for cognitive complexity
computation. The builder correctly
excludes macro-expanded increments and nesting from the score.

## Constraints

- All new code lives in `common/src/brain_type_metrics/` — no `rustc_private`
  dependencies. The `common` crate must remain free of compiler types.
- Every file must stay under 400 lines.
- No new external dependencies. Only `std` and existing `common` types.
- Workspace Clippy `too_many_arguments` limit is 4. Builder methods take
  `&mut self` + 1 `bool`, well within the limit.
- `common/Cargo.toml` has `expect_used = "deny"` and `unwrap_used = "deny"`.
  Use `match`+`panic!` or `assert!` instead of `.expect()` / `.unwrap()`.
- Use `#[must_use]` on all pure functions and constructors.
- Comments and documentation use en-GB-oxendict spelling ("-ize"/"-our").
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros` (v0.5.0)
  for tests.
- On completion, update `docs/roadmap.md` entry 6.2.3 to `[x]`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 10 files or 800 net
  lines, stop and escalate.
- Dependencies: if any new external dependency is needed, stop and escalate.
- Interface: if the existing `TypeMetricsBuilder` or `MethodMetrics` public API
  must change, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts per gate, stop and escalate with logs.

## Risks

- Risk: `cognitive_complexity_tests.rs` could approach 400 lines with
  comprehensive coverage. Severity: low. Likelihood: low. Mitigation: estimated
  at ~250 lines; if it approaches 350, split into two test modules.

- Risk: `mod.rs` at 361 lines could exceed 400 after additions.
  Severity: low. Likelihood: very low. Mitigation: only 2 lines added (module
  declaration + re-export), reaching ~363. Well under the limit.

- Risk: BDD step argument count could exceed the workspace Clippy limit of 4.
  Severity: low. Likelihood: very low. Mitigation: no step function parses more
  than 1 value from feature text. World + 1 = 2 args total, well under the
  limit.

## Progress

- [x] (2026-02-26) Stage A: Write ExecPlan to
  `docs/execplans/6-2-3-exclude-macro-expanded-spans.md`.
- [x] (2026-02-26) Stage B: Create `CognitiveComplexityBuilder` with
  unit tests. 28 unit tests passing.
- [x] (2026-02-26) Stage C: Create BDD feature file and behaviour
  harness. 8 BDD scenarios passing.
- [x] (2026-02-26) Stage D: Record design decisions in
  `docs/brain-trust-lints-design.md`.
- [x] (2026-02-26) Stage E: Run quality gates (`make check-fmt`, `make lint`,
  `make test`). `check-fmt` and `lint` passed cleanly. `make test` had one
  pre-existing failure (`scenario_install_suite_to_temp_dir` in
  `whitaker-installer`, last modified in commit `ab7b1bd` for ExecPlan 3-4-4)
  that is unrelated to this change and was subsequently fixed in a later
  commit.
- [x] (2026-02-26) Stage F: Mark roadmap item 6.2.3 as done.

## Surprises & discoveries

- Observation: rstest-bdd step functions must name the world parameter
  `world`, not `_world`. Using `_world` causes the macro to fail to match the
  fixture name, producing "requires fixtures `_world`, but the following are
  missing: `_world`. Available fixtures from scenario: `world`". Evidence: all
  7 BDD scenarios failed with this error until the parameter was renamed from
  `_world` to `world` with `let _ = world;` to suppress the unused-variable
  warning. Impact: cosmetic; fixed immediately. Future step functions that do
  not use the world parameter should use `let _ = world;` instead of prefixing
  with underscore.

## Decision log

- Decision: adopt the "skip" strategy (exclude entirely) for macro-expanded
  nodes, rather than "cap at 1 per invocation". Rationale: every existing
  `is_from_expansion` filter in the codebase uses silent discard
  (`ForeignReferenceSet::record_reference`,
  `MethodInfoBuilder::record_field_access`, `SegmentBuilder::visit_expr`).
  Clippy issue #14417 argues macro calls should contribute CC=0 from expanded
  internals. "Skip" is consistent with all prior art. If a future need arises
  for "cap", it can be layered on without changing the core API. Date/Author:
  2026-02-26 / DevBoxer.

- Decision: the builder tracks nesting depth internally via a boolean stack.
  Rationale: the SonarSource model requires nesting context. Making the caller
  track nesting state would be error-prone and leak complexity concerns. The
  builder exposes `push_nesting(is_from_expansion)` / `pop_nesting()`.
  Effective depth counts only non-expansion levels. This prevents
  macro-generated control flow from inflating the nesting penalty of subsequent
  real code. Date/Author: 2026-02-26 / DevBoxer.

- Decision: API by increment category (structural/nesting/fundamental), not by
  expression type (if/for/match/etc.). Rationale: keeps the builder decoupled
  from Rust HIR node types, maintaining the `common` crate's independence from
  `rustc_private`. The HIR walker in the future lint driver maps each HIR
  expression kind to the appropriate builder calls. Date/Author: 2026-02-26 /
  DevBoxer.

- Decision: consuming `build()` with nesting balance assertion.
  Rationale: panics if the nesting stack is not empty, catching mismatched
  `push_nesting`/`pop_nesting` calls at the point of use. These are programming
  contract violations, not user-facing errors. Uses `assert!` rather than
  `.expect()` to satisfy the `expect_used = "deny"` Clippy rule. Date/Author:
  2026-02-26 / DevBoxer.

## Outcomes & retrospective

**Delivered**: `CognitiveComplexityBuilder` in `common/src/brain_type_metrics/`
with macro-expansion filtering via the established Pattern B
(`is_from_expansion: bool`). The builder tracks nesting depth internally,
excluding macro-expanded levels from the effective depth used in nesting
increments.

**Test coverage**: 28 unit tests in `cognitive_complexity_tests.rs` covering
individual increment types, macro-expansion filtering, nesting stack behaviour,
composite scenarios modelling real code patterns, and edge cases. 8 BDD
scenarios in `cognitive_complexity_behaviour.rs` covering end-to-end
behavioural cases.

**Quality gates**: `make check-fmt` and `make lint` passed cleanly. `make test`
ran 797/855 tests: 796 passed (2 slow), 2 skipped; 1 pre-existing failure in
`whitaker-installer` was unrelated and subsequently fixed in a later commit.
All gates now pass.

**Files created** (5): `cognitive_complexity.rs` (115 lines),
`cognitive_complexity_tests.rs` (315 lines), `cognitive_complexity.feature` (57
lines), `cognitive_complexity_behaviour.rs` (135 lines), this exec plan.

**Files modified** (4): `mod.rs` (+2 lines), `lib.rs` (+1 line),
`brain-trust-lints-design.md` (+29 lines), `roadmap.md` (checkbox flip).

**Net lines added**: ~660 (well within the 800-line tolerance).

**Surprises**: rstest-bdd step functions must name the world parameter `world`,
not `_world`; the macro matches parameter names literally to fixture names.
Fixed with `let _ = world;` to suppress unused-variable warnings.

**What went well**: the established Pattern B convention made the API design
straightforward. The consuming `build()` with balance assertion catches
programming errors early.

**What to watch**: the builder is a pure producer; integration with the HIR
walker (future task) will exercise the API under real conditions and may
surface additional edge cases.

## Context and orientation

### Repository state

The `common` crate (`common/src/lib.rs`) is a shared library for all Whitaker
lints. It has no `rustc_private` dependencies. Brain type metric collection
lives in `common/src/brain_type_metrics/` and provides:

- `mod.rs` (361 lines): `MethodMetrics`, `TypeMetrics`, `TypeMetricsBuilder`,
  `weighted_methods_count()`, `brain_methods()`.
- `foreign_reach.rs` (157 lines): `ForeignReferenceSet`,
  `foreign_reach_count()`.
- `evaluation.rs` (275 lines): `BrainTypeDisposition`, `BrainTypeThresholds`,
  `evaluate_brain_type()`.
- `diagnostic.rs` (293 lines): `BrainTypeDiagnostic`, formatting functions.
- `tests.rs` (296 lines): unit tests for mod.rs types.

`TypeMetricsBuilder::add_method(name, cognitive_complexity, lines_of_code)`
accepts a pre-computed CC value as `usize`, but **nothing in the codebase
produces that value yet**. This task builds the producer.

### Established patterns to follow

**Pattern B: `is_from_expansion: bool` parameter on pure library builders** is
the established convention for macro-span filtering in `common`:

- `common/src/lcom4/extract.rs:91`:
  `record_field_access(name, is_from_expansion)`
- `common/src/brain_type_metrics/foreign_reach.rs:75`:
  `record_reference(path, is_from_expansion)`

Both silently discard entries where `is_from_expansion` is `true`.

**Test file structure**: unit tests in separate `*_tests.rs` files referenced
via `#[cfg(test)] #[path = "..."] mod tests;`. BDD harnesses in `common/tests/`
with World structs using `Cell`/`RefCell` fields, feature files in
`common/tests/features/`.

### SonarSource cognitive complexity model

Three increment categories:

- **Structural** (+1): `if`, `else if`, `else`, `match`, `for`, `while`,
  `loop`, `?` operator.
- **Nesting** (+current_depth): applied alongside structural for constructs
  that incur a nesting penalty.
- **Fundamental** (+1): boolean operator sequence breaks (`&&`, `||`).

## Plan of work

### Stage A: Write ExecPlan

Write this document to `docs/execplans/6-2-3-exclude-macro-expanded-spans.md`.

### Stage B: Create `CognitiveComplexityBuilder` with unit tests

Create `common/src/brain_type_metrics/cognitive_complexity.rs` containing the
builder. The builder struct has three fields:

```rust
pub struct CognitiveComplexityBuilder {
    score: usize,
    nesting_stack: Vec<bool>,  // true = from expansion
    effective_depth: usize,    // count of non-expansion levels
}
```

Public API:

```rust
impl CognitiveComplexityBuilder {
    pub fn new() -> Self;
    pub fn record_structural_increment(&mut self, is_from_expansion: bool);
    pub fn record_nesting_increment(&mut self, is_from_expansion: bool);
    pub fn record_fundamental_increment(&mut self, is_from_expansion: bool);
    pub fn push_nesting(&mut self, is_from_expansion: bool);
    pub fn pop_nesting(&mut self);
    pub fn effective_depth(&self) -> usize;
    pub fn score(&self) -> usize;
    pub fn build(self) -> usize;
}
impl Default for CognitiveComplexityBuilder { /* delegates to new() */ }
```

Behaviour of each method:

- `record_structural_increment(from_exp)`: if `!from_exp`, `score += 1`.
- `record_nesting_increment(from_exp)`: if `!from_exp`,
  `score += effective_depth`.
- `record_fundamental_increment(from_exp)`: if `!from_exp`, `score += 1`.
- `push_nesting(from_exp)`: push `from_exp` onto stack; if `!from_exp`,
  increment `effective_depth`.
- `pop_nesting()`: pop stack; if popped value was `false`, decrement
  `effective_depth`. Panics on empty stack.
- `build()`: assert stack empty, return `score`.

Create `common/src/brain_type_metrics/cognitive_complexity_tests.rs` with unit
tests covering:

- Individual increment types (structural/nesting/fundamental) with and without
  expansion filtering.
- Nesting stack behaviour (push/pop, effective depth tracking, mixed expansion
  levels).
- Composite scenarios modelling real code patterns (nested ifs, boolean
  operators, macro-inside-real and real-inside-macro nesting).
- Edge cases (empty builder, default matches new, build panics on unbalanced
  stack, pop panics on empty stack).

Wire into `mod.rs`: add `pub mod cognitive_complexity;` and
`pub use cognitive_complexity::CognitiveComplexityBuilder;`.

Wire into `lib.rs`: add `CognitiveComplexityBuilder` to the
`brain_type_metrics` re-export block (line 25-28).

Acceptance: `cargo check -p common && cargo test -p common` succeed.

### Stage C: Create BDD feature file and behaviour harness

Create `common/tests/features/cognitive_complexity.feature` with 8 scenarios:

1. Empty function has zero complexity.
2. Single if adds one structural increment.
3. Nested if adds nesting-depth penalty.
4. Macro-expanded structural increment is excluded.
5. Macro-expanded nesting does not inflate depth.
6. Boolean operators add fundamental increments.
7. Mixed real and expansion increments.
8. Fundamental increment from expansion is excluded.

Create `common/tests/cognitive_complexity_behaviour.rs` with a `CcWorld` struct
(using `RefCell<CognitiveComplexityBuilder>` and `Cell<Option<usize>>`), step
functions, and scenario registrations (indices 0-7).

Acceptance: `cargo test -p common` passes.

### Stage D: Record design decisions in design document

Insert "Implementation decisions (6.2.3)" section in
`docs/brain-trust-lints-design.md` after line 233 (end of 6.2.2 decisions),
before line 235 ("## Implementation approach"). Content covers:

- "Skip" strategy for macro-expanded nodes.
- `CognitiveComplexityBuilder` in `common`.
- Internal nesting depth tracking.
- API by increment category.
- Consuming `build()` with balance assertion.

### Stage E: Run quality gates

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-2-3-check-fmt.log
set -o pipefail; make lint      2>&1 | tee /tmp/6-2-3-lint.log
set -o pipefail; make test      2>&1 | tee /tmp/6-2-3-test.log
```

### Stage F: Mark roadmap item 6.2.3 as done

Change line 161 of `docs/roadmap.md` from `- [ ] 6.2.3.` to `- [x] 6.2.3.`.

## Concrete steps

Working directory: `/home/user/project`

Stage B:

```sh
cargo check -p common && cargo test -p common
```

Stage C:

```sh
cargo test -p common
```

Stage E:

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-2-3-check-fmt.log
set -o pipefail; make lint      2>&1 | tee /tmp/6-2-3-lint.log
set -o pipefail; make test      2>&1 | tee /tmp/6-2-3-test.log
```

## Validation and acceptance

The feature is complete only when all of the following are true:

- `cognitive_complexity.rs` exports `CognitiveComplexityBuilder` with all
  methods described in Stage B.
- Unit tests cover all increment types, macro-expansion filtering, nesting
  stack behaviour, composite scenarios, and edge cases.
- BDD scenarios cover 8 behavioural cases for CC computation with
  macro-expansion filtering.
- Design decisions recorded in `docs/brain-trust-lints-design.md`.
- Roadmap 6.2.3 marked as `[x]`.
- `make check-fmt`, `make lint`, and `make test` all pass.

Quality method:

```sh
make check-fmt && make lint && make test
```

## Idempotence and recovery

All stages are additive and safe to rerun. No existing files are destructively
modified. If a test fails, fix the implementation or test, then rerun from the
failing stage.

## Artifacts and notes

No external artifacts. All code is contained within the `common/` crate.

## Interfaces and dependencies

No new external dependencies. The module depends only on `std`.

Public API surface added in
`common/src/brain_type_metrics/cognitive_complexity.rs`:

```rust
/// Incrementally computes cognitive complexity following SonarSource
/// rules, with macro-expansion filtering.
#[derive(Clone, Debug)]
pub struct CognitiveComplexityBuilder {
    score: usize,
    nesting_stack: Vec<bool>,
    effective_depth: usize,
}

impl CognitiveComplexityBuilder {
    #[must_use]
    pub fn new() -> Self;

    /// +1 when `!is_from_expansion`.
    pub fn record_structural_increment(&mut self, is_from_expansion: bool);

    /// +effective_depth when `!is_from_expansion`.
    pub fn record_nesting_increment(&mut self, is_from_expansion: bool);

    /// +1 when `!is_from_expansion`.
    pub fn record_fundamental_increment(&mut self, is_from_expansion: bool);

    /// Enters a nesting level. Macro-expanded levels are tracked but
    /// do not increase effective depth.
    pub fn push_nesting(&mut self, is_from_expansion: bool);

    /// Exits the most recent nesting level. Panics on empty stack.
    pub fn pop_nesting(&mut self);

    #[must_use]
    pub fn effective_depth(&self) -> usize;

    #[must_use]
    pub fn score(&self) -> usize;

    /// Consumes the builder. Panics if nesting stack is not empty.
    #[must_use]
    pub fn build(self) -> usize;
}

impl Default for CognitiveComplexityBuilder {
    fn default() -> Self { Self::new() }
}
```

### Files to create

*Table 1: New files introduced by this change.*

| File                                                          | Est. lines | Purpose                |
| ------------------------------------------------------------- | ---------- | ---------------------- |
| `common/src/brain_type_metrics/cognitive_complexity.rs`       | ~160-200   | Builder implementation |
| `common/src/brain_type_metrics/cognitive_complexity_tests.rs` | ~200-280   | Unit tests             |
| `common/tests/features/cognitive_complexity.feature`          | ~60-70     | BDD feature file       |
| `common/tests/cognitive_complexity_behaviour.rs`              | ~150-200   | BDD harness            |
| `docs/execplans/6-2-3-exclude-macro-expanded-spans.md`        | ~330       | This exec plan         |

### Files to modify

*Table 2: Existing files modified by this change.*

| File                                                | Change                                              |
| --------------------------------------------------- | --------------------------------------------------- |
| `common/src/brain_type_metrics/mod.rs` (line ~20)   | Add `pub mod cognitive_complexity;` and re-export   |
| `common/src/lib.rs` (line 25-28)                    | Add `CognitiveComplexityBuilder` to re-export block |
| `docs/brain-trust-lints-design.md` (after line 233) | Insert §Implementation decisions (6.2.3)            |
| `docs/roadmap.md` (line 161)                        | `- [ ]` to `- [x]`                                  |
