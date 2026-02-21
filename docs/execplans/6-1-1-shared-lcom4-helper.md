# Add shared LCOM4 cohesion helper (roadmap 6.1.1)

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

The canonical plan file is
`docs/execplans/6-1-1-shared-lcom4-helper.md`.

## Purpose / big picture

Whitaker's brain trust lints (`brain_type` and `brain_trait`, roadmap §6)
require a shared cohesion analysis foundation. Roadmap item 6.1.1 tracks
delivery of a shared LCOM4 (Lack of Cohesion in Methods, version 4) helper
in the `common` crate.

LCOM4 models each method as a node in an undirected graph, adding edges when
two methods share a field access or when one method directly calls another on
the same type. The metric equals the number of connected components in this
graph: LCOM4 == 1 indicates high cohesion, while LCOM4 >= 2 suggests the
type bundles unrelated responsibilities.

The helper is a **pure library module** (`common/src/lcom4.rs`) that receives
pre-extracted method metadata (names, accessed fields, called methods as
plain strings) and returns connected component counts. It does **not** depend
on `rustc_private` or any HIR types — the HIR traversal that populates
`MethodInfo` is a separate task (roadmap 6.1.2).

This separation matches the project's established pattern: `complexity_signal`
in `common` provides pure signal-processing helpers consumed by the
`bumpy_road_function` lint driver, which handles the HIR traversal.

After this change the `common` crate exports a `cohesion_components` function
and a `MethodInfo` data type that future lints (`brain_type`, `brain_trait`)
can consume directly.

Success is observable when:

1. `common/src/lcom4.rs` exists and exports `MethodInfo` and
   `cohesion_components`.
2. Unit tests (`#[rstest]`) cover happy, unhappy, and edge cases inline.
3. Behavioural tests (`rstest-bdd` v0.5.0) in
   `common/tests/lcom4_behaviour.rs` with Gherkin scenarios in
   `common/tests/features/lcom4.feature` cover the same contract.
4. `docs/brain-trust-lints-design.md` records implementation decisions.
5. `docs/roadmap.md` marks 6.1.1 as done.
6. `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Implement as a new module in `common/src/lcom4.rs` following existing
  patterns (closest analogue: `common/src/complexity_signal.rs`).
- Keep file sizes under 400 lines.
- No new external dependencies. Connected component counting uses an inline
  union-find implementation (private to the module). The typical method
  count per type is small (< 100), making union-find the simplest correct
  choice without adding `petgraph` or similar.
- Use `BTreeSet<String>` for field and method-call sets in `MethodInfo` to
  ensure deterministic iteration and derive compatibility (`Eq`, `Ord`).
- The module must not depend on `rustc_private` or any HIR types. It is a
  pure library operating on pre-extracted string metadata.
- Use workspace-pinned dependencies; `rstest-bdd` and `rstest-bdd-macros`
  at `0.5.0`.
- Provide both unit tests and behavioural tests for happy/unhappy/edge
  paths.
- Record design decisions in `docs/brain-trust-lints-design.md`.
- On completion, update `docs/roadmap.md` entry 6.1.1 to `[x]`.
- Keep comments/docs in en-GB-oxendict spelling and wrap Markdown prose at
  80 columns.
- Observe `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"`
  enforced by `common/Cargo.toml` — no `expect()` or `unwrap()` in
  non-test code.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 10 files or 400
  net lines, stop and escalate.
- Dependencies: if any new external dependency is needed, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts, stop and escalate with logs.

## Risks

- Risk: `MethodInfo` API may prove insufficient when 6.1.2 (HIR extraction)
  begins — for example, if field access needs to distinguish reads from
  writes, or if associated-function-style calls (no `self`) need separate
  handling.
  - Severity: medium.
  - Likelihood: low (the design document treats field access uniformly and
    only considers methods on the same type).
  - Mitigation: keep `MethodInfo` minimal and additive. New fields or
    variants can be added in a backwards-compatible way later.

- Risk: union-find path compression uses mutable self, which complicates
  ownership if future callers need shared references.
  - Severity: low.
  - Likelihood: low (the union-find is an internal implementation detail,
    constructed and consumed within `cohesion_components`).
  - Mitigation: keep the union-find private and expose only the immutable
    public API.

## Progress

- [x] Draft ExecPlan for roadmap item 6.1.1.
- [x] Stage A: Implement `common/src/lcom4.rs` with `MethodInfo` and
  `cohesion_components`.
- [x] Stage B: Wire module into `common/src/lib.rs` with re-exports.
- [x] Stage C: Add inline unit tests with `rstest`.
- [x] Stage D: Add BDD behavioural tests with `rstest-bdd` v0.5.0.
- [x] Stage E: Record design decisions in
  `docs/brain-trust-lints-design.md`.
- [x] Stage F: Run quality gates and capture logs.
- [x] Stage G: Mark roadmap item 6.1.1 as done.

## Surprises & Discoveries

- **Gherkin quoted strings and `rstest-bdd` parsing**: the original plan
  used double-quoted values in Gherkin steps (e.g.,
  `Given a method "b" accessing fields "x, y"`). The `rstest-bdd` v0.5.0
  `{placeholder}` parser did not correctly capture comma-separated values
  within quoted strings — the comma was treated as a delimiter, causing
  multi-field steps to split incorrectly. The transitive-sharing scenario
  returned 3 components instead of the expected 1 because method `b` only
  received `"x` as its field set. The fix was to remove all quotes from
  Gherkin steps and use the word `called` as a natural-language separator
  (e.g., `Given a method called b accessing fields x, y`). This matches
  the pattern used by the existing `complexity_signal.feature`.

## Decision Log

- Decision: use `BTreeSet<String>` for field and method-call sets rather
  than `HashSet<String>`.
  - Rationale: `BTreeSet` derives `Eq`, `Ord`, `Hash` without extra effort,
    provides deterministic iteration order (important for reproducible
    diagnostics), and the set sizes are small enough that B-tree overhead
    is negligible.
  - Date/Author: 2026-02-21 / DevBoxer.

- Decision: use inline union-find with path compression and union-by-rank
  rather than DFS or an external graph library.
  - Rationale: union-find is O(n α(n)) amortized for n methods, trivially
    correct for connected-component counting, and requires no external
    dependency. The alternative (adjacency list + DFS) is equally valid but
    slightly more code for the same result. `petgraph` is overkill for
    this use case.
  - Date/Author: 2026-02-21 / DevBoxer.

- Decision: return `usize` directly from `cohesion_components` rather than
  `Result<usize, _>`.
  - Rationale: the function accepts a well-typed slice of `MethodInfo`
    values. There are no invalid inputs — an empty slice yields 0
    components, which is a valid degenerate case. Wrapping in `Result`
    would force callers to handle an error that cannot occur.
  - Date/Author: 2026-02-21 / DevBoxer.

- Decision: keep `MethodInfo` as a plain data struct without validation
  errors (no `Result`-returning constructor).
  - Rationale: method names and field names are strings extracted from HIR
    by the caller. There is no domain-level invariant to enforce at
    construction time (empty names, empty sets, and duplicate entries are
    all valid inputs). This keeps the API simple and avoids unnecessary
    error types.
  - Date/Author: 2026-02-21 / DevBoxer.

- Decision: use unquoted Gherkin step text with `called` separator rather
  than double-quoted placeholders.
  - Rationale: `rstest-bdd` v0.5.0 `{placeholder}` parsing does not
    reliably capture comma-separated values within quoted strings. Using
    unquoted text with a natural-language separator (`called`) avoids the
    issue and matches the existing `complexity_signal.feature` convention.
  - Date/Author: 2026-02-21 / DevBoxer.

## Outcomes & Retrospective

All acceptance criteria met:

- `common/src/lcom4.rs` (~310 lines) exports `MethodInfo` and
  `cohesion_components`. The module follows the `complexity_signal` pattern
  with a module-level `//!` doc comment, private fields, `#[derive]`
  macros, `const fn` / `#[must_use]` accessors, and Rustdoc `# Examples`.
- 17 inline unit tests cover happy paths (single method, shared field,
  direct call, transitive sharing, common field), unhappy paths (disjoint
  methods, multiple clusters), and edge cases (empty input, isolated
  methods, self-calls, unknown callees, mixed connections). Two tests
  validate the `UnionFind` data structure directly.
- 7 BDD scenarios in `common/tests/features/lcom4.feature` with step
  definitions in `common/tests/lcom4_behaviour.rs` cover the same contract
  from a behavioural perspective.
- `docs/brain-trust-lints-design.md` records five implementation decisions
  under "### Implementation decisions (6.1.1)".
- `docs/roadmap.md` marks 6.1.1 as `[x]`.
- `make check-fmt`, `make lint`, and `make test` all pass (660 tests,
  660 passed).

Files created (3):

- `common/src/lcom4.rs`
- `common/tests/lcom4_behaviour.rs`
- `common/tests/features/lcom4.feature`

Files modified (3):

- `common/src/lib.rs` — added `pub mod lcom4;` and re-exports
- `docs/brain-trust-lints-design.md` — appended implementation decisions
- `docs/roadmap.md` — marked 6.1.1 as `[x]`

Net lines added: ~400 (within tolerance threshold).

Lessons learned:

- When writing Gherkin scenarios for `rstest-bdd`, avoid double-quoted
  values in step text. The `{placeholder}` parser does not handle quoted
  strings containing commas as a single token. Use unquoted text with
  natural-language separators instead.

## Context and orientation

Current repository state relevant to this task:

- `docs/roadmap.md` marks 1.1.1 (common crate helpers) as complete, which
  was the prerequisite for 6.1.1.
- `common/src/lib.rs` exports modules: `attributes`, `complexity_signal`,
  `context`, `diagnostics`, `expr`, `i18n`, `lcom4`, `path`, `span`,
  `test_support`.
- The closest analogue is `common/src/complexity_signal.rs` (365 lines):
  - Module-level `//!` doc comment.
  - Data structures with private fields, `#[derive]`, and `const fn`
    accessors.
  - Error enums with `thiserror::Error`.
  - Validation in helper functions returning `Result`.
  - Public functions with `///` docs, `# Examples`, `# Errors` sections.
  - Inline `#[cfg(test)] mod tests` with `#[rstest]`.
- BDD tests for `complexity_signal` live in
  `common/tests/complexity_signal_behaviour.rs` with feature file at
  `common/tests/features/complexity_signal.feature`.
- BDD test pattern uses a `World` struct with `Cell`/`RefCell` fields,
  `#[fixture]`, `#[given]`/`#[when]`/`#[then]` step macros, and
  `#[scenario]` binding to feature file indices.
- The `common` crate's `Cargo.toml` already includes `rstest`, `rstest-bdd`,
  and `rstest-bdd-macros` in dev-dependencies.

Files created:

- `common/src/lcom4.rs`
- `common/tests/lcom4_behaviour.rs`
- `common/tests/features/lcom4.feature`

Files modified:

- `common/src/lib.rs` — added `pub mod lcom4;` and re-exports
- `docs/brain-trust-lints-design.md` — appended decision log entries
- `docs/roadmap.md` — marked 6.1.1 as `[x]`

## Plan of work

### Stage A: Implement `common/src/lcom4.rs`

Create `common/src/lcom4.rs` with the following structure:

**Module doc comment** (`//!`): explain that this module provides LCOM4
cohesion analysis for method graphs, used by the brain trust lints.

**`MethodInfo` struct:**

```rust
/// Metadata for a single method, used to build the LCOM4 method graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodInfo {
    name: String,
    accessed_fields: BTreeSet<String>,
    called_methods: BTreeSet<String>,
}
```

- Constructor: `pub fn new(name: impl Into<String>, accessed_fields:
  BTreeSet<String>, called_methods: BTreeSet<String>) -> Self`
- Accessors: `pub fn name(&self) -> &str`,
  `pub fn accessed_fields(&self) -> &BTreeSet<String>`,
  `pub fn called_methods(&self) -> &BTreeSet<String>` — all `#[must_use]`.

**`UnionFind` struct (private):**

```rust
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}
```

- `fn new(n: usize) -> Self` — initializes each element as its own parent.
- `fn find(&mut self, x: usize) -> usize` — with path compression.
- `fn union(&mut self, x: usize, y: usize)` — with union-by-rank.
- `fn component_count(&self) -> usize` — counts distinct roots. Note: call
  `find` on all elements first to flatten the tree, then count unique
  parents.

**`cohesion_components` function:**

```rust
/// Counts connected components in the method relationship graph (LCOM4).
///
/// Returns `0` for an empty method slice, `1` when all methods form a
/// single cohesive group, and `n >= 2` when the type contains `n`
/// unrelated method clusters.
#[must_use]
pub fn cohesion_components(methods: &[MethodInfo]) -> usize
```

Algorithm:

1. If `methods` is empty, return 0.
2. Build `field_index: HashMap<&str, Vec<usize>>` mapping each field name
   to the indices of methods that access it.
3. Build `method_index: HashMap<&str, usize>` mapping each method name to
   its index.
4. Initialize `UnionFind::new(methods.len())`.
5. For each field in `field_index`, union all method indices that share it
   (pairwise union of the first element with each subsequent element).
6. For each method at index `i`, for each name in `called_methods`, look up
   the callee's index in `method_index` and union `i` with it.
7. Return `uf.component_count()`.

Acceptance: module compiles, exports `MethodInfo` and
`cohesion_components`, and follows all code style requirements.

### Stage B: Wire module into `common/src/lib.rs`

Add to `common/src/lib.rs`:

```rust
pub mod lcom4;
```

And add re-exports:

```rust
pub use lcom4::{MethodInfo, cohesion_components};
```

Place the new module declaration alphabetically (after `i18n`, before
`path`). Place the re-export line after the `i18n` re-exports block.

Acceptance: `cargo check -p common` succeeds.

### Stage C: Add inline unit tests

Add `#[cfg(test)] mod tests` at the bottom of `common/src/lcom4.rs` with
`#[rstest]` tests covering:

**Happy paths:**

- `single_method_yields_one_component` — one method -> 1.
- `two_methods_sharing_field_yields_one_component` — methods A and B both
  access field `x` -> 1.
- `two_methods_with_direct_call_yields_one_component` — A calls B -> 1.
- `transitive_field_sharing_yields_one_component` — A shares field with B,
  B shares field with C -> 1.
- `all_methods_share_common_field` — N methods all accessing field `x`
  -> 1.

**Unhappy paths:**

- `two_disjoint_methods_yield_two_components` — A accesses `x`, B accesses
  `y`, no calls -> 2.
- `three_methods_two_clusters` — A-B share `x`, C isolated -> 2.
- `four_methods_three_clusters` — A-B share `x`, C isolated, D isolated
  -> 3.

**Edge cases:**

- `empty_methods_yields_zero` — empty slice -> 0.
- `methods_with_empty_fields_and_no_calls_are_isolated` — three methods
  with no fields and no calls -> 3.
- `self_call_does_not_connect_to_others` — A calls A, B calls B, no shared
  fields -> 2.
- `mixed_field_sharing_and_calls` — A shares field with B, C calls A -> 1.
- `method_calls_unknown_method` — A calls "nonexistent" -> call is ignored,
  A remains in its own component.

Acceptance: `cargo test -p common` passes all new tests.

### Stage D: Add BDD behavioural tests

**Create `common/tests/features/lcom4.feature`:**

```gherkin
Feature: LCOM4 cohesion analysis
  LCOM4 counts connected components in a method relationship graph to
  measure type cohesion. A result of 1 indicates high cohesion; 2 or
  more indicates the type bundles unrelated responsibilities.

  Scenario: Single method is always cohesive
    Given a method called process accessing fields data
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two methods sharing a field are cohesive
    Given a method called read accessing fields buffer
    And a method called write accessing fields buffer
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two methods with a direct call are cohesive
    Given a method called validate accessing no fields
    And a method called process accessing no fields calling validate
    When I compute LCOM4
    Then the component count is 1

  Scenario: Two disjoint methods indicate low cohesion
    Given a method called parse accessing fields input
    And a method called render accessing fields output
    When I compute LCOM4
    Then the component count is 2

  Scenario: Transitive field sharing connects a chain
    Given a method called a accessing fields x
    And a method called b accessing fields x, y
    And a method called c accessing fields y
    When I compute LCOM4
    Then the component count is 1

  Scenario: Empty type has zero components
    When I compute LCOM4
    Then the component count is 0

  Scenario: Methods with no fields and no calls are isolated
    Given a method called alpha accessing no fields
    And a method called beta accessing no fields
    And a method called gamma accessing no fields
    When I compute LCOM4
    Then the component count is 3
```

**Create `common/tests/lcom4_behaviour.rs`:**

Follow the pattern from `complexity_signal_behaviour.rs`:

- Define `LcomWorld` struct with `RefCell<Vec<MethodInfo>>` for methods and
  `Cell<Option<usize>>` for the computed result.
- `#[fixture] fn world() -> LcomWorld`
- Step definitions:
  - `#[given("a method called {name} accessing fields {fields}")]` — parse
    comma-separated field names, create `MethodInfo`, push to world.
  - `#[given("a method called {name} accessing no fields")]` — empty field
    set.
  - `#[given("a method called {name} accessing no fields calling
    {callee}")]` — empty fields, one called method.
  - `#[when("I compute LCOM4")]` — call `cohesion_components` and store
    result.
  - `#[then("the component count is {count}")]` — assert equality.
- `#[scenario]` bindings for each feature file index.

Acceptance: `cargo test -p common` passes all BDD scenarios.

### Stage E: Record design decisions

Append implementation decisions to `docs/brain-trust-lints-design.md` in
the "Cohesion analysis (LCOM4)" section, after the existing paragraph about
the shared helper. Cover data representation, graph algorithm, edge
semantics, empty input handling, and infallibility.

Acceptance: design document is updated and passes `make check-fmt`.

### Stage F: Run quality gates and capture logs

Run required checks with `tee` and `set -o pipefail`:

```bash
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-1-1-check-fmt.log
set -o pipefail; make lint      2>&1 | tee /tmp/6-1-1-lint.log
set -o pipefail; make test      2>&1 | tee /tmp/6-1-1-test.log
```

If any command fails, fix and rerun until all pass or a tolerance trigger
is hit.

Acceptance: all three commands exit successfully with logs retained for
review.

### Stage G: Mark roadmap item 6.1.1 as done

Change in `docs/roadmap.md`:

```diff
-- [ ] 6.1.1. Add a shared LCOM4 helper in `common` that builds ...
+- [x] 6.1.1. Add a shared LCOM4 helper in `common` that builds ...
```

Acceptance: roadmap reflects the shipped state.

## Validation and acceptance checklist

The feature is complete only when all are true:

- [x] `common/src/lcom4.rs` exists and exports `MethodInfo` and
  `cohesion_components`.
- [x] Unit tests cover happy/unhappy/edge cases with `#[rstest]`.
- [x] Behavioural tests use `rstest-bdd` v0.5.0 and cover the same
  contract.
- [x] `docs/brain-trust-lints-design.md` records design decisions.
- [x] `docs/roadmap.md` marks 6.1.1 as done.
- [x] `make check-fmt`, `make lint`, and `make test` succeed.

## Idempotence and recovery

- Stages are additive and safe to rerun.
- If a test fails, fix the implementation or test, then rerun from the
  failing stage.
- If tolerance thresholds are exceeded, stop, document the issue, and
  escalate.
