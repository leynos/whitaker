# Emit decomposition diagnostic notes for brain-trust lints (roadmap 6.4.2)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETED

This document must be maintained in accordance with `AGENTS.md`.

Do not begin implementation until the user explicitly approves this plan.

## Purpose / big picture

Roadmap item 6.4.2 turns the structured clustering output from roadmap 6.4.1
into developer-facing diagnostic notes for `brain_type` and `brain_trait`.
After this change, a large or incohesive type or trait will not only report its
measured metrics, it will also show concise decomposition guidance such as
"these grammar methods belong together" or "these I/O defaults should become a
sub-trait", while staying short enough to remain readable for very large
subjects.

Observable outcome:

1. `common` exposes a shared note-rendering helper that maps
   `DecompositionSuggestion` values to concise English diagnostic note text.
2. `common::brain_type_metrics` and `common::brain_trait_metrics` gain
   diagnostic-note entry points that delegate to the shared renderer without
   changing existing primary-message or metric-note behaviour.
3. Unit tests cover happy, unhappy, and edge cases for suggestion ordering,
   phrasing, omission behaviour, and cap handling.
4. Behaviour tests using `rstest-bdd` v0.5.0 exercise end-to-end note
   rendering for both type and trait subjects.
5. `docs/brain-trust-lints-design.md` records the final note template and cap
   policy for 6.4.2.
6. `docs/roadmap.md` marks 6.4.2 done only after implementation and all
   quality gates succeed.
7. `make check-fmt`, `make lint`, and `make test` pass at the end of the
   implementation turn.

## Constraints

- Scope only roadmap item 6.4.2. Do not change the clustering algorithm from
  6.4.1, add configuration loading, add localization wiring, add SARIF output,
  or start the Verus/Kani tasks from 6.4.3 onwards in this change.
- Keep the rendering logic in `common` and keep `common` free of
  `rustc_private` dependencies. Diagnostic-note rendering must remain
  compiler-independent.
- Reuse `common::decomposition_advice::DecompositionSuggestion` as the source
  of truth. Do not duplicate community-detection or labelling logic inside the
  diagnostic modules.
- Preserve existing public constructors and accessors on
  `BrainTypeDiagnostic` and `BrainTraitDiagnostic`. Prefer additive APIs over
  signature-breaking changes.
- Keep existing metric-note and help formatting behaviour stable unless a test
  is intentionally updated to reflect the new note channel. The decomposition
  text should appear as a separate note, not by overloading the existing
  metric-explanation note.
- Keep source files under 400 lines. Split note rendering into sibling modules
  before either diagnostic file or `decomposition_advice` grows too large.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for tests.
- Behaviour tests must respect the workspace Clippy threshold of 4 arguments.
  Each BDD step can parse at most 3 values from feature text in addition to the
  world fixture.
- Any new public helper in `common` requires Rustdoc comments with examples
  that follow `docs/rust-doctest-dry-guide.md`.
- Record final implementation decisions in `docs/brain-trust-lints-design.md`.
- Do not mark roadmap item 6.4.2 as done until implementation, documentation,
  and all requested quality gates succeed.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 10 touched files or 900 net lines of
  code, stop and escalate.
- Interface: if 6.4.2 appears to require breaking changes to
  `BrainTypeDiagnostic`, `BrainTraitDiagnostic`, or `DecompositionSuggestion`,
  stop and escalate before changing those public interfaces.
- Dependency: if a new external dependency appears necessary for note
  rendering, stop and escalate before adding it.
- Rendering ambiguity: if the later lint-driver integration proves that a
  separate note channel is impossible and the text must instead live in help or
  the existing metric note, stop and escalate with the concrete trade-offs.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix iterations, stop and escalate with captured logs.
- Ambiguity: if the cap policy that keeps notes concise cannot be implemented
  without making a product decision not covered by this plan, stop and present
  the options before proceeding.

## Risks

- Wording risk: roadmap 6.6.2 will later move brain-trust diagnostics into
  Fluent, but 6.4.2 needs English strings now. Severity: medium. Likelihood:
  high. Mitigation: keep note assembly in one shared renderer with short,
  template-driven phrasing that can later move behind localization keys.
- Length risk: raw cluster data can produce unwieldy notes for very large
  types. Severity: high. Likelihood: high. Mitigation: cap both the number of
  communities shown and the number of method names shown per community, then
  report omitted counts explicitly.
- Duplication risk: brain type and brain trait may drift if they each format
  suggestions independently. Severity: medium. Likelihood: medium. Mitigation:
  implement one renderer in `common::decomposition_advice` and keep per-lint
  modules as thin wrappers.
- Stability risk: note ordering must be deterministic or UI output will drift.
  Severity: medium. Likelihood: medium. Mitigation: preserve the 6.4.1 sort
  order, add unit tests for ties, and keep omission wording deterministic.
- Overreach risk: name synthesis for hypothetical extracted types or modules is
  tempting but not required for 6.4.2. Severity: medium. Likelihood: medium.
  Mitigation: map each cluster to an extraction kind and method list only; do
  not invent concrete type/module names in this roadmap item.

## Progress

- [x] 2026-03-13: Draft this ExecPlan and capture the current repository
  state.
- [x] Stage B: Add failing unit tests and BDD scenarios that define the
  decomposition-note rendering contract.
- [x] Stage C: Implement shared decomposition-note rendering in `common`.
- [x] Stage D: Wire brain-type and brain-trait diagnostic wrappers to the
  shared renderer.
- [x] Stage E: Make tests green and refactor for readability while preserving
  current metric-note and help behaviour.
- [x] Stage F: Record implementation decisions for 6.4.2 in
  `docs/brain-trust-lints-design.md`.
- [x] Stage G: Mark roadmap item 6.4.2 done.
- [x] Stage H: Run `make check-fmt`, `make lint`, and `make test`
  successfully.
- [x] Stage I: Finalize the living sections in this document.

## Surprises & Discoveries

- Roadmap 6.4.1 already shipped the shared analysis engine in
  `common/src/decomposition_advice/`, including `DecompositionContext`,
  `DecompositionSuggestion`, and deterministic ordering. 6.4.2 should consume
  that output rather than extending the clustering layer.
- The current brain-type and brain-trait diagnostic modules only expose one
  metric-oriented note and one help string. There is no existing decomposition
  note channel to extend.
- `common/src/lib.rs` re-exports the brain-type formatting helpers but not the
  brain-trait ones, specifically to avoid name collisions. 6.4.2 should avoid
  introducing another ambiguous top-level re-export.
- The design document's decomposition example sits under "help output", but
  the roadmap item explicitly calls for "diagnostic notes". The safest
  interpretation is to add a dedicated decomposition-note formatter rather than
  changing existing help text.
- Existing BDD coverage in `common/tests/` follows indexed `#[scenario]`
  bindings, fixture-backed world structs, and `Result`-returning step functions
  to stay compatible with Clippy's workspace denies.
- `common::decomposition_advice::format_diagnostic_note()` fits cleanly as a
  shared renderer, and the type/trait modules only need subject-kind wrappers
  plus re-exports through their existing `evaluation` modules.
- The workspace `clippy::too_many_arguments` threshold also applies to plain
  unit-test helpers in `common/src/**/tests.rs`; helper fixtures for note
  rendering had to use a small input struct instead of a five-parameter
  function.

## Decision Log

- Decision: treat 6.4.2 as a separate diagnostic-note channel, not as a change
  to the existing metric note or help text. Rationale: the roadmap wording is
  explicit about notes, and isolating the new text avoids unintentional churn
  in already-tested metric/help behaviour. Date/Author: 2026-03-13 / Codex.
- Decision: keep note rendering shared under `common::decomposition_advice`
  and let brain-type and brain-trait diagnostics delegate to it. Rationale:
  both lints consume the same `DecompositionSuggestion` data model and should
  stay textually consistent. Date/Author: 2026-03-13 / Codex.
- Decision: cap note output by showing at most 3 communities and at most 3
  method names per community, then append omitted counts such as
  "`+2 more methods`" and "`2 more areas omitted`". Rationale: this keeps the
  note concise for very large subjects while still exposing the most important
  decomposition signals. Record the final wording in the design doc during
  implementation. Date/Author: 2026-03-13 / Codex.
- Decision: do not synthesize suggested extracted type/module names in 6.4.2.
  Rationale: 6.4.1 provides stable labels and extraction kinds, but not safe
  canonical names. Inventing names here would add product behaviour not
  required by the roadmap. Date/Author: 2026-03-13 / Codex.
- Decision: render note bullets as `- [label] <kind> for <methods>` and cap the
  visible list at 3 suggestions and 3 methods per suggestion. Rationale: the
  format stays short, deterministic, and easy to migrate to Fluent while still
  naming the extracted area and the methods that motivate it. Date/Author:
  2026-03-14 / Codex.

## Context and orientation

### Repository state

The project is a Rust workspace. Shared compiler-independent logic lives in
`common/`. For this roadmap area:

- `common/src/decomposition_advice/` contains the 6.4.1 clustering and
  suggestion engine.
- `common/src/brain_type_metrics/diagnostic.rs` formats primary messages,
  metric notes, and help for brain-type diagnostics.
- `common/src/brain_trait_metrics/diagnostic.rs` does the same for
  brain-trait diagnostics.
- `common/src/brain_type_metrics/diagnostic_tests.rs` and
  `common/src/brain_trait_metrics/diagnostic_tests.rs` hold unit coverage for
  the existing formatting behaviour.
- `common/tests/decomposition_advice_behaviour.rs` and
  `common/tests/features/decomposition_advice.feature` already cover the
  structured suggestions produced by 6.4.1.

There are still no dedicated `brain_type` or `brain_trait` lint crates in the
repository. Current roadmap work for 6.x continues to land in `common` as pure
data models and formatters, with future lint-driver work to consume them.

### Design requirements from `docs/brain-trust-lints-design.md`

The "Decomposition advice" section requires concise advice that maps clustered
methods to extraction suggestions and explicitly says that advice may be capped
for extremely large types. The current design document already fixes the
community input data:

1. `suggest_decomposition()` returns stable `DecompositionSuggestion` values.
2. Each suggestion already carries a label, extraction kind, method names, and
   rationale.
3. Advice should only appear when clustering yields meaningful groups.

That means 6.4.2 should focus on rendering and integration, not on changing the
underlying analysis.

## Proposed implementation shape

Add one small shared rendering module under `common/src/decomposition_advice/`,
for example:

- `common/src/decomposition_advice/note.rs`

The public surface should remain narrow. A concrete, implementation-friendly
shape is:

```rust
pub fn format_diagnostic_note(
    context: &DecompositionContext,
    suggestions: &[DecompositionSuggestion],
) -> Option<String>;
```

The renderer should apply the default cap policy internally:

- show at most 3 suggestions, ordered by the existing 6.4.1 order,
- show at most 3 method names per suggestion, sorted as already stored,
- append `+N more methods` inside a suggestion when method names are trimmed,
- append `N more areas omitted` when the suggestion list is trimmed.

Suggested note template:

```plaintext
Potential decomposition for `Foo`:
- [grammar] helper struct for `parse_nodes`, `parse_tokens`
- [serde::json] module for `decode_json`, `encode_json`
- [std::fs] module for `load_from_disk`, `save_to_disk`
```

When capped:

```plaintext
Potential decomposition for `Foo`:
- [grammar] helper struct for `parse_nodes`, `parse_tokens`, `parse_stream`,
  +2 more methods
- [serde::json] module for `decode_json`, `encode_json`
- [std::fs] module for `load_from_disk`, `save_to_disk`
2 more areas omitted
```

The exact wording may change during implementation, but the final result must
remain short, deterministic, and easy to migrate to Fluent later.

## Plan of work

### Stage B: Write failing tests first (red)

Add tests that define the note-rendering contract before implementing it.

Files to add or update:

- `common/src/decomposition_advice/tests.rs` or
  `common/src/decomposition_advice/note_tests.rs`
- `common/src/brain_type_metrics/diagnostic_tests.rs`
- `common/src/brain_trait_metrics/diagnostic_tests.rs`
- `common/tests/features/decomposition_diagnostic_notes.feature`
- `common/tests/decomposition_diagnostic_notes_behaviour.rs`

Unit-test coverage matrix:

1. The shared renderer returns `None` for an empty suggestion list.
2. A type suggestion renders `helper struct` and `module` wording exactly as
   expected.
3. A trait suggestion renders `sub-trait` wording exactly as expected.
4. Suggestions remain ordered by descending community size and then label when
   rendered.
5. Only the first 3 suggestions are shown, with an omitted-area suffix when
   more communities exist.
6. Only the first 3 method names are shown per suggestion, with a
   `+N more methods` suffix when additional methods are hidden.
7. Existing `format_note()` and `format_help()` outputs remain unchanged when
   decomposition-note formatting is not invoked.
8. Brain-type and brain-trait wrapper functions delegate to the shared
   renderer without changing subject naming or extraction-kind wording.

Behaviour-test scenarios (`rstest-bdd` v0.5.0):

1. Happy path: a brain type with grammar, serde, and filesystem communities
   renders three concise note entries.
2. Happy path: a brain trait renders sub-trait suggestions for two focused
   default-method communities.
3. Unhappy path: no suggestions mean no decomposition note is emitted.
4. Edge path: more than three communities yields a capped note with an omitted
   areas line.
5. Edge path: a large community yields a per-community `+N more methods`
   suffix instead of an overlong method list.

The BDD world should store `DecompositionContext`, synthetic
`DecompositionSuggestion` values, and the rendered `Option<String>`. Use helper
types instead of many scalar parameters so each step stays within the workspace
argument limit.

### Stage C: Implement shared note rendering

Implement the renderer in `common/src/decomposition_advice/`.

Recommended helper breakdown:

1. `format_diagnostic_note()` as the public entry point returning
   `Option<String>`.
2. A small helper that renders one suggestion line from label, extraction kind,
   and method names.
3. A helper that applies method-name caps and emits the `+N more methods`
   suffix.
4. A helper that applies the overall suggestion cap and emits the
   omitted-areas line.

Implementation rules:

- Use the existing suggestion ordering from 6.4.1; do not re-sort by a new
  heuristic.
- Keep note text deterministic and English-only for now.
- Use multi-line note text for readability, but keep each line concise.
- Do not consume `rationale()` in the first implementation unless a failing
  test proves it is necessary. Labels, extraction kinds, and method names are
  sufficient for the roadmap requirement.

### Stage D: Wire diagnostic wrappers

Add thin wrapper functions in:

- `common/src/brain_type_metrics/diagnostic.rs`
- `common/src/brain_trait_metrics/diagnostic.rs`

Recommended shapes:

```rust
pub fn format_decomposition_note(
    diagnostic: &BrainTypeDiagnostic,
    suggestions: &[DecompositionSuggestion],
) -> Option<String>;
```

```rust
pub fn format_decomposition_note(
    diagnostic: &BrainTraitDiagnostic,
    suggestions: &[DecompositionSuggestion],
) -> Option<String>;
```

Each wrapper should:

1. Reconstruct a `DecompositionContext` from the diagnostic's subject name and
   fixed subject kind.
2. Delegate to the shared renderer.
3. Avoid mutating or widening the existing diagnostic struct unless later
   implementation evidence shows that is necessary.

Do not re-export ambiguous new formatter names from `common/src/lib.rs` unless
there is a clear need and the naming collision problem is solved cleanly.

### Stage E: Green and refactor

Make the red tests pass, then refactor without changing behaviour.

Refactoring guardrails:

- keep public APIs additive,
- keep note helpers small and well named,
- avoid repeated string-template logic across the type and trait modules,
- split files before they approach 400 lines,
- add succinct comments only where cap logic or omission wording would
  otherwise be unclear.

### Stage F: Record design decisions

Update `docs/brain-trust-lints-design.md` with a new subsection:

- `### Implementation decisions (6.4.2)`

Record at least:

1. the final note template and whether it is multi-line,
2. the cap policy for suggestions and per-suggestion method names,
3. the rule for when no note is emitted,
4. the decision to keep this renderer English-only until localization work,
5. the reason for using a shared renderer plus per-lint wrappers.

### Stage G: Mark roadmap complete

After implementation and successful validation, update `docs/roadmap.md`:

```markdown
- [x] 6.4.2. Emit concise diagnostic notes mapping clusters to extraction
  suggestions, capped for large types. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice.
```

Do not update this checkbox during the draft-planning turn.

### Stage H: Validate

Because the implementation will change Rust code and documentation, run all of
the following with `set -o pipefail` and `tee` to capture full logs:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/6-4-2-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/6-4-2-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/6-4-2-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/6-4-2-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/6-4-2-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/6-4-2-test.log
```

Expected implementation-time outcomes:

- the new unit tests fail before implementation and pass afterwards,
- the new BDD scenarios fail before implementation and pass afterwards,
- `make check-fmt`, `make lint`, and `make test` all finish successfully,
- existing decomposition-analysis and brain-metric tests do not regress.

## Outcomes & Retrospective

Implemented roadmap 6.4.2 with a shared note renderer at
`common/src/decomposition_advice/note.rs`, plus thin
`format_decomposition_note()` wrappers for `brain_type` and `brain_trait`.
Diagnostic notes now render as:

```plaintext
Potential decomposition for `Foo`:
- [grammar] helper struct for `parse_nodes`, `parse_tokens`
- [serde::json] module for `decode_json`, `encode_json`
```

The renderer emits no note for an empty suggestion list, caps output at 3
suggestions and 3 method names per suggestion, and reports omissions as
`+N more methods` and `N more areas omitted`.

Validation completed with:

- `make fmt`
- `make markdownlint`
- `make nixie`
- `make check-fmt`
- `make lint`
- `make test`

Final test result: `1119 passed, 2 skipped`.
