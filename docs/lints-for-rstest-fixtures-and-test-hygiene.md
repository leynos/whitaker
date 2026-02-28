# Designing Whitaker Dylint lints for `rstest` fixtures and test hygiene

## Executive summary

Whitaker already has the core structure needed for sophisticated and
configurable lints:

- Per-lint crates under `crates/*`.
- A suite aggregator library.
- UI fixtures (`ui/pass_*.rs`, `ui/fail_*.rs`, and `ui/fail_*.stderr`).
- A documented split between standard and experimental lint sets.

This document proposes three experimental lints for `rstest`-based tests:

- Lint A, `RSTEST_HELPER_SHOULD_BE_FIXTURE`: identifies repeated helper calls
  in `#[rstest]` tests where arguments are absent, fixture-backed, or stable
  constants, and recommends fixture extraction.
- Lint B, `SINGLE_BINDING_PARAGRAPH`: identifies contiguous statement
  paragraphs that compute a single binding and are suitable for extraction.
- Lint C, `RSTEST_PARAGRAPH_SHOULD_BE_FIXTURE`: identifies repeated,
  assertion-free setup paragraphs across `#[rstest]` tests and recommends
  fixture extraction.

All three lints are designed as late lints, with conservative span handling,
local analysis boundaries, and deterministic output suitable for UI testing.

## Integration constraints

Whitaker uses Dylint's dynamic-library model, so each lint should live in an
independent crate with explicit configuration. The following constraints guide
implementation:

- Implement as late lints so HIR and type checking information are available.
- Recover user-editable spans where possible to avoid flagging macro-only glue.
- Keep first release in the experimental lint set and promote only after
  tuning against real repositories.

## Lint A: call-site fixture extraction

### Intent

This lint detects helper functions repeatedly called inside `#[rstest]` tests
where call arguments suggest fixture semantics. It then recommends converting
that helper into a `#[fixture]` and injecting it as a test parameter.

- Crate name: `rstest_helper_should_be_fixture`
- Lint name: `RSTEST_HELPER_SHOULD_BE_FIXTURE`

### Trigger conditions for lint A

The lint emits only when all conditions hold:

- Callsite is within a function recognised as an `#[rstest]` test.
- Callee resolves to a function definition, and by default is local to the
  crate.
- Distinct test count is at least `min_distinct_tests`.
- Total call count is at least `min_calls`.
- Every argument list satisfies one of:
  - no arguments,
  - fixture locals only,
  - constants only with identical fingerprints, or
  - fixture locals plus constants with positional constant matches.

### `#[rstest]` test detection

Detection should be conservative:

- Match function attributes `rstest` and `rstest::rstest`.
- Support optional, config-gated fallback through expansion trace metadata when
  attributes are not directly available.

### Fixture-local classification

For each function parameter:

- Mark as non-fixture when annotated with provider-oriented attributes such as
  `case`, `values`, `files`, `future`, or `context`.
- Otherwise mark as fixture-local for this lint.

Version one should accept only simple identifier bindings for fixture-local
classification and defer destructuring support to a later refinement.

### Argument fingerprint model

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ArgAtom {
    FixtureLocal { name: String },
    ConstLit { text: String },
    ConstPath { def_path: String },
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ArgFingerprint {
    atoms: Vec<ArgAtom>,
}
```

Fingerprint rules:

- `FixtureLocal`: path expression resolving to a recognised fixture parameter.
- `ConstLit`: literal expression captured as source text.
- `ConstPath`: path expression resolving to `const`, associated `const`, or
  `static`, keyed by a stable definition path.
- `Unsupported`: any other expression shape.

### Diagnostic strategy

Use `span_lint_hir_and_then` with the best user-editable span. Primary text
should identify fixture semantics, with notes for counts and fingerprint
consistency.

Example diagnostic wording:

- Primary: `Helper 'make_db' behaves like a fixture in rstest tests.`
- Note: `Found 4 calls across 3 tests with stable fixture/constant arguments.`
- Help: `Convert to #[fixture] and inject as a test parameter.`

### Configuration example for lint A

```toml
[rstest_helper_should_be_fixture]
min_calls = 2
min_distinct_tests = 2
require_identical_fixture_arg_names = false
provider_param_attributes = ["case", "values", "files", "future", "context"]
use_source_callee_fallback = false
```

## Lint B: single-binding paragraph detection

### Intent for lint B

This lint identifies contiguous statement paragraphs in a single block that
compute one output binding and can be extracted to reduce test or helper noise.
It is intentionally narrower than clone detection.

- Crate name: `single_binding_paragraph`
- Lint name: `SINGLE_BINDING_PARAGRAPH`

### Formal rule

Given statements `S[0..n)`, a paragraph candidate is an inclusive range
`[k..=i]` where:

- `S[i]` is `let out = <expr>;` with a simple binding pattern.
- Length is between `min_len` and `max_len` (defaults: 3 and 8).
- The range contains no control-flow constructs and no macro-only spans.
- Intermediate locals defined in `[k..i)` are not used in `(i+1)..n`.
- External input count does not exceed `max_inputs`.

### Backward-slice algorithm

For each sink binding `S[i]`:

1. Initialise `needed` to local uses inside the sink expression.
2. Walk backward from `i - 1` while contiguity is preserved.
3. Include statement `S[j]` when it defines or mutates an item in `needed`.
4. Update `needed` with statement uses and remove newly satisfied definitions.
5. Stop at the first non-contributing statement.

This preserves deterministic behaviour and keeps complexity bounded to local
block analysis.

### Statement I/O model

```rust
#[derive(Clone, Debug, Default)]
struct StmtIO {
    defs: std::collections::BTreeSet<rustc_hir::HirId>,
    uses: std::collections::BTreeSet<rustc_hir::HirId>,
    muts: std::collections::BTreeSet<rustc_hir::HirId>,
    has_control_flow: bool,
    has_macro_only_span: bool,
    has_closure: bool,
}
```

`BTreeSet` keeps iteration order stable for deterministic diagnostics and UI
outputs.

### Configuration example for lint B

```toml
[single_binding_paragraph]
min_len = 3
max_len = 8
max_inputs = 3
treat_mutating_method_calls_as_defs = true
reject_closures = true
reject_async = true
skip_external_macro_expansions = true
```

## Lint C: repeated fixture paragraph detection

### Intent for lint C

This lint bridges paragraph extraction and fixture extraction for tests. It
identifies repeated setup paragraphs across `#[rstest]` tests when the
paragraphs are assertion-free and input-compatible with fixture extraction.

- Crate name: `rstest_paragraph_should_be_fixture`
- Lint name: `RSTEST_PARAGRAPH_SHOULD_BE_FIXTURE`

### Trigger conditions for lint C

The lint emits for paragraph groups that satisfy all conditions:

- Candidates pass lint B structural constraints.
- Paragraph contains no assertions.
- Paragraph inputs are fixtures and/or stable constants.
- Input fingerprints match across occurrences.
- Distinct test count meets `min_distinct_tests`.

### Assertion detection

Use combined matching:

- macro names such as `assert`, `assert_eq`, `assert_ne`, and `debug_assert`,
- optionally configured project-specific assertion helpers.

### Paragraph fingerprint model

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ParagraphFingerprint {
    shapes: Vec<StmtShape>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum StmtShape {
    Let { init: ExprShape },
    MutCall { receiver: Option<LocalSlot>, callee: CalleeShape },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ExprShape {
    Call { callee: CalleeShape, argc: usize },
    MethodCall { method: String, argc: usize },
    Path,
    Lit,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum CalleeShape {
    DefPath(String),
    Unknown,
}
```

Local identifiers should be normalised to deterministic slots by
first-appearance order. Deep AST canonicalisation is intentionally out of scope.

### Emission strategy

Collect candidates during block/body checks, then emit during
`check_crate_post` after cross-test grouping completes. Support both modes:

- emit at each occurrence, or
- emit once per group with location notes.

### Configuration example for lint C

```toml
[rstest_paragraph_should_be_fixture]
min_distinct_tests = 2
min_len = 3
max_len = 8
max_inputs = 2
assertion_macros = ["assert", "assert_eq", "assert_ne", "debug_assert"]
require_fixture_or_constant_inputs = true
require_identical_input_fingerprint = true
emit_once_per_group = false
```

## Comparison and rollout guidance

| Lint                                 | Primary target                                 | Precision (expected) | Compute cost  | Key configuration                                      | Complexity |
| ------------------------------------ | ---------------------------------------------- | -------------------: | ------------: | ------------------------------------------------------ | ---------: |
| `RSTEST_HELPER_SHOULD_BE_FIXTURE`    | Repeated helper calls in `#[rstest]` tests     | High                 | Medium        | `min_calls`, `min_distinct_tests`, argument strictness | Medium     |
| `SINGLE_BINDING_PARAGRAPH`           | Contiguous single-output paragraphs            | Medium               | Low to medium | `min_len`, `max_len`, `max_inputs`, mutation policy    | Medium     |
| `RSTEST_PARAGRAPH_SHOULD_BE_FIXTURE` | Repeated setup paragraphs in `#[rstest]` tests | High                 | Medium        | `min_distinct_tests`, assertion set, input strictness  | High       |

_Table 1: Comparison of target scope, cost, and implementation complexity._

For screen readers: The following flowchart summarises lint data flow from HIR
traversal to per-lint diagnostics.

```mermaid
flowchart TD
    A[HIR traversal] --> B{In rstest test?}
    B -- no --> C[Skip rstest-only logic]
    B -- yes --> D[Collect callsites for lint A]
    B -- yes --> E[Collect paragraph candidates for lint C]
    C --> F[Collect paragraph candidates for lint B]
    D --> G[Aggregate by callee DefId]
    E --> H[Fingerprint and group paragraphs]
    G --> I{Thresholds and arguments pass?}
    H --> J{Thresholds and inputs pass?}
    I --> K[Emit lint A]
    F --> L[Emit lint B]
    J --> M[Emit lint C]
```

_Figure 1: High-level analysis and emission flow for the three proposed lints._

For screen readers: The following Gantt chart outlines a staged implementation
sequence from shared helpers through integration.

```mermaid
gantt
    title Whitaker lint implementation plan
    dateFormat  YYYY-MM-DD
    axisFormat  %d %b

    section Foundations
    Shared rstest detection and fingerprints          :a1, 2026-02-24, 5d
    Span hygiene utilities                            :a2, after a1, 3d

    section Lint A
    Callsite collection and aggregation               :b1, after a2, 6d
    UI tests and threshold tuning                     :b2, after b1, 4d

    section Lint B
    Statement I/O model and backward slice            :c1, 2026-03-06, 7d
    UI tests and false-positive controls              :c2, after c1, 5d

    section Lint C
    Paragraph fingerprinting and grouping             :d1, after c2, 6d
    UI tests and fixture-guidance messaging           :d2, after d1, 5d

    section Integration
    Experimental-set wiring and feature gates         :e1, after b2, 3d
    Documentation and localisation updates            :e2, after d2, 3d
```

_Figure 2: Proposed phased implementation timeline for lints A, B, and C._

## Non-goals and boundaries

The following work is intentionally excluded from this design:

- whole-program dependence graphs,
- MIR or SSA-level transformations,
- general-purpose clone detection, and
- aggressive canonicalisation aimed at maximal recall.

These constraints keep runtime cost predictable, diagnostics explainable, and
false-positive control practical for iterative promotion from experimental to
standard lints.
