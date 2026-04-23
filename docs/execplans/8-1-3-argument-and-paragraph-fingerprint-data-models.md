# Add shared argument and paragraph fingerprint data models

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`. The canonical
plan file is
`docs/execplans/8-1-3-argument-and-paragraph-fingerprint-data-models.md`.

This draft must be approved before implementation begins. Do not start code
changes for roadmap item 8.1.3 until the user explicitly approves this plan.

## Purpose / big picture

Roadmap item 8.1.3 fills the remaining shared-data gap between strict `rstest`
detection and the later lint implementations. After this change, Whitaker will
have one pure, reusable place that can represent and compare the two kinds of
repeatable setup evidence the later lints need:

1. Argument fingerprints for repeated helper calls in `#[rstest]` tests
   (roadmap 8.2.x / lint A).
2. Paragraph fingerprints for repeated assertion-free setup blocks in
   `#[rstest]` tests (roadmap 8.4.x / lint C).

Success is observable when:

1. `whitaker-common` exposes public, documented argument and paragraph
   fingerprint types under `common::rstest`.
2. The paragraph API includes a deterministic local-slot normalization seam, so
   equivalent paragraphs with different local variable names compare equal.
3. Unit tests cover happy paths, unhappy paths, and determinism edges for both
   fingerprint families.
4. Behavioural tests using `rstest-bdd` v0.5.0 describe the same contracts in
   user terms through `common/tests/`.
5. `docs/lints-for-rstest-fixtures-and-test-hygiene.md` records the final
   implementation decisions for 8.1.3.
6. `docs/roadmap.md` marks 8.1.3 done only after implementation,
   documentation, and all required gates succeed.
7. The implementation turn ends with `make fmt`, `make markdownlint`,
   `make nixie`, `make check-fmt`, `make lint`, and `make test` all passing.

## Constraints

- Scope only roadmap item 8.1.3. Do not implement lint crates, HIR traversal,
  call-site collection, paragraph slicing, diagnostics, crate-post emission, or
  UI fixtures in this change.
- Keep the new fingerprint layer pure and `rustc_private`-free inside
  `whitaker-common`. Compiler-aware lowering from HIR into these models belongs
  to later roadmap items.
- Keep the public API in `common::rstest`, because 8.2.x and 8.4.x both depend
  on the same shared models and should not duplicate them in crate-local code.
- Preserve the design-doc shape from
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md` unless implementation
  proves a narrowly scoped refinement is necessary. Any refinement must be
  recorded in the design doc and in this plan's `Decision Log`.
- Determinism is part of the feature, not an implementation detail. The shared
  models must use stable ordering and stable equality semantics across runs and
  test ordering.
- Keep unsupported and unknown cases explicit in the models rather than
  silently dropping them. Later lint passes need to distinguish
  "groupable/consistent" from "present but unsupported".
- Use public constructors or builders that can be exercised from unit tests,
  doctests, and `rstest-bdd` scenarios without requiring HIR values.
- Keep every Rust source file under 400 lines. The current
  `common/src/rstest/tests.rs` is already 248 lines, so new fingerprint tests
  must be split into dedicated test modules rather than appended until the file
  violates the repository limit.
- Public APIs added to `common` require Rustdoc comments with examples that
  compile under the doctest model described in `docs/rust-doctest-dry-guide.md`.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros` at
  `0.5.0`.
- Behaviour tests must respect the workspace Clippy threshold of 4 arguments
  per step function. Each step may parse at most 3 values in addition to the
  world fixture.
- Do not mark roadmap item 8.1.3 done until implementation, design-doc
  updates, and all quality gates succeed.

## Tolerances

- Scope: if implementation needs more than 10 touched files or roughly 900 net
  new lines, stop and escalate before continuing.
- API shape: if keeping the paragraph fingerprint layer deterministic requires
  exposing raw `rustc_hir` or unstable compiler identifiers in `common`, stop
  and escalate with the competing shapes.
- Model drift: if the design-doc fingerprint sketches prove insufficient and
  the implementation needs materially different public types, stop and review
  that drift before proceeding.
- Test support: if behaviour tests cannot exercise the public fingerprint API
  without adding non-trivial test-only adapters, stop and justify that seam
  before adding it.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix iterations, stop and escalate with the saved logs.

## Risks

- Semantics risk: argument fingerprints and paragraph fingerprints are similar
  only at a distance. If they share too much internal machinery, the public API
  could become vague or over-generalized. Mitigation: keep the two model
  families sibling modules under `common::rstest`, with only the deterministic
  conventions shared.
- Determinism risk: paragraph grouping depends on local-name normalization by
  first appearance order. A leaky API could let later callers assign slots
  inconsistently and produce unstable grouping. Mitigation: put slot
  normalization inside the shared layer rather than leaving it to each future
  lint crate.
- Over-validation risk: the design sketches show mostly data models, not rich
  validation rules. If constructors reject too much, later HIR lowering may
  become awkward; if they reject too little, later lints may duplicate checks.
  Mitigation: validate only the invariants needed for determinism and expose
  explicit `Unsupported` or `Unknown` variants for everything else.
- File-size risk: the current `common::rstest` module tree is compact, but the
  test file is close enough to the 400-line cap that naive additions will
  breach it. Mitigation: split tests by topic as part of the implementation.
- BDD ergonomics risk: the repository has already hit `rstest-bdd` gotchas
  around `And` keyword binding and workspace-wide Clippy denials in tests.
  Mitigation: keep the BDD world small, prefer explicit `Given`/`When`/`Then`
  transitions, and avoid `.expect()` in test code.

## Progress

- [x] (2026-04-23) Reviewed roadmap item 8.1.3, the linked design document,
  the current `common::rstest` module, and the prior 8.1.1 and 8.1.2 ExecPlans.
- [x] (2026-04-23) Reviewed the current `rstest` unit and behaviour test
  patterns in `common/`.
- [x] (2026-04-23) Drafted this ExecPlan at
  `docs/execplans/8-1-3-argument-and-paragraph-fingerprint-data-models.md`.
- [ ] Establish the red baseline with focused unit and behavioural tests that
  describe the missing fingerprint contracts.
- [ ] Implement the shared argument fingerprint models and public constructors.
- [ ] Implement the shared paragraph fingerprint models and deterministic
  local-slot normalization helpers.
- [ ] Re-export the new API from `common/src/rstest/mod.rs` and
  `common/src/lib.rs`, with Rustdoc examples.
- [ ] Record implementation decisions in
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.
- [ ] Mark roadmap item 8.1.3 done in `docs/roadmap.md`.
- [ ] Run `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint`, and `make test`.
- [ ] Finalize the living sections in this document after implementation.

## Surprises & Discoveries

- `common::rstest` already contains the right architectural precedent for this
  work: 8.1.1 added pure detection and parameter classification, and 8.1.2
  added pure span recovery with a thin compiler-aware adapter elsewhere. That
  means 8.1.3 should remain pure as well.
- The design document already sketches the exact public vocabulary for both
  fingerprint families (`ArgAtom`, `ArgFingerprint`, `ParagraphFingerprint`,
  `StmtShape`, `ExprShape`, and `CalleeShape`), so the implementation should
  not invent a materially different naming scheme without a strong reason.
- `common/src/rstest/tests.rs` is already 248 lines long, so even moderate
  new coverage should trigger an early test split rather than a late cleanup.
- The stale `rstest-bdd` comment in `common/Cargo.toml` has already been fixed
  to `0.5.x`; 8.1.3 does not need to revisit that documentation hygiene.
- The current behaviour harnesses in
  `common/tests/rstest_detection_behaviour.rs`
  and `common/tests/rstest_span_recovery_behaviour.rs` are good templates for a
  small, public-API-first fingerprint harness.
- Previous 8.1.x work already documented one `rstest-bdd` caveat: `And`
  continues the previous keyword family. The fingerprint harness should prefer
  explicit step types instead of relying on subtle keyword transitions.

## Decision Log

- Decision: keep 8.1.3 in `common::rstest` rather than creating a separate
  top-level `common::fingerprint` module. Rationale: these fingerprints are not
  generic clone-detection hashes or cross-domain utilities; they exist to serve
  later `rstest` fixture-hygiene lints. Date/Author: 2026-04-23 / plan author.
- Decision: implement deterministic paragraph local-slot normalization inside
  the shared layer rather than in future lint crates. Rationale: "same
  paragraph, different local names" is the core cross-test grouping contract,
  so the shared foundation should own it. Date/Author: 2026-04-23 / plan author.
- Decision: keep unsupported and unknown states explicit in the public models.
  Rationale: later lints need to know whether a candidate was unsupported, not
  merely absent, and silent dropping would make false-positive control harder.
  Date/Author: 2026-04-23 / plan author.
- Decision: split new unit tests into dedicated modules instead of growing
  `common/src/rstest/tests.rs` in place. Rationale: the repository's 400-line
  limit is a hard constraint, not a cleanup suggestion. Date/Author: 2026-04-23
  / plan author.

## Context and orientation

### Repository state

The relevant code and tests already live in one shared cluster:

- `common/src/rstest/detection.rs` contains the pure 8.1.1 detection helpers.
- `common/src/rstest/parameter.rs` contains parameter classification for
  fixture-local versus provider-driven inputs.
- `common/src/rstest/span.rs` contains the pure 8.1.2 span-recovery models.
- `common/src/rstest/mod.rs` re-exports the shared `rstest` API surface.
- `common/src/lib.rs` re-exports that API at the crate root.
- `common/tests/rstest_detection_behaviour.rs` and
  `common/tests/rstest_span_recovery_behaviour.rs` show the current BDD shape
  for pure shared helpers.

The relevant documentation already exists:

- `docs/roadmap.md` defines 8.1.3 as the shared fingerprint foundation needed
  before 8.2.x and 8.4.x.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` defines the argument and
  paragraph fingerprint sketches under lint A and lint C.
- `docs/rust-testing-with-rstest-fixtures.md` explains the fixture-oriented
  testing style already used in this repository.
- `docs/rstest-bdd-users-guide.md` documents the `rstest-bdd` conventions and
  limitations that the behaviour harness must follow.
- `docs/rust-doctest-dry-guide.md` defines how public Rustdoc examples should
  stay compileable without unnecessary execution complexity.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` reinforces the
  "small, focused helpers" bias that matters here because fingerprint lowering
  logic can otherwise become a future bumpy-road hotspot.

### Relevant skills

The later implementation should explicitly lean on these skills:

- `execplans` to keep this document current during implementation.
- `rust-router` to choose the smallest Rust-specialist skill when code work
  begins.
- `rust-types-and-apis` for the shared public enums, structs, constructors,
  and newtypes used to model deterministic fingerprints.
- `nextest` for understanding the repository's `make test` behaviour and the
  distinction between targeted Rust test runs and full workspace validation.
- `en-gb-oxendict-style` for the design-doc and roadmap updates.

### What 8.1.3 must provide

This roadmap item is narrower than "implement lint A" or "implement lint C". It
provides the shared data models those later tasks will lower into and group by.

For lint A, the shared layer must represent:

- fixture-local arguments,
- stable literal arguments,
- stable constant-path arguments, and
- unsupported argument shapes.

For lint C, the shared layer must represent:

- normalized statement shapes,
- normalized callee identity where known,
- normalized local slots by first appearance order, and
- explicit unknown shapes where canonicalization stops.

The shared layer does not need to walk HIR, recover spans, decide thresholds,
or emit diagnostics. Later roadmap items will do that lowering and policy work.

## Proposed implementation shape

Add new shared modules under `common/src/rstest/` and keep them sibling to the
existing detection and span helpers:

- `common/src/rstest/argument_fingerprint.rs`
- `common/src/rstest/paragraph_fingerprint.rs`
- `common/src/rstest/mod.rs`
- `common/src/lib.rs`

Split unit tests by topic so the file-size limit remains intact:

- `common/src/rstest/tests/mod.rs`
- `common/src/rstest/tests/detection.rs`
- `common/src/rstest/tests/span.rs`
- `common/src/rstest/tests/fingerprint.rs`

Add behaviour coverage:

- `common/tests/rstest_fingerprint_behaviour.rs`
- `common/tests/features/rstest_fingerprint.feature`

Update the docs:

- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`
- `docs/roadmap.md`

The public surface should stay close to the design document, with one likely
addition: a deterministic slot newtype or builder helper so paragraph
normalization is owned by the shared layer.

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArgAtom {
    FixtureLocal { name: String },
    ConstLit { text: String },
    ConstPath { def_path: String },
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArgFingerprint {
    atoms: Vec<ArgAtom>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalSlot(u16);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CalleeShape {
    DefPath(String),
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExprShape {
    Call { callee: CalleeShape, argc: usize },
    MethodCall { method: String, argc: usize },
    Path,
    Lit,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StmtShape {
    Let { init: ExprShape },
    MutCall { receiver: Option<LocalSlot>, callee: CalleeShape },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParagraphFingerprint {
    shapes: Vec<StmtShape>,
}
```

Whether `LocalSlot` is exposed directly or only through a builder is an
implementation detail to finalize during the implementation turn. The important
contract is that equivalent paragraphs must normalize to the same stable slot
sequence regardless of local variable names.

## Red-green contract

Start by adding failing tests that describe the public behaviour before any
production code is written.

Unit-test coverage in `common/src/rstest/tests/fingerprint.rs` must cover at
least:

1. Argument fingerprint equality for identical fixture-local and literal input
   sequences.
2. Argument fingerprint inequality when positionally different atoms are used.
3. Explicit unsupported argument atoms surviving in the fingerprint instead of
   being dropped.
4. Paragraph fingerprints normalizing different local names to the same slot
   sequence when first appearance order matches.
5. Paragraph fingerprints diverging when statement shape, callee identity, or
   argument counts differ.
6. Deterministic slot assignment for repeated normalization runs over the same
   logical paragraph.

Behaviour-test scenarios in `common/tests/rstest_fingerprint_behaviour.rs` and
`common/tests/features/rstest_fingerprint.feature` should cover:

1. Happy path: equivalent helper-call arguments yield the same fingerprint.
2. Happy path: equivalent setup paragraphs with renamed locals still group
   together.
3. Unhappy path: unsupported arguments remain explicitly unsupported.
4. Unhappy path: structurally different paragraphs do not group together.
5. Edge case: first-appearance order controls slot numbering, not lexical sort
   of local names.

## Concrete implementation steps

### Milestone 1: Lock the contract in tests first

Create the new unit-test module and the new BDD harness before implementing the
fingerprint code. Keep the tests public-API-first: they should construct
fingerprints through constructors or builders that a later lint crate could
also use.

### Milestone 2: Add argument fingerprint models

Implement the lint-A-facing pure models in
`common/src/rstest/argument_fingerprint.rs`. The code should stay close to the
design doc:

- store atoms in call-site order,
- preserve `Unsupported` atoms explicitly,
- derive equality and hashing in the stable order provided by `Vec`, and
- expose small constructors/helpers with Rustdoc examples.

This milestone should not try to infer literals or definition paths from HIR.
It only models already-lowered atoms.

### Milestone 3: Add paragraph fingerprint models

Implement the lint-C-facing pure models in
`common/src/rstest/paragraph_fingerprint.rs`. This is the place to encode
deterministic local-slot normalization. Prefer a small helper such as a builder
or normalizer that:

1. receives already-lowered local names or local references,
2. assigns slots by first appearance order,
3. emits stable `LocalSlot` values, and
4. preserves `Unknown` or `Other` variants where canonicalization stops.

Keep the API explicit rather than clever. Future lint crates should be able to
read the model and predict the grouping outcome without reverse-engineering a
compact abstraction.

### Milestone 4: Re-export and document the shared API

Update `common/src/rstest/mod.rs` and `common/src/lib.rs` to re-export the new
types. Add Rustdoc examples that compile cleanly under the doctest guidance in
`docs/rust-doctest-dry-guide.md`. If a builder or normalizer is public,
document one example that shows renamed locals normalizing to equal paragraph
fingerprints.

### Milestone 5: Update design and roadmap docs

Add an `Implementation decisions (8.1.3)` section to
`docs/lints-for-rstest-fixtures-and-test-hygiene.md`. Record at least:

1. where the shared fingerprint models live,
2. whether local-slot normalization is builder-driven or constructor-driven,
3. how unsupported and unknown shapes are represented, and
4. any small naming refinement taken from the draft design.

Only after the code, tests, and gates pass should `docs/roadmap.md` mark 8.1.3
done.

## Validation commands

The implementation turn should capture focused and full validation logs with
`tee` and `set -o pipefail` from the repository root.

```sh
set -o pipefail && cargo test -p whitaker-common rstest:: 2>&1 | tee \
  /tmp/8-1-3-common-rstest-unit.log
set -o pipefail && cargo test -p whitaker-common --test rstest_fingerprint_behaviour \
  2>&1 | tee /tmp/8-1-3-common-rstest-bdd.log
set -o pipefail && cargo clippy -p whitaker-common --all-targets --all-features -- \
  -D warnings 2>&1 | tee /tmp/8-1-3-common-rstest-clippy.log
set -o pipefail && make fmt 2>&1 | tee /tmp/8-1-3-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/8-1-3-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/8-1-3-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/8-1-3-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/8-1-3-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/8-1-3-test.log
```

Expected success signals:

- the focused unit test run includes the new fingerprint assertions,
- the new BDD binary passes all fingerprint scenarios,
- the targeted Clippy run stays warning-free,
- the documentation gates pass because this task edits Markdown, and
- the full workspace gates pass unchanged.

## Outcomes & Retrospective

Pending implementation. This draft is complete enough for review and approval,
but no code, roadmap, or design-document completion state should change until
the implementation turn is approved and finished.
