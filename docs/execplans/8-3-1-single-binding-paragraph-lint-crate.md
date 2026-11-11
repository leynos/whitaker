# Create the `single_binding_paragraph` lint crate and statement I/O model (roadmap 8.3.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`. Implementation
must not begin until a human reviewer explicitly approves this plan.

## Purpose / big picture

Roadmap item 8.3.1 starts Lint B, `SINGLE_BINDING_PARAGRAPH`, from
[rstest fixture and test hygiene lints](../lints-for-rstest-fixtures-and-test-hygiene.md)
§Lint B: single-binding paragraph detection. Lint B will eventually tell a
developer: "these four statements exist only to compute `client`; extract them
into a helper or fixture." To say that safely, the lint must know, for every
statement in a block, which local variables the statement introduces, reads,
and mutates, and whether it contains control flow that makes extraction unsafe.
That per-statement fact sheet is the *statement I/O model*. Roadmap item 8.3.2
will run a backward slice over it; 8.3.3 will emit diagnostics; roadmap item
8.4.2 (Lint C) will reuse the same candidates.

After this change:

1. A new Dylint crate, `crates/single_binding_paragraph`, declares the
   `SINGLE_BINDING_PARAGRAPH` lint and runs a late pass over every function
   body. It emits no diagnostics yet. This is the same *diagnostic-silent*
   bootstrap shape that roadmap items 8.2.1 and 8.2.2 used for Lint A.
2. A compiler-free, generic statement I/O model lives in
   `whitaker_common::paragraph`. It records, per statement, the `defs` (locals
   the statement binds), `uses` (free locals the statement mentions), `muts`
   (free locals the statement may modify), and a set of control-flow *guards*
   (reasons the statement may not be moved into a helper). It also defines how
   statement facts compose over a contiguous range of statements, which is the
   contract that 8.3.2's slicer and 8.4.2's Lint C will rely on.
3. A thin High-level Intermediate Representation (HIR) adapter inside the lint
   crate lowers each `rustc_hir::Block` into that model, using rustc's
   `ExprUseVisitor` for mutation facts.
4. A machine-checked Verus proof establishes the algebra of range composition
   (associativity, identity, agreement with a declarative definition, and
   equality of forward and backward incremental construction).
5. A test-only, environment-gated summary hook lets the UI harness observe the
   model computed by real rustc over a fixture, and an `insta` snapshot pins
   that observation.

Observable success: running

```sh
cargo nextest run -p single_binding_paragraph --all-targets --all-features
```

passes, including the snapshot test `ui::example_harness_records_statement_io`,
whose snapshot shows, for a fixture statement such as `items.push(seed);`, the
line
`stmt[1] form=semi defs=[] uses=[items#0, seed#0] muts=[items#0] guards=[]`.
Running `make verus-paragraph` reports every lemma in
`verus/paragraph_io_summary.rs` verified with zero errors. `make check-fmt`,
`make lint`, and `make test` pass, and the lint emits no diagnostics on any
fixture.

## Relevant documentation and skills

Read these repository documents before starting each stage:

- `docs/roadmap.md` §8.3 for the exact scope split between 8.3.1, 8.3.2,
  8.3.3, 8.3.4, and 8.5.1.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` §Lint B (formal rule,
  backward-slice algorithm, statement I/O model, configuration example), the
  §Implementation decisions (8.1.2), (8.2.1), and (8.2.2) sections, and
  §Non-goals and boundaries.
- `docs/execplans/8-2-1-create-the-rstest-helper-lint-crate.md` and
  `docs/execplans/8-2-2-call-site-collection-in-rstest-tests.md` for the
  diagnostic-silent bootstrap pattern, the test-only summary hook, and the
  Dylint example harness lessons.
- `docs/whitaker-dylint-suite-design.md` §1 (workspace layout), §2 (common
  crate), and §7 (testing strategy).
- `docs/whitaker-clone-detector-design.md` and
  `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`
  for the repository's Verus "sidecar model" convention and trust-boundary
  wording.
- `docs/developers-guide.md` §"Proof workflows", §"Verus scope and trust
  boundary", §"Module layout and test placement", §"The 400-line module
  budget", §"Standard vs Experimental Lints", and §"Adding a new lint".
- `docs/adrs/adr-0006-directory-based-module-layout.md` for `mod.rs`-rooted
  directory modules and `#[path]` sibling test files.
- `docs/rust-testing-with-rstest-fixtures.md` for fixture and `#[case]`
  structure.
- `docs/rstest-bdd-users-guide.md` for the `rstest-bdd` 0.6 feature-file and
  step-binding conventions.
- `docs/rust-doctest-dry-guide.md` for Rustdoc examples on the new public
  `whitaker_common::paragraph` API.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping the
  HIR visitors small and free of bumpy-road nesting.
- `docs/documentation-style-guide.md` for the ADR and design-doc format.

Load these skills during implementation:

- `execplans` to keep this document current.
- `rust-router`, then `rust-types-and-apis` (generic model API and
  newtypes), `rust-unit-testing` (rstest, `googletest`, `pretty_assertions`,
  `insta`), `proptest` (algebraic and builder properties), and `verus` (the
  sidecar proof).
- `rust-verification` only if the proof or test split needs to be revisited.
- `codegraph-mcp` and `leta` for navigation (callers of
  `recover_user_editable_hir_span`, `with_env_var`, and the suite registration
  code).
- `arch-decision-records` for the ADR.
- `en-gb-oxendict` for prose and comments.
- `commit-message` for each commit.

## Constraints

- Scope is roadmap item 8.3.1 only: crate creation and the statement I/O
  model. The backward slice, the single-output check, the `min_len`, `max_len`,
  and `max_inputs` limits, and all configuration keys belong to 8.3.2.
  Diagnostics belong to 8.3.3. UI pass/fail coverage belongs to 8.3.4. Suite
  and installer wiring belongs to 8.5.1. Fluent localization belongs to 8.5.2.
- The lint must emit no diagnostics in this milestone, on any input.
- `whitaker_common` must not gain a `rustc_private` dependency. The model in
  `whitaker_common::paragraph` must be generic over the local-identifier type
  and compile on stable-compatible code paths used by the rest of `common`.
- Do not modify `whitaker_common::rstest` public types (`ParagraphFingerprint`,
  `StmtShape`, `ParagraphNormalizer`, and the span-recovery types). Lint C's
  fingerprinting is out of scope.
- Do not change the suite (`suite/`), installer (`installer/`), command-line
  interface (`cli/`), or the `whitaker` CLI configuration surface.
- Every new code file must stay under 400 lines (`AGENTS.md`), use
  `mod.rs`-rooted directory modules (ADR-0006), start with a `//!` module
  comment, and document every function with `///`.
- No `unsafe`, no `.expect()` or `.unwrap()` outside tests, and no lint
  suppression without a tightly scoped `#[expect(..., reason = "...")]`.
- Tests must not mutate the process environment directly. Use
  `whitaker_common::test_support::with_env_var`, as the 8.2.2 harness does.
- Verus proofs must not use `assume`, `admit`, `#[verifier::external_body]`, or
  any trusted shortcut to discharge a planned obligation.
- Size accounting excludes this ExecPlan file from every file-count and
  line-count limit below.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, and escalate.

## Tolerances (exception triggers)

- Scope: if the change needs more than 35 non-ExecPlan files, or more than
  3,200 net added lines of code and other documentation, stop and escalate.
  This ExecPlan file is excluded from both counts.
- Interface: if any existing public API in `whitaker_common`,
  `whitaker_lint_core`, or `whitaker_suite` must change signature, stop and
  escalate. Adding the new `whitaker_common::paragraph` module is approved by
  this plan.
- Dependencies: the plan pre-approves exactly these additions: a workspace
  proxy crate `crates/rustc_hir_typeck` that re-exports the pinned toolchain's
  `rustc_hir_typeck` (mirroring `crates/rustc_middle`), and the existing
  workspace dependencies `googletest`, `pretty_assertions`, and `insta` as
  dev-dependencies of `whitaker-common` and the new lint crate. Any other new
  dependency requires escalation.
- Toolchain: if `ExprUseVisitor::for_clippy` or the `Delegate` trait cannot be
  used from the proxy crate on `nightly-2026-05-28`, stop after one prototype
  attempt and escalate with the fallback described in `Decision log`.
- Iterations: if a focused test or proof still fails after five fix attempts,
  stop and escalate.
- Proof effort: if the Verus file exceeds 400 lines or needs trigger
  management beyond ordinary `assert(... =~= ...)` extensionality, stop and
  record the verification conditions before escalating.
- Ambiguity: if the design document and this plan disagree on a semantic
  point (for example, whether contained `if` expressions are control flow),
  record the options and stop.

## Risks

- Risk: `ExprUseVisitor` reports mutations against `diag_expr_id`s whose
  ancestor chain does not reach the expected statement (for example, pattern
  bindings in `let ref mut y = x;`, or desugared `for` loops). Severity:
  medium. Likelihood: medium. Mitigation: attribute each event by walking
  `tcx.hir_parent_id_iter` to the nearest enclosing statement of the block
  under analysis, and cover each shape in the snapshot fixture.

- Risk: `check_fn` runs for closures and async bodies as well as ordinary
  functions, which could double-count blocks or analyse compiler-generated
  coroutine bodies. Severity: medium. Likelihood: medium. Mitigation: analyse
  each body once, skip blocks whose span recovers to `MacroOnly`, and make the
  summary key include the body owner and block span so duplicates are visible
  in the snapshot.

- Risk: the Dylint example harness is slow and must serialize with other UI
  harnesses; a new harness may time out or contend for the package cache.
  Severity: medium. Likelihood: medium. Mitigation: reuse the existing
  `whitaker_lint_core::testing::ui::run_with_runner` path, add the new tests to
  the `serial-dylint-ui` group in `.config/nextest.toml` and
  `tests/workflow_contracts/nextest_overrides.py` if the generic filters do not
  already match, and keep one example fixture.

- Risk: aliasing hides mutation. `let r = &mut v; r.push(1);` records
  `muts = {v}` for the borrow and `muts = {r}` for the push, never `v` at the
  push. Interior mutability (`Cell::set`) records no mutation at all. Severity:
  low for 8.3.1. Likelihood: high. Mitigation: document both as known model
  limits; 8.3.2's backward slice stops at a non-contributing statement, so
  these limits cause missed candidates rather than unsafe suggestions. Record
  the argument in the design document.

- Risk: the generic model and the Verus sidecar drift apart. Severity: medium.
  Likelihood: low. Mitigation: the `proptest` conformance suite checks the
  executable `ParagraphIo` against an executable transcription of the same
  declarative specification the proof uses, and the developers' guide records
  the mirroring rule.

- Risk: adding a fourth `rustc_*` proxy crate requires edits to
  `TEST_EXCLUDES`, `DOCTEST_EXCLUDES`, and the `AGENTS.md` mirror of the
  doctest command. Severity: low. Likelihood: high. Mitigation: make those
  edits in the same commit as the proxy crate and run `make test` to confirm.

## Progress

- [x] (2026-11-11T00:00Z) Renamed branch to
  `8-3-1-single-binding-paragraph-lint-crate`.
- [x] (2026-11-11T00:00Z) Reconnaissance: sibling crate anatomy, registration
  touchpoints, span-recovery API, verification infrastructure, and prior art
  (Clippy `mutated_variables`, rust-analyzer extract-function, Komondoor and
  Horwitz procedure extraction).
- [x] (2026-11-11T00:00Z) Drafted this ExecPlan.
- [ ] Expert review of the draft and revision.
- [ ] Plan approved by a human reviewer.
- [ ] EP-M1: pure statement I/O model in `whitaker_common::paragraph`.
- [ ] EP-M2: Verus proof of range-composition algebra.
- [ ] EP-M3: lint crate bootstrap and `rustc_hir_typeck` proxy.
- [ ] EP-M4: HIR adapter and environment-gated summary snapshot.
- [ ] EP-M5: documentation, ADR, roadmap update, and final gates.

## Surprises & discoveries

- Observation: `rustc_hir::HirId` does not implement `Ord` on the pinned
  `nightly-2026-05-28` toolchain. Evidence: `rustc_hir_id/src/lib.rs` derives
  only `Copy, Clone, PartialEq, Eq, Hash, Encodable, Decodable, StableHash` for
  `HirId`, while `ItemLocalId` is declared `#[orderable]` in the same file.
  Impact: the design document's `BTreeSet<rustc_hir::HirId>` would not compile.
  The adapter keys the model on `ItemLocalId`. Closures are not HIR owners, so
  every local referenced inside one function body, including inside its
  closures, shares the body's `OwnerId`; the adapter asserts that and skips any
  foreign-owner identifier.

- Observation: the workspace has no `rustc_hir_typeck` proxy crate and no use
  of `ExprUseVisitor`, and `crates/clippy_utils` is a local stub, not upstream
  Clippy utilities. Evidence: repository search;
  `crates/clippy_utils/src/lib.rs` exposes only `macros::is_panic`. Impact:
  mutation detection either needs a new proxy crate (chosen) or a hand-rolled
  syntactic visitor (fallback).

- Observation: the next free ADR number is 0014. Evidence: `docs/adrs/` on
  `main` ends at 0012, and remote branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces` already
  allocates `adr-0013`. Impact: allocate 0014, re-checking all fetched remote
  branches immediately before committing the ADR.

## Decision log

- Decision: keep the crate diagnostic-silent and unregistered in the suite and
  installer until roadmap item 8.5.1. Rationale: the 8.3.1 roadmap text says
  "create the crate and implement the model"; unlike 8.2.1 it does not say
  "register" or "wire configuration loading". 8.5.1 explicitly owns
  experimental-set wiring for all three lints, and adding a second late
  experimental pass to `suite/src/driver.rs` needs a new combination strategy
  for `define_suite_pass!` that is best designed once for Lints B and C.
  Date/Author: 2026-11-11, planning agent.

- Decision: place the compiler-free model in a new `whitaker_common::paragraph`
  module, generic over a local-identifier type `L: Ord + Copy`, and keep the
  HIR adapter in the lint crate. Rationale: Lint C (8.4.2) must reuse Lint B's
  candidate generation, and lint crates are `cdylib` libraries that should not
  depend on each other; `common` is the shared, compiler-free layer that
  already hosts `rstest` fingerprints. Genericity makes the model testable with
  `u8` identifiers, property-testable, and mirrorable in Verus. The 8.2.2
  precedent keeps compiler-aware adapters in the lint crate until a second
  consumer exists; 8.4.2 may promote the adapter to `whitaker_lint_core::hir`
  in a separate refactor commit. The module is named `paragraph`, not placed
  under `rstest`, because Lint B applies to all function bodies, not only
  `#[rstest]` tests. Date/Author: 2026-11-11, planning agent.

- Decision: define `uses` as every *free* local the statement mentions,
  including locals it only writes, and enforce `muts ⊆ uses` by construction.
  Rationale: for extraction, a written external local is still an input (the
  helper would need `&mut`), and treating it as a use keeps the external-input
  count in 8.3.2 honest. "Free" means referenced but not bound inside the
  statement, so locals bound by nested `let`, closure parameters, or match arms
  inside the statement are invisible to the paragraph. Date/Author: 2026-11-11,
  planning agent.

- Decision: model guards as a set of `GuardKind` values rather than the design
  document's three booleans. Rationale: 8.3.2 must apply configurable policy
  (`reject_closures`, `reject_async`) and the design's rule "no control-flow
  constructs"; a set of named kinds lets the policy live in 8.3.2 while the
  model records facts. It also avoids boolean soup in the API. The kinds are
  `Branch`, `Loop`, `EscapingExit`, `Diverging`, `Await`, `Closure`,
  `AsyncBlock`, `LetElse`, `NestedItem`, and `MacroOnly`. Date/Author:
  2026-11-11, planning agent.

- Decision: record guards found in the statement's own body, including inside
  macro expansions such as `assert!`, but not inside nested closure bodies. A
  `break` or `continue` whose target loop or labelled block lies inside the
  statement records only `Loop`, not `EscapingExit`; `return`, `?`, `yield`,
  `become`, and `break`/`continue` to an outer target record `EscapingExit`.
  Rationale: control flow inside a closure body or a fully contained loop does
  not escape the statement, so it cannot make extraction unsafe; recording it
  would make `reject_closures = false` meaningless. Macro-internal control flow
  is recorded because the user cannot see it and the design is conservative.
  Date/Author: 2026-11-11, planning agent.

- Decision: obtain mutation facts from `rustc_hir_typeck::expr_use_visitor`
  via `ExprUseVisitor::for_clippy`, run once per body with `consume_body`, and
  attribute each `mutate` or mutable `borrow` event to a statement through the
  HIR parent chain. Rationale: it already accounts for auto-reference method
  receivers (`v.push(x)`), compound assignment, overloaded `IndexMut`,
  `ref mut` patterns, and by-mutable-reference closure captures, which a
  syntactic visitor would have to re-derive from typeck adjustments. Clippy's
  `clippy_utils::usage::mutated_variables` is the prior art. Fallback if the
  proxy cannot link: a syntactic visitor over `Assign`, `AssignOp`,
  `AddrOf(Mut)`, and `typeck_results().expr_adjustments` with
  `Adjust::Borrow(AutoBorrow::Ref(.., Mut))`. Choosing the fallback requires
  escalation (see `Tolerances`). Date/Author: 2026-11-11, planning agent.

- Decision: include sequential composition of statement facts
  (`ParagraphIo::then`) in 8.3.1 and prove its algebra in Verus. Rationale: the
  composition is part of the I/O model's contract, and both 8.3.2 (which grows
  a paragraph backward one statement at a time) and Lint C depend on it giving
  the same answer however a range is assembled. Proving it now pressure-tests
  the API before the slicer is designed around it. The backward slice itself
  and its lemmas stay in 8.3.2. Date/Author: 2026-11-11, planning agent.

- Decision: observe the HIR adapter end to end through an environment-gated
  summary hook, `WHITAKER_SINGLE_BINDING_PARAGRAPH_STMT_IO_SUMMARY`, and pin
  the output with an `insta` snapshot. Rationale: this mirrors the accepted
  8.2.2 hook (`WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY`), keeps the lint
  user-silent, and gives a real-rustc oracle for every statement shape without
  linking `rustc_driver` into a bespoke test harness. Date/Author: 2026-11-11,
  planning agent.

- Decision: do not use Kani for 8.3.1. Rationale: the model's obligations are
  unbounded set and sequence identities, which Verus proves for all lengths;
  the executable model uses `BTreeSet`, which is expensive for bounded model
  checking and adds nothing that `proptest` conformance does not already cover.
  Date/Author: 2026-11-11, planning agent.

- Decision: leave `docs/users-guide.md` unchanged in 8.3.1. Rationale: the lint
  is not installable, emits nothing, and has no configuration; there is no
  user-visible behaviour to describe. 8.5.1 and 8.5.3 own the user-facing
  documentation. The `whitaker_common::paragraph` API is an internal interface
  and is documented in the developers' guide and the design document instead.
  Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and at
completion, reconciling discoveries with the documents named in
`Conformance basis`.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint crates as dynamic
libraries into a rustc driver. Each lint lives in its own crate under
`crates/`, and the workspace `Cargo.toml` already includes `crates/*`, so a new
crate directory becomes a workspace member automatically.

A *late lint pass* runs after type checking and sees the HIR, rustc's desugared
syntax tree with name resolution. In the HIR, a block (`rustc_hir::Block`)
holds a list of statements (`stmts`, each a `rustc_hir::Stmt` whose `kind` is
`Let`, `Item`, `Expr`, or `Semi`) and an optional trailing expression (`expr`).
A local variable is identified by the `HirId` of its binding pattern; a path
expression that reads it resolves to `Res::Local(binding_hir_id)`. Each `HirId`
is an `(OwnerId, ItemLocalId)` pair, and on the pinned toolchain only
`ItemLocalId` is orderable.

Key existing code:

- `crates/rstest_helper_should_be_fixture/` is the closest sibling. Its
  `Cargo.toml` shows the feature layout (`dylint-driver` turning on every
  optional compiler and Dylint dependency; `constituent` for suite builds;
  `crate-type = ["cdylib", "rlib"]`; `test = false`). `src/driver.rs` declares
  the lint with `dylint_linting::impl_late_lint!` inside a private
  `mod declaration`, and `check_crate_post` writes the test-only summary
  (`write_collection_summary`) using `cap_std` and `camino`. `tests/ui.rs` runs
  `examples/*.rs` through `dylint_testing::ui::Test::example` and asserts on
  the summary, using `with_env_var` and `ExampleHarnessLock`.
- `crates/bumpy_road_function/` shows the pattern of an ungated pure module
  beside a `dylint-driver`-gated driver.
- `crates/whitaker_lint_core/src/hir/mod.rs` provides
  `recover_user_editable_hir_span(span) -> Option<Span>` and
  `span_recovery_frames(span)`, the 8.1.2 adapter. Feeding the frames into
  `whitaker_common::recover_user_editable_span` yields
  `UserEditableSpan::{Direct, Recovered, MacroOnly}`.
- `crates/rustc_middle/` is a three-line proxy crate
  (`extern crate rustc_middle as upstream; pub use upstream::*;`). The new
  `crates/rustc_hir_typeck/` copies it.
- `common/src/rstest/paragraph_fingerprint.rs` holds Lint C's fingerprint
  types. They are keyed by local *names*, not identifiers; this plan does not
  touch them.
- `verus/*.rs` holds standalone Verus sidecar proofs. `scripts/run-verus.sh`
  lists them per group; `make verus` runs all groups. Proofs run in CI only in
  `.github/workflows/proofs.yml`, whose path filter already includes
  `common/**` and `verus/**`.
- `Makefile` `TEST_EXCLUDES` and `DOCTEST_EXCLUDES` list crates that cannot
  run ordinary tests or doctests because they link `rustc_private`. `AGENTS.md`
  mirrors the doctest command.

Terms used in this plan:

- *Paragraph*: a contiguous range of statements in one block.
- *Free local*: a local referenced by a statement whose binding is outside
  that statement.
- *Guard*: a reason recorded on a statement that may make it unsafe or
  undesirable to move into an extracted helper.
- *Sidecar proof*: a Verus file that models production logic in Verus's
  specification language, rather than verifying the compiled Rust body.

## Conformance basis

Upstream artefacts:

- Roadmap: `docs/roadmap.md` item 8.3.1 (`RM-8.3.1`), with dependency 8.1.2
  (done). Downstream consumers: 8.3.2, 8.3.3, 8.3.4, 8.4.2, 8.5.1.
- Design: `docs/lints-for-rstest-fixtures-and-test-hygiene.md` at `main`
  commit `d9eb8a09`, specifically:
  - `LB-INTENT`: §Intent for lint B (crate `single_binding_paragraph`, lint
    `SINGLE_BINDING_PARAGRAPH`).
  - `LB-IO`: §Statement I/O model (`defs`, `uses`, `muts`,
    `has_control_flow`, `has_macro_only_span`, `has_closure`; deterministic
    `BTreeSet` ordering).
  - `LB-RULE`: §Formal rule (no control flow or macro-only spans in a
    candidate; intermediate locals unused after the sink).
  - `LB-SLICE`: §Backward-slice algorithm (consumer of `LB-IO`, delivered in
    8.3.2).
  - `INT-LATE` and `INT-SPAN`: §Integration constraints (late lint; recover
    user-editable spans).
  - `NG-1`: §Non-goals (no MIR, no SSA, no whole-program graphs).
- Architecture decisions: ADR-0003 (Verus sidecar convention), ADR-0006
  (module layout). A new ADR-0014 is planned in EP-M5.
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`.
- No Terms of Reference document exists for this work.

Planned deviations from the design text (each recorded in `Decision log`):

- `DEV-1`: the identifier type is `ItemLocalId` behind a generic `L`, not
  `HirId` (forced by the toolchain).
- `DEV-2`: the three booleans become a `GuardKind` set with more kinds.
- `DEV-3`: the model adds `StmtForm` (so 8.3.2 can recognize a simple `let`
  sink) and `ParagraphIo` composition.

Trace links:

```plaintext
RM-8.3.1 -> LB-INTENT -> EP-M3 -> ui::example_compiles_without_diagnostics
RM-8.3.1 -> LB-IO -> EP-M1 -> paragraph::tests (builder and form tests)
LB-IO (determinism) -> EP-M1 -> paragraph::proptests::build_is_order_independent
LB-IO -> LB-SLICE -> EP-M1 + EP-M2 -> paragraph_io_summary.rs lemmas + paragraph::proptests::*
LB-RULE (control flow, macro-only) -> EP-M4 -> ui::example_harness_records_statement_io snapshot
INT-LATE + INT-SPAN -> EP-M4 -> driver uses late pass + span_recovery_frames
NG-1 -> EP-M4 -> adapter uses HIR + ExprUseVisitor only (no MIR)
```

## Verification plan

The model has two layers with different obligations. The *pure layer*
(`whitaker_common::paragraph`) owns set-algebraic contracts that hold for all
inputs; these get unit tests, property tests, and, for the composition algebra,
a Verus proof. The *adapter layer* (HIR lowering) owns the correspondence
between Rust source and model facts; that correspondence depends on rustc's
semantics, which we treat as axioms and exercise end to end through a snapshot
over real compiler output.

### Abstract specification

Let `L` be a set of local identifiers and `G` the finite set of guard kinds. A
statement fact is a tuple `s = (form, D, U, M, Γ)` where `D, U, M ⊆ L` are
defs, uses, and muts, and `Γ ⊆ G` the guards.

A statement fact is *well formed* when:

- `WF-1`: `M ⊆ U` (every mutated free local is mentioned).
- `WF-2`: `D ∩ U = ∅` (a statement cannot mention a local it binds at top
  level, because a `let` pattern's bindings are not in scope in its own
  initializer or `else` block).
- `WF-3`: if `form = SimpleLet(x)` then `D = {x}`.

A paragraph fact is `p = (D, I, X, Γ)`: all defs, external inputs, external
muts, and guards. The empty paragraph is `ε = (∅, ∅, ∅, ∅)`. Lifting a
statement gives `lift(s) = (D, U, M, Γ)`. Sequential composition is:

```plaintext
then(p, q) = ( p.D ∪ q.D,
               p.I ∪ (q.I \ p.D),
               p.X ∪ (q.X \ p.D),
               p.Γ ∪ q.Γ )
```

For a sequence `S = s_0 … s_{n-1}` the declarative summary is:

```plaintext
spec_D(S) = ⋃_j D_j
spec_I(S) = { u | ∃ j. u ∈ U_j ∧ ∀ k < j. u ∉ D_k }
spec_X(S) = { u | ∃ j. u ∈ M_j ∧ ∀ k < j. u ∉ D_k }
spec_Γ(S) = ⋃_j Γ_j
```

A sequence is *well scoped* when no statement uses or mutates a local that a
later statement defines (`∀ j < k. (U_j ∪ M_j) ∩ D_k = ∅`) and top-level defs
are pairwise disjoint (`∀ j ≠ k. D_j ∩ D_k = ∅`). Rust's lexical scoping and
the uniqueness of binding identifiers make every real block well scoped; that
is axiom `AX-SCOPE` below.

### Obligations for the pure layer

- Obligation `OB-B1` (builder well-formedness): for every sequence of builder
  events (`define`, `bind_internal`, `reference`, `mutate`, `guard`),
  `StmtIoBuilder::build` returns a fact satisfying `WF-1` and `WF-2`, with
  internal bindings absent from `D`, `U`, and `M`; and `WF-3` holds for every
  `SimpleLet` builder.
  - Method: `proptest` over generated event vectors on `u8` identifiers,
    plus named `rstest` cases for each event kind.
  - Rationale: the property quantifies over arbitrary event orders; the
    domain is cheap and the code is executable Rust with `BTreeSet`, so
    property testing is proportionate. The construction is simple enough that
    a proof adds little over the property.
  - Domain: up to 32 events over an identifier universe of 8 values, so
    collisions between defined, internal, referenced, and mutated identifiers
    are frequent.
  - Artefact: `common/src/paragraph/tests/builder_properties.rs`.
  - Evidence: `cargo nextest run -p whitaker-common paragraph` passes; before
    `build` subtracts internal bindings, the property fails with a shrunk
    counterexample naming an internal identifier in `uses`.
  - Non-vacuity: a `prop_assert!` classifier counts cases in which an
    identifier is both internally bound and referenced, and in which a
    `mutate` event precedes the matching `reference`; the test asserts both
    counts are non-zero across the run. Negative control: temporarily remove
    the `muts ⊆ uses` insertion in `mutate` and confirm the property fails.

- Obligation `OB-B2` (order independence): `build` is invariant under any
  permutation of the event sequence.
  - Method: `proptest` with `prop::sample::subsequence` and shuffle.
  - Rationale: determinism of the model is a design requirement (`LB-IO`),
    and the HIR visitors emit events in traversal order, which must not
    matter.
  - Artefact: same file, `build_is_order_independent`.
  - Evidence and non-vacuity: generated sequences contain at least two
    distinct event kinds in over half of cases (classified and asserted).

- Obligation `OB-C1` (identity): `then(ε, p) = p = then(p, ε)`.
- Obligation `OB-C2` (associativity):
  `then(then(a, b), c) = then(a, then(b, c))`.
- Obligation `OB-C3` (fold agrees with specification): folding `then` from
  the left over `lift(s_0) … lift(s_{n-1})`, starting from `ε`, equals
  `(spec_D, spec_I, spec_X, spec_Γ)` of the sequence, for every finite sequence.
- Obligation `OB-C4` (backward construction): building the summary by
  repeatedly *prepending* statements,
  `fold_right(S) = then(lift(s_0), fold_right(s_1 …))`, equals the left fold
  for every finite sequence. This is the property 8.3.2's backward slice relies
  on: extending a paragraph one statement upward yields the same summary as
  computing it afresh.
- Obligation `OB-C5` (well-formedness preserved): if every statement is well
  formed (`WF-1`, `WF-2`) and the sequence is well scoped, then the summary
  satisfies `X ⊆ I` and `D ∩ I = ∅`.
  - Method for `OB-C1` to `OB-C5`: Verus sidecar proof in
    `verus/paragraph_io_summary.rs`, run by `make verus-paragraph` (new group
    `paragraph`) and `make verus`; plus `proptest` conformance of the
    executable `ParagraphIo` against an executable transcription of `spec_*`
    in `common/src/paragraph/tests/summary_properties.rs`.
  - Rationale: these identities must hold for all sequence lengths, which is
    an unbounded claim; Verus discharges it by induction over `Seq`. The
    `proptest` suite ties the executable Rust to the same specification,
    because the sidecar proves a model, not the compiled code (ADR-0003
    convention).
  - Domain: Verus — all finite sequences over an arbitrary identifier type
    (`Set<int>` in the model). `proptest` — sequences of up to 12 statements
    over 8 identifiers and all ten guard kinds.
  - Evidence: `make verus-paragraph` prints `verification results:: N
    verified, 0 errors` with `N` equal to the number of lemmas and proof
    functions; `cargo nextest run -p whitaker-common paragraph` passes. Red
    stage: commit the lemma statements with empty bodies first; Verus must
    report each as failing (or with an unproved postcondition) before the
    proof bodies are written.
  - Non-vacuity (proof): `OB-C5`'s antecedent is inhabited by a concrete
    witness lemma `lemma_well_scoped_witness` that builds a three-statement
    well-scoped sequence (a `let`, a mutation of the first binding, and a
    sink) and asserts the precondition holds and the summary has non-empty
    `I`, `X`, and `D`. Negative control (proof): a lemma
    `lemma_naive_union_disagrees` proves that the naive composition
    `p.I ∪ q.I` differs from `spec_I` on a concrete two-statement sequence,
    showing that the specification distinguishes the correct definition from
    the most plausible wrong one. Inspect the file for `assume`, `admit`, and
    `external_body` with `rg` and record a zero count.
  - Non-vacuity (tests): the conformance generator classifies sequences in
    which a later statement uses an earlier statement's def (the case that
    makes `\ p.D` matter) and asserts it occurs. Negative control: replacing
    `q.I \ p.D` with `q.I` in `then` must fail the conformance property.

Lemma statements are hypotheses about the proof structure. Implementation may
show that `OB-C4` follows from `OB-C1` and `OB-C2` by a short induction, or that
`OB-C3` needs a helper lemma about `spec_I` of a snoc'd sequence. Record any
such refinement in `Decision log`.

### Obligations for the adapter layer

- Obligation `OB-A1` (statement correspondence): for each statement shape in
  the fixture `examples/stmt_io_model.rs`, the recorded `form`, `defs`, `uses`,
  `muts`, and `guards` match the hand-derived expectation.
  - Method: `insta` snapshot of the environment-gated summary produced by the
    real Dylint driver, reviewed line by line against the fixture comments;
    plus named `rstest` cases for the pure helpers the adapter calls (guard
    classification of `LoopSource` and `MatchSource`, form classification of
    `PatKind`).
  - Rationale: the correspondence depends on rustc's HIR and typeck
    semantics, which cannot be modelled faithfully; an end-to-end observation
    over real compiler output is the strongest proportionate evidence.
  - Domain: the fixture enumerates every shape in
    `Validation and acceptance`.
  - Artefact: `crates/single_binding_paragraph/examples/stmt_io_model.rs`,
    `crates/single_binding_paragraph/tests/ui.rs`, and
    `crates/single_binding_paragraph/tests/snapshots/ui__statement_io_summary.snap`.
  - Evidence: red — the snapshot test fails because no summary file is
    written; green — the summary matches the reviewed snapshot.
  - Non-vacuity: the fixture's comment beside each statement states the
    expected line, and the snapshot review compares each one. Negative
    control: temporarily drop `borrow(.., BorrowKind::Mutable)` handling in
    the delegate; the snapshot must change at the `items.push(seed)` line.

- Obligation `OB-A2` (silence): the lint emits no diagnostics on the fixture
  or on the bootstrap example.
  - Method: `dylint_testing` example run and `trybuild` pass case.
  - Artefact: `tests/ui.rs` (`example_compiles_without_diagnostics`,
    `trybuild_fixtures_compile_without_diagnostics`).

- Obligation `OB-A3` (well-formedness at the boundary): every `StmtIo`
  produced from the fixture satisfies `WF-1` to `WF-3`, and every block is well
  scoped.
  - Method: the summary renderer checks the predicates and writes
    `wf=ok`/`wf=violated` per block; the snapshot pins `wf=ok`.
  - Rationale: this checks `AX-SCOPE` and the builder contract on real HIR
    rather than assuming them.

### Axioms

- `AX-SCOPE`: in HIR, every binding pattern has a unique `HirId`; a `let`
  binding is not in scope in its own initializer or `else` block; and a local
  cannot be referenced before the statement that binds it. Evidence: Rust
  scoping rules; checked on the fixture by `OB-A3`.
- `AX-OWNER`: closure bodies share their enclosing function's `OwnerId`, so
  `ItemLocalId` identifies locals uniquely within one analysed body.
- `AX-EUV`: `ExprUseVisitor::consume_body` reports, for every place mutated
  or mutably borrowed in a body (including through auto-reference adjustments
  and closure captures), a `Delegate::mutate` or
  `Delegate::borrow(.., BorrowKind::Mutable)` call whose `PlaceBase::Local`
  names the base local. We rely on the documented interface only.
- `AX-SPAN`: `span_recovery_frames` and `recover_user_editable_span` classify
  spans as specified in 8.1.2; they are already tested in `common` and
  `whitaker_lint_core`.

### Residual gaps

Aliasing through references and interior mutability are not modelled (`Risks`).
The adapter's correctness is evidenced on a fixed fixture, not proven. The
8.3.2 slice lemmas — contiguity, "stop at the first non-contributing statement"
minimality, and the soundness argument that a selected paragraph's
intermediates are dead after the sink — are stated below as unproved hypotheses
for 8.3.2, not obligations of 8.3.1:

```plaintext
H-SLICE-1: the slice of sink i is the largest contiguous [k..=i] such that
           every s_j, k ≤ j < i, defines or mutates a local in the needed set.
H-SLICE-2: summary([k..=i]).I equals the needed set at the stop point.
H-SLICE-3: if no local in summary([k..i)).D is in uses of s_{i+1..n} or the
           tail, replacing [k..=i] by `let out = helper(I);` preserves the
           block's def-use relation.
```

## Plan of work

### Stage A — confirm and pressure-test (no code changes)

Re-read the documents in `Relevant documentation and skills`. Confirm the
toolchain facts in `Surprises & discoveries` still hold (`HirId` not `Ord`,
`ItemLocalId` orderable, `ExprUseVisitor::for_clippy` public). Write the Verus
lemma *statements* (no bodies) and the `ParagraphIo` signatures side by side,
and check that every field 8.3.2 will need (`form`, `defs`, `uses`, `muts`,
`guards`) is present and that no field is unused by any planned obligation.
Go/no-go: proceed only if the signatures in `Interfaces and dependencies`
survive this check unchanged, or record the change in `Decision log`.

### Stage B — EP-M1: pure model in `whitaker_common::paragraph`

Create `common/src/paragraph/` as a directory module:

- `mod.rs`: module documentation, re-exports, and the reuse policy comment.
- `guard.rs`: `GuardKind` and `GuardSet`.
- `stmt_io.rs`: `StmtForm`, `StmtIo`, `StmtIoBuilder`.
- `summary.rs`: `ParagraphIo` with `empty`, `from_stmt`, `then`, `prepend`,
  and `summarize`.
- `tests/mod.rs`, `tests/builder.rs`, `tests/builder_properties.rs`,
  `tests/summary.rs`, `tests/summary_properties.rs`, wired with
  `#[cfg(test)] mod tests;` from `mod.rs`.

Red: write the named `rstest` cases and the `proptest` properties first against
stub bodies that return `StmtIo` and `ParagraphIo` values with empty sets; run
them and confirm they fail for the stated reasons. Green: implement the builder
and composition. Refactor: keep each function small; run the focused suite
again.

Add a behavioural specification in `common/tests/features/paragraph_io.feature`
with steps in `common/tests/paragraph_io_behaviour.rs` (see
`Validation and acceptance` for the scenarios). Add `paragraph` re-exports to
`common/src/lib.rs` and update `common/tests/public_api_contract.rs` if it
enumerates the public surface. Add `googletest` and `pretty_assertions` to
`common/Cargo.toml` `[dev-dependencies]` as workspace dependencies.

Validation: `make check-fmt`, `make lint`, `make test`. Commit.

### Stage C — EP-M2: Verus proof

Create `verus/paragraph_io_summary.rs` following the shape of
`verus/string_continuation_rewrite.rs`: a `//!` header stating the trust
boundary, `use vstd::prelude::*;`, and a `verus! { … }` block containing
`struct StmtFact { defs: Set<int>, uses: Set<int>, muts: Set<int>, guards:
Set<int> }`,
`struct ParaFact { … }`, `spec fn then(…)`, `spec fn lift(…)`,
`spec fn fold_left(Seq<StmtFact>)`, `spec fn fold_right(Seq<StmtFact>)`,
`spec fn spec_inputs(Seq<StmtFact>)` and siblings, `spec fn well_formed`,
`spec fn well_scoped`, and the lemmas for `OB-C1` to `OB-C5`, the witness, and
the negative control. Add a `paragraph` group to `scripts/run-verus.sh` (both
the group case and `all`) and a `verus-paragraph` target to the `Makefile`
(`.PHONY` list included).

Red: lemma statements with empty bodies fail verification. Green: write the
induction proofs. Validation: `make verus-paragraph`, `make verus`, then
`make check-fmt`, `make lint`, `make test`. Commit.

### Stage D — EP-M3: crate bootstrap

Create `crates/rustc_hir_typeck/` by copying `crates/rustc_middle/` and
changing the crate name; add it to `TEST_EXCLUDES` and `DOCTEST_EXCLUDES` in the
`Makefile` and to the mirrored doctest command in `AGENTS.md`, and add the
workspace dependency entry in the root `Cargo.toml` beside the other proxies.

Create `crates/single_binding_paragraph/` with `Cargo.toml` modelled on
`crates/rstest_helper_should_be_fixture/Cargo.toml` (optional `rustc_hir`,
`rustc_hir_typeck`, `rustc_lint`, `rustc_middle`, `rustc_session`, `rustc_span`,
`dylint_linting`, `whitaker-common`, `whitaker_lint_core`, `log`, `camino`,
`cap-std` behind `dylint-driver`; `constituent`;
`crate-type = ["cdylib", "rlib"]`; `test = false`; dev-dependencies
`dylint_testing`, `rstest`, `proptest`, `googletest`, `pretty_assertions`,
`insta`, `trybuild`, `tempfile`). Write `src/lib.rs` (gated modules and
re-exports of `SINGLE_BINDING_PARAGRAPH` and `SingleBindingParagraph`) and
`src/driver/mod.rs` declaring the lint at `Warn` level through
`dylint_linting::impl_late_lint!` with an empty `LateLintPass`. Add
`examples/bootstrap_zero_diagnostic.rs`,
`tests/ui/bootstrap_zero_diagnostic.rs`, and `tests/ui.rs` with
`example_compiles_without_diagnostics` and
`trybuild_fixtures_compile_without_diagnostics`, reusing
`whitaker_lint_core::testing::ui::run_with_runner`. Add the crate's `examples`
directory to `.rustfmt.toml` `ignore`, and add the crate to `DOCTEST_EXCLUDES`
(and its `AGENTS.md` mirror).

Before copying `ExampleHarnessLock` from the sibling crate, sweep for a shared
equivalent (per the `AGENTS.md` abstraction policy). If none exists, prefer
placing the UI tests in the existing `serial-dylint-ui` nextest group over
copying the 600-line lock; if the group alone does not serialize the nested
example builds, escalate rather than duplicating the lock silently.

Red: `example_compiles_without_diagnostics` fails because the crate does not
build. Green: the crate builds and the example is silent. Validation:
`make check-fmt`, `make lint`, `make test`. Commit.

### Stage E — EP-M4: HIR adapter and summary snapshot

Create `crates/single_binding_paragraph/src/stmt_io/`:

- `mod.rs`: `lower_body(cx, body) -> Vec<BlockIo>` entry point, where
  `BlockIo` holds the block's recovered span, its `Vec<StmtIo<ItemLocalId>>`,
  and the tail expression's `StmtIo` (form `Tail`) if present.
- `references.rs`: a `rustc_hir::intravisit::Visitor` with
  `NestedFilter = nested_filter::OnlyBodies` that records `Res::Local`
  references and internal `PatKind::Binding` identifiers, descending into
  closure bodies so captures count as uses.
- `mutations.rs`: a `Delegate` implementation that records
  `(ItemLocalId, diag_expr_id)` for `mutate` and mutable `borrow` on
  `PlaceBase::Local`, plus `attribute_to_statement`, which walks
  `tcx.hir_parent_id_iter(diag_expr_id)` to the statement of the current block.
- `guards.rs`: a visitor with no nested-body filter that classifies
  `ExprKind::If`, `Match` (by `MatchSource`), `Loop` (by `LoopSource`), `Ret`,
  `Break`/`Continue` (escaping or contained, by destination target), `Yield`,
  `Become`, `Closure` (plain or coroutine), `Let` with `els`, nested
  `StmtKind::Item`, and never-typed statement expressions; and a pure
  classification helper per HIR enum, unit-tested with `rstest` cases.
- `form.rs`: `StmtForm` classification from `StmtKind` and `PatKind`.

Create `src/driver/summary.rs` with the summary renderer and writer. The
renderer names each local by `cx.tcx.hir_name(hir_id)` plus a per-body
first-appearance ordinal (`x#0`, `x#1`) so shadowing is visible and the output
is deterministic. The writer follows `write_collection_summary` in the sibling
driver exactly (UTF-8 path from
`WHITAKER_SINGLE_BINDING_PARAGRAPH_STMT_IO_SUMMARY`, `cap_std` directory
capability, append mode). Wire `check_fn` (or `check_body`) to call
`lower_body` and, when the environment variable is present, accumulate rendered
lines for `check_crate_post` to write. When the variable is absent the model is
still computed and discarded, so the zero-diagnostic example runs the full
adapter.

Add `examples/stmt_io_model.rs` (fixture) and its `tests/ui` trybuild twin, and
the snapshot test `example_harness_records_statement_io` in `tests/ui.rs`,
which runs the example with the variable set through `with_env_var` and asserts
with `insta::assert_snapshot!`.

Red: write the expected snapshot by hand from the fixture comments and commit
nothing until the test fails because the summary file is missing. Green:
implement the adapter until the snapshot matches; review every changed line of
the snapshot against the fixture comment, never accept blindly. Refactor: keep
visitors small; run the crate suite. Validation: `make check-fmt`, `make lint`,
`make test`. Commit.

### Stage F — EP-M5: documentation and close-out

- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: add
  "Implementation decisions (8.3.1)" recording `DEV-1` to `DEV-3`, the guard
  semantics, the `uses`/`muts` definitions, the aliasing and interior
  mutability limits, the Verus boundary, and a reference to ADR-0014. Update
  the §Statement I/O model code block so it no longer shows `BTreeSet<HirId>`.
- `docs/adrs/adr-0014-statement-io-model-for-paragraph-lints.md`: record the
  decision to host a generic, compiler-free statement I/O model and its
  composition algebra in `whitaker_common::paragraph`, with HIR lowering in the
  lint crate and a Verus sidecar for the algebra. Re-check the number against
  all fetched remote branches first. Add it to `docs/contents.md` under
  "Decision records".
- `docs/developers-guide.md`: document `whitaker_common::paragraph` (scope,
  permitted call sites — Lint B and Lint C drivers and their tests —
  composition rules, and the rule that the Verus sidecar must change with
  `then`), the summary hook, the new `verus-paragraph` target under "Proof
  workflows", and the `rustc_hir_typeck` proxy.
- `docs/whitaker-dylint-suite-design.md` §2: add a short paragraph on the
  `paragraph` module.
- `docs/repository-layout.md`: list `single_binding_paragraph/` and
  `rustc_hir_typeck/` under `crates/` and `paragraph_io_summary.rs` under
  verification assets.
- `docs/roadmap.md`: mark 8.3.1 done with a short status note.
- `docs/users-guide.md`: unchanged (see `Decision log`).

Validation: `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
`make lint`, `make test`, `make verus-paragraph`. Commit.

## Milestones and plateaus

- `EP-M1` — the generic model and composition exist in
  `whitaker_common::paragraph` with unit, property, and behavioural tests.
  - Requirements: `LB-IO`, `DEV-2`, `DEV-3`.
  - Acceptance: `cargo nextest run -p whitaker-common paragraph` passes; the
    BDD feature passes; gates pass.
  - Conformance check:
    - `LB-IO` facts and determinism covered by `OB-B1`, `OB-B2`.
    - Design followed except recorded deviations.
    - `AX-SCOPE` still the only scoping assumption.
    - Only the approved `paragraph` module added to the public API.
    - No trust-boundary or persisted-format change.
    - Trace links updated in `Conformance basis`.
  - Recovery: revert the milestone commit; nothing depends on it yet.
  - Remaining gaps: proof, adapter.
  - Compatibility decision: none (new, pre-1.0 internal API).

- `EP-M2` — the Verus sidecar proves `OB-C1` to `OB-C5`.
  - Requirements: `LB-IO` → `LB-SLICE` contract.
  - Acceptance: `make verus-paragraph` and `make verus` report zero errors;
    `rg -c 'assume|admit|external_body' verus/paragraph_io_summary.rs` prints
    nothing.
  - Conformance check: proof mirrors `then` exactly; developers' guide
    mirroring rule drafted; no production code changed.
  - Recovery: revert the proof commit; tests still cover the algebra.
  - Remaining gaps: adapter.
  - Compatibility decision: none.

- `EP-M3` — the lint crate and the proxy crate build, and the lint is silent.
  - Requirements: `LB-INTENT`, `INT-LATE`.
  - Acceptance: `example_compiles_without_diagnostics` and
    `trybuild_fixtures_compile_without_diagnostics` pass; gates pass.
  - Conformance check: no suite, installer, or CLI change; only the approved
    proxy dependency added.
  - Recovery: revert; no consumers.
  - Remaining gaps: adapter.
  - Compatibility decision: none.

- `EP-M4` — the adapter lowers real HIR into the model, observed by snapshot.
  - Requirements: `LB-IO`, `LB-RULE` (facts only), `INT-SPAN`, `NG-1`,
    `DEV-1`.
  - Acceptance: `example_harness_records_statement_io` passes with a reviewed
    snapshot; `wf=ok` on every block; gates pass.
  - Conformance check: HIR and typeck only, no MIR; silence preserved.
  - Recovery: revert the adapter commit; EP-M3 remains a valid plateau.
  - Remaining gaps: documentation.
  - Compatibility decision: none.

- `EP-M5` — documentation, ADR, and roadmap reflect the delivered design.
  - Acceptance: Markdown gates and all code gates pass; roadmap 8.3.1 ticked.
  - Conformance check: every deviation recorded in the design document and
    ADR; trace links final.
  - Recovery: documentation-only revert.
  - Compatibility decision: none.

## Concrete steps

Run every command from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/d6d34f12-f30c-4f75-a217-e1be1d9a4345`.
Capture long output with `tee` as `AGENTS.md` and the user's instructions
require, for example:

```sh
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BRANCH}.out"
make verus-paragraph 2>&1 | tee "/tmp/verus-paragraph-whitaker-${BRANCH}.out"
```

Focused loops while iterating:

```sh
cargo nextest run -p whitaker-common paragraph
cargo nextest run -p whitaker-common --test paragraph_io_behaviour
cargo nextest run -p single_binding_paragraph --all-targets --all-features
./scripts/run-verus.sh paragraph
```

Expected focused output after EP-M4 (abridged):

```plaintext
    PASS [   0.012s] whitaker-common paragraph::tests::builder::define_records_def
    PASS [  42.517s] single_binding_paragraph::ui example_harness_records_statement_io
     Summary [ ... ] N tests run: N passed, 0 skipped
```

Expected Verus output after EP-M2:

```plaintext
verification results:: N verified, 0 errors
```

Gate runs are delegated to the `scrutineer` agent where available; when it
reports a failure, read the cited log rather than re-running the gate.

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes, including the new `whitaker-common` unit,
  property, and behavioural tests and the new crate's UI tests.
- Verification: `OB-C1` to `OB-C5` discharged by `make verus-paragraph`;
  `OB-B1`, `OB-B2`, and the `OB-C*` conformance discharged by `proptest`;
  `OB-A1` to `OB-A3` discharged by the reviewed snapshot and silence tests.
- Lint and format: `make check-fmt` and `make lint` pass; Markdown gates
  (`make markdownlint`, `make nixie`) pass after documentation changes.

Behavioural specification for EP-M1, in
`common/tests/features/paragraph_io.feature`:

```gherkin
Feature: Paragraph I/O summaries
  Statement facts compose into a paragraph summary that reports what the
  paragraph needs from outside and what it changes outside.

  Scenario: A later statement consumes an earlier definition
    Given a statement that defines "a" and uses "seed"
    And a statement that defines "b" and uses "a"
    When the statements are summarised in order
    Then the paragraph inputs are "seed"
    And the paragraph defines "a" and "b"

  Scenario: Mutating an outside local makes it an input and an external mutation
    Given a statement that uses and mutates "items"
    When the statements are summarised in order
    Then the paragraph inputs are "items"
    And the paragraph externally mutates "items"

  Scenario: Mutating a local the paragraph defined stays internal
    Given a statement that defines "v"
    And a statement that uses and mutates "v"
    When the statements are summarised in order
    Then the paragraph has no external mutations

  Scenario: Guards accumulate across statements
    Given a statement guarded by "closure"
    And a statement guarded by "branch"
    When the statements are summarised in order
    Then the paragraph guards are "branch" and "closure"

  Scenario: Building backwards matches building forwards
    Given three statements forming a chain from "seed" to "out"
    When the statements are summarised by prepending from the last
    Then the summary equals the in-order summary
```

The fixture `crates/single_binding_paragraph/examples/stmt_io_model.rs` must
include, each with a comment stating the expected summary line:

- a simple chain of `let` statements ending in a `let out = …;` sink
  (`form=let(out#0)`);
- a destructuring `let (a, b) = …;` (`form=pattern-let`, two defs);
- shadowing (`let x = 1; let x = x + 1;` showing `x#0` and `x#1`);
- `items.push(seed);` (auto-reference mutation), `count += 1;`,
  `total = 0;` after `let total;`, `point.x = 3;`, `values[0] = 1;`, and
  `let r = &mut v;`;
- a closure that captures by mutable reference (`Closure` guard; captured
  local in `uses` and `muts`);
- `if`, `match`, `matches!` (`Branch`), `while` and `for` (`Loop`), a `loop`
  with a contained `break value` (only `Loop`), `return` and `?`
  (`EscapingExit`), `let … else { return; }` (`LetElse` and `EscapingExit`),
  `panic!` (`Diverging`), an `async` block and an `.await` inside an `async fn`
  (`AsyncBlock`, `Await`), a nested `fn` item (`NestedItem`);
- `vec![a, b]` and `format!("{x}")` (uses recorded through expansions, no
  guard), `assert!(x > 0)` (`Branch` from the expansion), and a statement
  generated entirely by a local `macro_rules!` (`MacroOnly`);
- a nested block expression `let y = { let t = a; t + 1 };` (`t` invisible);
- a block tail expression (`form=tail`).

Red-Green-Refactor evidence to record in `Artefacts and notes`:

- EP-M1 red: `cargo nextest run -p whitaker-common paragraph` fails with
  assertion failures in builder and summary tests (stub bodies).
- EP-M2 red: `./scripts/run-verus.sh paragraph` reports failing
  postconditions for the empty lemma bodies.
- EP-M3 red: `cargo nextest run -p single_binding_paragraph` fails to build.
- EP-M4 red: `example_harness_records_statement_io` fails because the summary
  file does not exist.
- Each green and refactor step: the focused command passes, then the three
  repository gates pass.

## Idempotence and recovery

Every step is additive and re-runnable. Re-running the example harness appends
to a fresh, uniquely named summary file (follow the sibling's
`unique_summary_path`). `insta` pending snapshots (`*.snap.new`) must be
reviewed and either accepted or deleted, never committed. If a gate fails
halfway through a milestone, fix and re-run that gate; do not amend a commit
after gating without re-running the gates (`^{tree}` must match the gated
tree). Each milestone is one or more commits that can be reverted independently
in reverse order.

## Artefacts and notes

Record here, during implementation, the red and green transcripts, the Verus
summary line, the `rg` zero count for proof shortcuts, the negative-control
outcomes, and the final snapshot excerpt for `items.push(seed);`.

## Interfaces and dependencies

In `common/src/paragraph/guard.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GuardKind {
    Branch,
    Loop,
    EscapingExit,
    Diverging,
    Await,
    Closure,
    AsyncBlock,
    LetElse,
    NestedItem,
    MacroOnly,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuardSet(std::collections::BTreeSet<GuardKind>);

impl GuardSet {
    pub fn contains(&self, kind: GuardKind) -> bool;
    pub fn is_empty(&self) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = GuardKind> + '_;
    pub fn union(&self, other: &Self) -> Self;
}
```

In `common/src/paragraph/stmt_io.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StmtForm<L> {
    SimpleLet { binding: L },
    PatternLet,
    Expr,
    Semi,
    Item,
    Tail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StmtIo<L: Ord> { /* private fields */ }

impl<L: Ord + Copy> StmtIo<L> {
    pub fn form(&self) -> StmtForm<L>;
    pub fn defs(&self) -> &std::collections::BTreeSet<L>;
    pub fn uses(&self) -> &std::collections::BTreeSet<L>;
    pub fn muts(&self) -> &std::collections::BTreeSet<L>;
    pub fn guards(&self) -> &GuardSet;
}

pub struct StmtIoBuilder<L: Ord> { /* private fields */ }

impl<L: Ord + Copy> StmtIoBuilder<L> {
    pub fn new(form: StmtForm<L>) -> Self;
    pub fn define(&mut self, local: L);
    pub fn bind_internal(&mut self, local: L);
    pub fn reference(&mut self, local: L);
    pub fn mutate(&mut self, local: L);
    pub fn guard(&mut self, kind: GuardKind);
    pub fn build(self) -> StmtIo<L>;
}
```

`StmtIoBuilder::new` with `StmtForm::SimpleLet { binding }` records `binding`
as a def. `mutate` also records a reference, so `muts ⊆ uses` holds by
construction. `build` removes internal bindings and top-level defs from `uses`
and `muts`.

In `common/src/paragraph/summary.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphIo<L: Ord> { /* private: defs, inputs, external_muts, guards */ }

impl<L: Ord + Copy> ParagraphIo<L> {
    pub fn empty() -> Self;
    pub fn from_stmt(stmt: &StmtIo<L>) -> Self;
    pub fn then(&self, next: &Self) -> Self;
    pub fn prepend(&self, earlier: &StmtIo<L>) -> Self;
    pub fn summarize<'a, I>(stmts: I) -> Self
    where
        I: IntoIterator<Item = &'a StmtIo<L>>,
        L: 'a;
    pub fn defs(&self) -> &std::collections::BTreeSet<L>;
    pub fn inputs(&self) -> &std::collections::BTreeSet<L>;
    pub fn external_muts(&self) -> &std::collections::BTreeSet<L>;
    pub fn guards(&self) -> &GuardSet;
}
```

In `crates/single_binding_paragraph/src/stmt_io/mod.rs` (gated by
`dylint-driver`, crate-private):

```rust
pub(crate) struct BlockIo {
    pub(crate) span: rustc_span::Span,
    pub(crate) stmts: Vec<whitaker_common::paragraph::StmtIo<rustc_hir::ItemLocalId>>,
    pub(crate) tail: Option<whitaker_common::paragraph::StmtIo<rustc_hir::ItemLocalId>>,
}

pub(crate) fn lower_body<'tcx>(
    cx: &rustc_lint::LateContext<'tcx>,
    body: &'tcx rustc_hir::Body<'tcx>,
) -> Vec<BlockIo>;
```

In `crates/single_binding_paragraph/src/driver/mod.rs`:

```rust
dylint_linting::impl_late_lint! {
    pub SINGLE_BINDING_PARAGRAPH,
    Warn,
    "contiguous statements that compute one binding could be extracted",
    SingleBindingParagraph::default()
}
```

Dependencies: `rustc_hir_typeck` through the new workspace proxy crate; all
other crates already exist in the workspace. No new third-party crates.

## Revision note

- 2026-11-11: initial draft from reconnaissance and prior-art research.
