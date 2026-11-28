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
overwrites, and modifies in place, and whether it contains control flow that
makes extraction unsafe. That per-statement fact sheet is the *statement I/O
model*. Roadmap item 8.3.2 will run a backward slice over it; 8.3.3 will emit
diagnostics; roadmap item 8.4.2 (Lint C) will reuse the same candidates.

After this change:

1. A new Dylint crate, `crates/single_binding_paragraph`, declares the
   `SINGLE_BINDING_PARAGRAPH` lint and runs a late pass over every function,
   closure, and coroutine body. It emits no diagnostics yet. This is the same
   *diagnostic-silent* bootstrap shape that roadmap items 8.2.1 and 8.2.2 used
   for Lint A.
2. A compiler-free, generic statement I/O model lives in
   `whitaker_common::paragraph`. It records, per statement, the `defs` (locals
   the statement binds), `kills` (free locals the statement overwrites whole
   without reading), `uses` (free locals the statement reads), `muts` (free
   locals the statement modifies in place, each tagged with how), and a set of
   *guards* (reasons the statement may not be moved into a helper). It also
   defines how statement facts compose over a contiguous range of statements
   (the *paragraph algebra*), which 8.3.2's slicer and 8.4.2's Lint C rely on.
3. A thin High-level Intermediate Representation (HIR) adapter inside the lint
   crate lowers each user-written `rustc_hir::Block` into that model, using
   rustc's `ExprUseVisitor` for mutation facts.
4. A machine-checked Verus proof establishes the paragraph algebra: identity,
   associativity, agreement of the incremental fold with a declarative
   definition, the backward-slice update rule that 8.3.2 will use, and
   preservation of well-formedness.
5. A test-only, environment-gated summary hook lets the UI harness observe the
   model computed by real rustc over a fixture, and an `insta` snapshot pins
   that observation.

Observable success: running

```sh
cargo nextest run -p single_binding_paragraph --all-targets --all-features
```

passes, including the snapshot test `example_harness_records_statement_io`,
whose snapshot shows, for the fixture statement `items.push(seed);`, the line

```plaintext
L23 semi defs=[] kills=[] uses=[items#0, seed#0] muts=[items#0:receiver] guards=[]
```

and, for `total = compute(seed);`, a line with `kills=[total#0]` and
`uses=[seed#0]`. Running `make verus-paragraph` reports every lemma in
`verus/paragraph_io_summary.rs` verified with zero errors. `make check-fmt`,
`make lint`, and `make test` pass, and the lint emits no diagnostics on any
fixture.

## Relevant documentation and skills

Read these repository documents before starting each stage:

- `docs/roadmap.md` §8.3 for the exact scope split between 8.3.1, 8.3.2,
  8.3.3, 8.3.4, and 8.5.1.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` §Lint B (formal rule,
  backward-slice algorithm, statement I/O model, configuration example), §Lint
  C (assertion detection, which interacts with `Branch` guards from `assert!`),
  the §Implementation decisions (8.1.2), (8.2.1), and (8.2.2) sections, and
  §Non-goals and boundaries.
- `docs/execplans/8-2-1-create-the-rstest-helper-lint-crate.md` and
  `docs/execplans/8-2-2-call-site-collection-in-rstest-tests.md` for the
  diagnostic-silent bootstrap, the test-only summary hook, and the Dylint
  example harness lessons.
- `docs/whitaker-dylint-suite-design.md` §1 (workspace layout), §2 (common
  crate), and §7 (testing strategy).
- `docs/whitaker-clone-detector-design.md` and
  `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`
  for the repository's Verus *sidecar model* convention and trust-boundary
  wording.
- `docs/developers-guide.md` §"Proof workflows", §"Verus scope and trust
  boundary", §"Module layout and test placement", §"The 400-line module
  budget", §"Standard vs Experimental Lints", §"Adding a new lint", and §"UI
  test fixtures".
- `docs/adrs/adr-0006-directory-based-module-layout.md` for `mod.rs`-rooted
  directory modules and `#[path]` sibling test files.
- `docs/rust-testing-with-rstest-fixtures.md` for fixture and `#[case]`
  structure.
- `docs/rstest-bdd-users-guide.md` for the `rstest-bdd` feature-file and
  step-binding conventions (the workspace pins `rstest-bdd` 0.6.0).
- `docs/rust-doctest-dry-guide.md` for Rustdoc examples on the new public
  `whitaker_common::paragraph` API.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping the
  HIR visitors small and free of bumpy-road nesting.
- `docs/documentation-style-guide.md` for the ADR and design-doc format.

Load these skills during implementation:

- `execplans` to keep this document current.
- `rust-router`, then `rust-types-and-apis` (generic model API, bitset
  newtype), `rust-errors` (the builder's error enum), `rust-unit-testing`
  (`rstest`, `googletest`, `pretty_assertions`, `insta`), `proptest` (algebraic
  and builder properties), and `verus` (the sidecar proof).
- `nextest` for the per-package overrides and test groups.
- `rust-verification` only if the proof or test split needs to be revisited.
- `codegraph-mcp` and `leta` for navigation (callers of
  `recover_user_editable_hir_span`, `span_recovery_frames`, `with_env_var`, and
  `write_collection_summary`).
- `arch-decision-records` for the ADR.
- `en-gb-oxendict` for prose and comments.
- `commit-message` for each commit.

## Constraints

- Scope is roadmap item 8.3.1: crate creation, the statement I/O model, and
  the paragraph algebra it needs. The backward slice itself, the single-output
  check, the `min_len`, `max_len`, and `max_inputs` limits, and all
  configuration keys belong to 8.3.2. Diagnostics, helper-signature inference,
  and moved-versus-borrowed input classification belong to 8.3.3. UI pass/fail
  coverage belongs to 8.3.4. Suite and installer wiring belongs to 8.5.1.
  Fluent localization belongs to 8.5.2.
- The lint must emit no diagnostics in this milestone, on any input.
- `whitaker_common` must not gain a `rustc_private` dependency. The model in
  `whitaker_common::paragraph` must be generic over the local-identifier type.
- Do not modify `whitaker_common::rstest` public types (`ParagraphFingerprint`,
  `StmtShape`, `ParagraphNormalizer`, and the span-recovery types).
- Do not change the suite (`suite/`), installer (`installer/`), or command-line
  interface (`cli/`).
- The only change permitted to `crates/rstest_helper_should_be_fixture` is the
  atomic refactor that replaces its private summary writer with the shared
  helper (EP-M2), with behaviour unchanged and its tests passing before and
  after.
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

- Scope: if the change needs more than 50 non-ExecPlan files, or more than
  4,000 net added lines of code and other documentation, stop and escalate.
  This ExecPlan file is excluded from both counts. `Cargo.lock` and accepted
  `insta` snapshot files are counted as files but not as lines. The implementer
  may deliver the work as two stacked pull requests split at the EP-M2/EP-M3
  boundary; the limits apply to the sum.
- Interface: if any existing public API in `whitaker_common`,
  `whitaker_lint_core`, or `whitaker_suite` must change signature, stop and
  escalate. Adding `whitaker_common::paragraph` and
  `whitaker_lint_core::testing::summary` is approved by this plan.
- Dependencies: the plan pre-approves exactly these additions: a workspace
  proxy crate `crates/rustc_hir_typeck` that re-exports the pinned toolchain's
  `rustc_hir_typeck` (mirroring `crates/rustc_middle`); `cap-std` (already a
  workspace dependency) as a dependency of `whitaker_lint_core` for the shared
  summary writer; and the existing workspace dependencies `googletest`,
  `pretty_assertions`, and `insta` as dev-dependencies of `whitaker-common` and
  the new lint crate. Any other new dependency requires escalation.
- Toolchain: if `ExprUseVisitor::for_clippy` or the `Delegate` trait cannot be
  used from the proxy crate on `nightly-2026-05-28`, stop after one prototype
  attempt and escalate with the fallback described in `Decision log`.
- Iterations: if a focused test or proof still fails after five fix attempts,
  stop and escalate.
- Proof effort: if the Verus file exceeds 400 lines or needs trigger
  management beyond ordinary `assert(... =~= ...)` extensionality and sequence
  induction, stop and record the verification conditions before escalating.
- Test time: if the new crate's UI binary needs more than the sibling crate's
  allowances (75 minutes for the example harness, 120 minutes for the
  `trybuild` harness) under `make test`, stop and escalate.
- Ambiguity: if the design document and this plan disagree on a semantic
  point, record the options and stop.

## Risks

- Risk: `ExprUseVisitor` events are attributed to the wrong statement, for
  example a closure capture whose `diag_expr_id` lies inside the closure body,
  or `let ref mut y = x;` whose borrow is reported against the initializer.
  Severity: medium. Likelihood: medium. Mitigation: a single parent walk per
  event, bounded by the body root, attributes the event to the statement of
  *every* listed enclosing block (the ancestor statement whose parent is that
  block). Each shape has a fixture line.

- Risk: closure and coroutine bodies report captured locals as
  `PlaceBase::Upvar`, not `PlaceBase::Local`, so their mutations are silently
  lost if only `Local` is handled; and a closure that writes through a captured
  `&mut` reference borrows it as `BorrowKind::UniqueImmutable`, not `Mutable`.
  Severity: high (pre-mortem scenario 1). Likelihood: high without mitigation.
  Mitigation: map `Upvar(u)` to `u.var_path.hir_id.local_id`; treat `Mutable`
  and `UniqueImmutable` as modifications; fixture lines and a negative control
  for both.

- Risk: compiler desugarings masquerade as user statements or user control
  flow: `.await` lowers to a `Match(AwaitDesugar)` around a `Loop` containing
  `Yield(.., YieldSource::Await)` and a contained `break`; `format_args!`
  lowers to a block of `super let` statements; `async fn` bodies begin with
  `LocalSource::AsyncFn` lets. Severity: high (pre-mortem scenario 2).
  Likelihood: high without mitigation. Mitigation: classify `AwaitDesugar` as
  `Await` only and visit only its operand; treat only `YieldSource::Yield` as
  an escape; skip statements whose `LetStmt.super_` is `Some` or whose
  `LocalSource` is not `Normal`; list only blocks whose span recovers to
  `UserEditableSpan::Direct`. A pure `rstest` table pins each classification.

- Risk: `ExprUseVisitor` can raise an internal compiler error through
  `span_bug!` or `unreachable!` on unusual bodies, and `catch_unwind` cannot
  contain it because the error is reported before the panic. Severity: high
  once 8.5.1 ships the lint; nil while the crate is unregistered. Likelihood:
  low. Mitigation: skip lowering when the lint level at the body is `Allow`, so
  users have a kill switch; record the residual risk in ADR-0014.

- Risk: the Dylint example and `trybuild` harnesses exceed nextest's 300-second
  base timeout or run concurrently with other nested Cargo builds, because the
  broad `serial-dylint-ui` filter does not match plain test names such as
  `example_compiles_without_diagnostics` and the sibling's overrides are scoped
  to its own package. Severity: medium. Likelihood: certain without mitigation.
  Mitigation: add `package(single_binding_paragraph)` overrides to both nextest
  profiles, mirror them in the workflow-contract tables, and name the
  `trybuild` test so that it does not match `TRYBUILD_CONTRACT_FILTER`
  unintentionally (EP-M2).

- Risk: the snapshot is unstable across toolchain bumps (standard-library
  macro internals, lowering changes that shift `ItemLocalId` order, absolute
  paths). Severity: medium. Likelihood: medium. Mitigation: render only
  `Direct` blocks from the fixture file; sort locals by rendered name; print
  fixture-relative line numbers instead of spans; key shadowing ordinals per
  body owner.

- Risk: aliasing hides mutation (`let r = &mut v; r.push(1);` records `v` as
  modified by the borrow and `r` by the push), and interior mutability
  (`Cell::set`) records no modification. Severity: low for 8.3.1. Likelihood:
  high. Mitigation: document both as model limits. 8.3.2's slice stops at the
  first non-contributing statement, so these limits cause missed candidates
  rather than unsafe suggestions; the argument is recorded in the design
  document.

- Risk: extracting a paragraph moves its intermediates' drops to the end of
  the helper, so an intermediate with a significant destructor (for example a
  `MutexGuard`) would be released earlier. Severity: medium for 8.3.2.
  Likelihood: low. Mitigation: out of scope for 8.3.1; recorded as an open
  question for 8.3.2, which may add a per-def significant-drop fact.

- Risk: the generic model and the Verus sidecar drift apart. Severity: medium.
  Likelihood: low. Mitigation: the `proptest` conformance suite uses an
  executable transcription of the sidecar's specification with the same
  function names and a header pointing at the sidecar, and the developers'
  guide adds the rule "changing `followed_by` requires changing the sidecar".

- Risk: adding the eighth `rustc_*` proxy crate breaks workflow contracts that
  enumerate the proxies. Severity: low. Likelihood: high. Mitigation: update
  `TEST_EXCLUDES`, `DOCTEST_EXCLUDES`, the `AGENTS.md` doctest command, the root
  `Cargo.toml` proxy block, and the forbidden set in
  `tests/workflow_contracts/lint_makefile_test.py` in the same commit.

## Progress

- [x] (2026-11-11T00:00Z) Renamed branch to
  `8-3-1-single-binding-paragraph-lint-crate`.
- [x] (2026-11-11T00:00Z) Reconnaissance with a Wyvern team: sibling crate
  anatomy, registration touchpoints, span-recovery API, verification
  infrastructure; prior art (Clippy `mutated_variables`, rust-analyzer
  extract-function, Komondoor and Horwitz procedure extraction).
- [x] (2026-11-11T00:00Z) Drafted this ExecPlan.
- [x] (2026-11-11T00:00Z) Community-of-experts review (structure and
  contracts; failure modes and operations; alternatives and rigour). Verdict:
  proceed with conditions. Plan revised; see `Revision note`.
- [ ] Plan approved by a human reviewer.
- [ ] EP-M1: pure statement I/O model and paragraph algebra in
  `whitaker_common::paragraph`.
- [ ] EP-M2: shared summary writer refactor, proxy crate, lint crate
  bootstrap, and nextest wiring.
- [ ] EP-M3: HIR adapter and environment-gated summary snapshot.
- [ ] EP-M4: Verus proof of the paragraph algebra.
- [ ] EP-M5: documentation, ADR, roadmap update, and final gates.

## Surprises & discoveries

- Observation: `rustc_hir::HirId` does not implement `Ord` on the pinned
  `nightly-2026-05-28` toolchain. Evidence: `rustc_hir_id/src/lib.rs` derives
  only `Copy, Clone, PartialEq, Eq, Hash, Encodable, Decodable, StableHash` for
  `HirId`, while `ItemLocalId` is declared `#[orderable]` in the same file.
  Impact: the design document's `BTreeSet<rustc_hir::HirId>` would not compile.
  The adapter keys the model on `ItemLocalId`. Closures and coroutines are not
  HIR owners (`OwnerNode` has no closure variant), so every local referenced
  inside one item, including inside its closures, shares the item's `OwnerId`.

- Observation: the workspace has no `rustc_hir_typeck` proxy crate and no use
  of `ExprUseVisitor`, and `crates/clippy_utils` is a local stub exposing only
  `macros::is_panic`. Impact: mutation detection needs a new proxy crate.

- Observation: `ExprUseVisitor`'s `Delegate::bind` defaults to `mutate`, and
  `Delegate::copy` defaults to an immutable `borrow`. Evidence:
  `rustc_hir_typeck/src/expr_use_visitor.rs` lines 81 to 98 on the pinned
  toolchain. Impact: the delegate overrides `bind` to do nothing, so binding
  events are not mistaken for modifications.

- Observation: the next free ADR number is 0014. Evidence: `docs/adrs/` on
  `main` ends at 0012, and remote branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces` already
  allocates `adr-0013`. Impact: allocate 0014, re-checking all fetched remote
  branches immediately before committing the ADR.

- Observation: the `AGENTS.md` doctest command has already drifted from
  `Makefile` `DOCTEST_EXCLUDES` (each lists crates the other omits). Impact:
  this plan adds the new crates to both but does not reconcile the existing
  drift, which is outside 8.3.1's scope; the drift is noted for a separate
  housekeeping change.

## Decision log

- Decision: keep the crate diagnostic-silent and unregistered in the suite and
  installer until roadmap item 8.5.1. Rationale: the 8.3.1 roadmap text says
  "create the crate and implement the model"; unlike 8.2.1 and 8.4.1 it does
  not say "register". `define_suite_pass!` in `suite/src/driver.rs` handles one
  optional late pass through two `cfg` arms, so each further experimental late
  pass doubles the arms; that combinatorial problem should be solved once in
  8.5.1 for Lints B and C. The design document records this for 8.5.1, and
  notes that 8.5.1's "all three lints" wording is stale because 8.2.1 already
  wired Lint A. Date/Author: 2026-11-11, planning agent.

- Decision: place the compiler-free model and paragraph algebra in a new
  `whitaker_common::paragraph` module, generic over a local-identifier type,
  and keep the HIR adapter in the lint crate. Commit 8.3.2's pure slicer to the
  same module. Rationale: Lint C (8.4.2) must reuse Lint B's candidate
  generation, and lint crates are `cdylib` libraries that should not depend on
  each other. Genericity makes the model testable with small integer
  identifiers, property-testable, and mirrorable in Verus. The 8.2.2 precedent
  keeps compiler-aware adapters in the lint crate until a second consumer
  exists; 8.4.2 may promote the adapter to `whitaker_lint_core::hir` in a
  separate refactor. The module is named `paragraph`, not placed under
  `rstest`, because Lint B applies to all function bodies. Its types are not
  re-exported at the crate root, so `paragraph::StmtForm` is not confused with
  `rstest::StmtShape`. Date/Author: 2026-11-11, planning agent; boundary
  confirmed by expert review.

- Decision: distinguish whole-local overwrites (`kills`) from in-place
  modifications (`muts`), and tag each modification with a `MutationKind`
  (`CompoundAssign`, `ProjectionWrite`, `MutBorrow`, `MethodReceiver`).
  Rationale: two reviewers independently found that one undifferentiated `muts`
  set cannot support the design's `treat_mutating_method_calls_as_defs` key,
  and that counting `total = 0;` as a read makes an uninitialized `total` a
  paragraph input. A plain `x = e;` with no projection is a kill and is not a
  read unless `e` mentions `x`. `x += e`, `x.f = e`, `x[i] = e`, `&mut x`,
  `ref mut` bindings, mutable closure captures, and auto-referenced `&mut self`
  receivers are modifications and also reads, because the prior value survives
  in part. Date/Author: 2026-11-11, revised after expert review.

- Decision: define `uses` as the free locals a statement reads, and enforce
  `muts ⊆ uses` by construction. "Free" means referenced but not bound inside
  the statement, so locals bound by nested `let`, closure parameters, or match
  arms inside the statement are invisible to the paragraph. A statement that
  both binds and reads the same local at top level is an error from the builder
  rather than a silently repaired fact, so the boundary well-formedness check
  can genuinely fail. Date/Author: 2026-11-11, revised after expert review.

- Decision: model guards as a `GuardSet` bitset of `GuardKind` values rather
  than the design document's three booleans. Rationale: 8.3.2 applies
  configurable policy (`reject_closures`, `reject_async`,
  `skip_external_macro_expansions`) as masks; a `Copy` bitset with
  `is_disjoint` makes that a one-line test, and avoids boolean soup. The kinds
  are `Branch`, `Loop`, `EscapingExit`, `Diverging`, `Await`, `Closure`,
  `AsyncBlock`, `LetElse`, `MacroGenerated`, and `ExternalMacroGenerated`. A
  nested item statement is represented by `StmtForm::Item`, which 8.3.2 always
  treats as a paragraph boundary, rather than by a duplicate guard. An `unsafe`
  block is not a guard, because extracting it does not change control flow.
  Date/Author: 2026-11-11, revised after expert review.

- Decision: record guards found in the statement's own body, including inside
  macro expansions such as `assert!` and `matches!`, but not inside nested
  closure or coroutine bodies. A `break` or `continue` whose target loop or
  labelled block was entered within the statement records only `Loop`; any other
  `break` or `continue`, including an unresolved target, plus `return`, `?`,
  `become`, and `yield` from `YieldSource::Yield`, records `EscapingExit`.
  `.await` records only `Await`. Rationale: control flow that cannot leave the
  statement cannot make extraction unsafe, and recording it would make
  `reject_closures = false` meaningless; macro-internal control flow is
  recorded because the user cannot see it. The design document notes for 8.4.2
  that `assert!` therefore yields a `Branch` guard. Date/Author: 2026-11-11,
  revised after expert review.

- Decision: obtain modification and kill facts from
  `rustc_hir_typeck::expr_use_visitor` via `ExprUseVisitor::for_clippy`, run
  once per body in `check_fn` with `consume_body`. Classify a `mutate` event as
  a kill when the place has no projections and the assignee's parent is
  `ExprKind::Assign`; as `CompoundAssign` when the parent is
  `ExprKind::AssignOp`; and as `ProjectionWrite` otherwise. Classify a `borrow`
  with `BorrowKind::Mutable` or `BorrowKind::UniqueImmutable` as
  `MethodReceiver` when it comes from an auto-reference adjustment on a method
  receiver, and as `MutBorrow` otherwise. Map `PlaceBase::Upvar` to its
  variable's `ItemLocalId`. Rationale: `ExprUseVisitor` already accounts for
  adjustments, overloaded `IndexMut`, `ref mut` patterns, and closure captures;
  Clippy's `clippy_utils::usage::mutated_variables` is the prior art. Fallback
  if the proxy cannot link: a syntactic visitor over `Assign`, `AssignOp`,
  `AddrOf(Mut)`, and `typeck_results().expr_adjustments`; choosing it requires
  escalation. Date/Author: 2026-11-11, revised after expert review.

- Decision: include the paragraph algebra (`followed_by`, `preceded_by`,
  `summarize`) in 8.3.1 as its own milestone deliverable and prove it in Verus,
  but run the proof milestone after the adapter (EP-M4 after EP-M3). Rationale:
  8.3.2 and Lint C both depend on the algebra, so pressure-testing it now
  shapes the API; but the adapter snapshot is what validates the field set, and
  proving an algebra over the wrong fields would waste the proof. The
  statements of the lemmas are fixed in this plan (proof-sketch first); their
  proofs follow the snapshot. Date/Author: 2026-11-11, revised after expert
  review.

- Decision: discharge the backward-slice update rule (H-SLICE-2 in the draft)
  in 8.3.1 as obligation `OB-C6`, and keep the slice's safety lemma for 8.3.2.
  Rationale: the update rule needs only `preceded_by`, and fixing its operand
  order now is exactly the API pressure test the proof-first approach asks for:
  with kills, `(needed ∪ U) \ (D ∪ K)` is wrong for `x = x + 1;` while
  `(needed \ (D ∪ K)) ∪ U` is right. Date/Author: 2026-11-11, added after
  expert review.

- Decision: extract the environment-gated summary writer from
  `crates/rstest_helper_should_be_fixture/src/driver.rs`
  (`write_collection_summary`) into
  `whitaker_lint_core::testing::summary::append_summary_if_requested` as an
  atomic refactor commit before the new crate uses it. Rationale: the
  `AGENTS.md` abstraction policy forbids copying the same `cap_std` writer into
  a second driver; the writer is about 40 lines and has one existing caller.
  Date/Author: 2026-11-11, added after expert review.

- Decision: do not copy the sibling crate's `ExampleHarnessLock` (about 800
  production and 620 test lines). Serialize the new crate's harnesses through
  the `serial-dylint-ui` nextest group with `max-threads = 1`, as
  `no_expect_outside_tests` already does. Rationale: within one nextest run the
  group gives the same protection; the lock only adds protection across
  separate processes sharing one target directory, which `make test` does not
  do. That residual risk is accepted and recorded; if contention appears, the
  remedy is promoting the lock to shared test support in a separate change, not
  copying it. Date/Author: 2026-11-11, decided after expert review.

- Decision: do not use Kani for 8.3.1. Rationale: the algebra's obligations
  are unbounded set and sequence identities, which Verus proves for all
  lengths; the executable model uses `BTreeSet`, which is expensive for bounded
  model checking and adds nothing that `proptest` conformance does not already
  cover. Date/Author: 2026-11-11, confirmed by expert review.

- Decision: defer a `moves` fact (by-value consumption of inputs) and a
  significant-drop fact to 8.3.3 and 8.3.2 respectively. Rationale: neither is
  needed to define defs, kills, uses, modifications, or guards; both are
  additive fields recorded from the same `Delegate` (`consume`) or from type
  queries, so adding them later does not disturb the algebra. Date/Author:
  2026-11-11, decided after expert review.

- Decision: leave `docs/users-guide.md` unchanged in 8.3.1. Rationale: the lint
  is not installable, emits nothing, and has no configuration; there is no
  user-visible behaviour to describe. 8.5.1 and 8.5.3 own the user-facing
  documentation. The `whitaker_common::paragraph` API is an internal interface
  documented in the developers' guide and the design document. Date/Author:
  2026-11-11, planning agent; confirmed by expert review.

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
`ItemLocalId` is orderable. `check_fn` runs once for every function, closure,
and coroutine body (`rustc_lint/src/late.rs`).

Key existing code:

- `crates/rstest_helper_should_be_fixture/` is the closest sibling. Its
  `Cargo.toml` shows the feature layout (`dylint-driver` turning on every
  optional compiler and Dylint dependency; `constituent` for suite builds;
  `crate-type = ["cdylib", "rlib"]`; `test = false`). `src/driver.rs` declares
  the lint with `dylint_linting::impl_late_lint!` inside a private
  `mod declaration`, and `check_crate_post` writes the test-only summary
  (`write_collection_summary`) using `cap_std` and `camino`. `tests/ui.rs` runs
  `examples/*.rs` through `dylint_testing::ui::Test::example` and asserts on
  the summary using `with_env_var`.
- `crates/bumpy_road_function/` shows an ungated pure module beside a
  `dylint-driver`-gated driver.
- `crates/whitaker_lint_core/src/hir/mod.rs` provides
  `span_recovery_frames(span)`; feeding the frames into
  `whitaker_common::recover_user_editable_span` yields
  `UserEditableSpan::{Direct, Recovered, MacroOnly}`.
  `recover_user_editable_hir_span` returns only `Option<Span>` and cannot
  distinguish `Direct` from `Recovered`, so the adapter uses the frames.
- `crates/rustc_middle/` is a proxy crate (`extern crate rustc_driver;`,
  `extern crate rustc_middle as upstream; pub use upstream::*;`). It is one of
  seven proxies; `crates/rustc_hir_typeck/` becomes the eighth.
- `common/src/rstest/paragraph_fingerprint.rs` holds Lint C's fingerprint
  types, keyed by local *names*; this plan does not touch them.
- `verus/*.rs` holds standalone Verus sidecar proofs. `scripts/run-verus.sh`
  lists them per group; `make verus` runs all groups. Proofs run in CI only in
  `.github/workflows/proofs.yml`, whose path filter already includes
  `common/**` and `verus/**`.
- `.config/nextest.toml` defines `[profile.default]` and `[profile.ci]`, the
  `serial-dylint-ui` group, and per-package overrides for the sibling's example
  and `trybuild` harnesses. `tests/workflow_contracts/nextest_overrides.py` and
  `crates/whitaker_lint_core/tests/nextest_ui_filter.rs` check those overrides
  exactly. `Makefile` `TRYBUILD_CONTRACT_FILTER` selects `trybuild` tests by
  name.
- `Makefile` `TEST_EXCLUDES` and `DOCTEST_EXCLUDES` list crates that cannot run
  ordinary tests or doctests because they link `rustc_private`.

Terms used in this plan:

- *Paragraph*: a contiguous range of statements in one block.
- *Free local*: a local referenced by a statement whose binding is outside
  that statement.
- *Kill*: an assignment that replaces a local's whole value without reading
  it.
- *Modification*: a write that keeps part of the old value or exposes it
  through a mutable reference.
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
  - `LB-CFG`: §Configuration example for lint B (the model must make
    `treat_mutating_method_calls_as_defs`, `reject_closures`, `reject_async`,
    and `skip_external_macro_expansions` computable).
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
- `DEV-2`: the three booleans become a `GuardSet` of named kinds.
- `DEV-3`: the model adds `StmtForm` (so 8.3.2 can recognize a simple `let`
  sink) and the paragraph algebra.
- `DEV-4`: the model splits writes into `kills` and kind-tagged `muts`.

Trace links:

```plaintext
RM-8.3.1 -> LB-INTENT -> EP-M2 -> ui::example_compiles_without_diagnostics
RM-8.3.1 -> LB-IO + DEV-4 -> EP-M1 -> paragraph::tests::builder (named cases)
LB-IO (determinism) -> EP-M1 -> paragraph::tests::builder_properties::build_is_order_independent
LB-CFG -> EP-M1 -> GuardSet masks + MutationKind tags (Stage A go/no-go check)
LB-IO -> LB-SLICE -> EP-M1 + EP-M4 -> verus/paragraph_io_summary.rs + paragraph::tests::summary_properties
LB-RULE (control flow, macro) -> EP-M3 -> example_harness_records_statement_io snapshot
INT-LATE + INT-SPAN -> EP-M3 -> check_fn lowering + span_recovery_frames (Direct blocks only)
NG-1 -> EP-M3 -> adapter uses HIR + ExprUseVisitor only (no MIR)
```

## Verification plan

The model has two layers with different obligations. The *pure layer*
(`whitaker_common::paragraph`) owns set-algebraic contracts that hold for all
inputs; these get unit tests, property tests, and, for the paragraph algebra, a
Verus proof. The *adapter layer* (HIR lowering) owns the correspondence between
Rust source and model facts; that correspondence depends on rustc's semantics,
which we treat as axioms and exercise end to end through a snapshot over real
compiler output.

### Abstract specification

Let `L` be a set of local identifiers and `G` the finite set of guard kinds. A
statement fact is `s = (form, D, K, U, M, Γ)` where `D, K, U ⊆ L` are defs,
kills, and uses; `M ⊆ L` is the domain of the modification map (kinds are
carried alongside but play no part in the algebra); and `Γ ⊆ G` the guards.

A statement fact is *well formed* when:

- `WF-1`: `M ⊆ U` (every modified free local is also read).
- `WF-2`: `D ∩ (U ∪ K ∪ M) = ∅` (a `let` pattern's bindings are not in scope
  in its own initializer or `else` block).
- `WF-3`: if `form = SimpleLet(x)` then `D = {x}`.

Kills may overlap uses: `x = x + 1;` reads and kills `x`.

A paragraph fact is `p = (D, K, I, X, Γ)`: all defs, all kills, external
inputs, external modifications, and guards. The empty paragraph is
`ε = (∅, ∅, ∅, ∅, ∅)`. Lifting a statement gives `lift(s) = (D, K, U, M, Γ)`.
Write `P(p) = p.D ∪ p.K` for the locals `p` *produces*. Sequential composition
is:

```plaintext
followed_by(p, q) = ( p.D ∪ q.D,
                      p.K ∪ q.K,
                      p.I ∪ (q.I \ P(p)),
                      p.X ∪ (q.X \ P(p)),
                      p.Γ ∪ q.Γ )
```

External overwrites, the extra outputs 8.3.2's single-output check must reject,
are derived as `K \ D`.

For a sequence `S = s_0 … s_{n-1}` the declarative summary is:

```plaintext
spec_D(S) = ⋃_j D_j            spec_K(S) = ⋃_j K_j
spec_I(S) = { u | ∃ j. u ∈ U_j ∧ ∀ k < j. u ∉ D_k ∪ K_k }
spec_X(S) = { u | ∃ j. u ∈ M_j ∧ ∀ k < j. u ∉ D_k ∪ K_k }
spec_Γ(S) = ⋃_j Γ_j
```

A sequence is *use-before-def free* when no statement reads, kills, or modifies
a local that a later statement defines: `∀ j < k. (U_j ∪ K_j ∪ M_j) ∩ D_k = ∅`.
Rust's lexical scoping and the uniqueness of binding identifiers make every
real block satisfy this; that is axiom `AX-SCOPE` below.

### Obligations for the pure layer

- Obligation `OB-B1` (builder well-formedness): for every sequence of builder
  events (`define`, `bind_internal`, `read`, `kill`, `modify`, `guard`),
  `StmtIoBuilder::build` either returns a fact satisfying `WF-1` to `WF-3` with
  internal bindings absent from `D`, `K`, `U`, and `M`, or returns
  `StmtIoError::DefinedAndUsed` exactly when a top-level def was also read,
  killed, or modified, or `StmtIoError::DefinedAndInternal` exactly when one
  identifier was both defined and bound internally.
  - Method: `proptest` over generated event vectors on `u8` identifiers, plus
    named `rstest` cases for each event kind and each error.
  - Rationale: the property quantifies over arbitrary event orders on cheap,
    executable Rust; a proof would add little over the property.
  - Domain: up to 32 events over 8 identifiers. Two strategies are used: a
    fully random one, and one that always plants an internal-binding-and-read
    pair, a modify-before-read pair, and a kill-and-read pair, so the
    interesting classes occur by construction rather than by chance.
  - Artefact: `common/src/paragraph/tests/builder_properties.rs`.
  - Evidence: red — against stub bodies the property fails with a shrunk
    counterexample; green — `cargo nextest run -p whitaker-common paragraph`
    passes.
  - Non-vacuity: the planted strategy guarantees the classes. Negative
    control: remove the read insertion in `modify` and confirm the property
    fails with a `WF-1` counterexample.

- Obligation `OB-B2` (order independence): `build` is invariant under any
  permutation of the event sequence.
  - Method: `proptest` with a shuffled copy of each generated sequence.
  - Rationale: determinism is a design requirement (`LB-IO`), and the HIR
    visitors emit events in traversal order, which must not matter.
  - Artefact: same file, `build_is_order_independent`.
  - Non-vacuity: the planted strategy always contains at least three distinct
    event kinds.

- Obligation `OB-B3` (guard masks): `GuardSet` union, membership, and
  `is_disjoint` agree with a `BTreeSet<GuardKind>` model for all subsets.
  - Method: exhaustive `rstest` table is impractical (1,024 subsets per
    operand), so `proptest` over pairs of subsets with the `BTreeSet` oracle.
  - Artefact: `common/src/paragraph/tests/guard.rs`.

Paragraph-algebra obligations, stated now and proved in EP-M4:

- `OB-C1` (identity): `followed_by(ε, p) = p = followed_by(p, ε)`.
  Supporting lemma.
- `OB-C2` (associativity):
  `followed_by(followed_by(a, b), c) = followed_by(a, followed_by(b, c))`.
  Supporting lemma. Hand check: the `I` component of both sides is
  `a.I ∪ (b.I \ P(a)) ∪ (c.I \ (P(a) ∪ P(b)))`.
- `OB-C3` (fold agrees with specification, headline): folding `followed_by`
  from the left over `lift(s_0) … lift(s_{n-1})`, starting from `ε`, equals
  `(spec_D, spec_K, spec_I, spec_X, spec_Γ)` for every finite sequence, with no
  preconditions.
- `OB-C4` (backward construction, corollary of `OB-C1` and `OB-C2`): building
  the summary by repeated `preceded_by` from the last statement equals the left
  fold.
- `OB-C5a` (`X ⊆ I` preserved): if every statement satisfies `WF-1`, the
  summary satisfies `X ⊆ I`.
- `OB-C5b` (defs are never inputs): if every statement satisfies `WF-2` and
  the sequence is use-before-def free, the summary satisfies `D ∩ I = ∅`.
- `OB-C6` (slicer update rule, headline): for any paragraph `p` and statement
  `s`, `preceded_by(p, s).I = (p.I \ (D_s ∪ K_s)) ∪ U_s`. This is the
  `needed`-set update 8.3.2's backward slice will use, stated in the order that
  is correct when a statement both reads and kills a local.
  - Method for `OB-C1` to `OB-C6`: Verus sidecar proof in
    `verus/paragraph_io_summary.rs`, run by `make verus-paragraph` (new group
    `paragraph`) and `make verus`; plus `proptest` conformance of the
    executable `ParagraphIo` against an executable transcription of `spec_*`
    in `common/src/paragraph/tests/summary_properties.rs`.
  - Rationale: `OB-C3` and `OB-C5b` are claims about all sequence lengths, so
    an unbounded proof is warranted; `OB-C1`, `OB-C2`, and `OB-C4` are
    standard monoid facts kept as supporting lemmas because the induction for
    `OB-C3` and the corollary `OB-C4` use them. The `proptest` suite ties the
    executable Rust to the same specification, because the sidecar proves a
    model, not the compiled code (ADR-0003 convention).
  - Domain: Verus — all finite sequences over `Set<int>` identifiers.
    `proptest` — sequences of up to 12 statements over 8 identifiers and all
    guard kinds, generated well formed and use-before-def free for `OB-C5`,
    and unconstrained for `OB-C1` to `OB-C4` and `OB-C6`.
  - Evidence: `make verus-paragraph` prints
    `verification results:: N verified, 0 errors`; the focused `proptest`
    suite passes.
  - Red stage (proof): before writing the real composition, the sidecar's
    `followed_by` uses the naive union `p.I ∪ q.I`; Verus must reject
    `OB-C3` and `OB-C6` with a failing postcondition. Then the correct
    definition is substituted and the proofs written. Empty-body lemmas are
    not used as the red stage, because Verus can discharge `OB-C1` without a
    body.
  - Non-vacuity (proof): `lemma_well_scoped_witness` builds a concrete
    three-statement, use-before-def-free sequence (a `let`, an in-place
    modification of that binding, and a sink) and asserts the antecedents of
    `OB-C5a` and `OB-C5b` hold and that the summary has non-empty `D`, `I`, and
    `X`. Counterexample lemmas show each precondition is necessary:
    `lemma_c5a_needs_wf1` (a statement modifying an unread local gives
    `X ⊄ I`), `lemma_c5b_needs_wf2` (a statement defining and reading `x`),
    `lemma_c5b_needs_scoping` (`s_0` reads `x`, `s_1` defines `x`), and
    `lemma_c6_union_first_is_wrong` (for `x = x + 1;`,
    `(needed ∪ U) \ (D ∪ K)` drops `x`). `lemma_naive_union_disagrees` keeps
    the red-stage mutant as a standing negative control. `rg` confirms zero
    occurrences of `assume`, `admit`, and `external_body` in the file.
  - Non-vacuity (tests): the conformance generator plants a later read of an
    earlier def and an earlier kill, so the subtraction of `P(p)` matters in
    every planted case. Negative control: replacing `q.I \ P(p)` with
    `q.I \ p.D` must fail conformance on a kill-then-read case.

Lemma statements are hypotheses about the proof structure. Implementation may
show that `OB-C3` needs a helper lemma about `spec_I` of a sequence extended by
one statement, or that a generic "first-unproduced" lemma serves both `spec_I`
and `spec_X`. Record any such refinement in `Decision log`.

### Obligations for the adapter layer

- Obligation `OB-A1` (statement correspondence): for each statement shape in
  the fixture `examples/stmt_io_model.rs`, the recorded form, defs, kills,
  uses, modifications with kinds, and guards match the hand-derived expectation
  in the fixture comment beside it.
  - Method: `insta` snapshot of the environment-gated summary produced by the
    real Dylint driver, reviewed line by line against the fixture comments;
    plus named `rstest` tables for the pure classification helpers the adapter
    calls (`MatchSource`, `LoopSource`, `YieldSource`, `LocalSource`, and
    `PatKind` to `StmtForm`).
  - Rationale: the correspondence depends on rustc's HIR and typeck semantics,
    which cannot be modelled faithfully; an end-to-end observation over real
    compiler output is the strongest proportionate evidence.
  - Domain: the fixture enumerates every shape listed in
    `Validation and acceptance`.
  - Artefact: `crates/single_binding_paragraph/examples/stmt_io_model.rs`,
    `crates/single_binding_paragraph/tests/ui.rs`, and
    `crates/single_binding_paragraph/tests/snapshots/`.
  - Evidence: red — with a stub adapter that returns empty facts for every
    statement, the handwritten snapshot fails on every non-empty line; green
    — the summary matches the reviewed snapshot.
  - Non-vacuity: every fixture line carries an expectation comment. Negative
    controls, each run once and recorded: drop `UniqueImmutable` handling (the
    closure writing through a captured `&mut` must change); drop `Upvar`
    mapping (the async block modifying an outer local must change); classify
    `AwaitDesugar` as `Branch` (the `.await` line must change).

- Obligation `OB-A2` (silence): the lint emits no diagnostics on the fixture
  or on the bootstrap example.
  - Method: `dylint_testing` example run and `trybuild` pass case.
  - Artefact: `tests/ui.rs` (`example_compiles_without_diagnostics` and
    `trybuild_cases_compile_without_diagnostics`).

- Obligation `OB-A3` (well-formedness at the boundary): every statement in
  the fixture builds without `StmtIoError`, and every listed block is
  use-before-def free.
  - Method: the summary renderer writes `wf=ok` or `wf=violated(<reason>)`
    per block; the snapshot pins `wf=ok`.
  - Rationale: because `build` reports rather than repairs def/use overlap,
    this check can fail, and it exercises `AX-SCOPE` and `AX-OWNER` on real
    HIR rather than assuming them.

### Axioms

- `AX-SCOPE`: in HIR, every binding has a unique `HirId`; a `let` binding is
  not in scope in its own initializer or `else` block; a local cannot be
  referenced before the statement that binds it; and for or-patterns
  (`A(x) | B(x)`) `Res::Local` names the first alternative's binding, so defs
  are collected with `Pat::each_binding_or_first`. Checked on the fixture by
  `OB-A3`.
- `AX-OWNER`: closures and coroutines are not HIR owners, so `ItemLocalId`
  identifies locals uniquely within one item's bodies.
- `AX-EUV`: `ExprUseVisitor::consume_body` reports every place assigned,
  compound-assigned, or mutably borrowed in a body, including through
  auto-reference adjustments and closure captures, as `Delegate::mutate` or
  `Delegate::borrow` with `BorrowKind::Mutable` or
  `BorrowKind::UniqueImmutable`, with `PlaceBase::Local` in the defining body
  and `PlaceBase::Upvar` in a capturing body. We rely on that documented
  behaviour only.
- `AX-SPAN`: `span_recovery_frames` and `recover_user_editable_span`
  classify spans as specified in 8.1.2.

### Residual gaps

Aliasing through references and interior mutability are not modelled (`Risks`).
Moves and significant drops are deferred (`Decision log`). The adapter's
correctness is evidenced on a fixed fixture, not proven. The 8.3.2 slice lemmas
below remain unproved hypotheses for that milestone:

```plaintext
H-SLICE-1: the slice of sink i is the largest contiguous [k..=i] such that
           every s_j, k ≤ j < i, defines, kills, or (by policy) modifies a
           local in the needed set.
H-SLICE-3: if no local in summary([k..i)).D is read, killed, or modified by
           s_{i+1..n} or the tail, replacing [k..=i] by
           `let out = helper(I);` preserves the block's def-use relation.
```

## Plan of work

### Stage A — confirm and pressure-test (no code changes)

Re-read the documents in `Relevant documentation and skills`. Confirm the
toolchain facts in `Surprises & discoveries` still hold. Write the Verus lemma
*statements* and the `ParagraphIo` signatures side by side. Walk the design's
backward-slice algorithm and every `LB-CFG` key against the API:
`treat_mutating_method_calls_as_defs` must be computable from
`MutationKind::MethodReceiver`, `reject_closures` and `reject_async` from
`GuardSet` masks, and `skip_external_macro_expansions` from
`ExternalMacroGenerated`. Go/no-go: proceed only if every key is computable and
the signatures in `Interfaces and dependencies` survive unchanged, or the
change is recorded in `Decision log`.

### Stage B — EP-M1: pure model and paragraph algebra

Create `common/src/paragraph/` as a directory module:

- `mod.rs`: module documentation, the reuse policy, and re-exports within the
  module (not at the crate root).
- `guard.rs`: `GuardKind` and the `GuardSet` bitset.
- `stmt_io.rs`: `StmtForm`, `MutationKind`, `MutationKinds`, `StmtIo`,
  `StmtIoBuilder`, and `StmtIoError` (derived with `thiserror`, which `common`
  already depends on).
- `summary.rs`: `ParagraphIo` with `empty`, `From<&StmtIo<L>>`,
  `followed_by`, `preceded_by`, `summarize`, and accessors including
  `external_overwrites`.
- `tests/mod.rs`, `tests/guard.rs`, `tests/builder.rs`,
  `tests/builder_properties.rs`, `tests/summary.rs`, and
  `tests/summary_properties.rs`, wired with `#[cfg(test)] mod tests;`.

Red: write the named `rstest` cases and `proptest` properties against stub
bodies that return empty facts; confirm they fail for the stated reasons.
Green: implement. Refactor: keep functions small; rerun.

Add `common/tests/features/paragraph_io.feature` and
`common/tests/paragraph_io_behaviour.rs` with the four scenarios in
`Validation and acceptance`; no step may repeat an `rstest` assertion. Add
`pub mod paragraph;` to `common/src/lib.rs`. Add `googletest` and
`pretty_assertions` to `common/Cargo.toml` `[dev-dependencies]` as workspace
dependencies, and correct its stale "rstest-bdd 0.5.x" comment to 0.6.x.

Validation: `make check-fmt`, `make lint`, `make test`. Commit.

### Stage C — EP-M2: shared summary writer, proxy, crate bootstrap

1. Refactor commit. Add `crates/whitaker_lint_core/src/testing/summary.rs`
   with
   `append_summary_if_requested(env_key: &str, text: &str) ->
   std::io::Result<()>`,
   moved from the sibling's `write_collection_summary` (UTF-8 path from the
   variable, `cap_std` parent-directory capability, append mode, no-op when
   unset). Add `cap-std` to `whitaker_lint_core`. Replace the sibling's private
   writer with a call. Unit-test the helper with `rstest` and `tempfile`
   (unset, set, non-UTF-8, no file name) using `with_env_var`. The sibling's
   `example_harness_collects_call_site_evidence` must pass unchanged before and
   after. Run the gates; commit.
2. Proxy commit. Create `crates/rustc_hir_typeck/` by copying
   `crates/rustc_middle/` and changing the crate name. Add it to the root
   `Cargo.toml` proxy block, `TEST_EXCLUDES`, `DOCTEST_EXCLUDES`, the
   `AGENTS.md` doctest command, and the forbidden set in
   `tests/workflow_contracts/lint_makefile_test.py`. Run the gates; commit.
3. Crate commit. Create `crates/single_binding_paragraph/` with `Cargo.toml`
   modelled on the sibling (optional `rustc_hir`, `rustc_hir_typeck`,
   `rustc_lint`, `rustc_middle`, `rustc_session`, `rustc_span`,
   `dylint_linting`, `whitaker-common`, `whitaker_lint_core`, and `log` behind
   `dylint-driver`; `constituent`; `crate-type = ["cdylib", "rlib"]`;
   `test = false`; dev-dependencies `dylint_testing`, `rstest`, `proptest`,
   `googletest`, `pretty_assertions`, `insta`, `trybuild`, `tempfile`). Write
   `src/lib.rs` and `src/driver/mod.rs` declaring the lint at `Warn` through
   `dylint_linting::impl_late_lint!` with an empty `LateLintPass`. Add
   `examples/bootstrap_zero_diagnostic.rs`,
   `tests/ui/bootstrap_zero_diagnostic.rs`, and `tests/ui.rs` with
   `example_compiles_without_diagnostics` and
   `trybuild_cases_compile_without_diagnostics` (named so that it does not match
   `TRYBUILD_CONTRACT_FILTER`; if it must, extend the filter constants and
   `nextest_profile_test.py` deliberately). Add the crate's `examples`
   directory to `.rustfmt.toml` `ignore`, and the crate to `DOCTEST_EXCLUDES`
   and the `AGENTS.md` mirror.
4. Nextest wiring, in the same commit: add
   `package(single_binding_paragraph) & binary(ui)` overrides to both
   `[profile.default]` and `[profile.ci]` in `.config/nextest.toml` (75-minute
   example allowance, 120-minute `trybuild` allowance,
   `test-group = "serial-dylint-ui"`), mirror them in the per-profile tables of
   `tests/workflow_contracts/nextest_overrides.py`, and add a
   `PackageName("single_binding_paragraph")` subject to
   `crates/whitaker_lint_core/tests/nextest_ui_filter.rs`.

Red: `example_compiles_without_diagnostics` fails because the crate does not
exist. Green: the crate builds and the example is silent. Validation:
`make check-fmt`, `make lint`, `make test`, `make test-workflow-contracts`.
Commit.

### Stage D — EP-M3: HIR adapter and summary snapshot

Create `crates/single_binding_paragraph/src/stmt_io/`:

- `mod.rs`: `lower_body(cx, body) -> Vec<BlockIo>`. It skips the body when
  the lint level at the body is `Allow`; lists only blocks of *this* body (no
  descent into nested bodies) whose span recovers to `Direct`; and skips
  statements whose `LetStmt.super_` is `Some` or whose `LocalSource` is not
  `Normal`. `BlockIo` carries the block `HirId`, and its statement entries are
  index-aligned with `block.stmts` (skipped statements are kept as `None` so
  indices stay aligned), each with the statement's `HirId` and `Span`.
- `facts.rs`: one `intravisit::Visitor` pass per listed statement with
  `NestedFilter = nested_filter::OnlyBodies` that records `Res::Local` reads
  and internal bindings (descending into closure bodies so captures count as
  reads), except that the bare-local left-hand side of `ExprKind::Assign` is
  not a read (it is the kill recorded from `ExprUseVisitor`), and, while
  outside nested bodies, guards: a stack of loop expression IDs and labelled
  block IDs decides whether `break` and `continue` are contained.
- `classify.rs`: pure functions from HIR enums to guard kinds and forms
  (`MatchSource::AwaitDesugar` to `Await`, `MatchSource::TryDesugar` to
  `EscapingExit`, `LoopSource` to `Loop`, `YieldSource::Yield` to
  `EscapingExit`, `PatKind::Binding(BindingMode(ByRef::No, _), _, _, None)`
  with an initializer and no `else` to `SimpleLet`), each with an `rstest`
  table.
- `mutations.rs`: a `Delegate` that overrides `bind` and `copy` to do
  nothing, records `mutate` and `borrow(Mutable | UniqueImmutable)` on
  `PlaceBase::Local` and `PlaceBase::Upvar`, classifies each event as a kill or
  a `MutationKind`, and attributes it with one bounded parent walk to the
  statement of every listed enclosing block, using a `HirId -> (block, index)`
  map built once per body.

Create `src/driver/summary.rs` with the renderer. Each line has the
fixture-relative line number, the form, and sorted, rendered local names
(`cx.tcx.hir_opt_name(hir_id)` with a `_#n` fallback, plus a per-owner
first-appearance ordinal such as `x#0`, `x#1`). `check_crate_post` passes the
accumulated text to
`whitaker_lint_core::testing::summary::append_summary_if_requested` with
`WHITAKER_SINGLE_BINDING_PARAGRAPH_STMT_IO_SUMMARY`. The model is always
computed and discarded when the variable is unset, so the zero-diagnostic
example runs the whole adapter.

Add `examples/stmt_io_model.rs` (the fixture) and its `tests/ui` `trybuild`
twin, and `example_harness_records_statement_io` in `tests/ui.rs`, which runs
the example under `with_env_var` and asserts with `insta::assert_snapshot!`.

Red: write the expected snapshot by hand from the fixture comments and make the
adapter a stub returning empty facts; the test must fail on every non-empty
line. Green: implement until the snapshot matches; review every changed
snapshot line against the fixture comment, never accept blindly. Refactor;
rerun. Validation: `make check-fmt`, `make lint`, `make test`. Commit.

### Stage E — EP-M4: Verus proof

Create `verus/paragraph_io_summary.rs` following
`verus/string_continuation_rewrite.rs`: a `//!` header stating the trust
boundary and naming `common/src/paragraph/summary.rs` and
`common/src/paragraph/tests/summary_properties.rs` as its mirrors;
`use vstd::prelude::*;`; and a `verus! { … }` block with `StmtFact`, `ParaFact`,
`spec fn followed_by`, `spec fn lift`, `spec fn fold_left`,
`spec fn fold_right`, `spec fn spec_inputs` and siblings, `spec fn well_formed`,
`spec fn use_before_def_free`, the lemmas for `OB-C1` to `OB-C6`, the witness,
and the counterexample lemmas. Add a `paragraph` group to
`scripts/run-verus.sh` (the group case, the `all` case, and the argument check)
and a `verus-paragraph` target to the `Makefile`, including its `.PHONY` list.

Red: the naive-union `followed_by` fails `OB-C3` and `OB-C6`. Green: the
correct definition and the proofs. Validation: `make verus-paragraph`,
`make verus`, then `make check-fmt`, `make lint`, `make test`. Commit.

### Stage F — EP-M5: documentation and close-out

- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: add "Implementation
  decisions (8.3.1)" recording `DEV-1` to `DEV-4`, the guard semantics
  (including that `assert!` and `matches!` yield `Branch`, which matters for
  Lint C's assertion filter), the `uses`, `kills`, and `muts` definitions, the
  aliasing, interior-mutability, move, and drop limits, the Verus boundary, the
  8.5.1 suite-macro note, and a reference to ADR-0014. Update the §Statement
  I/O model code block so it no longer shows `BTreeSet<HirId>`.
- `docs/adrs/adr-0014-statement-io-model-for-paragraph-lints.md`: record the
  generic compiler-free model and algebra in `whitaker_common::paragraph` (with
  8.3.2's pure slicer committed to the same module), the HIR adapter in the
  lint crate, the `rustc_hir_typeck` proxy and `ExprUseVisitor` toolchain
  coupling, the internal-compiler-error residual risk and the `Allow` kill
  switch, and the Verus sidecar. Re-check the number against all fetched remote
  branches first. Add it to `docs/contents.md` under "Decision records".
- `docs/developers-guide.md`: document `whitaker_common::paragraph` (scope,
  permitted call sites — Lint B and Lint C drivers and their tests —
  composition rules, and "changing `followed_by` requires changing the
  sidecar"), the shared `append_summary_if_requested` helper and its reuse
  policy, the rule that each crate with a nested-Cargo UI harness needs its own
  nextest overrides in both profiles plus the contract-table entries, the
  `verus-paragraph` target under "Proof workflows", and the `rustc_hir_typeck`
  proxy with a toolchain-bump note about the `Delegate` trait.
- `docs/whitaker-dylint-suite-design.md` §2: a short paragraph on the
  `paragraph` module.
- `docs/repository-layout.md`: list `single_binding_paragraph/` and
  `rustc_hir_typeck/` under `crates/` and `paragraph_io_summary.rs` under
  verification assets.
- `docs/roadmap.md`: mark 8.3.1 done with a short status note.
- `docs/users-guide.md`: unchanged (see `Decision log`).

Validation: `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
`make lint`, `make test`, `make verus-paragraph`. Commit.

## Milestones and plateaus

- `EP-M1` — the generic model and paragraph algebra exist in
  `whitaker_common::paragraph` with unit, property, and behavioural tests.
  - Requirements: `LB-IO`, `LB-CFG`, `DEV-2`, `DEV-3`, `DEV-4`.
  - Acceptance: `cargo nextest run -p whitaker-common paragraph` and the BDD
    feature pass; gates pass.
  - Conformance check:
    - `LB-IO` facts and determinism covered by `OB-B1` to `OB-B3`.
    - Every `LB-CFG` key computable (Stage A check repeated).
    - `AX-SCOPE` still the only scoping assumption.
    - Only the approved `paragraph` module added to the public API.
    - No trust-boundary or persisted-format change.
    - Trace links updated.
  - Recovery: revert the milestone commit; nothing depends on it yet.
  - Remaining gaps: crate, adapter, proof.
  - Compatibility decision: none (new, pre-1.0 internal API).

- `EP-M2` — the shared writer, proxy crate, and silent lint crate exist, with
  nextest wiring.
  - Requirements: `LB-INTENT`, `INT-LATE`.
  - Acceptance: the sibling's summary test passes unchanged;
    `example_compiles_without_diagnostics` and
    `trybuild_cases_compile_without_diagnostics` pass; `make test` and
    `make test-workflow-contracts` pass.
  - Conformance check: no suite, installer, or CLI change; only the approved
    proxy and `cap-std` dependency added; sibling behaviour unchanged.
  - Recovery: revert in reverse commit order.
  - Remaining gaps: adapter, proof, documentation.
  - Compatibility decision: none.

- `EP-M3` — the adapter lowers real HIR into the model, observed by snapshot.
  - Requirements: `LB-IO`, `LB-RULE` (facts only), `INT-SPAN`, `NG-1`,
    `DEV-1`.
  - Acceptance: `example_harness_records_statement_io` passes with a reviewed
    snapshot; `wf=ok` on every block; negative controls recorded; gates pass.
  - Conformance check: HIR and typeck only, no MIR; silence preserved; field
    set validated (input to EP-M4).
  - Recovery: revert the adapter commit; EP-M2 remains a valid plateau.
  - Remaining gaps: proof, documentation.
  - Compatibility decision: none.

- `EP-M4` — the Verus sidecar proves `OB-C1` to `OB-C6`.
  - Requirements: `LB-IO` → `LB-SLICE` contract.
  - Acceptance: `make verus-paragraph` and `make verus` report zero errors;
    `rg -c 'assume|admit|external_body' verus/paragraph_io_summary.rs` prints
    nothing.
  - Conformance check: the sidecar's `followed_by` matches
    `common/src/paragraph/summary.rs` exactly.
  - Recovery: revert the proof commit; `proptest` still covers the algebra.
  - Remaining gaps: documentation.
  - Compatibility decision: none.

- `EP-M5` — documentation, ADR, and roadmap reflect the delivered design.
  - Acceptance: Markdown gates and all code gates pass; roadmap 8.3.1 ticked.
  - Conformance check: every deviation recorded in the design document and
    ADR; trace links final.
  - Recovery: documentation-only revert.
  - Compatibility decision: none.

## Concrete steps

Run every command from the repository root. Capture long output with `tee`:

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

Expected focused output after EP-M3 (abridged):

```plaintext
    PASS [   0.012s] whitaker-common paragraph::tests::builder::define_records_def
    PASS [  42.517s] single_binding_paragraph::ui example_harness_records_statement_io
     Summary [ ... ] N tests run: N passed, 0 skipped
```

Expected Verus output after EP-M4:

```plaintext
verification results:: N verified, 0 errors
```

Gate runs are delegated to the `scrutineer` agent where available; when it
reports a failure, read the cited log rather than re-running the gate.

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes, including the new `whitaker-common` unit,
  property, and behavioural tests, the shared writer's tests, and the new
  crate's UI tests.
- Verification: `OB-C1` to `OB-C6` discharged by `make verus-paragraph`;
  `OB-B1` to `OB-B3` and the conformance properties by `proptest`; `OB-A1` to
  `OB-A3` by the reviewed snapshot and silence tests.
- Lint and format: `make check-fmt` and `make lint` pass; Markdown gates
  (`make markdownlint`, `make nixie`) pass after documentation changes.

Behavioural specification for EP-M1, in
`common/tests/features/paragraph_io.feature`:

```gherkin
Feature: Paragraph I/O summaries
  Statement facts compose into a paragraph summary that reports what the
  paragraph needs from outside and what it changes outside.

  Scenario: A later statement consumes an earlier definition
    Given a statement that defines "a" and reads "seed"
    And a statement that defines "b" and reads "a"
    When the statements are summarized in order
    Then the paragraph inputs are "seed"
    And the paragraph defines "a" and "b"

  Scenario: Modifying an outside local makes it an input and an external modification
    Given a statement that modifies "items" as a method receiver
    When the statements are summarized in order
    Then the paragraph inputs are "items"
    And the paragraph externally modifies "items"

  Scenario: Modifying a local the paragraph defined stays internal
    Given a statement that defines "v"
    And a statement that modifies "v" as a method receiver
    When the statements are summarized in order
    Then the paragraph has no external modifications

  Scenario: Overwriting an outside local is an output, not an input
    Given a statement that kills "total" and reads "seed"
    And a statement that defines "out" and reads "total"
    When the statements are summarized in order
    Then the paragraph inputs are "seed"
    And the paragraph externally overwrites "total"
```

The fixture `crates/single_binding_paragraph/examples/stmt_io_model.rs` must
include, each with a comment stating the expected summary line:

- a simple chain of `let` statements ending in a `let out = …;` sink
  (`let(out#0)`), plus `let out: Client = …;` (ascribed simple `let`) and
  `let ref r = …;` (not simple);
- a destructuring `let (a, b) = …;` and an or-pattern
  `let (Ok(x) | Err(x)) = …;` (one def);
- shadowing (`let x = 1; let x = x + 1;` showing `x#0` and `x#1`);
- `items.push(seed);` (`receiver`), `count += 1;` (`compound`),
  `let total; total = compute(seed);` (kill), `x = x + 1;` (read and kill),
  `point.x = 3;` and `values[0] = 1;` (`projection`), `let r = &mut v;`
  (`borrow`), and `let ref mut y = z;` (`borrow`);
- a closure that captures an outer local by mutable reference, and a closure
  that writes through a captured `&mut` reference (`UniqueImmutable`), each
  with a `Closure` guard and the captured local read and modified;
- an `async` block that modifies an outer local, and an `.await` inside an
  `async fn` (`AsyncBlock`; `Await` only, with no `Loop`, `Branch`, or
  `EscapingExit`);
- `if`, `match`, and `matches!` (`Branch`); `while` and `for` (`Loop`); a
  `loop` with a contained `break value` and a labelled block with a contained
  `break 'a` (`Loop` only, or nothing for the block); `return` and `?`
  (`EscapingExit`); `let … else { return; }` (`LetElse` and `EscapingExit`);
  `panic!` (`Diverging`); a nested `fn` item (`item` form);
- `vec![a, b]` and `format!("{x}")` (reads recorded through expansions, no
  guard, no `super let` lines), `assert!(x > 0)` (`Branch` from the expansion),
  and a statement produced by a local `macro_rules!` (`MacroGenerated`);
- a nested block expression `let y = { let t = a; t + 1 };` (`t` invisible);
- a block tail expression (rendered as the block's `tail` line).

Red-Green-Refactor evidence to record in `Artefacts and notes`:

- EP-M1 red: `cargo nextest run -p whitaker-common paragraph` fails with
  shrunk counterexamples and assertion failures (stub bodies).
- EP-M2 red: `cargo nextest run -p single_binding_paragraph` cannot build the
  missing crate; the shared writer's tests fail before the helper exists.
- EP-M3 red: `example_harness_records_statement_io` fails on every non-empty
  line with the stub adapter.
- EP-M4 red: `./scripts/run-verus.sh paragraph` rejects `OB-C3` and `OB-C6`
  for the naive-union `followed_by`.
- Each green and refactor step: the focused command passes, then the
  repository gates pass.

## Idempotence and recovery

Every step is additive and re-runnable. The example harness writes to a fresh,
uniquely named summary file for each run. `insta` pending snapshots
(`*.snap.new`) must be reviewed and either accepted or deleted, never
committed. If a gate fails halfway through a milestone, fix and re-run that
gate; do not amend a commit after gating without re-running the gates (the
committed `^{tree}` must match the gated tree). Each milestone is one or more
commits that can be reverted independently in reverse order.

## Artefacts and notes

Record here, during implementation, the red and green transcripts, the Verus
summary line, the `rg` zero count for proof shortcuts, the negative-control
outcomes, and the final snapshot excerpts for `items.push(seed);` and
`total = compute(seed);`.

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
    MacroGenerated,
    ExternalMacroGenerated,
}

/// A set of guard kinds stored as a bitset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GuardSet(u16);

impl GuardSet {
    #[must_use]
    pub const fn empty() -> Self;
    #[must_use]
    pub const fn with(self, kind: GuardKind) -> Self;
    #[must_use]
    pub const fn contains(self, kind: GuardKind) -> bool;
    #[must_use]
    pub const fn is_empty(self) -> bool;
    #[must_use]
    pub const fn is_disjoint(self, other: Self) -> bool;
    #[must_use]
    pub const fn union(self, other: Self) -> Self;
    pub fn iter(self) -> impl Iterator<Item = GuardKind>;
}

impl FromIterator<GuardKind> for GuardSet { /* … */ }
```

In `common/src/paragraph/stmt_io.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StmtForm<L> {
    /// `let x = init;` or `let mut x: T = init;` with no `ref`, subpattern,
    /// or `else`.
    SimpleLet { binding: L, is_ascribed: bool },
    PatternLet,
    Expr,
    Semi,
    Item,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MutationKind {
    CompoundAssign,
    ProjectionWrite,
    MutBorrow,
    MethodReceiver,
}

/// The set of ways one local is modified within a statement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MutationKinds(u8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StmtIo<L> { /* private fields */ }

impl<L: Ord + Copy> StmtIo<L> {
    pub fn form(&self) -> StmtForm<L>;
    pub fn defs(&self) -> &BTreeSet<L>;
    pub fn kills(&self) -> &BTreeSet<L>;
    pub fn uses(&self) -> &BTreeSet<L>;
    pub fn muts(&self) -> &BTreeMap<L, MutationKinds>;
    pub fn guards(&self) -> GuardSet;
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StmtIoError<L> { DefinedAndUsed(L), DefinedAndInternal(L) }

pub struct StmtIoBuilder<L> { /* private fields */ }

impl<L: Ord + Copy> StmtIoBuilder<L> {
    pub fn new(form: StmtForm<L>) -> Self;
    pub fn define(&mut self, local: L);
    pub fn bind_internal(&mut self, local: L);
    pub fn read(&mut self, local: L);
    pub fn kill(&mut self, local: L);
    pub fn modify(&mut self, local: L, kind: MutationKind);
    pub fn guard(&mut self, kind: GuardKind);
    pub fn build(self) -> Result<StmtIo<L>, StmtIoError<L>>;
}
```

`StmtIoBuilder::new` with `StmtForm::SimpleLet { binding, .. }` records
`binding` as a def. `modify` also records a read, so `muts ⊆ uses` holds by
construction. `build` removes internal bindings from every set and reports the
errors described under `OB-B1`. The final `thiserror` derive shape (for example
whether `L` needs a `Debug` bound for the message) is settled during
implementation.

In `common/src/paragraph/summary.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParagraphIo<L> { /* private: defs, kills, inputs, external_muts, guards */ }

impl<L: Ord + Copy> ParagraphIo<L> {
    #[must_use]
    pub fn empty() -> Self;
    #[must_use]
    pub fn followed_by(&self, later: &Self) -> Self;
    #[must_use]
    pub fn preceded_by(&self, earlier: &StmtIo<L>) -> Self;
    #[must_use]
    pub fn summarize<'a, I>(stmts: I) -> Self
    where
        I: IntoIterator<Item = &'a StmtIo<L>>,
        L: 'a;
    pub fn defs(&self) -> &BTreeSet<L>;
    pub fn kills(&self) -> &BTreeSet<L>;
    pub fn inputs(&self) -> &BTreeSet<L>;
    pub fn external_muts(&self) -> &BTreeSet<L>;
    pub fn external_overwrites(&self) -> BTreeSet<L>;
    pub fn guards(&self) -> GuardSet;
}

impl<L: Ord + Copy> From<&StmtIo<L>> for ParagraphIo<L> { /* … */ }
```

In `crates/whitaker_lint_core/src/testing/summary.rs`:

```rust
pub fn append_summary_if_requested(env_key: &str, text: &str) -> std::io::Result<()>;
```

In `crates/single_binding_paragraph/src/stmt_io/mod.rs` (gated by
`dylint-driver`, crate-private):

```rust
pub(crate) struct StmtEntry {
    pub(crate) hir_id: rustc_hir::HirId,
    pub(crate) span: rustc_span::Span,
    pub(crate) io: StmtIo<rustc_hir::ItemLocalId>,
}

pub(crate) struct BlockIo {
    pub(crate) hir_id: rustc_hir::HirId,
    pub(crate) span: rustc_span::Span,
    /// Index-aligned with `block.stmts`; `None` marks a skipped desugared
    /// statement.
    pub(crate) stmts: Vec<Option<StmtEntry>>,
    pub(crate) tail: Option<StmtEntry>,
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

Dependencies: `rustc_hir_typeck` through the new workspace proxy crate;
`cap-std` added to `whitaker_lint_core`; test-only workspace dependencies as
listed in `Tolerances`. No new third-party crates.

## Revision note

- 2026-11-11: initial draft from reconnaissance and prior-art research.
- 2026-11-11: revised after a community-of-experts review (structure and
  contracts, failure modes and operations, alternatives and rigour; verdict
  "proceed with conditions"). Changes: split writes into `kills` and kind-tagged
  `muts` so `treat_mutating_method_calls_as_defs` is computable and overwrites
  are not inputs; revised the algebra to subtract produced (defined or killed)
  locals; made the builder report def/use overlap so the boundary check can
  fail; split `OB-C5` into minimal-precondition halves with counterexample
  lemmas; added `OB-C6` (the slicer update rule) and demoted `OB-C4` to a
  corollary; replaced empty-body and missing-file red stages with mutants and
  stub adapters; moved the Verus milestone after the adapter; added `Upvar`,
  `UniqueImmutable`, `bind` override, desugaring filters (`AwaitDesugar`,
  `super let`, `LocalSource`), break-containment stacks, and a once-per-body
  attribution map; made per-package nextest overrides and contract-table
  updates mandatory; extracted the summary writer into `whitaker_lint_core`;
  decided against copying the harness lock; made `GuardSet` a `Copy` bitset,
  renamed `then`/`prepend` to `followed_by`/`preceded_by`, removed struct-level
  bounds, the `Tail` form, and the `NestedItem` guard; added block and statement
  `HirId`/`Span` alignment for 8.3.3 and Lint C; corrected the proxy count and
  the `AGENTS.md` mirror claim; raised size tolerances to 50 files and 4,000
  lines and allowed a two-PR split. Remaining work is unchanged in kind: the
  plan still awaits human approval before EP-M1 starts.
