# Replace snippet parsing in `module_must_have_inner_docs` with HIR attribute inspection

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

Issue: [leynos/whitaker#69](https://github.com/leynos/whitaker/issues/69)

## Purpose / big picture

The `module_must_have_inner_docs` Dylint lint currently decides whether a
module is documented by reading the module body as a source snippet and
hand-parsing the leading text. That parser carries a private lexer
(`literal.rs`), a meta-list walker (`parser/mod.rs`), case-mismatch helpers
(`inner_attr.rs`), and a set of newtype aliases in `src/lib.rs`. The parser is
large, subtle, and duplicates work the compiler has already done.

After this change the lint asks the compiler for the module's HIR attributes,
filters them to the module body, sorts them by source position, and checks
whether the first one is a documentation attribute. The snippet parser and its
helpers are deleted. The four observable dispositions stay the same:
`HasLeadingDoc`, `MissingDocs`, `FirstInnerIsNotDoc(span)`, and `Unknown`.

A user can observe the result by running the lint's UI fixtures: a module that
opens with `//!` or `#![doc = "…"]` passes; a module with no inner attributes
warns at the body start; a module whose first inner attribute is not a doc
warns at that attribute.

## Constraints

- Keep the four `ModuleDocDisposition` variants and their meanings.
- Keep the diagnostic path: `emit_diagnostic`, the `module_header_span` note,
  the Fluent messages, and the English fallback.
- Do not change `module_body_span` or `module_header_span` in
  `crates/whitaker_lint_core/src/hir/mod.rs`; `module_max_lines` also uses
  `module_body_span`.
- Do not call `Attribute::path()` on `Attribute::Parsed` values; it panics.
- Do not call `Attribute::span()` on parsed kinds outside the kinds the pinned
  compiler supports (`DocComment`, `Deprecated`, `CfgTrace`); it panics for
  every other parsed kind.
- Skip macro-expanded modules exactly as today (`item.span.from_expansion()`).
- Keep the lint's public surface (`MODULE_MUST_HAVE_INNER_DOCS`,
  `ModuleMustHaveInnerDocs`) unchanged.
- Leave locale files unchanged.
- No new external dependencies.

## Tolerances (exception triggers)

- Scope: stop and escalate if the change needs more than 20 non-ExecPlan files
  or 1,500 net lines. Exclude this ExecPlan from both counts.
- Interface: stop and escalate if a public API signature must change.
- Dependencies: stop and escalate if a new external dependency is required.
- Iterations: stop and escalate if a focused test still fails after three
  distinct fix attempts.
- Ambiguity: stop and escalate if the pinned compiler's HIR representation
  contradicts the plan's assumptions in a way that changes observable behaviour
  beyond the documented deliberate changes.

## Risks

- Risk: the pinned compiler may not retain a usable span for some parsed
  attribute kinds (for example `RecursionLimit`). Severity: medium. Likelihood:
  high. Mitigation: Phase 1 instrumentation records every observed kind and its
  span accessor; unrecoverable kinds are excluded and the affected fixture is
  re-baselined or rewritten to use an `Unparsed` attribute.
- Risk: `cfg_attr` expansion may move the doc attribute's span outside
  `inner_span` or mark it as expansion-derived. Severity: medium. Likelihood:
  medium. Mitigation: Phase 1 records the expanded span and containment; the
  adapter keeps expansion-derived spans only when they are contained and
  recoverable.
- Risk: file-backed modules may attach inner attributes to a different HIR
  owner than the `mod` item. Severity: high. Likelihood: low. Mitigation: Phase
  1 includes a `#[path]` fixture and records `hir_attrs(item.hir_id())`.
- Risk: deleting the parser removes tests that covered real behaviour.
  Severity: medium. Likelihood: medium. Mitigation: replace snippet tests with
  extracted-record unit tests and compiler-backed UI fixtures before deleting
  the parser.

## Progress

- [ ] (2026-11-11) Phase 1: instrument the lint, add scratch fixtures, record
  HIR attribute evidence, then remove instrumentation and scratch fixtures.
- [ ] Phase 2: add the HIR attribute adapter and pure classifier; rewire
  `check_item`; delete the snippet parser and its helpers.
- [ ] Phase 3: rebuild unit, BDD, and UI tests; update design and user docs;
  run focused and workspace gates.
- [ ] Push the branch and open a draft pull request.

## Surprises & discoveries

- Observation: the pinned compiler lowers both `//!` and `#![doc = "…"]` to
  `Attribute::Parsed(AttributeKind::DocComment { style, span, comment, .. })`,
  so `doc_str()` recognises both forms. Evidence:
  `rustc_attr_parsing/src/interface.rs` lines 318–329 and 380–394 in the pinned
  `nightly-2026-05-28` source. Impact: the HIR approach can identify
  documentation without parsing text.
- Observation: lint-level attributes (`allow`, `warn`, `deny`, `forbid`,
  `expect`) are retained as `Attribute::Unparsed` with a style and span.
  Evidence: `rustc_attr_parsing/src/interface.rs` lines 442–454. Impact:
  `#![allow(dead_code)]` remains visible to the detector.
- Observation: `Attribute::span()` panics for arbitrary parsed kinds; it only
  supports `Unparsed`, `DocComment`, `Deprecated`, and `CfgTrace`. Evidence:
  `rustc_hir/src/hir.rs` lines 1470–1480. Impact: parsed kinds need a per-kind
  span strategy.

## Decision log

- Decision: use `cx.tcx.hir_attrs(item.hir_id())` and filter by containment in
  `module.spans.inner_span` rather than trusting an attribute style field.
  Rationale: parsed attribute kinds do not expose a generic style, and the
  module's outer declaration attributes share the same HIR owner. Date/Author:
  2026-11-11, implementation agent.
- Decision: identify documentation with `Attribute::doc_str().is_some()`.
  Rationale: it is the compiler's own notion of a doc comment and covers both
  sugared and `#[doc = "…"]` forms. Date/Author: 2026-11-11, implementation
  agent.

## Outcomes & retrospective

Pending.

## Context and orientation

The lint lives in `crates/module_must_have_inner_docs/`. Its driver is
`src/driver/mod.rs`; the snippet parser is split across
`src/driver/parser/mod.rs`, `src/driver/parser/tests.rs`,
`src/driver/inner_attr.rs`, and `src/driver/literal.rs`. The crate root
`src/lib.rs` re-exports the parser newtypes for the driver's submodules.

Tests live in `src/tests/` (`classifier.rs`, `behaviour.rs`,
`span_to_snippet.rs`, `ui.rs`) and in `tests/features/module_docs.feature`.
Compiler-backed fixtures live in `ui/` and `ui-cy/`.

The shared span helpers are in `crates/whitaker_lint_core/src/hir/mod.rs`.
`module_body_span` prefers `module.spans.inner_span`, then the definition span,
then the item span. `module_header_span` covers `mod name`.

The pinned toolchain is `nightly-2026-05-28` (`rust-toolchain.toml`). Its
compiler sources are available under
`$(rustc --print sysroot)/lib/rustlib/rustc-src/rust/compiler/`.

## Conformance basis

The upstream artefacts are the issue text for
[leynos/whitaker#69](https://github.com/leynos/whitaker/issues/69) and the
existing design section `docs/whitaker-dylint-suite-design.md` §3.4. There is
no separate Terms of Reference or ADR for this lint. The trace is:

```plaintext
ISSUE-69 -> design §3.4 -> EP-M1 (evidence) -> EP-M2 (HIR detector) -> EP-M3 (tests/docs)
```

## Verification plan

- Obligation: `classify` returns `Unknown` when no body span is supplied.
  Method: parameterized `rstest`. Artefact: `src/tests/classifier.rs`.
  Evidence: the case fails before the classifier exists. Non-vacuity: the case
  supplies `None` and asserts the exact variant.
- Obligation: an empty record list yields `MissingDocs`; a first doc record
  yields `HasLeadingDoc`; a first non-doc record yields
  `FirstInnerIsNotDoc(span)` with the earliest non-doc span. Method:
  parameterized `rstest` over record lists with distinct byte positions.
  Artefact: `src/tests/classifier.rs`. Non-vacuity: cases include both
  attribute orders and unsorted input.
- Obligation: the compiler-backed detector agrees with the classifier for
  inline, file-backed, `cfg_attr`, comment, and outer-attribute fixtures.
  Method: Dylint UI fixtures with `.stderr` baselines. Artefact: `ui/` and
  `ui-cy/`. Non-vacuity: at least one pass and one fail fixture per behaviour
  class; the fail baselines assert the exact span.
- Obligation: the lint still skips macro-expanded modules.
  Method: existing UI fixture `ui/pass_macro_generated.rs`. Evidence: no
  diagnostic is emitted.
- Axiom: the pinned compiler's HIR attribute representation is as recorded in
  `Surprises & discoveries`. This is verified directly against the pinned
  compiler source and by Phase 1 instrumentation.

## Plan of work

Stage A (evidence). Add temporary, environment-gated tracing to `check_item`.
Add scratch fixtures under `ui/` covering outer declaration attributes, all doc
forms, `doc(hidden)`, lint levels, `recursion_limit`, a leading ordinary
comment, an empty body, and active/inactive/nested/mixed `cfg_attr`. Run the UI
tests, record the trace, then delete the tracing and scratch fixtures.

Stage B (red tests). Rewrite `src/tests/classifier.rs` around extracted records
and add the new UI fixtures with expected `.stderr` baselines. Run the focused
tests and confirm they fail for the expected reason.

Stage C (implementation). Add `src/driver/hir_attrs.rs` with the adapter and
the pure classifier. Rewire `check_item`. Delete `inner_attr.rs`, `literal.rs`,
`parser/`, `detect_module_docs_from_snippet`, the parser aliases in `lib.rs`,
and all `span_to_snippet` usage.

Stage D (refactor and docs). Update the BDD feature and world, the design
document, and the user guide. Run the focused crate tests, the suite tests,
`make lint`, `make typecheck`, and `make check-fmt`.

## Milestones and plateaus

- EP-M1: evidence recorded and instrumentation removed. Acceptance: the
  ExecPlan contains the observed attribute shapes and spans; the tree is clean
  of scratch fixtures.
- EP-M2: HIR detector in place and snippet parser deleted. Acceptance: the
  crate compiles and the focused unit/BDD tests pass.
- EP-M3: UI baselines and documentation updated. Acceptance: focused UI tests,
  suite tests, lint, typecheck, and format checks pass.

## Concrete steps

Run from the worktree root.

```sh
make test TEST_CARGO_FLAGS='-p module_must_have_inner_docs --features dylint-driver'
make test TEST_CARGO_FLAGS='-p whitaker_suite --features dylint-driver'
make test TEST_CARGO_FLAGS='-p module_max_lines --features dylint-driver'
make lint
make typecheck
make check-fmt
```

## Validation and acceptance

The change is accepted when the focused crate tests pass, the suite baseline is
unchanged, `module_max_lines` is unaffected, and the workspace lint, typecheck,
and format gates pass. The UI fixtures must show the documented deliberate
changes: ordinary comments are ignored, expanded `cfg_attr` docs count,
`doc(hidden)` is not documentation, and outer attributes on the first child
item no longer count as a misordered inner attribute.

## Idempotence and recovery

The instrumentation and scratch fixtures are additive and are deleted in EP-M1.
If a gate fails, fix the cause and re-run the same command; do not re-bless
`.stderr` files automatically.

## Artefacts and notes

Phase 1 evidence is recorded in `Surprises & discoveries` and the decision log.

## Interfaces and dependencies

In `crates/module_must_have_inner_docs/src/driver/hir_attrs.rs`, define a
private record and adapter:

```rust
struct ModuleAttr {
    is_doc: bool,
    span: Span,
}

fn module_attrs(cx: &LateContext<'_>, item: &hir::Item<'_>, inner_span: Span) -> Vec<ModuleAttr>;
```

and a pure classifier:

```rust
fn classify(attrs: &[ModuleAttr], has_body: bool) -> ModuleDocDisposition;
```

The classifier sorts by `span.lo()` and returns the earliest non-doc span in
`FirstInnerIsNotDoc`.
