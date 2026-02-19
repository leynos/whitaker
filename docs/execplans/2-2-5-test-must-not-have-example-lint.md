# Implement `test_must_not_have_example` lint (roadmap 2.2.5)

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

The canonical plan file is
`docs/execplans/2-2-5-test-must-not-have-example-lint.md`.

## Purpose / big picture

Whitaker currently ships lints for attribute order, `expect`, module docs,
predicate branching, module length, panic fallbacks, and `std::fs` usage.
Roadmap item 2.2.5 tracks delivery of `test_must_not_have_example`.

After this change, test-like functions (`#[test]`, `#[tokio::test]`,
`#[rstest]`, and recognised equivalents) will trigger a warning when their
documentation contains either an Examples heading or a fenced code block. This
preserves readability goals by keeping test docs focused on intent rather than
user examples.

The test-context predicate must be shared with `no_expect_outside_tests` so
Whitaker maintains one canonical definition of what counts as a test.

Success is observable when:

1. `crates/test_must_not_have_example/` exists and exports a working Dylint
   lint.
2. UI fixtures show warning diagnostics for unhappy paths and clean output for
   happy paths.
3. Unit tests and behavioural tests (`rstest-bdd` v0.5.0) cover happy, unhappy,
   and edge cases for heading/code-fence heuristics.
4. `docs/whitaker-dylint-suite-design.md` records the implementation decisions.
5. `docs/roadmap.md` marks 2.2.5 as done.
6. `make check-fmt`, `make lint`, and `make test` pass.

## Constraints

- Implement as a dedicated lint crate at
  `crates/test_must_not_have_example/` following existing lint-crate patterns.
- Keep file sizes under 400 lines by splitting helpers/tests into focused
  modules where required.
- Preserve existing public interfaces of current crates unless a local,
  additive change is needed for suite registration.
- Reuse existing test-context detection logic from `common` and
  `no_expect_outside_tests`; do not introduce a second independent matrix of
  recognised test attributes.
- If reuse requires moving logic, extract shared helpers into `common` rather
  than copying implementation details into the new crate.
- Use workspace-pinned dependencies; `rstest-bdd` and `rstest-bdd-macros` must
  remain at `0.5.0` (already pinned in workspace `Cargo.toml`).
- Provide both unit tests and behavioural tests for happy/unhappy/edge paths.
- Add or update UI tests under the new crate to validate user-visible lint
  diagnostics.
- Record design decisions taken during implementation in
  `docs/whitaker-dylint-suite-design.md`.
- On completion, update `docs/roadmap.md` entry 2.2.5 to `[x]`.
- Keep comments/docs in en-GB-oxendict spelling and wrap Markdown prose at
  80 columns.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 16 files or 550 net
  lines, stop and escalate.
- Interface: if enabling the lint requires breaking changes to suite or
  installer public interfaces, stop and escalate.
- Dependencies: if any new external dependency is needed, stop and escalate.
- Reuse boundary: if a canonical shared helper cannot be used without changing
  lint semantics unexpectedly, stop and escalate with options.
- Heuristic ambiguity: if heuristic choices materially change which docs are
  flagged and requirements are unclear, stop and present concrete options.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail after
  3 targeted fix attempts, stop and escalate with logs.

## Risks

- Risk: simplistic code-fence detection could over-report prose containing
  literal backticks.
  - Severity: medium.
  - Likelihood: medium.
  - Mitigation: use line-anchored fence detection heuristics and add edge-case
    tests for inline/backslash-escaped patterns.

- Risk: relying only on `# Examples` may under-detect headings that use
  alternative casing or heading levels.
  - Severity: medium.
  - Likelihood: medium.
  - Mitigation: define explicit heading-matching behaviour in a pure helper and
    lock it with unit + BDD cases.

- Risk: suite/integration wiring may omit the new crate, leaving the lint
  unavailable in aggregated flows.
  - Severity: high.
  - Likelihood: medium.
  - Mitigation: update `suite/Cargo.toml`, `suite/src/lints.rs`, and any
    packaging lists that enumerate standard lint crates.

- Risk: duplicated test detection logic diverges over time and causes lints to
  disagree about whether code is a test context.
  - Severity: high.
  - Likelihood: medium.
  - Mitigation: enforce shared helper usage and add parity tests that lock
    expected outcomes for representative attribute/module/file contexts.

## Progress

- [x] (2026-02-18) Draft ExecPlan for roadmap item 2.2.5.
- [x] Stage A: Scaffold crate and workspace/suite wiring.
- [x] Stage B: Implement detection heuristics in pure helper modules.
- [x] Stage C: Implement lint driver and diagnostics.
- [x] Stage D: Add unit and behavioural tests (`rstest-bdd` v0.5.0).
- [x] Stage E: Add UI tests for happy and unhappy diagnostics.
- [x] Stage F: Update design and user-facing docs.
- [x] Stage G: Run quality gates and capture logs.
- [x] Stage H: Mark roadmap item 2.2.5 as done.

## Surprises & Discoveries

- The default `whitaker::declare_ui_tests!` harness does not apply per-fixture
  `.rustc-flags`, so this lint switched to a custom `lib_ui_tests.rs` runner
  that loads fixture-specific rustc flags and per-fixture `dylint.toml`.
- Because fixtures run in temporary workspaces, `BLESS=1` updates temp
  snapshots instead of source fixtures. Expected stderr files were updated from
  `*.stage-id.stderr` outputs.
- Adding a `rustc_private`-backed HIR helper to `common` caused `make test`
  linkage failures for `common` integration tests under `--all-features`.
  Resolution: keep the canonical test matrix in `common` while keeping HIR
  attribute adapters local to lint crates.

## Decision Log

- Decision: implement heading and fence checks as pure, testable helper
  functions before wiring into `LateLintPass`.
  - Rationale: this keeps heuristics deterministic, simplifies BDD coverage,
    and avoids brittle rustc-HIR-only tests for text parsing.
  - Date/Author: 2026-02-18 / Codex.

- Decision: ship the lint as a standard suite member (not experimental).
  - Rationale: roadmap 2.2.* is core lint delivery and this lint is listed as a
    core lint in the design document.
  - Date/Author: 2026-02-18 / Codex.

- Decision: keep dependencies unchanged and use workspace `rstest-bdd` 0.5.0.
  - Rationale: the required version is already pinned; adding/changing versions
    increases churn without benefit.
  - Date/Author: 2026-02-18 / Codex.

- Decision: treat test-context detection as a shared concern and avoid
  duplicate implementations between lints.
  - Rationale: conflicting test definitions would create user-visible policy
    drift and inconsistent diagnostics.
  - Date/Author: 2026-02-18 / Codex.

- Decision: keep HIR-to-path adaptation inside lint crates, but route
  classification through `common::Attribute::is_test_like_with`.
  - Rationale: this preserves one canonical attribute matrix without introducing
    `rustc_private` linkage into `common` test binaries.
  - Date/Author: 2026-02-19 / Codex.

- Decision: UI warning fixtures use
  `additional_test_attributes = ["allow"]` with `#[allow(dead_code)]`.
  - Rationale: this validates lint diagnostics deterministically in UI tests
    without relying on `#[test]` attributes that may be rewritten by harness
    lowering.
  - Date/Author: 2026-02-19 / Codex.

## Outcomes & Retrospective

Implemented and shipped.

- Final heuristic contract:
  - Flag docs containing Markdown headings `# Examples` (any heading level,
    case-insensitive, optional trailing colon).
  - Flag lines starting with fenced code markers (three or more backticks).
  - Ignore inline backticks and plain prose.
  - Evaluate in source order and report the first violation encountered.
- Test inventory:
  - Unit tests in `crates/test_must_not_have_example/src/heuristics.rs`.
  - Behaviour tests in
    `crates/test_must_not_have_example/src/behaviour.rs` with feature scenarios
    in `crates/test_must_not_have_example/tests/features/doc_examples.feature`.
  - UI tests in `crates/test_must_not_have_example/ui/` with stderr snapshots
    for unhappy cases and pass fixtures for non-violations.
- Reuse contract:
  - `no_expect_outside_tests` and `test_must_not_have_example` share the same
    canonical test-attribute matrix via common attribute classification.
  - HIR-specific adaptation remains local per lint to keep `common` free of
    `rustc_private` test-linkage hazards.
- Quality gates:
  - `make check-fmt` passed (`/tmp/2-2-5-check-fmt.log`).
  - `make lint` passed (`/tmp/2-2-5-lint.log`).
  - `make test` passed (`/tmp/2-2-5-test.log`).
- Roadmap:
  - `docs/roadmap.md` item 2.2.5 marked complete.

## Context and orientation

Current repository state relevant to this task:

- `docs/roadmap.md` marks 2.2.5 as not done.
- `suite/src/lints.rs` and `suite/Cargo.toml` do not reference
  `test_must_not_have_example` yet.
- Existing lint crates show two common patterns:
  - simple `src/lib.rs` with `driver` module and optional `stub` path;
  - pure helper logic plus `#[cfg(test)]` unit and BDD modules (often with
    feature files under `tests/features/`).
- Existing reusable detection helpers already exist:
  - `common::context::{is_test_fn_with, in_test_like_context_with}`;
  - `crates/no_expect_outside_tests/src/context.rs` for HIR ancestor collection
    and `cfg(test)` handling (`collect_context`, `summarise_context`).
- Existing BDD tests use `rstest_bdd_macros::{given, when, then, scenario}` and
  are executed under `cargo test`.

Likely files to add or modify:

- `crates/test_must_not_have_example/Cargo.toml` (new)
- `crates/test_must_not_have_example/src/lib.rs` (new)
- `crates/test_must_not_have_example/src/driver.rs` (new)
- `crates/test_must_not_have_example/src/<heuristic_module>.rs` (new)
- `crates/test_must_not_have_example/tests/features/<feature>.feature` (new)
- `crates/test_must_not_have_example/ui/*.rs` and `*.stderr` (new)
- `suite/Cargo.toml`
- `suite/src/lints.rs`
- `Makefile` (if `LINT_CRATES` list must include the new core lint)
- `common/src/context.rs` and/or a new shared context collector module in
  `common` (if extraction from `no_expect_outside_tests` is required)
- `docs/whitaker-dylint-suite-design.md`
- `docs/roadmap.md`
- `README.md` and/or `docs/users-guide.md` if current text still says this lint
  is planned rather than shipped.

## Plan of work

### Stage A: Scaffold crate and wire suite membership

Create `crates/test_must_not_have_example/` using the established lint crate
layout (`cdylib` + `rlib`, `dylint-driver` feature, `constituent` feature,
workspace dependencies, test dependencies including `rstest-bdd`).

Before coding lint behaviour, select and document the reuse path for
test-context detection:

- preferred: consume an existing shared helper from `common`;
- fallback: extract reusable context collection from
  `no_expect_outside_tests` into `common` and migrate both lints.

Wire the crate into aggregated delivery:

- add dependency and feature linkage in `suite/Cargo.toml`.
- add descriptor and declaration entries in `suite/src/lints.rs`.
- update any crate enumeration lists used for packaging/release if the list is
  expected to include all standard lints.

Acceptance for Stage A: workspace resolves and compiles with the new crate
included in the suite wiring (even with placeholder logic), and a single
canonical test-context helper is identified.

### Stage B: Implement doc-text heuristics as pure logic

Create a pure helper module that accepts collected doc text and returns a
classification describing whether disallowed content exists, plus why.

Implement and document the heuristic contract explicitly, including:

- what qualifies as an Examples heading;
- what qualifies as a fenced code block start;
- how whitespace/casing is treated;
- what patterns are intentionally out of scope.

Keep this logic independent of rustc traversal so it can be heavily unit tested
and exercised via BDD scenarios.

Acceptance for Stage B: helper functions compile and expose predictable,
documented outcomes for heading/fence checks, without embedding test-context
classification rules.

### Stage C: Implement lint pass and diagnostics

Implement `LateLintPass` in `src/driver.rs` to:

- recognise test-like functions through the shared canonical helper path;
- collect doc comments/attributes for each candidate function;
- pass normalised doc text into Stage B helpers;
- emit a warning for disallowed Examples/fence content using localised messages
  from the existing Fluent bundles.

Reuse established localisation flow (`get_localizer_for_lint`, fallback-safe
message resolution) and include the offending test/function name in diagnostic
arguments where useful.

Acceptance for Stage C: lint emits stable diagnostics for clear positive cases
and remains silent for clear negative cases, while sharing the same
test-context semantics used by `no_expect_outside_tests`.

### Stage D: Unit tests and BDD behavioural tests (`rstest-bdd` v0.5.0)

Add unit tests for heuristic helpers and policy decisions. Cover at least:

- happy paths: no heading/fence; non-test context ignored.
- unhappy paths: Examples heading found; fenced block found.
- edge paths: inline backticks without fences, heading variants, empty docs,
  unmatched fence lines.
- parity paths: representative attribute/module/file contexts align with the
  shared test-context helper used by `no_expect_outside_tests`.

Add behavioural coverage with `rstest-bdd` v0.5.0 using feature files and step
bindings. Include scenarios that mirror the above happy/unhappy/edge classes so
behaviour remains understandable to reviewers.

Acceptance for Stage D: unit + BDD tests run under `cargo test` and capture the
agreed heuristic contract.

### Stage E: UI tests for observable lint behaviour

Add UI fixtures under `crates/test_must_not_have_example/ui/` for:

- failing test docs with Examples heading;
- failing test docs with fenced code;
- passing test docs without examples;
- passing non-test docs that include examples (if policy is test-only).

Generate and verify `.stderr` outputs via the existing UI harness pattern.

Acceptance for Stage E: UI tests prove diagnostics and spans for both failing
and passing source examples.

### Stage F: Document decisions and user-facing status

Update `docs/whitaker-dylint-suite-design.md` section 3.6 with the final
heuristic decisions and any explicit limitations.

If applicable after implementation, update:

- `README.md` project status text (remove this lint from "planned").
- `docs/users-guide.md` lint catalogue/configuration text for the new lint.

Acceptance for Stage F: design doc clearly records what was implemented and why.

### Stage G: Run quality gates and record evidence

Run required checks with `tee` and `set -o pipefail`:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/2-2-5-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/2-2-5-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/2-2-5-test.log

If any command fails, fix and rerun until all pass or a tolerance trigger is
hit.

Acceptance for Stage G: all three commands exit successfully with logs retained
for review.

### Stage H: Roadmap completion update

After all implementation and validation stages succeed, mark roadmap item 2.2.5
as complete in `docs/roadmap.md`:

- change
  `- [ ] 2.2.5. Implement test_must_not_have_example covering code-fence heuristics.`
   to `[x]`.

Acceptance for Stage H: roadmap reflects the shipped state.

## Validation and acceptance checklist

The feature is complete only when all are true:

- `test_must_not_have_example` lint crate exists and is wired into suite build.
- Test-context determination reuses the canonical shared helper path and does
  not define a separate attribute matrix.
- Unit tests cover happy/unhappy/edge heuristic behaviour.
- Behaviour tests use `rstest-bdd` v0.5.0 and cover the same behavioural
  contract.
- UI tests demonstrate failing and passing source cases.
- `docs/whitaker-dylint-suite-design.md` records design decisions.
- `docs/roadmap.md` marks 2.2.5 as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Idempotence and recovery

- Stages are additive and safe to rerun.
- If UI expectations drift, regenerate only affected `.stderr` files and rerun
  the stage.
- If a heuristic decision changes mid-implementation, update `Decision Log`,
  adjust tests first, then code.
- If tolerance thresholds are exceeded, stop, document the issue, and escalate.

## Revision note

- 2026-02-18: Initial draft created for roadmap task 2.2.5.
- 2026-02-18: Updated to require maximal reuse for test-context detection and
  prevent divergent definitions across lints.
- 2026-02-19: Completed implementation, validation, and roadmap/status updates.
