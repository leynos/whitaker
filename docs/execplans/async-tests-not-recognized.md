# Preserve parsed builtin test attributes for async test detection

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / Big Picture

Whitaker currently recognises `#[tokio::test]` only when the generated builtin
test marker survives HIR lowering as an unparsed attribute path. In real
consumer crates, Tokio generates a sync wrapper with
`#[::core::prelude::v1::test]`, and Whitaker can lose that marker before the
`no_expect_outside_tests` lint classifies the enclosing function. The user
visible result is a false positive against Tokio's own generated
`Runtime::build().expect(...)` wrapper, even though the source function is a
test.

After this change, Whitaker must preserve parsed builtin test attributes during
HIR conversion, treat the Tokio-generated wrapper as test-only code, and keep a
regression suite that fails if parsed builtin test markers are dropped again.
Success is observable when the regression tests cover the parsed builtin case,
the existing Tokio UI fixture still passes, and a real `#[tokio::test]`
reproduction no longer trips `no_expect_outside_tests`.

## Constraints

- Keep the work scoped to test detection for `no_expect_outside_tests` and the
  shared HIR attribute helpers it depends on.
- Preserve existing behaviour for direct source-written test markers such as
  `#[test]`, `#[rstest]`, `#[tokio::test]`, and `cfg(test)` module detection.
- Do not change Corbusier code as part of this plan; Corbusier is only the
  reproducer.
- Do not add new external dependencies or build tooling.
- Keep documentation in en-GB Oxford spelling and wrap prose at 80 columns.
- Run repository validation through Makefile targets or existing documented
  commands, capturing logs with `tee`.
- Do not begin implementation until the user explicitly approves this plan.

## Tolerances (Exception Triggers)

- Scope: if the fix needs changes in more than 8 files or more than 300 net
  lines, stop and escalate.
- Interface: if preserving parsed builtin test attributes requires a public API
  change in `common` or another shared crate, stop and escalate.
- Compiler model: if the available HIR API cannot surface a recoverable path or
  builtin marker for parsed test attributes, stop and document the exact rustc
  limitation before proceeding.
- Test harness: if reproducing the real Tokio case requires replacing the UI
  test strategy with a heavier end-to-end harness, stop and confirm the added
  maintenance cost.
- Validation: if `make lint` or the focused lint-crate tests fail twice after
  targeted fixes, stop and escalate with log paths.

## Risks

- Risk: parsed builtin test attributes may not expose the same path metadata as
  unparsed attributes, making preservation more subtle than a direct path copy.
  - Severity: high
  - Likelihood: medium
  - Mitigation: inspect the HIR API first, document the exact representation,
    and add unit coverage for the recovered form before updating lint logic.
- Risk: fixing only `no_expect_outside_tests` could leave other shared users of
  `whitaker::hir` with inconsistent attribute behaviour.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: route preservation through shared helpers where practical and
    re-run focused consumers that rely on test-like attribute detection.
- Risk: the current UI fixture uses an auxiliary proc-macro stub that simulates
  only the token output shape, not the real compiler representation.
  - Severity: high
  - Likelihood: high
  - Mitigation: add at least one regression that exercises the parsed builtin
    attribute path or a real Tokio expansion path, not only the stub.

## Progress

- [x] (2026-03-29 21:35Z) Draft ExecPlan capturing the parsed builtin attribute
  regression and the required regression-test work.
- [ ] Confirm the exact parsed HIR representation used for Tokio-generated
  builtin test attributes on `nightly-2025-09-18`.
- [ ] Preserve parsed builtin test markers in shared HIR conversion.
- [ ] Update `no_expect_outside_tests` context handling to consume the
  preserved parsed builtin test attribute.
- [ ] Add regression tests that fail before the fix and pass after it.
- [ ] Run focused and repository-level validation and record outcomes.

## Surprises & Discoveries

- Discovery: the previous Tokio fix covered the path matcher but not the HIR
  conversion layer. The UI fixture proved that `core::prelude::*::test` is
  accepted as a path, but did not prove that a real Tokio-generated builtin
  test attribute survives as an inspectable HIR attribute.
- Discovery: the current driver tests explicitly treat parsed attributes as
  non-test markers, which explains why the blind spot persisted after the
  earlier path-matcher fix.

## Decision Log

- Decision: target parsed builtin attribute preservation rather than adding more
  fallback heuristics based on file names or test harness context. Rationale:
  the false positive is rooted in lost attribute information, so the durable
  fix is to preserve the compiler signal instead of guessing after the fact.
  Date/Author: 2026-03-29 / Codex.
- Decision: keep Corbusier out of the implementation scope and use it only as a
  reproducer. Rationale: Whitaker should ship the regression fix without
  coupling the change to a consumer repository. Date/Author: 2026-03-29 / Codex.

## Outcomes & Retrospective

Not started. This section must be updated after implementation with the final
behavioural result, validation evidence, and any lessons about rustc HIR
attribute handling.

## Context and Orientation

The relevant Whitaker layers are split across a shared HIR helper and the lint
crate:

- `src/hir.rs` contains `has_test_like_hir_attributes` and
  `attribute_from_hir`, which convert HIR attributes into `common::Attribute`
  instances for test-like matching.
- `common/src/attributes/attribute.rs` contains the test-like path matching,
  including the recent `core/std::prelude::*::test` support.
- `crates/no_expect_outside_tests/src/context.rs` converts ancestor attributes
  into `ContextEntry` values that drive `summarise_context`.
- `crates/no_expect_outside_tests/src/driver/mod.rs` performs the lint check
  and falls back to heuristics only when direct test detection fails.
- `crates/no_expect_outside_tests/src/driver/tests.rs` and
  `crates/no_expect_outside_tests/src/context/tests.rs` currently model test
  attributes using synthetic `hir::Attribute::Unparsed` values and treat parsed
  attrs as opaque placeholders.
- `crates/no_expect_outside_tests/ui/pass_expect_in_tokio_test.rs` uses
  `ui/auxiliary/tokio.rs`, an auxiliary proc-macro that emits only the
  prelude-qualified token form, not the real rustc-lowered representation.

External reproducer context: in Corbusier, `#[tokio::test]` expands to a sync
wrapper whose runtime builder calls `.expect("Failed building the Runtime")`.
Whitaker currently reports that generated `expect` as outside test-only code,
which means the wrapper function is no longer being recognised as a test by the
time the lint inspects HIR.

## Plan of Work

Stage A: verify the compiler representation. Confirm exactly how
`nightly-2025-09-18` represents Tokio-generated builtin test markers in HIR and
document the result in this plan. The goal is to stop guessing about whether
the generated `#[::core::prelude::v1::test]` arrives as `Parsed(_)`,
`Unparsed(_)`, or another recoverable form.

Stage B: preserve parsed builtin test attributes. Update the shared HIR helper
and the lint-context conversion path so builtin parsed test attributes are not
collapsed to `None` or the parsed placeholder path. The output must feed the
existing `common::Attribute` matcher with enough information to recognise the
attribute as test-like.

Stage C: harden regression coverage. Extend unit tests to cover the parsed
builtin case directly, not only synthetic unparsed paths. Revisit the Tokio UI
fixture so it continues to cover the path shape while an additional regression
covers the real compiler representation boundary that previously went untested.

Stage D: validate end-to-end. Run focused lint-crate tests and repository
gates, then confirm the Corbusier reproduction no longer reports Tokio's
generated runtime `.expect(...)` as non-test code.

## Concrete Steps

1. Record the current failure mode in the plan with the exact Corbusier
   diagnostic and the Tokio macro source location that emits
   `#[::core::prelude::v1::test]`.

2. Inspect the available HIR attribute API and identify how to distinguish a
   parsed builtin `#[test]` marker from unrelated parsed attrs such as
   `#[must_use]`. If the representation is not recoverable, stop and document
   that limit before coding.

3. Update `src/hir.rs` so `attribute_from_hir` preserves builtin parsed test
   markers instead of discarding every `Parsed(_)` attribute wholesale.

4. Update `crates/no_expect_outside_tests/src/context.rs` so
   `convert_attribute` preserves the same parsed builtin test marker and feeds
   it into the existing `ContextEntry` summarisation logic.

5. Extend unit coverage in:

   - `crates/no_expect_outside_tests/src/driver/tests.rs`
   - `crates/no_expect_outside_tests/src/context/tests.rs`
   - `common/src/attributes/attribute.rs` only if additional matcher coverage
     is required

   The new tests must demonstrate the parsed builtin case, not only the
   unparsed path case.

6. Rework or supplement the Tokio regression fixture so the suite catches the
   HIR-representation bug. If the auxiliary proc-macro remains useful, keep it
   for path-shape coverage and add a second regression that exercises the real
   compiler boundary.

7. Run validation with logs from the repository root:

    ```sh
    set -o pipefail; make fmt 2>&1 | tee \
      /tmp/fmt-whitaker-async-tests-not-recognized.out
    set -o pipefail; make markdownlint 2>&1 | tee \
      /tmp/markdownlint-whitaker-async-tests-not-recognized.out
    set -o pipefail; make nixie 2>&1 | tee \
      /tmp/nixie-whitaker-async-tests-not-recognized.out
    set -o pipefail; cargo test -p no_expect_outside_tests \
      --features dylint-driver driver::tests 2>&1 | tee \
      /tmp/test-no-expect-driver-whitaker-async-tests-not-recognized.out
    set -o pipefail; cargo test -p no_expect_outside_tests \
      --features dylint-driver context::tests 2>&1 | tee \
      /tmp/test-no-expect-context-whitaker-async-tests-not-recognized.out
    set -o pipefail; cargo test -p no_expect_outside_tests \
      --features dylint-driver ui:: -- --nocapture 2>&1 | tee \
      /tmp/test-no-expect-ui-whitaker-async-tests-not-recognized.out
    set -o pipefail; make check-fmt 2>&1 | tee \
      /tmp/check-fmt-whitaker-async-tests-not-recognized.out
    set -o pipefail; make lint 2>&1 | tee \
      /tmp/lint-whitaker-async-tests-not-recognized.out
    set -o pipefail; make test 2>&1 | tee \
      /tmp/test-whitaker-async-tests-not-recognized.out
    ```

8. Re-run the Corbusier reproduction against the freshly staged Whitaker suite
   and confirm that the `#[tokio::test]` functions are recognised as tests.

## Validation and Acceptance

The work is complete only when all of the following are true:

- A focused regression fails before the fix and passes after it for the parsed
  builtin test-attribute case.
- The Tokio UI regression still passes, demonstrating that the prelude-path
  matcher remains intact.
- `cargo test -p no_expect_outside_tests --features dylint-driver driver::tests`
  passes.
- `cargo test -p no_expect_outside_tests --features dylint-driver context::tests`
  passes.
- `cargo test -p no_expect_outside_tests --features dylint-driver ui:: -- --nocapture`
  passes.
- `make check-fmt` passes.
- `make lint` passes.
- `make test` passes, or any unrelated long-running failure is documented with
  logs and explicitly approved before proceeding.
- The Corbusier reproducer no longer emits
  `Avoid calling expect on std::result::Result<tokio::runtime::Runtime,`
  `std::io::Error> outside test-only code` for the affected `#[tokio::test]`
  functions.

## Approval gates

Do not start implementation when this document is first drafted. Wait for the
user to approve the plan explicitly.

Before marking the work complete, obtain confirmation that the plan still
matches the intended fix if either of these happens:

1. the HIR representation turns out not to expose a recoverable parsed builtin
   test attribute path; or
2. the regression requires a heavier end-to-end test harness than the existing
   lint-crate tests and UI fixtures.

## Idempotence and Recovery

The investigative commands in this plan are safe to re-run. If an exploratory
probe or focused test leaves scratch data in `/tmp`, it can be removed without
affecting the repository. If a coding step misclassifies parsed attributes,
revert the touched files with Git, restore the plan state, and re-run the
focused driver and context tests before attempting a different approach.

## Artifacts and Notes

- Corbusier reproducer path:
  `/data/leynos/Projects/corbusier.worktrees/enable-whitaker-linting`
- Whitaker branch for the planned fix:
  `async-tests-not-recognized`
- Existing Tokio macro source that emits the builtin test marker:
  `/home/leynos/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-macros-2.6.1/src/entry.rs`
