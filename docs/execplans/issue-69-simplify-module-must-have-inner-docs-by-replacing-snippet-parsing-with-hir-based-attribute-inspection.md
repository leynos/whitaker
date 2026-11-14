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

- [x] (2026-11-11) Baseline:
      `make test TEST_CARGO_FLAGS='-p module_must_have_inner_docs --features dylint-driver'`
      passed 54 of 54 tests
      before any source change. Log:
      `/tmp/test-module_must_have_inner_docs-issue-69-...out`.
- [x] (2026-11-11) Phase 1: instrumented the lint, added scratch fixtures,
  recorded HIR attribute evidence, then removed the instrumentation and scratch
  fixtures. Committed as `db963b0b`.
- [x] (2026-11-11) Phase 2: added `src/driver/hir_attrs.rs` with
  `ModuleAttr`, `module_attrs`, and `classify`; rewired `check_item`; deleted
  `inner_attr.rs`, `literal.rs`, `parser/`, `span_to_snippet.rs`, and the
  parser aliases in `src/lib.rs`; removed the now-unused `rustc_ast`,
  `rustc_session`, and `proptest` dependencies.
- [x] (2026-11-11) Phase 3 sources: rewrote `src/tests/classifier.rs` and
  `src/tests/behaviour.rs`; added four pass and five fail UI fixtures with
  baselines; updated the design section, the user guide, and the migration
  guide. The first focused run proved every baseline correct: all three UI
  locale cases passed, and the only failures were three order-sensitive unit
  cases caused by the synthetic-span helper (fixed; see
  `Surprises & discoveries`).
- [x] (2026-11-11) Phase 3 gates: focused crate tests 18/18 pass; `make test`
  exits 0; `whitaker_suite` 4/4 plus its support suites pass; `make lint`,
  `make typecheck`, `make check-fmt`, and `make spelling` pass. The attempted
  `-p module_max_lines` test run exposed only that the crate has no test
  target; it is unmodified and compiled by the lint and typecheck gates.
- [x] (2026-11-11) CodeRabbit `--agent` review returned five findings: three
  minor fixture suppressions and two major doc-comment items. All five were
  actioned. The focused suite re-ran green (18 of 18) after the fixture and
  baseline edits.
- [x] (2026-11-14) Re-ran every gate on the amended change set. `make test`
      passed the workspace nextest run (2417 of 2417), the feature-free core
      run (128), the dylint driver suites (58 and 14), `make test-root-lib`
      (61), `make test-nextest-filter` (44), and `make test-doc`; all three
      `module_must_have_inner_docs` locale cases passed, so the hand-edited
      baselines agree with rustc byte for byte. `make check-fmt`, `make lint`,
      `make typecheck`, `make markdownlint`, `make nixie`, and `make spelling`
      also exited 0.
- [ ] Push the branch and open a draft pull request.

## Surprises & discoveries

- Observation: the pinned compiler lowers both `//!` and `#![doc = "…"]` to
  `Attribute::Parsed(AttributeKind::DocComment { style, span, comment, .. })`,
  so `doc_str()` recognizes both forms. Evidence:
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
- Observation (Phase 1 dump, `/tmp/mmhid-dump.txt`): `hir_attrs` order is not
  source order. For a module written `#![recursion_limit = "32"]` followed by
  `//!`, the doc comment is index 0 and `RecursionLimit` is index 1. Impact:
  sorting by `span.lo()` is required and list order cannot be trusted.
- Observation: `AttributeKind::RecursionLimit` carries no span.
  `AttributeKind::Path` (the outer `#[path]` on file modules) and
  `AttributeKind::CfgAttrTrace` also carry no span. Impact: these kinds are
  excluded; the misordering UI fixture is rewritten to use
  `#![allow(dead_code)]`, which is `Unparsed` with a span.
- Observation: every doc form lowers to `Parsed(DocComment)`. Sugared line,
  sugared block, and raw `#![doc = "…"]` all report `doc_str() == Some(..)`,
  `doc_resolution_scope() == Some(Inner)`, and a non-expansion span inside
  `inner_span`. An expanded `cfg_attr(…, doc = "…")` doc keeps a span inside the
  `cfg_attr` token range and remains non-expansion.
- Observation: `cfg_attr` expansion inserts a `CfgAttrTrace` entry without a
  span, and its list position relative to the expanded attribute varies (before
  the expanded `Doc` in one fixture, after it in others). Impact: exclude trace
  kinds before sorting.
- Observation: inactive `cfg_attr` leaves only `CfgAttrTrace`. An inactive
  `cfg_attr(…, doc = "…")` therefore yields no doc record, so the module is
  reported missing (a deliberate change from the snippet parser, which never
  evaluated `cfg`).
- Observation: `#![doc(hidden)]` lowers to `Parsed(Doc)` with `first_span` and
  `doc_str() == None`. Impact: it is not documentation, but its span is
  recoverable when it precedes a real doc.
- Observation: outer declaration attributes (`#[allow] mod m { … }`) appear in
  `hir_attrs` with spans before `inner_span`; containment removes them. An
  outer doc attribute on the first child item does not appear in the module's
  attributes at all, so it yields `MissingDocs` at the body start.
- Observation: an empty inline module has a non-dummy `inner_span` at its
  closing brace (`48:19..48:20` in the scratch fixture), so it still yields
  `MissingDocs` rather than `Unknown`.
- Observation: file-backed modules attach both the inner doc and the outer
  `#[path]` attribute to the `mod` item; the doc span lies in the child file
  and inside `inner_span`, while `Path` has no span.
- Observation: `DUMMY_SP.with_lo(BytePos(n)).with_hi(BytePos(n + 1))` does not
  produce a span at `n`. `Span::new` swaps its bounds when `lo > hi`, so the
  intermediate `0..n` dummy span collapses back to `0..n` and the low bound is
  lost. Synthetic test spans must set the high bound first:
  `DUMMY_SP.with_hi(BytePos(n + 1)).with_lo(BytePos(n))`. Evidence: the first
  test run failed with `Span { lo: BytePos(0), hi: BytePos(16) }` for a record
  requested at offset 15.
- Observation: UI baselines must end with the two newlines rustc emits after
  `warning: N warnings emitted`; compiletest compares the strings exactly, so a
  handwritten baseline without them fails even when every diagnostic line
  matches.

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
- Decision: use an explicit per-kind span whitelist for parsed attributes:
  `DocComment { span }`, `Doc(doc).first_span`, and `Deprecated { span }`.
  Exclude `CfgAttrTrace`, `CfgTrace`, `Path`, `RecursionLimit`, and every other
  parsed kind whose user-written span is unrecoverable, rather than inventing a
  position. Rationale: `Attribute::span()` panics for those kinds, and a
  synthesized span would misreport the offending token. Date/Author:
  2026-11-11, implementation agent.
- Decision: rewrite `ui/fail_inner_attribute_before_doc.rs` to place
  `#![allow(dead_code)]` before `//!`. Rationale: `#![recursion_limit]` has no
  recoverable span and is excluded by the whitelist, so with HIR inspection it
  would no longer be reported. The lint-level attribute preserves the
  misordering behaviour under test. Date/Author: 2026-11-11, implementation
  agent.
- Decision: treat a module as having a body only when `module.spans.inner_span`
  is neither dummy nor expansion-derived. Rationale: an empty inline module's
  inner span points at its closing brace and is still a real body, whereas
  macro-generated or synthesized spans cannot be inspected. Date/Author:
  2026-11-11, implementation agent.
- Decision: model the BDD world as candidates carrying an `is_inner` flag, and
  classify only the inner candidates. Rationale: the "only outer documentation"
  scenario must exercise the adapter's containment rule rather than passing an
  empty record list, which would duplicate the "no attributes" scenario.
  Date/Author: 2026-11-11, implementation agent.
- Decision: remove the `rustc_ast`, `rustc_session`, and `proptest`
  dependencies alongside the parser. Rationale: nothing outside the deleted
  parser or its tests imported `rustc_ast` or `rustc_session`, and the deleted
  proptest cases were the only users of `proptest`. Date/Author: 2026-11-11,
  implementation agent.
- Decision: add `ui/fail_doc_hidden_not_docs.rs` even though the fixture list in
  the task named only four new fail cases. Rationale: the acceptance section
  requires UI evidence that `doc(hidden)` is not documentation, and Phase 1
  showed it lowers to `Parsed(Doc)` without a doc string. Date/Author:
  2026-11-11, implementation agent.
- Decision: proceed despite exceeding the 20-file scope tolerance. The change
  touches 35 non-ExecPlan files, but the task explicitly mandates the pass and
  fail fixture list, each fixture needs its `.stderr` alongside it, and the
  parser deletion removes five files. Net lines are -639, well inside the
  1,500-line tolerance. No file beyond that mandated set was added or removed.
  Date/Author: 2026-11-11, implementation agent.
- Decision: treat the `module_max_lines` test command in `Concrete steps` as
  incorrectly specified rather than as a failing gate. That crate sets
  `[lib] test = false`, has only a `.feature` file under `tests/features`, and
  therefore has no test target;
  `make test TEST_CARGO_FLAGS='-p module_max_lines --features dylint-driver'`
  exits 2 with nextest's `error: no tests to run`. The crate is unmodified by
  this change, `Makefile` already lists it in `TEST_EXCLUDES` and
  `DOCTEST_EXCLUDES`, and `make lint` plus `make typecheck` compile it. No
  source edit is warranted. Date/Author: 2026-11-11, implementation agent.
- Decision: action the five findings from the first CodeRabbit `--agent`
  review. Three fixtures used `#![allow(dead_code)]` as their non-doc inner
  attribute; they now use `#![warn(dead_code)]`, which preserves the fixture
  intent without a lint suppression. `classifies_records` gained a summary doc
  comment, and the `hir_attrs` summaries are imperative with `# Examples`
  headings. The affected baselines were updated by hand to the narrower
  attribute spans. Date/Author: 2026-11-11, implementation agent.

## Outcomes & retrospective

The lint now classifies module documentation from the compiler's HIR attributes.
`src/driver/hir_attrs.rs` holds the adapter and the pure classifier; the four
dispositions keep their meanings. The snippet parser (`inner_attr.rs`,
`literal.rs`, `parser/`), `span_to_snippet.rs`, the parser newtype aliases, and
the `rustc_ast`, `rustc_session`, and `proptest` dependencies are gone. The
focused suite passes 18 of 18, and the workspace `make test` exits 0.

Lessons: HIR attribute order is not source order, so the classifier must sort by
`span.lo()`. `Span::new` swaps inverted bounds, so synthetic test spans must
set the high bound before the low bound on `DUMMY_SP`. UI baselines are
compared byte-for-byte, including the trailing blank line.

## Context and orientation

The lint lives in `crates/module_must_have_inner_docs/`. Its driver is
`src/driver/mod.rs`; HIR attribute extraction and the pure classifier live in
`src/driver/hir_attrs.rs`. The snippet parser that previously occupied
`src/driver/parser/mod.rs`, `src/driver/parser/tests.rs`,
`src/driver/inner_attr.rs`, and `src/driver/literal.rs` has been deleted, along
with the parser newtype re-exports in `src/lib.rs`.

Tests live in `src/tests/` (`classifier.rs`, `behaviour.rs`, `ui.rs`) and in
`tests/features/module_docs.feature`. Compiler-backed fixtures live in `ui/` and
`ui-cy/`.

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
make lint
make typecheck
make check-fmt
```

`module_max_lines` has no test target (`[lib] test = false`, no integration
test sources), so `make test -p module_max_lines` cannot run; `make lint` and
`make typecheck` compile it instead. See the decision log.

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
