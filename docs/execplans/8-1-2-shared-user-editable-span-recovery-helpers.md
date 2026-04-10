# Add shared user-editable span recovery helpers for macro-heavy `rstest` code paths

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`. The canonical
plan file is
`docs/execplans/8-1-2-shared-user-editable-span-recovery-helpers.md`.

This draft must be approved before implementation begins. Do not start code
changes for roadmap item 8.1.2 until the user explicitly approves this plan.

## Purpose / big picture

Roadmap item 8.1.2 fills the gap between strict `rstest` detection (implemented
in 8.1.1) and later diagnostic-emitting lints. After this change, Whitaker will
have a shared way to answer a new question for macro-heavy test code paths:

1. Is this span already user-editable?
2. If not, can a call-site or expansion-chain fallback recover a span the user
   can actually edit?
3. If every candidate is still macro-generated glue, should the lint skip the
   diagnostic instead of pointing at compiler-generated code?

Success is observable when:

1. `whitaker-common` exposes a pure, deterministic span-recovery policy that
   can be tested without `rustc_private`.
2. The `whitaker` crate exposes a `rustc_span::Span` adapter that walks source
   call-sites and feeds that policy.
3. At least one existing macro-aware lint path uses the shared helper instead
   of crate-local `source_callsite()` logic or unconditional `from_expansion()`
   skipping.
4. Unit tests and `rstest-bdd` v0.5.0 behavioural tests cover happy paths,
   unhappy paths, and edge cases.
5. `docs/lints-for-rstest-fixtures-and-test-hygiene.md` records the final
   8.1.2 design decisions.
6. `docs/roadmap.md` marks 8.1.2 done only after implementation, docs, and all
   gates succeed.
7. `make check-fmt`, `make lint`, and `make test` pass at the end of the
   implementation turn, with the documentation gates also passing because this
   work touches Markdown.

## Constraints

- Scope only roadmap item 8.1.2. Do not implement lint A, lint B, lint C,
  argument fingerprinting, call-site aggregation, or paragraph detection in
  this change.
- Keep the policy layer pure and shareable. The recovery decision should live
  in `whitaker-common`, while any `rustc_span::Span` walking stays in the
  `whitaker` crate behind the existing `dylint-driver` feature.
- Treat macro-only glue as a skip condition, not a best-effort diagnostic
  target. If no user-editable span can be recovered, the helper must report
  that outcome explicitly so callers can suppress the lint.
- Preserve the conservative posture from
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: recover user-editable
  spans where possible, but do not guess when the expansion chain stays fully
  synthetic.
- Keep public APIs small and documented. New public items need Rustdoc
  examples that compile under the doctest model described in
  `docs/rust-doctest-dry-guide.md`.
- Keep every source file under 400 lines. Split modules early if tests or
  helper logic start to crowd a single file.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  versions. The BDD coverage for this task must use `rstest-bdd` v0.5.0.
- Do not mark roadmap item 8.1.2 done until the shared helpers, adopter,
  design-doc updates, and all quality gates succeed.
- Because this work adds or changes Markdown, the implementation turn must run
  `make fmt`, `make markdownlint`, and `make nixie` in addition to the normal
  Rust gates.

## Tolerances

- Scope: if implementation needs more than 12 touched files or roughly 1,000
  net new lines, stop and escalate before continuing.
- API shape: if the pure policy cannot stay `rustc_private`-free without
  introducing a large abstraction stack, stop and escalate with the competing
  designs.
- Consumer breadth: if adopting the helper in one existing lint would require
  semantic changes in multiple unrelated lint crates, stop and escalate rather
  than broadening the blast radius.
- Span fidelity: if `source_callsite()` and related span APIs cannot
  distinguish a user-editable location from macro-only glue for the selected
  adopter, stop and escalate with concrete failing fixtures before adding
  heuristics.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after three targeted fix iterations, stop and escalate with the captured logs.

## Risks

- Recovery risk: `source_callsite()` can recover the macro invocation site, but
  nested `rstest` expansions may still land on generated companion modules or
  harness constants. Mitigation: model the outcome as `direct`, `recovered`, or
  `macro-only` rather than always returning a span.
- Drift risk: a pure policy in `whitaker-common` and a `rustc` adapter in
  `whitaker` can diverge if the adapter builds the frame list incorrectly.
  Mitigation: keep the adapter thin and give the pure policy the bulk of the
  branch logic.
- Overreach risk: adding a brand-new shared span module in the wrong crate
  could blur the current architecture. Mitigation: keep the pure rule with
  `common::rstest` and keep the compiler-aware adapter in `src/hir.rs`, which
  already hosts shared `rustc`-aware helpers.
- Test ergonomics risk: BDD feature-file-only edits can look stale on targeted
  reruns because the binary may not rebuild. Mitigation: touch the `.rs`
  harness when a targeted rerun appears stale and rely on the full `make test`
  gate before concluding.
- Adoption risk: replacing the existing local `source_callsite()` usage in
  `function_attrs_follow_docs` could change ordering behaviour for existing
  proc-macro fixtures. Mitigation: add a regression test around the current
  proc-macro aux fixture before switching the driver over.

## Progress

- [x] (2026-04-10 00:00Z) Stage A: Inspect the roadmap, the design doc, the
      prior 8.1.1 ExecPlan, the current `common::rstest` module, and existing
      macro-span handling patterns.
- [ ] Stage B: Add failing unit tests and `rstest-bdd` scenarios that define
      the 8.1.2 recovery contract.
- [ ] Stage C: Implement the pure shared recovery policy in
      `whitaker-common`.
- [ ] Stage D: Implement the `rustc_span::Span` adapter and public re-exports
      in the `whitaker` crate.
- [ ] Stage E: Replace one crate-local macro-span workaround with the shared
      helper and add adopter regression coverage.
- [ ] Stage F: Record implementation decisions in
      `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.
- [ ] Stage G: Mark roadmap item 8.1.2 done.
- [ ] Stage H: Run `make fmt`, `make markdownlint`, `make nixie`,
      `make check-fmt`, `make lint`, and `make test`.
- [ ] Stage I: Finalize the living sections in this document.

## Surprises & Discoveries

- `common::rstest` currently stops at strict test, fixture, and parameter
  classification. It has no span-recovery surface yet.
- The only shared pure span abstraction today is `common::span::SourceSpan`,
  which is useful for modelling doctestable examples but does not recover
  `rustc_span::Span` values on its own.
- `src/hir.rs` is already the shared `rustc_private`-aware home for span-ish
  helpers such as `module_body_span`, so extending it is less disruptive than
  inventing a new top-level feature gate just for 8.1.2.
- `crates/function_attrs_follow_docs/src/driver.rs` already uses
  `source_callsite()` locally to normalize macro-expanded attribute positions.
  That is the clearest in-repo precedent for a first shared adopter.
- Several other lints still take the coarse path shown below, so 8.1.2 should
  stay narrow and avoid promising a repository-wide cleanup in one turn.

  ```plaintext
  if span.from_expansion() {
      return;
  }
  ```

- The workspace already contains root-level `rstest-bdd` harnesses in `tests/`,
  so compiler-adjacent shared helper tests do not need to invent a new test
  style.

## Decision Log

- Decision: split the feature into a pure policy in `whitaker-common` and a
  thin `rustc_span::Span` adapter in `whitaker`. Rationale: this matches the
  8.1.1 pattern and keeps most tests free of `rustc_private`. Date/Author:
  2026-04-10 / Codex.
- Decision: extend `src/hir.rs` instead of creating a new top-level module.
  Rationale: `src/hir.rs` already hosts shared compiler-aware helpers and has
  ample headroom before hitting the 400-line limit. Date/Author: 2026-04-10 /
  Codex.
- Decision: the recovery API should distinguish a usable span from a
  macro-only outcome, not just return `Option`. Rationale: future lint drivers
  need to know whether to emit nothing because the code is generated, not
  because the helper forgot to inspect a fallback. Date/Author: 2026-04-10 /
  Codex.
- Decision: the first concrete adopter should be
  `function_attrs_follow_docs`. Rationale: it already contains local
  `source_callsite()` logic, already exercises proc-macro-based `#[fixture]`
  fixtures, and can validate the shared helper without prematurely implementing
  lint A. Date/Author: 2026-04-10 / Codex.

## Context and orientation

### Repository state

The relevant code already lives in four clusters.

- `common/src/rstest/` contains the pure 8.1.1 detection and parameter
  classification helpers.
- `common/src/span.rs` contains the existing pure span value objects used by
  the diagnostic builder and doctest examples.
- `src/hir.rs` contains shared `rustc_private`-aware helpers used by lint
  crates via the `whitaker` facade.
- `crates/function_attrs_follow_docs/src/driver.rs` contains the current local
  `source_callsite()` normalization precedent.

The relevant docs already exist.

- `docs/roadmap.md` defines 8.1.2 as “shared user-editable span recovery
  helpers for macro-heavy test code paths”.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` adds the design rule
  that diagnostics should recover user-editable spans where possible and avoid
  macro-only glue.
- `docs/execplans/8-1-1-shared-rstest-test-and-fixture-detection-helpers.md`
  shows the repository’s preferred split between pure shared logic and later
  compiler-aware consumers.

### What 8.1.2 must provide

The task is not “general macro span recovery for every lint in the workspace”.
It is the narrower foundation that later `rstest` hygiene lints will reuse.
That foundation needs three concrete capabilities.

1. A pure rule that receives an ordered chain of span candidates and returns
   the first user-editable one, or reports that the chain is macro-only.
2. A `rustc` adapter that can turn a `Span` plus its successive
   `source_callsite()` fallbacks into that ordered chain.
3. One real consumer that proves the helper replaces crate-local span
   normalization and suppresses diagnostics on synthetic glue instead of
   pointing at generated code.

## Proposed implementation shape

Create one new pure shared module and extend two existing integration points.

- `common/src/rstest/span.rs`
- `common/src/rstest/mod.rs`
- `common/src/rstest/tests.rs`
- `common/tests/rstest_span_recovery_behaviour.rs`
- `common/tests/features/rstest_span_recovery.feature`
- `common/src/lib.rs`
- `src/hir.rs`
- `src/lib.rs`
- `crates/function_attrs_follow_docs/src/driver.rs`
- `crates/function_attrs_follow_docs/src/tests/order_detection.rs`
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`
- `docs/roadmap.md`

The public surface should look close to this, even if the exact names change
slightly during implementation.

```plaintext
// common/src/rstest/span.rs

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRecoveryFrame<T> {
    value: T,
    from_expansion: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserEditableSpan<T> {
    Direct(T),
    Recovered(T),
    MacroOnly,
}

pub fn recover_user_editable_span<T: Clone>(
    frames: &[SpanRecoveryFrame<T>],
) -> UserEditableSpan<T>;

// src/hir.rs

pub fn span_recovery_frames(span: Span) -> Vec<SpanRecoveryFrame<Span>>;

pub fn recover_user_editable_hir_span(span: Span) -> Option<Span>;
```

The pure policy should stay deliberately small. The
`recover_user_editable_span` function should not know anything about
`rustc_span::Span`, `LateContext`, or `rstest`; it should only care about the
ordered frame list and whether each frame still comes from an expansion.

The adapter in `src/hir.rs` should build that ordered frame list by walking the
source-callsite chain until one of these stop conditions occurs:

1. the next span is dummy,
2. the next span is identical to the current span, or
3. the chain has already reached a non-expansion user span and no more
   fallbacks are needed.

The first adopter should switch from direct `source_callsite()` usage to the
shared adapter. For `function_attrs_follow_docs`, that means:

1. use the shared helper in `source_order_key`,
2. use the shared helper in `attribute_within_item`, and
3. drop attributes from the comparison set when the helper reports
   `MacroOnly`.

That adoption keeps the task grounded without prematurely implementing 8.2.3’s
call-site diagnostics.

## Implementation stages

### Stage B: define the contract with tests first

Start with failing tests, not helper code.

In `common/src/rstest/tests.rs`, add unit cases for:

- a direct non-expansion frame returning `Direct`,
- a macro frame followed by a user frame returning `Recovered`,
- multiple nested macro frames still recovering the first user frame,
- an empty frame list returning `MacroOnly`,
- an all-expansion frame list returning `MacroOnly`.

In `common/tests/rstest_span_recovery_behaviour.rs` plus
`common/tests/features/rstest_span_recovery.feature`, add `rstest-bdd` v0.5.0
scenarios that describe the same contract in user terms:

- “a direct user-editable span is kept”,
- “a nested macro chain recovers the invocation site”,
- “macro-only glue is skipped”,
- “the first non-expansion frame wins even when later frames also qualify”.

Keep the world pure. The behavioural harness should use simple structs or
`SourceSpan` values rather than real `rustc` spans.

### Stage C: implement the pure shared policy

Add `common/src/rstest/span.rs`, re-export it from `common/src/rstest/mod.rs`,
and surface the new types from `common/src/lib.rs`.

The algorithm should be intentionally boring.

1. Walk the frame slice from left to right.
2. Return `Direct` for the first frame if it is not from expansion.
3. Return `Recovered` for the first later frame that is not from expansion.
4. Return `MacroOnly` when no such frame exists.

Document the API with Rustdoc examples that mirror the unit tests. Keep the
examples short and use the existing `SourceSpan` helpers where that improves
clarity.

### Stage D: implement the `rustc` adapter

Extend `src/hir.rs` with helpers that convert a `rustc_span::Span` into the
ordered frame list expected by the pure policy.

The adapter should:

1. skip dummy spans instead of pushing invalid frames,
2. record whether each frame still originates from expansion,
3. stop when `source_callsite()` stops making progress, and
4. expose a single convenience function that returns `Option<Span>` for lint
   drivers that only need “emit or skip”.

Re-export the adapter from `src/lib.rs` behind the `dylint-driver` feature so
lint crates can call it through the existing `whitaker` facade.

### Stage E: adopt the helper in one real consumer

Update `crates/function_attrs_follow_docs/src/driver.rs` to use the shared
adapter.

The driver currently normalizes spans with raw `source_callsite()` calls in two
places. Replace those with the shared helper so the lint:

- still sorts real attributes by user source order,
- still checks whether an attribute belongs to the current item, and
- now drops macro-only synthetic attributes instead of trying to compare them.

Add a regression test in
`crates/function_attrs_follow_docs/src/tests/order_detection.rs` or a nearby
test file that exercises the proc-macro fixture path already used by that
crate. The test should prove that a recoverable call-site still participates in
ordering, while purely synthetic glue does not trigger a false diagnostic.

### Stage F: update design docs and roadmap

Once code and tests pass, append an “Implementation decisions (8.1.2)” section
to `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.

Capture at least these decisions if they remain true after implementation:

- the pure/common versus `rustc` adapter split,
- the explicit `macro-only` outcome,
- the stop conditions used for the call-site walk, and
- the chosen early adopter.

Then mark 8.1.2 done in `docs/roadmap.md`. Do not do that earlier.

## Validation and evidence

Use targeted commands during development, then run the full gates before the
turn ends. Because command output is truncated in this environment, always use
`tee` plus `set -o pipefail`.

Targeted inner-loop commands:

```plaintext
set -o pipefail
cargo test -p whitaker-common rstest:: 2>&1 | tee /tmp/8-1-2-common-unit.log

set -o pipefail
cargo test -p whitaker-common --test rstest_span_recovery_behaviour \
  2>&1 | tee /tmp/8-1-2-common-bdd.log

set -o pipefail
cargo test -p function_attrs_follow_docs --all-features \
  2>&1 | tee /tmp/8-1-2-function-attrs.log
```

Required end-of-turn gates:

```plaintext
set -o pipefail
make fmt 2>&1 | tee /tmp/8-1-2-fmt.log

set -o pipefail
make markdownlint 2>&1 | tee /tmp/8-1-2-markdownlint.log

set -o pipefail
make nixie 2>&1 | tee /tmp/8-1-2-nixie.log

set -o pipefail
make check-fmt 2>&1 | tee /tmp/8-1-2-check-fmt.log

set -o pipefail
make lint 2>&1 | tee /tmp/8-1-2-lint.log

set -o pipefail
make test 2>&1 | tee /tmp/8-1-2-test.log
```

Success criteria for the full validation step:

- `make markdownlint`, `make nixie`, and `make check-fmt` exit cleanly,
- `make lint` produces no warnings promoted to errors,
- `make test` finishes with zero failed tests, and
- the new unit and behavioural tests would fail on the pre-change code and
  pass after the helper lands.

## Outcomes & Retrospective

Not yet implemented. This section must be rewritten during execution with the
final result, the validation evidence, any scope adjustments, and the lessons
that future roadmap items 8.2.x, 8.3.x, and 8.4.x should inherit.
