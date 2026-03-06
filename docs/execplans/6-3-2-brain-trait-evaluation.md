# Apply warning and escalation thresholds and surface measured values in diagnostics for the brain trait lint (roadmap 6.3.2)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

This plan must also be written to
`docs/execplans/6-3-2-brain-trait-evaluation.md` as the first implementation
step.

## Purpose / big picture

Roadmap item 6.3.2 delivers the threshold evaluation and diagnostic formatting
layer for `brain_trait`. This is the trait analogue of roadmap 6.2.2
(`brain_type` evaluation), building on the metric collection delivered by 6.3.1.

After this change, given a `TraitMetrics` struct (produced by 6.3.1), a caller
can:

1. Evaluate whether the trait passes, warns, or is denied based on configurable
   thresholds.
2. Produce human-readable diagnostic messages that surface measured values
   inline.
3. Receive actionable decomposition guidance tailored to the specific signals.

Observable outcome:

1. `common` exports new evaluation and diagnostic types:
   `BrainTraitDisposition`, `BrainTraitThresholds`,
   `BrainTraitThresholdsBuilder`, `evaluate_brain_trait`,
   `BrainTraitDiagnostic`, `format_primary_message`, `format_note`,
   `format_help`.
2. Unit tests validate threshold boundary conditions for pass, warn, and deny.
3. Unit tests validate diagnostic message content and formatting.
4. Behaviour tests using `rstest-bdd` v0.5.0 validate end-to-end evaluation
   and diagnostic contracts.
5. `docs/brain-trust-lints-design.md` records implementation decisions for
   6.3.2.
6. `docs/roadmap.md` marks 6.3.2 done only after implementation and all quality
   gates pass.
7. `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Scope only roadmap item 6.3.2. Do not implement lint driver high-level
  intermediate representation (HIR) walking, configuration loading, or Static
  Analysis Results Interchange Format (SARIF) output.
- Keep `common` free of `rustc_private` dependencies. Accept plain Rust values
  only.
- Follow established module layout and line-count constraints (< 400 lines per
  source file). Split files if needed.
- Use builder patterns for `BrainTraitThresholds` for consistency with
  `BrainTypeThresholds`, even though the struct has only 3 fields.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`).
- Preserve deterministic behaviour (stable ordering for rendered metric lists).
- Use en-GB-oxendict spelling in comments and docs.
- Clippy `too_many_arguments` threshold is 4. Behaviour-driven development
  (BDD) step functions can parse at most 3 values from feature text (world + 3
  = 4 args max).
- Do not modify existing `brain_type_metrics` public APIs.
- Update design documentation with any final decisions made during
  implementation.
- Completion must include roadmap checkbox update for 6.3.2.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 12 touched files or 1200 net lines of
  code (LOC), stop and escalate.
- Interface: if implementing 6.3.2 requires changing existing
  `brain_type_metrics` or `brain_trait_metrics` public APIs, stop and escalate.
- Dependencies: if any new external dependency is required, stop and escalate.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail after
  3 targeted fix iterations, stop and escalate with captured logs.

## Risks

- Clippy risk in tests: `rstest` and `rstest-bdd` parameter counts can trigger
  `too_many_arguments`. Mitigation: use tuple-style `#[case]` inputs and keep
  BDD step placeholders to at most 3 parsed values (+ `world`). Severity: low.
  Likelihood: medium.
- File length risk: `evaluation.rs` or `diagnostic.rs` could approach 400 lines
  with full rustdoc examples. Mitigation: test files are in separate
  `evaluation_tests.rs` and `diagnostic_tests.rs` files via
  `#[path = "..."] mod tests;`. Severity: low. Likelihood: low.
- Method count ambiguity: design doc says "methods" which could be interpreted
  as `total_item_count` (which includes associated types/consts). Mitigation:
  explicitly compute `required_method_count() + default_method_count()` and
  record the decision. Severity: medium. Likelihood: low.
- Naming collision: `format_help`, `format_note`, `format_primary_message` in
  `brain_trait_metrics` would collide with same names already re-exported from
  `brain_type_metrics` at the `common::` root level. Mitigation: do NOT
  re-export the brain_trait formatting functions from `lib.rs`; callers use the
  module path `brain_trait_metrics::evaluation::format_help`. Severity: low.
  Likelihood: certain (by design).

## Progress

- [x] Stage A: Write this ExecPlan to
      `docs/execplans/6-3-2-brain-trait-evaluation.md`.
- [x] Stage B: Implement `evaluation.rs` (disposition, thresholds, evaluate
      function).
- [x] Stage C: Implement `diagnostic.rs` (diagnostic struct, format functions).
- [x] Stage D: Update `mod.rs` with module declarations and re-exports.
- [x] Stage E: Implement unit tests (`evaluation_tests.rs`,
      `diagnostic_tests.rs`).
- [x] Stage F: Implement BDD tests (feature file + behaviour test file).
- [x] Stage G: Update `lib.rs` re-exports.
- [x] Stage H: Record implementation decisions in design doc.
- [x] Stage I: Mark roadmap 6.3.2 done.
- [x] Stage J: Run `make check-fmt`, `make lint`, and `make test` successfully.
- [x] Stage K: Finalize living sections.

## Surprises & discoveries

1. **`too_many_arguments` in diagnostic test helper.** The `build_diagnostic`
   helper function in `diagnostic_tests.rs` initially accepted 5 parameters
   (name, required, default, cc_per_default, disposition), exceeding the
   workspace clippy threshold of 4. Resolved by introducing a
   `DiagnosticInput<'a>` parameter struct. This pattern was not needed in the
   `brain_type` reference implementation because its helper had fewer arguments.

2. **`excessive_nesting` in BDD CC distribution logic.** The
   `build_metrics()` method on `EvaluationWorld` contained a nested `if/else`
   inside a `for` loop inside an `if` block when distributing CC values across
   default methods. Resolved by extracting `add_distributed_defaults()` as a
   free function that uses an inline conditional expression instead of nested
   blocks. The `brain_type` BDD tests did not encounter this because they do
   not distribute CC across methods.

3. **Formatting differences from `brain_type` reference.** `cargo fmt`
   reformatted several import groups, builder chains, and string literals
   differently from what was written by hand. All formatting was resolved by a
   single `cargo fmt --all` pass before the lint gate.

## Decision log

1. **"Methods" means methods, not items.** Total method count for threshold
   evaluation is `required_method_count() + default_method_count()`, excluding
   associated types and consts. This matches the natural reading of "methods"
   in the design doc and is validated by the "associated items excluded" BDD
   scenario and a dedicated unit test.

2. **`format_*` functions not re-exported from `lib.rs`.** The
   `format_primary_message`, `format_note`, and `format_help` functions in
   `brain_trait_metrics` would collide with identically named functions already
   re-exported from `brain_type_metrics` at the `common::` root level. Callers
   must use the fully qualified path
   `brain_trait_metrics::evaluation::format_help` (or the module-level
   re-export `brain_trait_metrics::format_help`).

3. **`DiagnosticInput` parameter struct in tests.** Introduced to stay within
   the workspace `too_many_arguments` threshold of 4. All `build_diagnostic`
   call sites use struct initialization syntax for readability.

4. **`add_distributed_defaults` free function in BDD tests.** Extracted from
   the `EvaluationWorld::build_metrics` method to avoid `excessive_nesting`.
   Takes `(builder, count, cc_sum)` and distributes CC evenly with remainder on
   the last method.

5. **Builder pattern for `BrainTraitThresholds`.** Used for consistency with
   `BrainTypeThresholds` despite the struct having only 3 fields.

## Context and orientation

### Repository structure

The project is a Rust workspace. The `common` crate at `common/` provides
compiler-independent utilities shared across lint drivers. Evaluation and
diagnostic logic lives in `common`, not in individual lint crates.

### What 6.3.1 delivered

The module at `common/src/brain_trait_metrics/` contains:

- `mod.rs` (25 lines) -- module declarations and re-exports.
- `item.rs` (277 lines) -- `TraitItemKind`, `TraitItemMetrics`, and helper
  functions (`trait_item_count`, `required_method_count`,
  `default_method_count`, `default_method_cc_sum`).
- `metrics.rs` (328 lines) -- `TraitMetrics` struct with accessors and
  `TraitMetricsBuilder`.
- `tests.rs` (225 lines) -- unit tests.

Key `TraitMetrics` accessors:

- `trait_name() -> &str`
- `total_item_count() -> usize` (ALL items: methods + types + consts)
- `required_method_count() -> usize`
- `default_method_count() -> usize`
- `default_method_cc_sum() -> usize`
- `implementor_burden() -> usize` (derived: equals `required_method_count`)

Note: there is no `total_method_count()` accessor. The evaluation function must
compute this as `required_method_count() + default_method_count()`.

### Reference implementation: brain_type evaluation (6.2.2)

The canonical pattern lives at `common/src/brain_type_metrics/`:

- `evaluation.rs` (275 lines) -- `BrainTypeDisposition` enum,
  `BrainTypeThresholds` struct + builder with const defaults, private
  `is_deny_triggered()` (OR-based) and `is_warn_triggered()` (AND-based)
  helpers, `evaluate_brain_type()` function, re-exports diagnostic types.
- `diagnostic.rs` (293 lines) -- `BrainTypeDiagnostic` struct carrying measured
  values, `format_primary_message()`, `format_note()`, `format_help()`.
- `evaluation_tests.rs` (248 lines) -- rstest parameterized unit tests.
- `diagnostic_tests.rs` (304 lines) -- rstest unit tests for formatting.

BDD tests:

- `common/tests/features/brain_type_evaluation.feature` (73 lines)
- `common/tests/brain_type_evaluation_behaviour.rs` (246 lines)

### Threshold rules from the design doc

From `docs/brain-trust-lints-design.md` section "brain_trait rule set (initial
defaults)" (lines 110-114):

- **Warn (AND-based)**: fires when total method count >= 20 AND default method
  CC sum >= 40. Both conditions must hold simultaneously.
- **Escalate/Deny (OR-based)**: fires when total method count >= 30, regardless
  of complexity.
- Deny supersedes warn.
- Configuration keys: `methods_warn = 20`, `methods_deny = 30`,
  `default_cc_warn = 40`.

### Key design decision: "methods" means methods, not items

The design doc says "at least 20 methods". The `TraitMetrics` struct
distinguishes `total_item_count()` (includes associated types and consts) from
individual method counts. The threshold must use total method count =
`required_method_count() + default_method_count()`, NOT `total_item_count()`.
Associated types and consts are not methods and must not count toward the
method thresholds.

## Plan of work

### Stage A: Write ExecPlan

Write this document to `docs/execplans/6-3-2-brain-trait-evaluation.md`.

### Stage B: Implement `evaluation.rs`

Create `common/src/brain_trait_metrics/evaluation.rs` following the pattern of
`common/src/brain_type_metrics/evaluation.rs`.

Contents:

1. Module doc comment explaining threshold rules.
2. Re-export diagnostic types from sibling `diagnostic` module:
   `pub use super::diagnostic::{BrainTraitDiagnostic,`
   `format_help, format_note, format_primary_message};`
3. Test module declaration:
   `#[cfg(test)] #[path = "evaluation_tests.rs"] mod tests;`
4. `BrainTraitDisposition` enum with `Pass`, `Warn`, `Deny` variants.
   Derives: `Clone, Copy, Debug, Eq, PartialEq`.
5. `BrainTraitThresholds` struct with 3 private fields:
   - `methods_warn: usize` (default 20)
   - `methods_deny: usize` (default 30)
   - `default_cc_warn: usize` (default 40)

   Accessors: `methods_warn()`, `methods_deny()`, `default_cc_warn()`.
6. `BrainTraitThresholdsBuilder` with const defaults and fluent setters:
   `new()`, `methods_warn(usize)`, `methods_deny(usize)`,
   `default_cc_warn(usize)`, `build()`. Plus `impl Default`.
7. Private helper: `total_method_count(metrics: &TraitMetrics) -> usize`
   returning `metrics.required_method_count() + metrics.default_method_count()`.
8. Private helpers `is_deny_triggered()` and `is_warn_triggered()`:

   ```rust
   fn is_deny_triggered(metrics: &TraitMetrics, thresholds: &BrainTraitThresholds) -> bool {
       total_method_count(metrics) >= thresholds.methods_deny
   }

   fn is_warn_triggered(metrics: &TraitMetrics, thresholds: &BrainTraitThresholds) -> bool {
       total_method_count(metrics) >= thresholds.methods_warn
           && metrics.default_method_cc_sum() >= thresholds.default_cc_warn
   }
   ```

9. Public function
   `evaluate_brain_trait(&TraitMetrics, &BrainTraitThresholds)`
   `-> BrainTraitDisposition` that checks deny first (OR-based), then warn
   (AND-based), then returns pass.

Include rustdoc examples on all public types and functions.

Estimated: ~180 lines.

### Stage C: Implement `diagnostic.rs`

Create `common/src/brain_trait_metrics/diagnostic.rs` following the pattern of
`common/src/brain_type_metrics/diagnostic.rs`.

Contents:

1. Module doc comment.
2. Imports: `super::evaluation::BrainTraitDisposition`, `super::TraitMetrics`.
3. Test module declaration:
   `#[cfg(test)] #[path = "diagnostic_tests.rs"] mod tests;`
4. `BrainTraitDiagnostic` struct with private fields:
   - `trait_name: String`
   - `disposition: BrainTraitDisposition`
   - `required_method_count: usize`
   - `default_method_count: usize`
   - `default_method_cc_sum: usize`
   - `total_item_count: usize`
   - `implementor_burden: usize`

   Constructor: `new(&TraitMetrics, BrainTraitDisposition) -> Self`. Accessors
   for all fields, plus derived `total_method_count() -> usize`.
5. `format_primary_message(&BrainTraitDiagnostic) -> String`:
   Format: `` `{name}` has {N} methods ({R} required, ``
   `` {D} default) with default method complexity CC={CC}. `` Omit the CC
   clause when `default_method_cc_sum == 0`.
6. `format_note(&BrainTraitDiagnostic) -> String`:
   - Always mentions total method count as interface size.
   - Mentions default method CC sum when non-zero.
   - Mentions implementor burden when required method count is high.
7. `format_help(&BrainTraitDiagnostic) -> String`:
   - Many methods: suggest splitting into focused sub-traits.
   - High default CC: suggest extracting complex defaults into free functions.
   - High implementor burden: suggest providing more default implementations.
   - Fallback: general decomposition advice.

Include rustdoc examples on all public types and functions.

Estimated: ~180 lines.

### Stage D: Update `mod.rs`

Update `common/src/brain_trait_metrics/mod.rs` to:

1. Declare new modules: `pub mod diagnostic;` and `pub mod evaluation;`.
2. Re-export evaluation types:

   ```rust
   pub use evaluation::{
       BrainTraitDisposition, BrainTraitThresholds, BrainTraitThresholdsBuilder,
       evaluate_brain_trait,
   };
   pub use evaluation::{
       BrainTraitDiagnostic, format_help, format_note, format_primary_message,
   };
   ```

The file will grow from 25 lines to approximately 38 lines.

### Stage E: Implement unit tests

**`common/src/brain_trait_metrics/evaluation_tests.rs`** (~200 lines):

Helper:
`build_trait_metrics(name, required_count, default_count, cc_per_default)` that
creates `TraitMetrics` via `TraitMetricsBuilder`.

Test coverage:

1. Default threshold values (parameterized).
2. Builder overrides individual fields.
3. Builder chaining sets all fields.
4. Builder `Default` trait matches `new()`.
5. Pass cases (parameterized):
   - All below thresholds (10 methods, CC=20).
   - Many methods but low CC (19 methods, CC=10) -- just below warn threshold.
   - High CC but few methods (5 methods, CC=50).
   - Exactly at `methods_warn` but CC below threshold (20 methods, CC=39).
6. Warn cases (parameterized):
   - Exact warn thresholds (20 methods, CC=40).
   - Above warn below deny (25 methods, CC=60).
   - Just below `methods_deny` (29 methods, CC=40).
7. Deny cases (parameterized):
   - Method count at deny threshold (30 methods, CC=0).
   - Method count above deny (35 methods, CC=0).
   - Deny supersedes warn (30 methods, CC=50).
8. Custom threshold tests.
9. Associated items do not count as methods: build a trait with 19 required
   methods + 5 associated types + 5 associated consts (total items=29, total
   methods=19). Verify this is Pass.

**`common/src/brain_trait_metrics/diagnostic_tests.rs`** (~200 lines):

Test coverage:

1. Primary message contains trait name in backticks.
2. Primary message contains method count breakdown.
3. Primary message contains CC sum when non-zero.
4. Primary message omits CC clause when zero.
5. Note mentions interface size.
6. Note mentions CC when non-zero.
7. Note omits CC when no default methods.
8. Help suggests splitting trait when many methods.
9. Help suggests extracting defaults when high CC.
10. Help provides default advice when no specific signals.
11. Accessor tests for all fields.

### Stage F: Implement BDD tests

**`common/tests/features/brain_trait_evaluation.feature`** (~75 lines):

```gherkin
Feature: Brain trait threshold evaluation
  Threshold evaluation determines whether a trait qualifies as a brain
  trait based on total method count and default method complexity. The
  warn rule requires both conditions to hold simultaneously (AND-based).
  The deny rule fires on method count alone (OR-based). Deny supersedes
  warn.

  Scenario: Trait within all limits passes
  Scenario: All warn conditions trigger a warning
  Scenario: Many methods alone does not trigger warn
  Scenario: High CC alone does not trigger warn
  Scenario: Method count at deny threshold triggers deny
  Scenario: Deny supersedes warn
  Scenario: Associated items do not count as methods
  Scenario: Diagnostic surfaces measured values
```

**`common/tests/brain_trait_evaluation_behaviour.rs`** (~230 lines):

World struct `EvaluationWorld` with `RefCell`/`Cell` fields for: `trait_name`,
`required_count`, `default_count`, `associated_type_count`,
`associated_const_count`, `default_cc_sum`, `thresholds`, `built_metrics`,
`disposition`, `primary_message`.

Helper `build_metrics(&self)` creates a `TraitMetricsBuilder`, adds required
methods (named `req_0`, `req_1`, …), distributes `default_cc_sum` across
`default_count` default methods (evenly with remainder on last), adds
associated types/consts if configured.

Step functions (max 3 parsed values per step):

- `given_trait_methods(world, name, required, default)` -- 3 parsed values
- `given_associated_items(world, types, consts)` -- 2 parsed values
- `given_default_cc_sum(world, sum)` -- 1 parsed value
- `given_default_thresholds(world)` -- 0 parsed values
- `when_evaluate(world)` -- 0 parsed values
- `when_format_diagnostic(world)` -- 0 parsed values
- `then_disposition_pass(world)` / `_warn` / `_deny` -- 0 parsed values
- `then_primary_message_contains(world, text)` -- 1 parsed value

Scenario binding functions using `#[scenario(path = "...", index = N)]`.

### Stage G: Update `lib.rs` re-exports

Update `common/src/lib.rs` to add re-exports for the new evaluation types.
Re-export the struct/enum/function types BUT NOT the `format_*` functions (to
avoid name collision with existing `brain_type` re-exports):

```rust
pub use brain_trait_metrics::evaluation::{
    BrainTraitDiagnostic, BrainTraitDisposition, BrainTraitThresholds,
    BrainTraitThresholdsBuilder, evaluate_brain_trait,
};
```

### Stage H: Record implementation decisions in design doc

Update `docs/brain-trust-lints-design.md` with a new subsection
`### Implementation decisions (6.3.2)` after the existing 6.3.1 section,
recording:

1. "Methods" threshold counts methods only, not all items.
2. Evaluation lives in `common`, not in a lint crate.
3. Warn is AND-based, deny is OR-based, deny supersedes warn.
4. `BrainTraitDiagnostic` carries all measured values.
5. Builder used for consistency despite only 3 fields.

### Stage I: Mark roadmap 6.3.2 done

Update `docs/roadmap.md`: change `- [ ] 6.3.2.` to `- [x] 6.3.2.`

### Stage J: Quality gates

Run all required gates with log capture:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/6-3-2-check-fmt.log
make lint 2>&1 | tee /tmp/6-3-2-lint.log
make test 2>&1 | tee /tmp/6-3-2-test.log
```

Expected: all three pass with exit code 0.

### Stage K: Finalize living sections

Update this execplan:

- Set `Status` to `COMPLETE`.
- Fill `Surprises & Discoveries`.
- Append final decisions with dates to `Decision Log`.
- Complete `Outcomes & Retrospective`.

## Interfaces and dependencies

### Public API surface (new types exported from `common`)

```rust
// common/src/brain_trait_metrics/evaluation.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrainTraitDisposition {
    /// All metrics are within acceptable limits.
    Pass,
    /// The warn rule fired: all warn conditions hold simultaneously.
    Warn,
    /// The deny rule fired: at least one deny condition holds.
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrainTraitThresholds {
    methods_warn: usize,      // default 20
    methods_deny: usize,      // default 30
    default_cc_warn: usize,   // default 40
}
// Accessors: methods_warn(), methods_deny(), default_cc_warn()

#[derive(Clone, Copy, Debug)]
pub struct BrainTraitThresholdsBuilder { /* mirrors fields */ }
// new(), methods_warn(usize), methods_deny(usize),
// default_cc_warn(usize), build() -> BrainTraitThresholds

pub fn evaluate_brain_trait(
    metrics: &TraitMetrics,
    thresholds: &BrainTraitThresholds,
) -> BrainTraitDisposition;
```

```rust
// common/src/brain_trait_metrics/diagnostic.rs

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainTraitDiagnostic { /* measured values + disposition */ }
// new(&TraitMetrics, BrainTraitDisposition) -> Self
// Accessors: trait_name(), disposition(), required_method_count(),
//   default_method_count(), total_method_count(), default_method_cc_sum(),
//   total_item_count(), implementor_burden()

pub fn format_primary_message(diagnostic: &BrainTraitDiagnostic) -> String;
pub fn format_note(diagnostic: &BrainTraitDiagnostic) -> String;
pub fn format_help(diagnostic: &BrainTraitDiagnostic) -> String;
```

### Dependencies

No new external dependencies. Uses only existing workspace-pinned crates:

- `rstest` (unit tests)
- `rstest-bdd` and `rstest-bdd-macros` version 0.5.0 (BDD tests)
- `std::cell::{Cell, RefCell}` (BDD world struct)
- `std::fmt::Write` (diagnostic string building, if needed)

### File inventory

New files (7):

*Table 1: New files introduced by 6.3.2.*

| File                                                   | Est. lines | Purpose                                    |
| ------------------------------------------------------ | ---------- | ------------------------------------------ |
| `common/src/brain_trait_metrics/evaluation.rs`         | ~180       | Disposition, thresholds, evaluate function |
| `common/src/brain_trait_metrics/diagnostic.rs`         | ~180       | Diagnostic struct, format functions        |
| `common/src/brain_trait_metrics/evaluation_tests.rs`   | ~200       | Unit tests for evaluation                  |
| `common/src/brain_trait_metrics/diagnostic_tests.rs`   | ~200       | Unit tests for diagnostics                 |
| `common/tests/features/brain_trait_evaluation.feature` | ~75        | BDD feature file                           |
| `common/tests/brain_trait_evaluation_behaviour.rs`     | ~230       | BDD step bindings                          |
| `docs/execplans/6-3-2-brain-trait-evaluation.md`       | ~350       | This execution plan                        |

Modified files (4):

*Table 2: Existing files modified by 6.3.2.*

| File                                    | Current lines | Change                                               |
| --------------------------------------- | ------------- | ---------------------------------------------------- |
| `common/src/brain_trait_metrics/mod.rs` | 25            | Add ~13 lines for module declarations and re-exports |
| `common/src/lib.rs`                     | 48            | Add ~4 lines for new type re-exports                 |
| `docs/brain-trust-lints-design.md`      | ~400          | Add ~20 lines for implementation decisions (6.3.2)   |
| `docs/roadmap.md`                       | ~387          | Change `[ ]` to `[x]` on 6.3.2 line                  |

Total: 11 files touched (within tolerance of 12).

## Validation and acceptance

Quality criteria (what "done" means):

- `make check-fmt` passes (exit code 0).
- `make lint` passes (exit code 0) -- no clippy warnings, no
  `too_many_arguments` violations.
- `make test` passes (exit code 0) -- all existing tests still pass, new tests
  pass.
- All new source files are under 400 lines.
- No new external dependencies added.
- Design doc updated with 6.3.2 implementation decisions.
- Roadmap 6.3.2 marked done.

Key test scenarios to verify:

- A trait with 10 methods and CC=20 is classified as Pass.
- A trait with 20 methods and CC=40 is classified as Warn (exact boundary).
- A trait with 19 methods and CC=40 is classified as Pass (below method
  threshold).
- A trait with 20 methods and CC=39 is classified as Pass (below CC threshold).
- A trait with 30 methods and CC=0 is classified as Deny (regardless of CC).
- A trait with 30 methods and CC=50 is classified as Deny (deny supersedes
  warn).
- Associated types and consts do NOT count toward method thresholds.
- Diagnostic messages contain measured values: method count, CC sum, trait name.

## Outcomes & retrospective

### Deliverables

All observable outcomes from the purpose section are met:

1. `common` exports `BrainTraitDisposition`, `BrainTraitThresholds`,
   `BrainTraitThresholdsBuilder`, `evaluate_brain_trait`, and
   `BrainTraitDiagnostic` at the crate root. Formatting functions are
   accessible via `brain_trait_metrics::evaluation::format_*`.
2. Unit tests cover threshold boundary conditions for pass, warn, and deny
   (evaluation) and diagnostic message content (diagnostic).
3. BDD tests validate 8 end-to-end scenarios including the "associated items
   excluded" edge case and diagnostic message content.
4. Design doc updated with 6.3.2 implementation decisions.
5. Roadmap 6.3.2 marked done.
6. All three quality gates pass: `make check-fmt`, `make lint`, `make test`
   (957 tests: 957 passed, 2 skipped).

### File metrics

*Table 3: Final line counts for new source files.*

| File                                  | Lines | Under 400? |
| ------------------------------------- | ----- | ---------- |
| `evaluation.rs`                       | 250   | Yes        |
| `diagnostic.rs`                       | 240   | Yes        |
| `evaluation_tests.rs`                 | 241   | Yes        |
| `diagnostic_tests.rs`                 | 278   | Yes        |
| `brain_trait_evaluation.feature`      | 66    | Yes        |
| `brain_trait_evaluation_behaviour.rs` | 239   | Yes        |

Modified files: `mod.rs` (34 lines), `lib.rs` (51 lines), plus design doc and
roadmap updates.

Total: 11 files touched (within 12-file tolerance). No new external
dependencies.

### What went well

- The `brain_type` reference implementation provided an excellent template.
  Structural parallelism between the two evaluation modules will aid future
  maintainers.
- BDD scenarios caught the "associated items excluded" edge case early,
  confirming that the `total_method_count()` computation was intentionally
  excluding associated types and consts.
- The `rstest-bdd` v0.5.0 macro interface (`#[given]`, `#[when]`, `#[then]`,
  `#[scenario]`) worked smoothly with the 3-parsed-values-plus-world constraint.

### What could be improved

- The `too_many_arguments` and `excessive_nesting` clippy findings required
  two additional fix iterations. Future plans should anticipate parameter
  struct needs when test helpers exceed 3 non-world parameters.
- The ExecPlan estimated `diagnostic_tests.rs` at ~200 lines; it came in at
  278 due to the `DiagnosticInput` struct and more comprehensive parameterized
  test cases. Estimates should include overhead for parameter objects.
