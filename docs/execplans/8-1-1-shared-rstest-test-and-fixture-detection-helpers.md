# Add shared `rstest` test and fixture detection helpers (roadmap 8.1.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

This document must be maintained in accordance with `AGENTS.md`. The canonical
plan file is
`docs/execplans/8-1-1-shared-rstest-test-and-fixture-detection-helpers.md`.

## Purpose / big picture

Roadmap item 8.1.1 adds the shared detection layer that later `rstest` hygiene
lints depend on. After this change, the `common` crate will expose a small
pure-library API that can answer four questions without depending on
`rustc_private`:

1. Does this function look like an `#[rstest]` test?
2. Does this function look like an `#[fixture]`?
3. Which function parameters are fixture locals, and which are provider-driven
   inputs such as `#[case]` or `#[values]`?
4. When direct attributes are unavailable, does optional expansion-trace
   metadata still show that the item came from `rstest`?

This task is intentionally foundational. It does not implement lint A itself,
and it does not add argument fingerprinting, user-editable span recovery, or
crate-post aggregation. Those belong to roadmap items 8.1.2, 8.1.3, and 8.2.x.

Success is observable when:

1. `common` exports a dedicated shared module for `rstest` detection.
2. Unit tests cover attribute-based detection, fixture-local classification,
   disabled fallback behaviour, enabled fallback behaviour, and unhappy paths.
3. Behavioural tests using `rstest-bdd` v0.5.0 exercise the same contract via
   `common/tests/`.
4. `docs/lints-for-rstest-fixtures-and-test-hygiene.md` records the final
   implementation decisions for 8.1.1.
5. `docs/roadmap.md` marks 8.1.1 done only after the implementation, tests,
   and gates all succeed.
6. `make check-fmt`, `make lint`, and `make test` pass at the end of the
   implementation turn.

## Constraints

- Scope only roadmap item 8.1.1. Do not implement lint crates, argument
  fingerprinting, paragraph detection, diagnostics, or UI fixtures here.
- Keep the new helpers in `common` and keep them free of `rustc_private`
  dependencies. The caller must pass simplified metadata rather than
  `rustc_hir` or `rustc_span` types.
- Attribute-based detection is the primary path. Expansion-trace fallback must
  be optional and explicit so future lint drivers can keep the conservative
  default.
- Do not reuse the existing generic test-like path set as the final `rstest`
  answer. `common::attributes::TEST_LIKE_PATHS` intentionally treats `case` and
  `rstest::case` as test-like for broader context detection, but 8.1.1 needs a
  stricter distinction between test attributes, fixture attributes, and
  provider parameter attributes.
- Version one accepts only simple identifier parameter bindings for
  fixture-local classification. Destructuring patterns must be treated as
  unsupported and left for a later roadmap item.
- Keep every source file under 400 lines. Split the module early instead of
  allowing a single file to absorb all logic and tests.
- Public APIs added to `common` require Rustdoc comments with examples that
  compile under the doctest model described in `docs/rust-doctest-dry-guide.md`.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros` at
  `0.5.0`.
- Behaviour tests must obey the workspace Clippy threshold of 4 arguments per
  step function. Each step may parse at most 3 values in addition to the world
  fixture.
- Do not mark roadmap item 8.1.1 done until implementation, documentation,
  and all quality gates succeed.

## Tolerances

- Scope: if implementation needs more than 9 touched files or roughly 800 net
  new lines, stop and escalate before continuing.
- API: if the pure helper API cannot represent expansion fallback without
  leaking `rustc_private` concepts into `common`, stop and escalate with the
  competing shapes.
- Compatibility: if delivering 8.1.1 requires changing the behaviour of
  existing generic helpers such as `is_test_fn` or `in_test_like_context`, stop
  and escalate instead of silently broadening the blast radius.
- Dependencies: if a new crate appears necessary, stop and escalate before
  adding it.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix iterations, stop and escalate with the captured logs.

## Risks

- Semantics risk: `rstest` has overlapping concepts across test functions,
  fixtures, and provider parameters. If the API is too loose, lint A will later
  misclassify `#[case]` inputs as fixtures. Mitigation: model test, fixture,
  and provider detection as separate predicates with separate path sets.
- Fallback risk: expansion-trace metadata is less direct than item attributes
  and could easily become too permissive. Mitigation: keep fallback disabled by
  default and require the caller to pass an explicit trace object.
- Duplication risk: this repository already has generic attribute/context
  helpers and some crate-local HIR attribute matching. Mitigation: introduce a
  dedicated `common::rstest` module rather than mutating unrelated helpers, and
  document the division of responsibility in the design doc.
- Test ergonomics risk: BDD scenarios can trip Clippy on
  `too_many_arguments`, `expect_used`, and stale feature recompilation.
  Mitigation: keep the world small, return `Result` from fallible steps, and
  touch the `.rs` harness if a feature-file-only rerun appears stale.
- Documentation drift risk: `common/Cargo.toml` still contains a stale comment
  mentioning `rstest-bdd` 0.2.x even though the workspace uses 0.5.0.
  Mitigation: either fix the comment during implementation or call out the
  discrepancy in the design-doc decision section so future work is not misled.

## Progress

- [x] (2026-03-26) Stage A: Draft this ExecPlan and capture repository state.
- [x] (2026-03-28) Stage B: Add unit tests that define the 8.1.1 detection
  contract.
- [x] (2026-03-28) Stage C: Add `rstest-bdd` scenarios for the same contract.
- [x] (2026-03-28) Stage D: Implement the shared pure-library `rstest`
      detection module in
  `common`.
- [x] (2026-03-28) Stage E: Re-export the new API from `common/src/lib.rs` and
      add Rustdoc
  examples.
- [x] (2026-03-28) Stage F: Record implementation decisions in
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.
- [x] (2026-03-28) Stage G: Mark roadmap item 8.1.1 done.
- [x] (2026-03-28) Stage H: Run documentation gates plus `make check-fmt`,
      `make lint`, and
  `make test`.
- [x] (2026-03-28) Stage I: Finalize the living sections in this document.

## Surprises & Discoveries

- `common/src/context.rs` already exposes generic helpers such as `is_test_fn`
  and `in_test_like_context`, but those currently delegate to a broader
  test-like attribute matcher that includes `case` and `rstest::case`. That is
  helpful for broad "test-like context" reasoning, but it is too permissive for
  strict `rstest` test/fixture detection.
- `crates/no_expect_outside_tests/src/driver/tests.rs` already has crate-local
  HIR tests that reject `rstest::fixture` as a test attribute. That is a good
  signal that 8.1.1 should keep test and fixture detection separate.
- The repository already uses the right behavioural test shape for this work:
  `common/tests/context_behaviour.rs` is a small, pure, fixture-backed BDD
  harness that can be copied with minimal adaptation.
- Workspace dependencies already pin `rstest-bdd = "0.5.0"`, but
  `common/Cargo.toml` still has a stale comment mentioning `0.2.x`.
- Existing pure-library helpers for macro filtering use the
  `is_from_expansion: bool` pattern. 8.1.1 needs an analogous pure metadata
  seam for optional expansion-trace fallback.
- `rstest-bdd` binds `And` to the keyword family of the previous step. The
  fixture-local behaviour scenario therefore cannot use an `And` line to
  trigger a `#[when]` step after a `#[then]`; the harness now derives
  `fixture_local_names` lazily inside the final assertion instead.
- Targeted validation passed before the full gate run:
  `cargo test -p common rstest::`,
  `cargo test -p common --test rstest_detection_behaviour`, and
  `cargo clippy -p common --all-targets --all-features -- -D warnings`.

## Decision Log

- Decision: implement 8.1.1 as a dedicated `common::rstest` module instead of
  extending `common::context` or `common::attributes` alone. Rationale: the new
  work is narrower than generic context detection and needs stricter semantics
  around fixtures and provider parameters. Date/Author: 2026-03-26 / Codex.
- Decision: keep expansion fallback in a pure metadata type owned by `common`
  rather than by passing raw rustc spans or expansion backtraces. Rationale:
  this preserves the established "pure library in `common`, compiler-aware
  caller outside" architecture. Date/Author: 2026-03-26 / Codex.
- Decision: include fixture-local parameter classification in 8.1.1 even
  though the roadmap title mentions only test and fixture detection. Rationale:
  the linked lint-A design section defines fixture-local classification as part
  of the same shared foundation, and 8.2.2 depends directly on it. Date/Author:
  2026-03-26 / Codex.
- Decision: keep destructuring support out of scope for version one. Rationale:
  the design document already permits deferring it, and simple identifier-only
  binding keeps the API deterministic and easy to test. Date/Author: 2026-03-26
  / Codex.

## Context and orientation

### Repository state

Relevant files and modules today:

- `common/src/attributes/` contains the generic `Attribute`, `AttributePath`,
  and helper predicates for outer/doc/test-like attributes.
- `common/src/context.rs` contains higher-level test-context helpers built on
  those generic attributes.
- `common/src/lib.rs` re-exports both modules and is the place where the new
  `rstest` API must become public.
- `common/tests/context_behaviour.rs` and
  `common/tests/features/context_detection.feature` provide the closest
  behavioural-test template for this work.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` defines the contract for
  8.1.1, especially the `#[rstest]` test detection and fixture-local
  classification rules under Lint A.
- `docs/roadmap.md` lists 8.1.1 as the prerequisite for 8.1.3, 8.2.1, and
  8.4.1.

### What 8.1.1 must provide

The design document narrows this task to the reusable detection substrate
needed by later lints:

- strict detection of `#[rstest]` tests,
- strict detection of `#[fixture]` functions,
- classification of parameters into fixture locals or provider-driven inputs,
- optional fallback through expansion-trace metadata when direct attributes are
  unavailable.

This task does not need user-editable span recovery or call-site
fingerprinting. Those are separate roadmap items.

## Proposed implementation shape

Create a new shared module tree:

- `common/src/rstest/mod.rs`
- `common/src/rstest/detection.rs`
- `common/src/rstest/parameter.rs`
- `common/src/rstest/tests.rs`

Then update:

- `common/src/lib.rs`
- `common/tests/rstest_detection_behaviour.rs`
- `common/tests/features/rstest_detection.feature`
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`
- `docs/roadmap.md`

The exact split may vary to stay under the 400-line limit, but the public API
should look close to this:

```rust
use crate::attributes::{Attribute, AttributePath};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpansionTrace {
    frames: Vec<AttributePath>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterBinding {
    Ident(String),
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RstestParameter {
    binding: ParameterBinding,
    attributes: Vec<Attribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RstestParameterKind {
    FixtureLocal { name: String },
    Provider,
    UnsupportedPattern,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RstestDetectionOptions {
    provider_param_attributes: Vec<AttributePath>,
    use_expansion_trace_fallback: bool,
}

pub fn is_rstest_test(attrs: &[Attribute]) -> bool;
pub fn is_rstest_test_with(
    attrs: &[Attribute],
    trace: Option<&ExpansionTrace>,
    options: &RstestDetectionOptions,
) -> bool;
pub fn is_rstest_fixture(attrs: &[Attribute]) -> bool;
pub fn is_rstest_fixture_with(
    attrs: &[Attribute],
    trace: Option<&ExpansionTrace>,
    options: &RstestDetectionOptions,
) -> bool;
pub fn classify_rstest_parameter(
    parameter: &RstestParameter,
    options: &RstestDetectionOptions,
) -> RstestParameterKind;
pub fn fixture_local_names(
    parameters: &[RstestParameter],
    options: &RstestDetectionOptions,
) -> std::collections::BTreeSet<String>;
```

Important semantic rules for the implementation:

1. Test attributes match only `rstest` and `rstest::rstest`.
2. Fixture attributes match only `fixture` and `rstest::fixture`.
3. Provider attributes default to both bare and namespaced forms of `case`,
   `values`, `files`, `future`, and `context`.
4. Expansion fallback only runs when
   `options.use_expansion_trace_fallback == true`.
5. Expansion fallback should consult the same strict path sets as direct
   attribute matching.
6. Unsupported bindings must not be classified as fixture locals.

## Plan of work

### Stage B: Add failing unit tests first

Create `common/src/rstest/tests.rs` and write unit tests before the
implementation lands. The tests should fail because the new module does not yet
exist or because stubbed functions return placeholder values.

Cover at least these cases:

- `is_rstest_test` returns true for `rstest`.
- `is_rstest_test` returns true for `rstest::rstest`.
- `is_rstest_test` returns false for `test`, `tokio::test`, `case`, and
  `rstest::fixture`.
- `is_rstest_fixture` returns true for `fixture` and `rstest::fixture`.
- `is_rstest_fixture` returns false for `rstest` and unrelated attributes.
- `classify_rstest_parameter` returns `FixtureLocal` for a simple identifier
  with no provider attribute.
- `classify_rstest_parameter` returns `Provider` for `case`, `values`, `files`,
  `future`, and `context`.
- `classify_rstest_parameter` returns `UnsupportedPattern` for a non-identifier
  binding.
- Fallback disabled means trace metadata is ignored.
- Fallback enabled means a trace containing `rstest` or `fixture` is honoured.
- `fixture_local_names` collects only supported fixture-local identifiers and
  returns deterministic `BTreeSet` ordering.

Keep test helpers small. If the cases become argument-heavy, use compact helper
structs instead of wide `#[case]` signatures to stay below the Clippy threshold.

### Stage C: Add failing behavioural tests

Create `common/tests/rstest_detection_behaviour.rs` and
`common/tests/features/rstest_detection.feature`.

Use the `common/tests/context_behaviour.rs` pattern:

- A small world fixture storing attributes, parameters, optional expansion
  trace, options, and the most recent evaluation result.
- `#[given]` steps that configure function attributes, parameter shapes, and
  fallback options.
- `#[when]` steps that run the detection helper under test.
- `#[then]` steps that assert either positive or negative outcomes.
- Indexed `#[scenario(path = ..., index = N)]` bindings.

The feature file should cover the contract at the behaviour level, not every
unit edge case. A good first set is:

1. Detect an `rstest` test from a direct attribute.
2. Detect an `rstest::fixture` fixture from a direct attribute.
3. Treat a plain identifier parameter as a fixture local.
4. Treat a `#[case]` parameter as provider-driven, not fixture-local.
5. Ignore unsupported parameter bindings.
6. Ignore expansion trace metadata while fallback is disabled.
7. Detect an `rstest` test from expansion trace metadata when fallback is
   enabled.

If a step might fail because the world is not configured, make it return
`Result<(), String>` or `Result<T, String>` instead of using `.expect()`.

### Stage D: Implement the shared `common::rstest` module

Implement the module in small files:

- `mod.rs` should define the public surface and re-export sibling modules.
- `detection.rs` should hold strict path matching and the pure expansion-trace
  fallback logic.
- `parameter.rs` should hold `ParameterBinding`, `RstestParameter`,
  `RstestParameterKind`, and the fixture-local classification helpers.
- `tests.rs` should contain the inline unit coverage declared in Stage B.

Keep the design purely data-driven:

- Use `AttributePath` from `common::attributes` for all matching.
- Use plain owned strings for identifier bindings.
- Keep defaults deterministic and visible through a small options type.
- Avoid hidden globals or ambient configuration.

### Stage E: Re-export and document the API

Update `common/src/lib.rs` to export the new module and its public types and
functions.

Add Rustdoc examples for each public constructor or predicate. The examples
must compile as external doctests, so they should use only exported `common`
types and must not rely on crate-private helpers.

### Stage F: Update the design document

Append a short `Implementation decisions (8.1.1)` section to
`docs/lints-for-rstest-fixtures-and-test-hygiene.md`.

Record the final decisions taken during delivery, especially:

- why a dedicated `common::rstest` module was chosen,
- which exact attribute paths count as tests, fixtures, and providers,
- how expansion-trace fallback is modelled in a pure way,
- why unsupported/destructured bindings are deferred.

If the stale `common/Cargo.toml` `rstest-bdd` comment is corrected during the
implementation turn, mention that as documentation hygiene rather than as a
feature decision.

### Stage G: Mark the roadmap item done

After the code, tests, and docs are complete, change `docs/roadmap.md` entry
8.1.1 from `[ ]` to `[x]`.

Do not mark 8.1.1 done earlier than the successful gate run.

### Stage H: Validate the change

Run documentation and code gates with `tee` and `set -o pipefail` so failures
are visible even when output is truncated:

```plaintext
set -o pipefail && make fmt 2>&1 | tee /tmp/8-1-1-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/8-1-1-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/8-1-1-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/8-1-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/8-1-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/8-1-1-test.log
```

For faster local iteration before the final gate, targeted commands are
acceptable, for example:

```plaintext
cargo test -p common rstest::
cargo test -p common --test rstest_detection_behaviour
cargo clippy -p common --all-targets --all-features -- -D warnings
```

Remember that `make test` can run for a long time with sparse output near the
end. Poll rather than assuming it has hung.

## Acceptance checklist

The implementation is complete only when all of the following are true:

1. `common` exposes a strict `rstest` detection API that is independent of
   `rustc_private`.
2. Direct attributes and optional expansion-trace fallback both work exactly as
   documented.
3. Provider parameters are not misclassified as fixture locals.
4. Unsupported/destructured bindings stay out of fixture-local sets.
5. Unit tests and `rstest-bdd` behaviour tests both pass.
6. The design doc records the 8.1.1 decisions.
7. The roadmap entry is marked done.
8. `make check-fmt`, `make lint`, and `make test` pass, with documentation
   gates passing as well because the implementation updates Markdown files.

## Outcomes & Retrospective

- Final file list:
  `common/src/lib.rs`, `common/src/rstest/mod.rs`,
  `common/src/rstest/detection.rs`, `common/src/rstest/parameter.rs`,
  `common/src/rstest/tests.rs`, `common/tests/rstest_detection_behaviour.rs`,
  `common/tests/features/rstest_detection.feature`, `common/Cargo.toml`,
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md`, `docs/roadmap.md`,
  `docs/execplans/8-1-1-shared-rstest-test-and-fixture-detection-helpers.md`.
- Accepted API surface:
  `ExpansionTrace`, `RstestDetectionOptions`, `is_rstest_test`,
  `is_rstest_test_with`, `is_rstest_fixture`, `is_rstest_fixture_with`,
  `ParameterBinding`, `RstestParameter`, `RstestParameterKind`,
  `classify_rstest_parameter`, and `fixture_local_names`, all re-exported from
  `common`.
- Behaviour scenarios added:
  direct `rstest` test detection, direct `rstest::fixture` detection,
  fixture-local identifier classification, provider classification for
  `#[case]`, unsupported binding handling, fallback-disabled trace ignoring,
  and fallback-enabled trace detection.
- Deviation from the draft plan:
  the behaviour harness computes `fixture_local_names` lazily inside the final
  assertion instead of using a separate `When` step because `rstest-bdd`
  carries `And` into the prior keyword family.
- Final gate results:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test` all passed on 2026-03-28. `make test` finished with
  `Summary [123.240s] 1186 tests run: 1186 passed, 2 skipped`.
- Follow-up work:
  8.1.2 can now consume the strict `rstest` detection and `ExpansionTrace` seam
  for user-editable span recovery, and 8.1.3 can layer argument fingerprinting
  on top of `RstestParameterKind` without needing to re-solve
  provider-versus-fixture classification.
