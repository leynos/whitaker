# Add `common::ownership_shape` model types, evaluation helpers, and diagnostic argument builders (9.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

Roadmap item: 9.1.1, "Add `common::ownership_shape` model types, evaluation
helpers, and diagnostic argument builders", in [the roadmap](../roadmap.md). It
requires 1.1.1, which is complete (`docs/roadmap.md`, "1.1.1. Implement the
`common` crate helpers …", marked `[x]`).

Implementation must not begin until this plan has been explicitly approved.

## Purpose / big picture

Phase 9 of the roadmap adds three ownership-shape lints:
`clone_only_used_by_borrow`, `owned_param_causes_clone`, and
`local_shared_ownership`. The design in
[ownership shape lints design](../ownership-shape-lints-design.md) splits each
lint into two halves. The compiler-facing half walks the compiler's High-level
Intermediate Representation (HIR) and Mid-level Intermediate Representation
(MIR) inside a per-lint crate. The decision-making half is pure Rust that needs
no compiler internals and lives in the shared `common` crate. This plan
delivers that pure half, so the three lint crates (roadmap 9.2.1, 9.3.1 and
9.4.1) can be thin drivers that classify what the compiler shows them and then
ask `common` whether to report it.

After this change a contributor can:

1. describe an observed clone, an owned parameter, or a locally created
   wrapper using small, deterministic, `Copy`, compiler-independent summary
   values;
2. call one pure `const fn` per lint and receive either a typed finding or a
   typed, lint-specific suppression reason; and
3. turn a finding plus the names of the things involved into a fixed,
   deterministic set of Fluent diagnostic arguments that the localization layer
   (`common::i18n`) can format.

Success is observable without any lint crate existing. `make test` runs new
unit, property, snapshot and behaviour tests that exercise every emission and
suppression rule in design §12 to §14, and `make test-doc` runs the new
doctests. `make kani-ownership-shape` checks the production decision functions
against independently written specifications over their complete input space.
`make verus-ownership-shape` proves that the decisions behave sensibly over
observation streams of unbounded length: extra evidence can only suppress,
never create, a finding, and findings survive merging evidence from several
places. A Fluent integration test shows that every argument the builders
produce formats without a Fluent error.

There is no end-user-visible behaviour change in this item: no lint is
registered, no configuration is read, and no message appears in any build.

## Constraints

- `common` must remain free of `rustc_private` dependencies. No type from
  `rustc_*` crates (`DefId`, `Ty`, `Span`, `rustc_errors::Applicability`) may
  appear in `common/src/ownership_shape/`. Identity (which `DefId` a summary
  belongs to) stays in the lint crates.
- Evaluation inputs and findings contain no strings. Display names enter only
  through the diagnostic "subject" structs in `diagnostics/`.
- The roadmap scope boundary must hold. Resolved-path classifiers
  (`path_kinds`, roadmap 9.1.2), the exact borrow-mapping classifier (roadmap
  9.1.3), MIR summary helpers (roadmap 9.1.4), lint crates (9.2.1, 9.3.1,
  9.4.1), configuration loading from `dylint.toml` or `whitaker.toml`, and
  Fluent `.ftl` locale entries (roadmap 9.5.1) are out of scope. This plan
  defines the closed `ExactBorrowKind` enumeration that 9.1.3's classifier will
  produce, but not the classifier.
- No `.ftl` file may be added under `common/locales/`; Fluent entries for the
  three lints belong to roadmap 9.5.1. Integration tests use an inline,
  test-only Fluent resource instead.
- The workspace Clippy policy in the root `Cargo.toml` and `clippy.toml`
  applies unchanged. In particular:
  - `must_use_candidate` is denied, so every public value-returning function
    and getter carries `#[must_use]`, except the `evaluate_*` functions,
    whose return type `Evaluation` is itself `#[must_use]` (adding the
    attribute there triggers `double_must_use`);
  - `missing_const_for_fn` is denied, so every function that can be `const`
    must be;
  - `indexing_slicing` is denied and slice `get` is not `const`, so counters
    are named fields reached through exhaustive `match`, never arrays indexed
    by discriminant;
  - `cognitive-complexity-threshold = 9` and `too-many-arguments-threshold =
    4` (`clippy.toml`), so each evaluation is split into one small `const`
    helper per check stage, and BDD steps parse at most three values besides
    `world`;
  - `struct_excessive_bools` allows at most three `bool` fields per struct;
  - `needless_pass_by_value`, `expect_used`, `unwrap_used`, `shadow_reuse`,
    and `allow_attributes` are denied; suppressions are a last resort and use
    `#[expect(..., reason = "...")]` on one item.
- Every source file stays under 400 lines (`AGENTS.md`), including Verus
  files. Modules use `mod.rs`-rooted directories and test modules wired with
  `#[cfg(test)] #[path = "..."] mod ...;` per
  [ADR 0006](../adrs/adr-0006-directory-based-module-layout.md).
- Every module begins with `//!` documentation, and every function and method
  (public or private) carries a `///` comment with an imperative summary.
  Public functions include a runnable doctest example; test functions must not
  carry examples (Whitaker's `test_must_not_have_example` lint). Function
  attributes go after doc comments.
- Comments and documentation use en-GB Oxford spelling (`-ize`), enforced by
  `make spelling`.
- Verus proofs must not use `assume`, `admit`, `#[verifier::external_body]`,
  `external_fn_specification`, `assume_specification`, or any trusted body to
  discharge an obligation. Unproved lemma statements stay in this plan until
  the proof milestone.
- Kani harnesses do not use `kani::assume` to narrow the domain except to
  restate a type's own invariant (for example `NonZeroUsize` being non-zero).
- No new external crates. `googletest`, `pretty_assertions`, and `insta` are
  already workspace dependencies (root `Cargo.toml` lines 22, 23 and 32) and
  may be added to `common/Cargo.toml` `[dev-dependencies]` with
  `.workspace = true`; the task brief authorizes this.
- Behaviour-driven development (BDD) `given` and `when` steps must not
  assert; assertions belong in `then` steps (`panic_in_result_fn` is denied).

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

The size limits below exclude this ExecPlan file. Count only non-ExecPlan files
towards the file-count limit, and count code and other documentation changes
towards the line limit, including when this ExecPlan is updated in the same
change.

- Scope: the estimate is about 52 non-ExecPlan files and 4,600 net added lines
  (the module and test tree in `Plan of work` is 34 files, plus 3 Verus files,
  8 wiring files and 7 documentation files). If the implementation needs more
  than 60 files or 5,500 net lines, stop and escalate. Pre-agreed fallback, to
  be proposed when this tolerance is threatened or a reviewer asks for it:
  split the work into two stacked pull requests at the EP-M4 boundary (PR A:
  EP-M0 to EP-M4, which unblocks 9.2.1, 9.3.1 and 9.4.1; PR B: EP-M5 and EP-M6).
- Interface: if any existing public item in `common` (outside the new
  `ownership_shape` module) must change signature, stop and escalate. Adding
  the new module and one `pub mod` line to `common/src/lib.rs` is in scope.
- Dependencies: if any crate not already declared in the root
  `[workspace.dependencies]` is required, stop and escalate.
- Design: if a decision listed under "Proposed design clarifications" in
  `Decision log` is rejected at approval, or implementation evidence shows any
  other departure from [the design](../ownership-shape-lints-design.md) §11 to
  §17 is needed, record it, set the status to `BLOCKED`, and escalate.
- Verification: if a Kani harness does not reach a verdict within the
  repository's 10-minute, 8 GiB `scripts/run-kani-limited.sh` budget, or a
  Verus lemma cannot be proved without a trusted construct after three
  materially different attempts, stop and escalate with the open verification
  condition rather than weakening the statement.
- Iterations: if a gate still fails after three fix attempts for the same root
  cause, stop and escalate.
- Time: if any single milestone takes more than one working day of elapsed
  effort, stop and escalate.
- Ambiguity: if a suppression or emission rule in design §12 to §14 admits two
  readings that produce different outcomes for the same summary, and the
  `Decision log` does not already settle it, stop and present the options.

## Risks

- Risk: Kani's `Arbitrary` cannot be derived for the private bitsets and
  counter structs without producing states that no public call can reach.
  Severity: medium. Likelihood: high if derived naively. Mitigation: derive
  `kani::Arbitrary` only on fieldless enums and on the all-`bool` policy
  structs; write `#[cfg(kani)] impl kani::Arbitrary` by hand inside
  `model/use_profile.rs`, `model/escape.rs` and the evaluation modules (where
  private fields are visible), masking unused bits. Confirm in Stage A that
  Kani 0.67 provides `Arbitrary` for `NonZeroUsize`; if it does not, generate a
  `usize` and build with `NonZeroUsize::new`, discarding zero by `kani::assume`
  (a type invariant).
- Risk: The Verus model drifts from production code, so the proof shows a
  property of a function nobody runs. Severity: high. Likelihood: medium.
  Mitigation: Kani step-refinement harnesses (K0) tie each production
  accumulator step to the model step the Verus file uses, and the Kani
  specification functions are compiled into `make test` through a proptest
  (P2), so a decision-rule change that is not mirrored fails on every pull
  request. The developers' guide records the three-way edit rule.
- Risk: Splitting the Verus proof into three files needs the shared stream
  model in each. Severity: low. Likelihood: medium. Mitigation: try a `#[path]`
  module include of `verus/ownership_shape_stream_model.rs` from the per-lint
  files; if the pinned Verus release rejects that layout, duplicate the roughly
  40-line model with a comment naming the canonical copy, and add a `scripts/`
  check that the copies are byte-identical (the repository already guards a
  bridge this way in `scripts/check-verus-fragment-id-bridge.sh`).
- Risk: `fluent_bundle::FluentValue` lacks `From<usize>`. Severity: low.
  Likelihood: low. Mitigation: confirm in Stage A; if absent, convert through
  an explicitly documented integer type.
- Risk: The Fluent selector tokens chosen here constrain the translations
  written in roadmap 9.5.1. Severity: medium. Likelihood: low. Mitigation:
  tokens are a closed, documented set exposed as `as_str` constants; adding a
  token later is additive; the Fluent contract helper is reusable by 9.5.1.
- Risk: Phase 14 (RFC 0008 and RFC 0009, both "Proposed") will revisit
  `clone_only_used_by_borrow` conflict handling and clone-removal fix
  confidence. Severity: medium. Likelihood: medium. Mitigation: decision D9
  adds the path-sensitive conflict state now and isolates fix confidence in one
  `const fn`, so an accepted RFC changes one function, its Kani specification
  and its tests, not the summary shape.
- Risk: `scripts/run-kani.sh` routes unknown first arguments to the
  decomposition filter, and
  `tests/workflow_contracts/kani_selection_contract_test.py` pins runner
  behaviour. Severity: low. Likelihood: high. Mitigation: EP-M4 adds an explicit
  `ownership-shape` arm and extends the contract tests in the same commit;
  `make test-workflow-contracts` gates it.
- Risk: A test or evaluation file passes 400 lines (the 6.3.2 brain-trait plan
  estimated 200 lines for a test file that reached 278). Severity: low.
  Likelihood: medium. Mitigation: the layout already splits per lint; split
  further by rule family if a file passes 350 lines.
- Risk: The allocated ADR number collides with one claimed on an unmerged
  branch. Severity: low. Likelihood: medium. Mitigation: ADRs 0013 and 0014
  already exist on unmerged remote branches (scan of all fetched
  `refs/remotes/origin/*` on 2026-11-11), so this plan uses 0015; re-fetch and
  re-scan immediately before creating the file.

## Progress

- [x] (2026-11-11) Renamed branch to
  `9-1-1-common-ownership-shape-model-type`.
- [x] (2026-11-11) Reconnaissance of `common` precedent, verification wiring,
  and documentation conventions (three Wyvern agents).
- [x] (2026-11-11) Drafted this ExecPlan.
- [x] (2026-11-11) Expert design review (API and contracts; verification;
  pre-mortem, alternatives and process). Verdict: proceed with conditions.
- [x] (2026-11-11) Revised the plan to meet the review conditions (see
  `Revision note`).
- [ ] Plan approved by the user.
- [ ] EP-M0: confirmations, baseline gates, ADR 0015 and design §11.6 drafted
  as "Proposed".
- [ ] EP-M1: shared vocabulary (`model/`).
- [ ] EP-M2: evaluation, one commit per lint (EP-M2a, EP-M2b, EP-M2c), with
  Kani specification functions and the agreement proptest.
- [ ] EP-M3: diagnostic argument builders and Fluent contract test.
- [ ] EP-M4: Kani harnesses and runner wiring.
- [ ] EP-M5: Verus stream model and proofs and runner wiring.
- [ ] EP-M6: documentation finalized, ADR accepted, roadmap updated.

## Surprises & discoveries

- Observation: The brain-trust modules, the stated precedent for this work,
  do not build Fluent arguments; they format English strings directly. Evidence:
  `common/src/brain_trait_metrics/diagnostic.rs` (`format_primary_message`,
  `format_note`, `format_help` return `String`); no brain-trust `.ftl` files
  exist under `common/locales/`. Impact: `ownership_shape::diagnostics` is the
  first argument builder in `common`; it sets its own convention, documented in
  the developers' guide.
- Observation: Design §16 proposes slugs `<lint>.primary`, `<lint>.note`, and
  `<lint>.help`, but `common::i18n::resolve_message_set` resolves a message
  `<lint>` plus its `.note` and `.help` attributes, and every existing `.ftl`
  file uses that shape. Evidence: `common/src/i18n/diagnostics.rs`;
  `common/locales/en-GB/module_max_lines.ftl` line 3. Impact: decision D4.
- Observation: `common::i18n` discards Fluent formatting errors. A missing
  argument does not produce an `I18nError`. Evidence:
  `common/src/i18n/loader.rs` lines 140 to 160 call
  `LOADER.try_lookup_with_args`, which returns an `Option`; `I18nError` has only
  `MissingMessage` (lines 24 to 34). Impact: the Fluent contract test (E1)
  checks `FluentBundle::format_pattern`'s error list directly; resolving through
  `resolve_message_set` cannot detect a missing argument. Fixing the loader is
  out of scope and is noted for a follow-up issue.
- Observation: `make test` does not run doctests; `make test-doc` is "the only
  place they run". Evidence: `Makefile` line 433 and its comment. `AGENTS.md`
  still describes `make test` as running doctests. Impact: every gate in this
  plan runs `make test-doc` as well. The `AGENTS.md` drift is noted for a
  follow-up documentation fix outside this item.
- Observation: `common` already defines a compiler-independent
  `Applicability` mirror with the four rustc variants. Evidence:
  `common/src/diagnostics.rs` lines 7 to 16; `rustc_errors::Applicability`
  documents `MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders` and
  `Unspecified`. Impact: `FixConfidence` maps onto it.
- Observation: `common/Cargo.toml` lacks `insta`, `googletest` and
  `pretty_assertions` dev-dependencies although the workspace declares them.
  Impact: EP-M1 adds them.
- Observation: Kani's `kani::cover!` does not fail a run when a cover property
  is unsatisfiable, and the repository's runner does not check cover results.
  Its established non-vacuity mechanism is an expected-counterexample harness
  run through `run_expected_counterexample` in `scripts/run-kani.sh`, as used
  for `src/cli/dispatch.rs`. Impact: the verification plan uses
  expected-counterexample harnesses for reachability and discrimination.
- Observation: RFC 0008 (`docs/rfcs/rfc-0008-borrow-workaround-lint-family.md`,
  around line 903) proposes a three-state `OriginalPlaceConflict` for
  `clone_only_used_by_borrow`, and RFC 0009
  (`docs/rfcs/rfc-0009-validated-ownership-and-borrow-rewriter.md`, around line
  495) proposes that clone removal be review-required unless the `Clone`
  implementation is a trusted identity. Both are "Proposed". Impact: decision
  D9.

## Decision log

### Proposed design clarifications (require approval with this plan)

These refine or depart from the letter of
[the design](../ownership-shape-lints-design.md). Each is recorded in a new
design §11.6, "Implementation decisions (9.1.1)", drafted in EP-M0 with status
"Proposed" and finalized in EP-M6. D1, D2, D3, D5 and the verification split
are captured in ADR 0015 because they fix the contract all three lint crates
consume.

- Decision D1: Evaluation returns `Evaluation<F, R>`, an enum of `Emit(F)` or
  `Suppress(R)`, where each lint has its own finding type `F` and its own
  suppression enum `R`. This replaces the design's
  `EvaluationResult { emit: bool, level, fix_confidence, diagnostic_args }`.
  Rationale: the design's record allows impossible states (for example
  `emit == false` with a fix confidence and arguments), and a single shared
  reason enum would allow reasons a lint can never produce. Per-finding
  emission level is not modelled: lint levels are fixed per lint (design §19.1)
  and owned by rustc (`declare_lint!` default level, `#[allow]`, and
  configuration). Any future per-finding severity, such as design §20 question
  2's "softer notes for exported APIs", belongs on the finding type, not on
  `Evaluation`. Design §6.3's "downgrade" is delivered in this release as
  suppression or reduced fix confidence. Affected identifiers: DS-11.3,
  DS-15.3. Date/Author: 2026-11-11, planning agent, revised after expert
  review; awaiting approval.
- Decision D2: Evaluation functions take only compiler-independent, string-free
  facts and are `const fn`. A finding carries the semantic facts the diagnostic
  needs (fix confidence, use-shape token, call-site count, exact borrow kind,
  wrapper class, whether to emit a representative call-site note). Builders in
  `diagnostics/` combine a finding with a small "subject" struct of display
  names to produce Fluent arguments. Rationale: keeps the decision kernel
  `Copy`, `const`, and tractable for Kani and Verus, and keeps display concerns
  in one place. Design §15.3's requirement that evaluation yields "the
  diagnostic argument map needed for Fluent" is met by the finding-to-arguments
  builder, which is total. Affected identifiers: DS-15.3, DS-16. Date/Author:
  2026-11-11, planning agent; awaiting approval.
- Decision D3: `local_shared_ownership` aggregates escape and API-constraint
  evidence as sets (`EscapeSet`, `ApiConstraintSet`), not as one representative
  kind. `EscapeKind::No` and `ApiConstraint::None` are kept, as the design
  specifies, and act as identities: inserting them changes nothing, and "no
  escape" is the empty set. Suppression payloads use narrower enums that exclude
  `No` and `None`. Rationale: the policy waives some escape and constraint
  kinds but not others; reducing several observations to one representative
  would hide a non-waivable kind behind a waivable one and emit a false
  positive. Set union is order-independent, so "escape-kind reduction" (design
  §18.1) is well defined. Affected identifiers: DS-11.3, DS-14.3, DS-14.6,
  DS-18.1. Date/Author: 2026-11-11, planning agent; awaiting approval.
- Decision D4: The Fluent message identifier for each lint is the lint name
  (`clone_only_used_by_borrow`), with `.note` and `.help` attributes, matching
  `resolve_message_set` and every existing locale file. The design's
  `<lint>.primary` slug denotes the message value. The three identifiers are
  `MessageKey<'static>` constants. Affected identifiers: DS-16. Date/Author:
  2026-11-11, planning agent; awaiting approval.
- Decision D5: Each lint has a fixed argument-name set, always fully
  populated, so a Fluent message never meets a missing variable. Values that
  would otherwise be English prose are closed selector tokens for Fluent select
  expressions:
  - `use_summary`: `shared_borrow`, `shared_method`, `copy_read`, or
    `mixed_read`;
  - `clone_form`: `clone` or `to_owned`;
  - `borrow_hint`: `exact` or `generic`, with `exact_borrow_type` set to the
    empty string when `generic`;
  - `wrapper_class`: `interior_mutability`, `shared_handle`, or
    `shared_mutable` (design §14.4).

  `wrapper` is rendered from the classified wrapper shape (for example
  `Rc<RefCell<_>>`), not passed in as free text. Each token enum also offers a
  deterministic English phrase (`fallback_text`) so lint crates' English
  fallback wording (design §16, last paragraph) never shows a raw token.
  Rationale: prose inside an argument cannot be translated, and design §16
  lists argument names as examples ("such as"), not a closed set. This moves
  message keys and argument names for these lints into `common`, which the
  developers' guide section "Shared diagnostic localization" currently assigns
  to lint crates; EP-M6 amends that section. Affected identifiers: DS-16.
  Date/Author: 2026-11-11, planning agent; awaiting approval.
- Decision D6: Module layout groups shared vocabulary under `model/`, each
  lint's summary, policy, finding, suppression enum and evaluation function
  under `evaluation/<lint>.rs`, argument builders under `diagnostics/`, Kani
  specifications under `spec.rs` (compiled for `test` and `kani`), and Kani
  harnesses under `kani/`. The design's `path_kinds.rs` and
  `exact_borrow_mappings.rs` are not created here (roadmap 9.1.2 and 9.1.3).
  Nothing is re-exported at the `common` crate root; callers write
  `common::ownership_shape::...`. Rationale: honours the design's top-level
  names while following `AGENTS.md` "group by feature" and the 400-line limit,
  and avoids the root-level name collisions the 6.3.2 brain-trait work hit.
  Affected identifiers: DS-11.2. Date/Author: 2026-11-11, planning agent;
  awaiting approval.
- Decision D7: An empty use profile (no observed use of the clone or the
  parameter) suppresses with a `NoObservedUse` reason rather than counting as
  vacuously read-only. Rationale: the right fix for an unused clone or an
  unused by-value parameter is to remove it, not to borrow it, so "consider
  borrowing" would be the wrong advice; and design §6.2's false-positive budget
  favours silence. (Clippy's `redundant_clone` is a nursery lint and
  `_name: String` silences `unused_variables`, so this is not delegation to
  other lints.) Affected identifiers: DS-12.2, DS-13.2.2. Date/Author:
  2026-11-11, planning agent, rationale corrected after expert review; awaiting
  approval.
- Decision D8: Summaries carry no compiler identity (`DefId`, `TypeId`). The
  design's class diagram shows `callee_id: DefId` and `source_type: TypeId`;
  those stay in the lint crates, which group pressure by callee before building
  one `OwnedParamSummary` per parameter. Rationale: constraint 1. Affected
  identifiers: DS-11.3. Date/Author: 2026-11-11, planning agent; awaiting
  approval.
- Decision D9: `OriginalPlaceAccess` has four states: `Unconflicted`,
  `MovedBeforeLastUse`, `MutablyBorrowedBeforeLastUse`, and
  `PathSensitiveConflict`. All three conflict states suppress, each with its
  own reason, as design §12.6 requires. Fix confidence follows design §12.5
  (machine-applicable for same-expression and fully rewritable simple-`let`
  forms when machine suggestions are enabled) until RFC 0009 is accepted; it is
  computed by one `const fn` so that adopting RFC 0008 (emit a rewrite intent
  for path-sensitive conflicts) or RFC 0009 (review-required clone removal
  unless the `Clone` implementation is a trusted identity) changes one
  function, not the summary shape. Rationale: avoids reopening the summary,
  Kani specifications, Verus model and BDD scenarios in Phase 14 at the cost of
  one variant now. Affected identifiers: DS-12; RFC 0008; RFC 0009.
  Date/Author: 2026-11-11, planning agent after expert review; awaiting
  approval.
- Decision D10: The `TraitDeclaration`, `TraitImpl`, `Extern` and
  `RequiresLifetimeIntroduction` callee exemptions are never waived. Design
  §13.7 says trait-constrained callees are suppressed "unless configuration
  opts in", but design §17 defines no such flag; this plan does not invent one.
  Affected identifiers: DS-13.7, DS-17. Date/Author: 2026-11-11, planning agent
  after expert review; awaiting approval.
- Decision D11: The `local_shared_ownership` policy flags are read literally:
  when `ignore_static_callback_captures`, `ignore_trait_signature_constraints`
  or `ignore_external_api_boundaries` is set to `false`, evidence of that kind
  no longer suppresses. All three default to `true` (design §17), so default
  behaviour suppresses every boundary listed in design §14.3 and §14.6; a
  `false` setting is an explicit opt-in to reviewing such wrappers, which the
  diagnostic-only policy (§14.5) keeps safe. `ThreadBoundary`, `Return`,
  `Store`, `ClosureCapture` and `Unknown` escapes and the
  `ThreadSafeSharedRequired` constraint always suppress. Affected identifiers:
  DS-14.3, DS-14.6, DS-17. Date/Author: 2026-11-11, planning agent after expert
  review; awaiting approval.
- Decision D12: Call-site evidence for `owned_param_causes_clone` is one of
  `Retained`, `NotRetained`, `TemporarySource`, or `MacroExpanded`, with
  precedence `MacroExpanded` over `TemporarySource` over `NotRetained` over
  `Retained` when a site matches several. Only `Retained` counts towards the
  minimum. Rationale: design §13.2.1 requires the source to stay live after the
  call, and §13.7 excludes macro-only and temporary-source evidence; a defined
  precedence makes the lint crates' Pass A deterministic. Affected identifiers:
  DS-13.2.1, DS-13.7. Date/Author: 2026-11-11, planning agent after expert
  review; awaiting approval.
- Decision D13: `local_shared_ownership` handle evidence is two independent
  facts: whether any `Weak` handle is observed, and the strong-alias state
  (`None`, `Classified`, or `Unclassified`). A classified alias is a local
  strong clone whose every use is itself classified as local and sequential;
  classified aliases do not suppress, because design §14.4 still reports a
  non-escaping `Rc<RefCell<_>>` shared within one body. `Weak` handles and
  unclassified aliases always suppress (§14.7). For interior-mutability-only
  wrappers (`Cell`, `RefCell`, `Mutex`, `RwLock` without an outer `Rc` or
  `Arc`) the lint crates report `None` and no `Weak`. Affected identifiers:
  DS-14.4, DS-14.7. Date/Author: 2026-11-11, planning agent after expert
  review; awaiting approval.
- Decision D14: "Local, sequential operations" (design §14.3) means, in
  addition to the listed `get`, `set`, `borrow`, `borrow_mut`, `lock`, `read`
  and `write`: dereferencing an `Rc` or `Arc` handle, calling `&self` methods
  on the inner value through that dereference, and cloning the handle into a
  classified alias (D13), all within one body and without overlapping guard
  lifetimes. Without this, the `SharedHandle` class could never fire. Affected
  identifiers: DS-14.3, DS-14.4. Date/Author: 2026-11-11, planning agent after
  expert review; awaiting approval.
- Decision D15: Asynchronous captures are classified as follows: a capture
  into an `async` block or closure that stays in the body is `ClosureCapture`;
  a capture into a task handed to a spawner that requires `Send + 'static` is
  `ThreadBoundary`; a capture into a task or callback that requires `'static`
  without `Send` is `StaticCallback`. This is documented on the `EscapeKind`
  variants and is the contract for roadmap 9.1.2 and 9.1.4. Affected
  identifiers: DS-11.3, DS-14.3, DS-14.7. Date/Author: 2026-11-11, planning
  agent after expert review; awaiting approval.
- Decision D16: The policy structs carry every design §17 field that
  influences an evaluation or a finding, including
  `representative_callsite_note`, which sets a flag on the finding. They omit
  `enabled`, which governs lint registration and configuration loading, not
  evaluation. A by-value argument to a local function is classified as `Move`,
  which is how `clone_only_used_by_borrow` defers such clones to
  `owned_param_causes_clone` (design §12.6, last bullet); this is part of the
  contract for roadmap 9.1.4. Affected identifiers: DS-12.6, DS-13.5, DS-17.
  Date/Author: 2026-11-11, planning agent after expert review; awaiting
  approval.

### Implementation decisions

- Decision: Use `make kani-ownership-shape` and `make verus-ownership-shape`
  as new Makefile targets backed by an `ownership-shape` group in each runner
  script, and include the group in the default `make kani` and `make verus`
  runs. Rationale: mirrors `kani-clone-detector` and `verus-clone-detector`;
  the nightly `.github/workflows/proofs.yml` workflow already triggers on
  `common/**` and `verus/**`. Date/Author: 2026-11-11, planning agent.
- Decision: Verus proves properties of observation streams of unbounded length
  that Kani cannot state; Kani checks the production decision and accumulator
  functions over their complete input space; proptest ties the Kani
  specifications into every `make test`; example tests pin readable rules and
  suppression precedence. Verus does not re-prove the finite decision tables,
  because a mirrored "decision equals predicate" theorem over finite facts is
  propositional and would repeat Kani's check on a copy instead of the real
  code (the "too tautological" outcome recorded in
  `docs/execplans/7-2-4-verus-proofs-for-lsh-config-new-invariants.md`). This
  follows
  [ADR 0003](../adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md).
  Date/Author: 2026-11-11, planning agent after expert review.
- Decision: Assertion libraries: `googletest` matchers (`assert_that!` with
  `matches_pattern!`) for `Evaluation` variants and suppression reasons;
  `pretty_assertions::assert_eq` for findings, profiles, sets and argument maps;
  `insta` for one multi-variant snapshot per lint of the rendered argument
  maps. Date/Author: 2026-11-11, planning agent.
- Decision: No end-to-end test is added. Nothing in this item is externally
  observable: no lint is registered, no command-line behaviour changes, and no
  configuration is read. The UI tests in roadmap 9.2.5, 9.3.5 and 9.4.5 are the
  end-to-end coverage for this code. Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. To be completed at each milestone and at plan completion.

## Context and orientation

The repository is a Cargo workspace of Dylint lint crates. A Dylint lint is a
`cdylib` that the Rust compiler loads to run extra checks. The shared support
crate is `common/` (package `whitaker-common`, imported as `common`). It has no
compiler dependencies; everything compiler-specific lives in the lint crates
under `crates/`.

Key existing files a novice should read first:

- `docs/ownership-shape-lints-design.md`: the design. §10 (Servo calibration),
  §11 (shared implementation approach), §12 to §14 (the three lints' rules),
  §15 (common helper requirements), §16 (diagnostics and localization), §17
  (configuration), §18 (testing strategy).
- `docs/rfcs/rfc-0008-borrow-workaround-lint-family.md` and
  `docs/rfcs/rfc-0009-validated-ownership-and-borrow-rewriter.md`: proposed
  Phase 14 amendments that touch `clone_only_used_by_borrow` (decision D9).
- `common/src/lib.rs`: declares `pub mod` modules and root re-exports.
- `common/src/brain_trait_metrics/`: the nearest precedent, a pure
  `evaluation.rs` with a `const fn` decision function and a `diagnostic.rs`.
- `common/src/diagnostics.rs`: defines `common::diagnostics::Applicability`.
- `common/src/i18n/`: Fluent localization. `Arguments<'a>` is
  `HashMap<Cow<'a, str>, FluentValue<'a>>` (`loader.rs`); `MessageKey::new` is
  `const`; `resolve_message_set(lookup, key, args)` resolves a message and its
  `.note` and `.help` attributes through any `BundleLookup`, but Fluent
  formatting errors are not surfaced (see `Surprises & discoveries`).
- `common/tests/brain_trait_evaluation_behaviour.rs` and
  `common/tests/features/brain_trait_evaluation.feature`: the `rstest-bdd`
  pattern, with a `world` fixture declared with
  `#[allow_fixture_expansion_lints]` and `#[fixture]`, and steps declared with
  `#[given]`, `#[when]`, and `#[then]`. This plan binds scenarios by `name =`
  rather than `index =`, so inserting a scenario cannot silently rebind the
  others (`docs/rstest-bdd-users-guide.md`, around line 205).
- `verus/`: Verus proof sidecar, one file per proof, each a mirrored model;
  run by `scripts/run-verus.sh` (groups in `proof_files_for_group` and the
  argument `case` near line 62) and `make verus`.
  `verus/whitaker_cli_dispatch.rs` shows predicates defined from the input
  alone and `lemma_*_inhabited` witnesses.
- `scripts/run-kani.sh`: Kani runner with hard-coded harness groups, a default
  run, and `run_expected_counterexample` for harnesses that must fail; each
  harness runs through `scripts/run-kani-limited.sh` (8 GiB, 10 minutes).
  `tests/workflow_contracts/kani_selection_contract_test.py` and
  `kani_runner_contract_test.py` test the runner with stubs.
- `docs/developers-guide.md`: "Shared diagnostic localization" (around line
  143), "Make targets" (around line 2491), "Verus scope and trust boundary"
  (around line 2592), Kani harness guidance (around line 2884), and "Shared
  fingerprint helpers" (around line 3949), a model for documenting a shared
  helper and its re-use policy.

Terms used in this plan:

- *Use class*: how one observed use of a value treats it (shared borrow,
  `&self` method call, copy read, mutable borrow, `&mut self` method call,
  move, return, store, or call into unknown code). Design §11.3 `UseClass`.
- *Use profile*: the count of observed uses per use class for one value.
- *Observation stream*: the sequence of use classes (or escape kinds) a lint
  crate observes for one value, in any order.
- *Escape*: the value leaving the local body's control (returned, stored,
  captured by a closure or a `'static` callback, sent across a thread, passed
  to an external API that needs the shape). Design §11.3 `EscapeKind`.
- *Clone pressure*: a call site that clones an argument to satisfy a by-value
  parameter while the clone's source stays live (design §13.2.1).
- *Fix confidence*: how safe an automatic rewrite would be; it maps onto
  `Applicability`.
- *Finding*: the facts a lint needs to emit one diagnostic.
- *Suppression reason*: why an evaluation chose not to emit.
- *Selector token*: a fixed lowercase string used by a Fluent select
  expression to choose wording, never shown to users directly.
- *Expected-counterexample harness*: a Kani harness that must fail; the runner
  treats its failure as success, proving that a state is reachable or that a
  specification can tell a right answer from a wrong one.

Relevant skills and documents for the implementer: `rust-router` (then
`rust-types-and-apis` for the type surface, `rust-unit-testing` for `rstest`,
`googletest`, `pretty_assertions` and `insta`, `proptest`, `kani`, `verus`, and
`rust-verification`); `execplans` for maintaining this plan; `codegraph-mcp`
for navigation; `commit-message` for commits; and the documents
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md` (for keeping the
staged evaluation helpers under the cognitive-complexity threshold),
`docs/whitaker-dylint-suite-design.md` (localization notes under "Common crate
(common)"), and `docs/whitaker-clone-detector-design.md` (whose "Implementation
decisions" sections show how proof-driven design choices are recorded).

## Conformance basis

Upstream artefacts:

- Terms of Reference: none exists for Phase 9. None is invented here.
- Technical design: `docs/ownership-shape-lints-design.md` at commit
  `3515aed6` (last change to that file; repository `HEAD` `d9eb8a09` on
  2026-11-11). Status "Proposed".
- Roadmap: `docs/roadmap.md` at `d9eb8a09`, item 9.1.1.
- Related proposals (not binding, design-compatible hooks only): RFC 0008 and
  RFC 0009 at `d9eb8a09`, both "Proposed".
- ADRs:
  [ADR 0003](../adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md)
  (Verus over models, Kani over real code, tests for regression) and
  [ADR 0006](../adrs/adr-0006-directory-based-module-layout.md) (module layout
  and 400-line budget). ADR 0015 is created by this plan.
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`, root
  `Cargo.toml` `[workspace.lints]`, `clippy.toml`.

Traced design elements (identifier, then source):

- DS-11.2: module layout (§11.2).
- DS-11.3: shared domain model enums and summaries (§11.3).
- DS-12: `clone_only_used_by_borrow` firing rule (§12.2), diagnostics
  (§12.4), suggestion policy (§12.5), false-positive controls (§12.6).
- DS-13: `owned_param_causes_clone` pass A and B (§13.2), exemptions (§13.3),
  exact-mapping hint policy (§13.4), diagnostics (§13.5), false-positive
  controls (§13.7).
- DS-14: `local_shared_ownership` scope (§14.2), detection (§14.3), classes
  (§14.4), diagnostic-only policy (§14.5), Servo-style exemptions (§14.6),
  false-positive controls (§14.7), and §10 (Servo calibration).
- DS-15.3: pure evaluation helpers (§15.3).
- DS-16: Fluent slugs, arguments and English fallback (§16).
- DS-17: configuration defaults (§17).
- DS-18.1 and DS-18.2: unit and behaviour test expectations (§18.1, §18.2).

Trace links:

```plaintext
DS-11.3 -> EP-M1 -> model tests (use_profile, escape_sets, fix_confidence,
           wrapper_shape, accumulation_props) + K0 + K1 + V1
DS-12   -> EP-M2a -> clone_evaluation tests + BDD clone scenarios
           + K2 + R1 + V2; D7, D9, D16
DS-13   -> EP-M2b -> owned_param_evaluation tests + BDD owned-parameter
           scenarios + K3 + R2 + V3; D10, D12, D16
DS-14   -> EP-M2c -> shared_ownership_evaluation tests + BDD shared-ownership
           scenarios + K4 + R3 + V2; D11, D13, D14, D15
DS-15.3 -> EP-M2a..c -> evaluation tests + P2; D1, D2
DS-16   -> EP-M3 -> diagnostics tests + insta snapshots + E1; D4, D5
DS-17   -> EP-M2a..c -> policy_defaults tests; D10, D11, D16
DS-18.1 -> EP-M1..EP-M3 -> unit and property suites
DS-18.2 -> EP-M2a..c -> BDD feature (public-API suppression,
           callback-boundary suppression, machine-suggestion cases)
DS-11.2 -> EP-M1..EP-M3 -> module tree; D6
```

Design §18.2's "configuration precedence" and "localization fallback"
behaviours belong to configuration loading and to the lint crates' fallback
wiring, both out of scope. This plan covers the policy-value half of
configuration precedence (an explicit policy overrides defaults) and the
argument half of localization (arguments always format, and every token has an
English fallback phrase).

## Verification plan

### What is being verified, and why it matters

The three evaluation functions are contractual business logic: they decide
whether Whitaker accuses code of an ownership smell. A wrong "emit" is a false
positive, which the design treats as the dominant risk (§6.2); a wrong
"suppress" silently loses the lint's value. The accumulators (`UseProfile`,
`EscapeSet`, `ApiConstraintSet`, `OwnedParamPressure`) turn an
arbitrary-length, arbitrary-order stream of MIR observations into the facts the
decisions read. Lint crates will build these from several MIR blocks and merge
them, so two stream-level properties carry the false-positive budget: more
evidence must never turn a suppression into a finding, and merging two emitting
pieces of evidence must not suppress. Diagnostic argument shaping is plumbing
that needs determinism and a fixed key set; tests establish those adequately
and no proof is warranted.

### Abstract model (independent of implementation)

Let `U` be the nine use classes and `R ⊂ U` the read-only classes
`{SharedBorrow, MethodBySharedRef, CopyRead}`. An observation stream `s` is a
finite sequence over `U`; `count(s, c)` is the number of positions holding `c`.
The production profile stores saturating counters:
`sat(s, c) = min(count(s, c), usize::MAX)`.

The use verdict is a function of which classes occur:

- `NoUses` iff `s` is empty;
- `ReadOnlyBorrowed(shape)` iff `s` is non-empty and every position holds a
  class in `R`, where `shape` names the read-only classes that occur
  (`SharedBorrow`, `SharedMethod`, `CopyRead`, or `Mixed`);
- otherwise `NotReadOnly(k)` with `k` = `Consumed` if some position holds
  `Move`, `Return`, or `Store`; else `MutablyBorrowed` if some position holds
  `MutableBorrow` or `MethodByMutableRef`; else `Opaque` (only `UnknownCall`
  remains).

Let `E` be the eight escape kinds and `E* = E \ {No}`. For an escape stream `t`,
`escapes(t) = {e ∈ E* | e occurs in t}`. Constraints are modelled the same way
with `ApiConstraint::None` as identity.

Emission predicates (written out in full in `spec.rs` and in the Verus files;
the English statements follow):

- `clone_only_used_by_borrow` emits iff the clone is not from a macro
  expansion; the receiver is a local or field place; the form is `clone`, or
  the form is `to_owned` and the policy includes `to_owned`; the verdict is
  `ReadOnlyBorrowed`; and the original place is `Unconflicted` (design §12.2,
  §12.6; D7, D9, D16).
- `owned_param_causes_clone` emits iff the retained call-site count is at
  least the policy minimum (itself at least one); the parameter's verdict is
  `ReadOnlyBorrowed`; and every exemption present is waived, where `Exported`
  is waived only when `avoid_breaking_exported_api` is `false`, `Async` only
  when `ignore_async_fns` is `false`, and the others never (design §13.2,
  §13.3, §13.7, §17; D10, D12).
- `local_shared_ownership` emits iff the wrapper is not from a macro
  expansion; its shape is classifiable (§14.2); every escape in the set and
  every constraint in the set is waived under D11; operations are local and
  sequential (D14); no `Weak` handle and no unclassified strong alias is
  observed (D13); and the inner type is not a framework handle (design §14.3,
  §14.6, §14.7, §17).

Fix confidence:

- `clone_only_used_by_borrow`: `MachineApplicable` iff the policy enables
  machine suggestions and the rewrite shape is a same-expression form or a
  simple `let` binding whose every use rewrites directly; otherwise
  `DiagnosticOnly` (§12.5; D9).
- `owned_param_causes_clone`: `MaybeIncorrect` when an exact borrow kind is
  present, else `DiagnosticOnly`; never `MachineApplicable`, because a
  signature change also requires caller edits (§13.4).
- `local_shared_ownership`: always `DiagnosticOnly` (§14.5), enforced by
  construction: the finding type has no fix-confidence field and its accessor
  returns the constant.

### Invariants

- I1 (verdict depends only on which classes occur): profiles with the same
  set of non-zero classes have the same verdict, so saturating counters cannot
  change a verdict.
- I2 (order independence): the profile, escape set, constraint set and
  call-site pressure built from a stream are invariant under reordering it.
- I3 (identity elements): inserting `EscapeKind::No` or `ApiConstraint::None`
  leaves a set unchanged.
- I4 (emission soundness and completeness): for each lint, `evaluate_*`
  returns `Emit` iff that lint's emission predicate holds.
- I5 (fix-confidence ceiling): `MachineApplicable` is produced only by
  `clone_only_used_by_borrow` with machine suggestions enabled and a rewritable
  shape; `local_shared_ownership` findings are always `DiagnosticOnly`.
- I6 (pressure floor): every `owned_param_causes_clone` finding reports a
  retained call-site count of at least one and at least the policy minimum.
- I7 (reason truthfulness): every suppression reason returned names a
  condition that actually holds for the input.
- I8 (suppression precedence): when several suppression conditions hold, the
  reason reported is the first in the documented check order for that lint.
  This is a determinism property for tests and tracing; it carries no
  correctness weight and is verified by tests only.
- I9 (argument key-set stability and determinism): for each lint, the
  argument names produced are exactly that lint's documented constant set, for
  every finding and subject, and equal inputs give equal maps iterated in a
  fixed order.

### Lemmas (hypotheses about the eventual proof structure)

Implementation may merge, split, or discard these; the statements are what must
be shown, not a committed decomposition.

- L1 (fold homomorphism): for streams `s` and `t` and class `c`,
  `sat(s + t, c) == sat_add(sat(s, c), sat(t, c))`, and
  `sat(s, c) == min(count(s, c), usize::MAX)`. The same for escape and
  constraint sets with union. The other lemmas build on this.
- L2 (read-only characterization): `verdict(fold(s))` is `ReadOnlyBorrowed`
  iff `s.len() > 0` and `forall|i| 0 <= i < s.len() ==> read_only(s[i])`. The
  right-hand side quantifies over positions, not over counters, so the proof
  needs genuine induction over the fold rather than unfolding a definition.
- L3 (escape-set characterization): `e` is in `escapes(t)` iff `e != No` and
  `exists|j| t[j] == e`; hence the set is empty iff every position is `No`.
- L4 (permutation invariance): if `s1.to_multiset() =~= s2.to_multiset()`,
  the folded profiles are equal, proved through
  `count(s, c) == s.to_multiset().count(c)`; likewise for escape and constraint
  sets. Permutation is stated with Verus multisets, not defined as "same
  counts", so the lemma cannot hold by definition.
- L5 (evidence anti-monotonicity, the false-positive guard): for
  `clone_only_used_by_borrow`, if the lint emits for use stream `s + u` and
  facts `f`, and `s` is non-empty, then it emits for `s` and `f`; and if any
  `u[i]` is not read-only, it does not emit for `s + u`. For
  `local_shared_ownership`, if any position of an appended escape or constraint
  stream is unwaived under the policy, it does not emit. This is the property
  decision D3 exists to protect.
- L6 (merge closure): if a lint emits for `(s, t, f)` and for `(u, v, f)`,
  it emits for `(s + u, t + v, f)`. Lint crates merge evidence from several MIR
  blocks, so this guards against merge-induced silence.
- L7 (threshold survives saturation): for `1 <= m <= usize::MAX`,
  `sat(p, Retained) >= m` iff `count(p, Retained) >= m`. Consequently
  saturation never changes an `owned_param_causes_clone` decision; it can only
  cap the reported count.
- L8 (inhabitation): witnesses exist that satisfy the premises of L5 and L6
  for each lint (for example the one-element stream `[SharedBorrow]` with
  unconflicted facts for the clone lint), stated as `exists` lemmas in the
  style of `verus/whitaker_cli_dispatch.rs` lines 107 to 117. This guards the
  implications in L5 and L6 against vacuity.

### Axioms (trusted, not verified here)

- A1: Verus, Kani (and its CBMC back end), proptest and the Rust compiler are
  sound for the fragments used.
- A2: The lint crates (roadmap 9.2 to 9.4, using 9.1.2 and 9.1.4) map MIR
  facts onto `UseClass`, `EscapeKind`, `ApiConstraint`, and the other fact
  enums faithfully, following decisions D12 to D16. This is the largest
  residual gap: the proofs show the decisions are right given faithful
  summaries, not that summaries are faithful. Those items carry their own UI
  tests.
- A3: `fluent_bundle::FluentBundle::format_pattern` reports a referenced but
  absent variable in its error list and formats present string and number
  variables as their values. `common::i18n` does not surface those errors (see
  `Surprises & discoveries`), so the contract test checks them at the bundle
  level.
- A4: The decision half of counter saturation is proved (L7 and I1); A4 only
  states that reported counts above `usize::MAX` are capped, which affects
  diagnostic text, never a decision.

### Obligations, methods and evidence

Kani (production code; artefacts under `common/src/ownership_shape/kani/`;
command `make kani-ownership-shape`; harnesses use `#[kani::unwind(10)]` with
unwinding assertions enabled, and every loop in the checked code has a constant
trip count of at most nine, so each check is complete over its domain, not
merely bounded):

- K0 (step refinement, bridges production to the Verus model): harnesses
  `verify_use_profile_step_refines_model`,
  `verify_escape_set_step_refines_model`,
  `verify_api_constraint_set_step_refines_model`, and
  `verify_owned_param_pressure_step_refines_model` check, for a symbolic state
  and a symbolic class or kind, that one production `with_*` step equals the
  model step (`saturating_add(1)` on the chosen counter only; union for sets,
  with `No` and `None` as identity) and that `new()` is empty. Rationale: the
  Verus fold lemmas (L1 to L4, L7) are about the model step; K0 makes the
  production step provably the same, so `FromIterator` (a one-line fold,
  inspected and covered by P1) inherits the stream lemmas.
- K1 (I1): `verify_use_profile_verdict_matches_spec` checks
  `UseProfile::verdict` against `spec::verdict_from_support`, which reads only
  whether each counter is non-zero.
- K2 to K4 (I4, I5, I6, I7): `verify_clone_only_used_by_borrow_matches_spec`,
  `verify_owned_param_causes_clone_matches_spec`, and
  `verify_local_shared_ownership_matches_spec` check, for symbolic summaries
  and policies, that `matches!(evaluate(..), Emit(_)) == spec::emits(..)`,
  where the specification is an unordered conjunction (so it does not mirror
  the production check order); that on `Suppress(r)`,
  `spec::reason_holds(r, ..)` is true (I7); and that on `Emit(f)`, the
  finding's fields equal their specification (fix confidence; use shape equal
  to the verdict's shape; call-site count equal to the retained count and at
  least the minimum).
- R1 to R3 (reachability, non-vacuity of K2 to K4):
  `verify_<lint>_emit_is_reachable` builds one concrete emitting input, then
  `assert!(false, "...")`; run through `run_expected_counterexample`, so the
  run fails if `Emit` were unreachable.
- D-harnesses (discrimination, checked-in seeded faults): for each lint, an
  expected-counterexample harness asserts that production agrees with a
  deliberately wrong specification (clone lint: `CopyRead` treated as not
  read-only; owned parameter: `>` instead of `>=` against the minimum; shared
  ownership: `ThreadSafeSharedRequired` treated as waivable). Each must produce
  a counterexample, proving the K harnesses can tell a right rule from a
  plausible wrong one. These replace throwaway scratch-branch mutations, so the
  evidence is rerun every time.
- Arbitrary values: `#[cfg_attr(kani, derive(kani::Arbitrary))]` only on
  fieldless enums and all-`bool` policies; handwritten
  `#[cfg(kani)] impl kani::Arbitrary` inside the defining modules for
  `UseProfile`, `EscapeSet`, `ApiConstraintSet`, `CalleeExemptions`,
  `OwnedParamPressure`, and the summaries, generating only reachable states.
- Evidence: each K harness prints `VERIFICATION:- SUCCESSFUL`; each R and D
  harness is reported by the runner as the expected counterexample; the run
  exits zero. Runner and contract tests are updated in EP-M4 (see
  `Plan of work`).

Verus (model; artefacts `verus/ownership_shape_stream_model.rs`,
`verus/ownership_shape_clone_use.rs`,
`verus/ownership_shape_shared_ownership.rs`; command
`make verus-ownership-shape`):

- V1: L1 to L4 and L7 over the stream model.
- V2: L5, L6 and L8 for `clone_only_used_by_borrow` and
  `local_shared_ownership`.
- V3: L6 and L8 for `owned_param_causes_clone` (merge closure of call-site
  pressure and parameter uses), together with L7.
- Rationale: these properties quantify over streams of every length and
  order, which neither Kani's constant-bound checks nor sampling can establish;
  the stream model is small and stable. Predicates are defined from the inputs
  alone, never from the decision functions.
- Evidence: the run reports `0 errors`; record the observed verified-item
  count in `Artefacts and notes` and treat a later drop as a failure. The
  trusted-construct search listed under `Concrete steps` (proof runs) prints
  nothing.
- Non-vacuity: L8 witnesses inhabit every implication premise in L5 and L6;
  seeded fault, observed once and recorded in `Artefacts and notes`: deleting
  the non-empty conjunct from the model's `ReadOnlyBorrowed` verdict must make
  L2 fail to verify.

proptest (artefacts under `common/src/ownership_shape/tests/`; default 256
cases; run by `make test`):

- P1 (I2, I3 on production accumulators): `accumulation_props.rs` generates a
  stream with `prop::collection::vec(any::<UseClass>(), 0..=64)` and a shuffled
  copy with `prop_shuffle`, and asserts the two accumulated values are equal
  and agree with a `BTreeMap` or `BTreeSet` oracle; likewise for escape kinds,
  constraints and call-site evidence. No `prop_assume!` is used, so no case is
  discarded. The empty stream and a stream containing all nine classes are
  pinned as explicit `rstest` rows in `use_profile.rs`.
- P2 (I4 drift guard): `spec_agreement_props.rs` generates summaries and
  policies and asserts each `evaluate_*` agrees with the `spec.rs` predicate
  that the Kani harnesses use (compiled under `#[cfg(any(test, kani))]`). This
  runs on every pull request, unlike the nightly proof lane, so a rule change
  that is not mirrored in the specification fails immediately. Seeded fault
  observed once: inverting the macro-origin check must fail P2.

Example tests (run by `make test`):

- T1 (I4 examples, I8, design rule tables, DS-17 defaults): `rstest` tables
  per lint, one row per rule in design §12.6, §13.3, §13.7, §14.6, and §14.7,
  asserting the exact suppression reason with `googletest` matchers, plus
  happy-path rows asserting the exact finding with `pretty_assertions`.
  Includes the §12.6 last-bullet row (a `Move` use from a by-value argument
  suppresses the clone lint), mixed waivable and non-waivable escapes, and
  macro-only call-site pressure. `policy_defaults.rs` asserts design §17 values.
- T2 (wrapper classification, applicability mapping, token phrases):
  exhaustive `rstest` over all outer and inner wrapper combinations (six outer
  kinds times seven inner options) against a handwritten table, over all three
  `FixConfidence` values for `applicability()`, and over every selector token's
  `as_str` and `fallback_text`. Finite and small, so the tables are exhaustive
  and no prover is warranted.
- T3 (I9): `diagnostics.rs` asserts each builder's key set equals its
  constant and that rebuilding gives an equal map; `insta` holds one
  multi-variant snapshot per lint (three snapshot files) covering every
  selector token.
- E1 (A3 boundary, DS-16): `common/tests/ownership_shape_fluent_contract.rs`
  builds a real `FluentBundle` from an inline test-only resource that
  references every argument and selector token for each lint, formats each
  message and its `.note` and `.help` attributes with `to_fluent_arguments()`
  output, and asserts that the error list is empty and that the output contains
  no U+FFFD replacement character and no unexpanded `{$`. The assertion lives
  in a reusable helper, `common/tests/support/fluent_args.rs`, so roadmap 9.5.1
  can apply it to the real `.ftl` files. Negative control: removing one
  argument must produce a non-empty error list, proving the test notices a
  missing argument.
- B1 (DS-18.2): `rstest-bdd` scenarios exercise the public API from a
  contributor's perspective (see `Validation and acceptance`).
- Doctests: every public function's example runs under `make test-doc`.

Where Verus is not warranted: the finite decision tables (Kani checks the real
code completely), diagnostic argument shaping (I9), wrapper-shape
classification, applicability mapping, token phrases, policy defaults, and
suppression precedence (I8).

## Plan of work

### Stage A: confirm, baseline and record decisions (EP-M0)

No production code. Confirm by reading dependency sources under
`~/.cargo/registry` that `FluentValue` implements `From<usize>` and that Kani
0.67 provides `Arbitrary` for `NonZeroUsize`; confirm that the pinned Verus
release accepts a `#[path]` module include between proof files (a two-file
scratch proof outside the tree). Run the baseline gates and record results.
Re-fetch remote branches and confirm ADR 0015 is free. Write ADR 0015
("Ownership-shape evaluation contract and verification split") with status
"Proposed", and add design §11.6 "Implementation decisions (9.1.1)" recording
D1 to D16 with status "Proposed", plus a short note in §16 on D4 and D5.
Go/no-go: proceed when every confirmation is recorded in
`Surprises & discoveries` or `Decision log` and the documentation gates pass.

### Stages B to D per milestone

Each of EP-M1 to EP-M3 follows Red-Green-Refactor. Red: add the tests and a
module skeleton whose functions return a deliberately wrong but well-typed
value (not `todo!`, which would panic instead of failing an assertion), run the
focused command, and observe assertion failures for the intended reason. Green:
implement until the focused command passes. Refactor: tidy, then run the full
gate sequence. EP-M4 and EP-M5 follow the verification analogue: write the
harness or lemma statements first and observe the expected failure (a failing
harness against a stub, or an unproved goal), then complete.

### Module tree delivered

```plaintext
common/src/ownership_shape/
├── mod.rs                       # //! overview; pub mod model, evaluation, diagnostics;
│                                # cfg(any(test, kani)) mod spec; cfg(kani) mod kani
├── model/
│   ├── mod.rs                   # re-exports the shared vocabulary
│   ├── use_profile.rs           # UseClass, UseProfile, UseVerdict, ReadOnlyUseShape, NotReadOnly
│   ├── escape.rs                # EscapeKind, EscapeSet, ApiConstraint, ApiConstraintSet
│   ├── fix.rs                   # FixConfidence -> crate::diagnostics::Applicability
│   └── wrapper.rs               # WrapperKind, WrapperShape, WrapperClass
├── evaluation/
│   ├── mod.rs                   # Evaluation<F, R>, SpanOrigin
│   ├── clone_only_used_by_borrow.rs
│   ├── owned_param_causes_clone.rs
│   └── local_shared_ownership.rs
├── diagnostics/
│   ├── mod.rs                   # DiagnosticArgName, ArgValue, OwnershipDiagnosticArgs, MessageKey constants
│   └── builders.rs              # subject structs and per-lint builders
├── spec.rs                      # unordered-conjunction specifications for Kani and P2
├── kani/
│   ├── mod.rs
│   ├── model_harnesses.rs       # K0, K1
│   ├── evaluation_harnesses.rs  # K2 to K4
│   └── expected_failures.rs     # R1 to R3 and the discrimination harnesses
└── tests/
    ├── mod.rs
    ├── use_profile.rs
    ├── escape_sets.rs
    ├── wrapper_shape.rs
    ├── fix_confidence.rs
    ├── accumulation_props.rs
    ├── clone_evaluation.rs
    ├── owned_param_evaluation.rs
    ├── shared_ownership_evaluation.rs
    ├── policy_defaults.rs
    ├── spec_agreement_props.rs
    ├── diagnostics.rs
    └── snapshots/               # three multi-variant insta snapshots
```

Behaviour and integration tests:

```plaintext
common/tests/features/ownership_shape_evaluation.feature
common/tests/ownership_shape_evaluation_behaviour.rs
common/tests/ownership_shape_fluent_contract.rs
common/tests/support/fluent_args.rs
```

Proof sidecar: `verus/ownership_shape_stream_model.rs`,
`verus/ownership_shape_clone_use.rs`, and
`verus/ownership_shape_shared_ownership.rs`.

Wiring: `common/src/lib.rs` (one `pub mod ownership_shape;` line and a
crate-doc mention), `common/Cargo.toml` (dev-dependencies `googletest`,
`pretty_assertions`, `insta`), `Cargo.lock`, `scripts/run-kani.sh` (a
`run_ownership_shape_harnesses` function with an explicit harness list,
`--manifest-path common/Cargo.toml`, rejection of unknown names with exit 2, an
`ownership-shape` arm before the decomposition fallback, inclusion in the
no-argument default run, and the expected-counterexample harnesses routed
through `run_expected_counterexample`), `scripts/run-verus.sh` (the
`ownership-shape` group in `proof_files_for_group`, in `all`, and in the
argument `case`), `Makefile` (`kani-ownership-shape` and
`verus-ownership-shape` targets with `.PHONY` entries), and
`tests/workflow_contracts/kani_selection_contract_test.py` (the group selects
exactly its harnesses, an unknown harness is rejected, and a failing runner's
exit status propagates). If `.github/workflows/proofs.yml` states a harness
count in prose, update it.

### Documentation delivered

- `docs/adrs/adr-0015-ownership-shape-evaluation-contract.md`: drafted in
  EP-M0 ("Proposed"), accepted in EP-M6.
- `docs/ownership-shape-lints-design.md`: §11.6 "Implementation decisions
  (9.1.1)" (D1 to D16, with a reference to ADR 0015) and a §16 note on message
  keys and selector tokens.
- `docs/developers-guide.md`: new "Shared ownership-shape helpers" section
  (purpose, types, composition rules, re-use policy, and test and proof
  coverage, modelled on "Shared fingerprint helpers"); amended "Shared
  diagnostic localization" (ownership-shape message keys and argument names
  live in `common`); new targets in "Make targets"; the ownership-shape model
  in "Verus scope and trust boundary" (what the model proves, what Kani covers
  on production code); the new harness group in the Kani guidance.
- `docs/repository-layout.md`: mention ownership-shape models in the
  `common/` paragraph.
- `docs/contents.md`: list ADR 0015.
- `docs/roadmap.md`: mark 9.1.1 `[x]` after all gates pass.
- `docs/users-guide.md` is not changed: it documents installation,
  configuration and shipped lints only, and this item ships no lint and no
  configuration. Roadmap item 9.5.3 updates it when the lints land.

## Milestones and plateaus

### EP-M0: confirmations, baseline, and proposed decisions

- Outcome: Stage A facts recorded; baseline gate logs exist under `/tmp`; ADR
  0015 and design §11.6 exist with status "Proposed".
- Requirements and gaps: none discharged; de-risks K0 to K4, V1 to V3 and E1;
  makes D1 to D16 reviewable outside this plan.
- Acceptance evidence: `Surprises & discoveries` entries for each
  confirmation; baseline logs; `make markdownlint`, `make nixie` and
  `make spelling` pass.
- Conformance check:
  - No code changed.
  - Design changes are additive (§11.6, §16 note) and mirror the approved
    decisions exactly.
  - Trace links current.
- Recovery: repeatable; revert the documentation commit if needed.
- Remaining gaps: all implementation.
- Compatibility decision: none.

### EP-M1: shared vocabulary

- Outcome: `common::ownership_shape::model` compiles, is documented with
  doctests, and passes unit and property tests. `common/src/lib.rs` declares
  `pub mod ownership_shape;`.
- Requirements and gaps: DS-11.3 (enums), DS-18.1 (escape-kind reduction),
  I1 to I3 by test.
- Acceptance evidence: the focused unit command passes, including
  `tests::use_profile`, `tests::escape_sets`, `tests::wrapper_shape`,
  `tests::fix_confidence`, and `tests::accumulation_props`; each was observed
  failing against stubs first. Full gates pass.
- Conformance check:
  - DS-11.3 enum variants keep the design names exactly.
  - D3, D6 and D15 followed.
  - No rustc type, no string in model types.
  - Only the authorized dev-dependencies added.
  - Trace links current.
- Recovery: `git revert` the milestone commit; nothing depends on it yet.
- Remaining gaps: evaluation, diagnostics, proofs.
- Compatibility decision: none (new, pre-release internal API).

### EP-M2: evaluation, one commit per lint

EP-M2a delivers `evaluation/mod.rs`, `clone_only_used_by_borrow.rs`, the clone
half of `spec.rs`, its rows in `spec_agreement_props.rs`,
`clone_evaluation.rs`, the clone policy defaults, and the clone BDD scenarios.
EP-M2b and EP-M2c do the same for `owned_param_causes_clone` and
`local_shared_ownership`. Each sub-milestone is a commit and a plateau.

- Outcome: the three `evaluate_*` functions, their summary, policy, finding
  and suppression types, and `Evaluation<F, R>` exist and pass the rule tables,
  P2, and the BDD scenarios.
- Requirements and gaps: DS-12, DS-13, DS-14, DS-15.3, DS-17, DS-18.2; D1,
  D2, D7, D9 to D14, D16.
- Acceptance evidence: the rule-table tests, P2, and
  `cargo test -p whitaker-common --test ownership_shape_evaluation_behaviour`
  pass; each sub-milestone's scenarios were observed failing first. Full gates
  pass after each sub-milestone.
- Conformance check:
  - Every bullet of design §12.6, §13.3, §13.7, §14.6, §14.7 has a table row.
  - Policy defaults equal design §17 (D16 omissions recorded).
  - All `evaluate_*` are `const fn`; summaries and findings are `Copy`.
  - No rustc type; no string in evaluation inputs or findings.
  - Trace links current.
- Recovery: revert the latest sub-milestone commit; the previous one remains
  a coherent plateau.
- Remaining gaps: diagnostics, proofs, documentation.
- Compatibility decision: none.

### EP-M3: diagnostic argument builders

- Outcome: builders produce deterministic, fixed-key argument maps convertible
  to `common::i18n::Arguments`; message-key constants and token fallback
  phrases exist; the Fluent contract helper exists.
- Requirements and gaps: DS-16; D4, D5; I9; E1.
- Acceptance evidence: `tests::diagnostics`, the three `insta` snapshots, and
  `cargo test -p whitaker-common --test ownership_shape_fluent_contract` pass;
  E1's negative control passes by observing a non-empty error list. Full gates
  pass.
- Conformance check:
  - Argument names include every design §16 name that applies to each lint.
  - No `.ftl` file added under `common/locales/`.
  - Trace links current.
- Recovery: revert the milestone commit; EP-M2c remains coherent.
- Remaining gaps: proofs and documentation.
- Compatibility decision: none.

### EP-M4: Kani harnesses and runner wiring

- Outcome: K0 to K4 verify; R and D harnesses produce their expected
  counterexamples; `make kani-ownership-shape` exists; the default `make kani`
  runs the group.
- Requirements and gaps: I1, I2 (step level), I4 to I7 on production code.
- Acceptance evidence: `make kani-ownership-shape` exits zero, with
  `VERIFICATION:- SUCCESSFUL` for each K harness and the expected
  counterexample reported for each R and D harness (excerpts recorded in
  `Artefacts and notes`); `make test-workflow-contracts` passes; full gates
  pass.
- Conformance check: harnesses call production functions; no `kani::assume`
  beyond type invariants; runner lists and contract tests updated together.
- Recovery: revert the milestone commit.
- Remaining gaps: Verus and documentation.
- Compatibility decision: none.

### EP-M5: Verus stream model and proofs

- Outcome: the three Verus files prove L1 to L8; `make verus-ownership-shape`
  exists; the default `make verus` includes the group.
- Requirements and gaps: I2, I3, L1 to L8.
- Acceptance evidence: `make verus-ownership-shape` reports `0 errors`; the
  trusted-construct search prints nothing; the seeded fault was observed
  failing and reverted (excerpt recorded); full gates pass.
- Conformance check: model step equals the K0-checked production step; each
  file names the production functions it mirrors; predicates are defined from
  inputs only; no trusted constructs; each file under 400 lines.
- Recovery: revert the milestone commit.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation finalized and roadmap updated

- Outcome: ADR 0015 "Accepted"; design §11.6 updated with any implementation
  discoveries; developers' guide, repository layout and contents updated as
  listed above; `docs/roadmap.md` marks 9.1.1 `[x]`.
- Requirements and gaps: `AGENTS.md` documentation duties; the abstraction
  policy (sweep, scope, record).
- Acceptance evidence: `make markdownlint`, `make nixie`, `make spelling`,
  and the full gates pass.
- Conformance check: every accepted decision (D1 to D16) appears in the
  design and, where listed, the ADR; no upstream change unrecorded.
- Recovery: revert the documentation commit.
- Remaining gaps: none for 9.1.1; 9.1.2 onward.
- Compatibility decision: none.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/e706400c-e65a-4cdc-93f9-68dfd2930576`.
Log every gate with `tee` to
`/tmp/$ACTION-$(get-project)-$(git branch --show-current).out`, and run gates
sequentially, never in parallel for the same worktree.

Full gate sequence (baseline, and after every milestone and sub-milestone):

```sh
set -o pipefail
B="$(git branch --show-current)"; P="$(get-project)"
make fmt 2>&1 | tee "/tmp/fmt-$P-$B.out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-$P-$B.out"
make lint 2>&1 | tee "/tmp/lint-$P-$B.out"
make test 2>&1 | tee "/tmp/test-$P-$B.out"
make test-doc 2>&1 | tee "/tmp/test-doc-$P-$B.out"
```

Expected: `make test` ends with a nextest summary such as
`Summary [...] NNNN tests run: NNNN passed, M skipped`; `make test-doc` ends
with `test result: ok.` for each package. Check `git status` after `make fmt`
and revert churn in files this plan does not touch.

Focused loops during Red-Green-Refactor:

```sh
set -o pipefail
B="$(git branch --show-current)"; P="$(get-project)"
cargo test -p whitaker-common ownership_shape 2>&1 | tee "/tmp/unit-$P-$B.out"
cargo test -p whitaker-common --test ownership_shape_evaluation_behaviour 2>&1 \
  | tee "/tmp/bdd-$P-$B.out"
cargo test -p whitaker-common --test ownership_shape_fluent_contract 2>&1 \
  | tee "/tmp/fluent-$P-$B.out"
cargo insta test -p whitaker-common --review   # only when snapshots change; accept deliberately
```

Proof runs (EP-M4 and EP-M5):

```sh
set -o pipefail
B="$(git branch --show-current)"; P="$(get-project)"
make kani-ownership-shape 2>&1 | tee "/tmp/kani-$P-$B.out"
make verus-ownership-shape 2>&1 | tee "/tmp/verus-$P-$B.out"
make test-workflow-contracts 2>&1 | tee "/tmp/contracts-$P-$B.out"
rg -n '\bassume\s*\(|\badmit\s*\(|external_body|external_fn_specification|assume_specification' \
  verus/ownership_shape_*.rs
```

Expected Kani excerpt for each K harness:

```plaintext
SUMMARY:
 ** 0 of N failed
VERIFICATION:- SUCCESSFUL
```

Expected Verus excerpt per file:

```plaintext
verification results:: N verified, 0 errors
```

Documentation gates (EP-M0 and EP-M6, in addition to the full sequence):

```sh
set -o pipefail
B="$(git branch --show-current)"; P="$(get-project)"
make markdownlint 2>&1 | tee "/tmp/markdownlint-$P-$B.out"
make nixie 2>&1 | tee "/tmp/nixie-$P-$B.out"
make spelling 2>&1 | tee "/tmp/spelling-$P-$B.out"
```

Commit after each milestone and sub-milestone once its gates pass, following the
`commit-message` skill and `AGENTS.md` (imperative subject of about 50
characters, wrapped body explaining what and why).

## Validation and acceptance

Behaviour-driven development (BDD) feature specification, written in
`common/tests/features/ownership_shape_evaluation.feature` during EP-M2 (each
sub-milestone adds its scenarios) and kept synchronized with the
implementation. Scenarios are bound with `#[scenario(path = ..., name = ...)]`.
Count-and-noun steps use one pattern with a small `FromStr` phrase type that
accepts both singular and plural nouns (for example `shared borrow` and
`shared borrows`). Type names are quoted so that a later conversion to a
Scenario Outline cannot read angle brackets as placeholders. Every `And` after a
`Then` is a `#[then]` step. No pattern has more than three placeholders.

```gherkin
Feature: Ownership-shape evaluation
  The shared evaluation helpers decide whether an ownership-shape lint
  reports a finding, using only compiler-independent summaries.

  Scenario: A clone only used by shared borrows is reported
    Given a clone of a local receiver
    And the clone is observed in 2 shared borrows
    And the clone has a same-expression rewrite shape
    When clone-only-used-by-borrow is evaluated with the default policy
    Then a finding is emitted with machine-applicable fix confidence

  Scenario: A moved clone is suppressed
    Given a clone of a local receiver
    And the clone is observed in 1 move
    When clone-only-used-by-borrow is evaluated with the default policy
    Then the evaluation is suppressed because the uses are consumed

  Scenario: An unused clone is suppressed
    Given a clone of a local receiver
    When clone-only-used-by-borrow is evaluated with the default policy
    Then the evaluation is suppressed because no use was observed

  Scenario: A clone produced by a macro expansion is suppressed
    Given a clone of a local receiver inside a macro expansion
    And the clone is observed in 1 shared borrow
    When clone-only-used-by-borrow is evaluated with the default policy
    Then the evaluation is suppressed because of a macro expansion

  Scenario: Disabling machine suggestions keeps the finding but not the fix
    Given a clone of a local receiver
    And the clone is observed in 1 shared borrow
    And the clone has a same-expression rewrite shape
    When clone-only-used-by-borrow is evaluated without machine suggestions
    Then a finding is emitted with diagnostic-only fix confidence

  Scenario: A read-only owned parameter under clone pressure is reported
    Given a private callee parameter observed in 2 shared borrows
    And 2 retained clone-pressure call sites
    And an exact "String" to "&str" borrow mapping
    When owned-param-causes-clone is evaluated with the default policy
    Then a finding is emitted with maybe-incorrect fix confidence
    And the finding reports 2 call sites

  Scenario: Clone pressure only from macro expansions is suppressed
    Given a private callee parameter observed in 1 shared borrow
    And 2 macro-expanded clone-pressure call sites
    When owned-param-causes-clone is evaluated with the default policy
    Then the evaluation is suppressed because of insufficient call-site pressure

  Scenario: An exported callee is suppressed by default
    Given an exported callee parameter observed in 1 shared borrow
    And 1 retained clone-pressure call site
    When owned-param-causes-clone is evaluated with the default policy
    Then the evaluation is suppressed because the callee is exported

  Scenario: An exported callee is reported when exported-API avoidance is off
    Given an exported callee parameter observed in 1 shared borrow
    And 1 retained clone-pressure call site
    When owned-param-causes-clone is evaluated allowing exported API changes
    Then a finding is emitted with diagnostic-only fix confidence

  Scenario: A wrapper bridging a static callback stays quiet
    Given a local "Rc<Cell<_>>" wrapper
    And the wrapper escapes into a static callback
    When local-shared-ownership is evaluated with the default policy
    Then the evaluation is suppressed because the wrapper escapes

  Scenario: A waived escape does not hide an unwaivable one
    Given a local "Rc<Cell<_>>" wrapper
    And the wrapper escapes into a static callback
    And the wrapper escapes across a thread boundary
    When local-shared-ownership is evaluated without ignoring static callbacks
    Then the evaluation is suppressed because the wrapper escapes

  Scenario: A non-escaping shared mutable wrapper is reported diagnostic-only
    Given a local "Rc<RefCell<_>>" wrapper
    And the wrapper is only used by local sequential operations
    When local-shared-ownership is evaluated with the default policy
    Then a finding is emitted for a shared mutable wrapper
    And the finding is diagnostic-only
```

Red-Green-Refactor evidence to record per milestone in `Artefacts and notes`:
the focused command failing against stubs with assertion failures (for example a
`googletest` `Value of: ... Expected: ...` mismatch naming the expected
suppression reason); the same command passing after implementation; then the
full gates passing.

Quality criteria (what "done" means):

- Tests: `make test` and `make test-doc` pass, including the new unit,
  property, snapshot, behaviour, Fluent contract, and doctest suites.
- Verification: K0 to K4 verified; R and D harnesses produce their expected
  counterexamples; the Verus files report 0 errors with no trusted constructs;
  the seeded Verus and P2 faults were observed failing.
- Lint and formatting: `make check-fmt` and `make lint` pass (Clippy with
  `-D warnings` and Whitaker's own suite over `whitaker-common`).
- Documentation: `make markdownlint`, `make nixie`, and `make spelling` pass.
- Performance and security: not applicable; pure functions with no I/O.

## Idempotence and recovery

All steps are re-runnable. Tests, Kani and Verus have no side effects beyond
`target/` and the Verus and Kani caches under `~/.cache/whitaker/`. Each
milestone and sub-milestone is a separate commit, so `git revert <sha>`
restores the previous plateau. If a gate fails half-way, read its `/tmp/...out`
log rather than re-running it, fix the cause, then re-run that gate. Never use
bare `git stash` (the stash is shared between worktrees); use a temporary
work-in-progress commit instead.

## Artefacts and notes

To be filled during implementation: Red and Green transcript excerpts per
milestone, Kani summaries (including expected-counterexample reports), Verus
summaries with the verified-item counts, seeded-fault failure excerpts, and the
confirmed ADR number.

## Interfaces and dependencies

The signatures below are the target shape. Names may be refined during
implementation only if recorded in `Decision log`; variants of design enums
keep the design's names. Every type derives `Clone`, `Copy`, `Debug`,
`PartialEq` and `Eq`; set elements and map keys also derive `PartialOrd`, `Ord`
and `Hash`. Every public value-returning function carries `#[must_use]` except
the `evaluate_*` functions (see `Constraints`). Kani `Arbitrary` follows the
rule in `Verification plan`.

In `common/src/ownership_shape/model/use_profile.rs`:

```rust
pub enum UseClass {
    SharedBorrow, MethodBySharedRef, CopyRead,
    MutableBorrow, MethodByMutableRef,
    Move, Return, Store,
    UnknownCall,
}
impl UseClass {
    pub const ALL: [Self; 9];
    pub const fn is_read_only(self) -> bool;
}

/// Saturating per-class counters held in named fields (no array indexing).
pub struct UseProfile { /* private */ }
impl UseProfile {
    pub const fn new() -> Self;
    pub const fn with_use(self, class: UseClass) -> Self;
    pub const fn count(self, class: UseClass) -> usize;
    pub const fn total(self) -> usize; // explicit saturating_add chain
    pub const fn verdict(self) -> UseVerdict;
}
impl FromIterator<UseClass> for UseProfile { /* fold over with_use */ }

pub enum UseVerdict {
    NoUses,
    ReadOnlyBorrowed(ReadOnlyUseShape),
    NotReadOnly(NotReadOnly),
}
pub enum ReadOnlyUseShape { SharedBorrow, SharedMethod, CopyRead, Mixed }
impl ReadOnlyUseShape {
    pub const fn as_str(self) -> &'static str;        // selector token
    pub const fn fallback_text(self) -> &'static str; // deterministic English
}
pub enum NotReadOnly { Consumed, MutablyBorrowed, Opaque }
```

In `common/src/ownership_shape/model/escape.rs` (variant documentation records
D15):

```rust
pub enum EscapeKind {
    No, Return, Store, ClosureCapture, StaticCallback,
    ThreadBoundary, ExternalApiBoundary, Unknown,
}
/// A present escape: every `EscapeKind` except `No`.
pub enum Escape {
    Return, Store, ClosureCapture, StaticCallback,
    ThreadBoundary, ExternalApiBoundary, Unknown,
}
pub struct EscapeSet { /* private bitset over `Escape` */ }
impl EscapeSet {
    pub const fn new() -> Self;
    pub const fn with(self, kind: EscapeKind) -> Self; // `No` is the identity
    pub const fn contains(self, escape: Escape) -> bool;
    pub const fn is_empty(self) -> bool;
    /// Lowest present escape in declaration order, by a `contains` cascade.
    pub const fn first(self) -> Option<Escape>;
}
impl FromIterator<EscapeKind> for EscapeSet { /* fold over with */ }

pub enum ApiConstraint {
    None, SharedHandleRequired, TraitSignatureRequired,
    StaticCallbackRequired, ThreadSafeSharedRequired,
}
/// A present constraint: every `ApiConstraint` except `None`.
pub enum Constraint {
    SharedHandleRequired, TraitSignatureRequired,
    StaticCallbackRequired, ThreadSafeSharedRequired,
}
pub struct ApiConstraintSet { /* same shape as EscapeSet */ }
```

In `common/src/ownership_shape/model/fix.rs`:

```rust
pub enum FixConfidence { MachineApplicable, MaybeIncorrect, DiagnosticOnly }
impl FixConfidence {
    /// `None` means "emit no suggestion".
    pub const fn applicability(self) -> Option<crate::diagnostics::Applicability>;
}
```

In `common/src/ownership_shape/model/wrapper.rs`:

```rust
pub enum WrapperKind { Rc, Arc, Cell, RefCell, Mutex, RwLock }
pub struct WrapperShape { /* private: outer, optional inner */ }
impl WrapperShape {
    pub const fn new(outer: WrapperKind, inner: Option<WrapperKind>) -> Self;
    /// `None` for shapes outside design §14.2, such as `RefCell<Rc<_>>`.
    pub const fn class(self) -> Option<WrapperClass>;
    /// Locale-neutral code rendering such as `Rc<RefCell<_>>`.
    pub const fn render(self) -> &'static str;
}
pub enum WrapperClass { InteriorMutability, SharedHandle, SharedMutable }
impl WrapperClass {
    pub const fn as_str(self) -> &'static str;
    pub const fn fallback_text(self) -> &'static str;
}
```

In `common/src/ownership_shape/evaluation/mod.rs`:

```rust
#[must_use]
pub enum Evaluation<F, R> { Emit(F), Suppress(R) }
pub enum SpanOrigin { UserCode, MacroExpansion }
```

In `common/src/ownership_shape/evaluation/clone_only_used_by_borrow.rs`:

```rust
pub enum CloneForm { Clone, ToOwned }               // as_str: "clone" / "to_owned"
pub enum ReceiverPlace { Local, Field, Other }
pub enum OriginalPlaceAccess {
    Unconflicted, MovedBeforeLastUse, MutablyBorrowedBeforeLastUse, PathSensitiveConflict,
}
pub enum RewriteShape { SameExpression, SimpleLetBinding, Other }

pub struct CloneUseSummary {
    pub form: CloneForm,
    pub receiver: ReceiverPlace,
    pub uses: UseProfile,
    pub original: OriginalPlaceAccess,
    pub origin: SpanOrigin,
    pub rewrite: RewriteShape,
}
pub struct CloneOnlyUsedByBorrowPolicy {
    pub include_to_owned: bool,         // default true (design §17)
    pub emit_machine_suggestions: bool, // default true (design §17)
}

pub enum CloneSuppression {
    MacroExpansion,
    ReceiverNotSimplePlace,
    ToOwnedExcluded,
    NoObservedUse,
    UsesNotReadOnly(NotReadOnly),
    OriginalMovedBeforeLastUse,
    OriginalMutablyBorrowedBeforeLastUse,
    OriginalPathSensitiveConflict,
}

pub struct CloneOnlyUsedByBorrowFinding { /* private */ }
impl CloneOnlyUsedByBorrowFinding {
    pub const fn fix_confidence(self) -> FixConfidence;
    pub const fn use_shape(self) -> ReadOnlyUseShape;
    pub const fn form(self) -> CloneForm;
}

pub const fn evaluate_clone_only_used_by_borrow(
    summary: CloneUseSummary,
    policy: CloneOnlyUsedByBorrowPolicy,
) -> Evaluation<CloneOnlyUsedByBorrowFinding, CloneSuppression>;
```

Check order (I8): macro origin, receiver place, `to_owned` policy, use verdict
(`NoObservedUse`, then `UsesNotReadOnly`), original-place access. The escape
question of design §12.2 is answered by the use profile (`Move`, `Return`,
`Store`, `UnknownCall`), so the clone summary carries no escape set.

In `common/src/ownership_shape/evaluation/owned_param_causes_clone.rs`:

```rust
pub enum CallsiteEvidence { Retained, NotRetained, TemporarySource, MacroExpanded }
pub struct OwnedParamPressure { /* private saturating named counters */ }
impl OwnedParamPressure {
    pub const fn new() -> Self;
    pub const fn with_callsite(self, evidence: CallsiteEvidence) -> Self;
    pub const fn count(self, evidence: CallsiteEvidence) -> usize;
}
impl FromIterator<CallsiteEvidence> for OwnedParamPressure { /* fold */ }

pub enum CalleeExemption {
    Exported, TraitDeclaration, TraitImpl, Extern, Async, RequiresLifetimeIntroduction,
}
pub struct CalleeExemptions { /* private bitset */ }

/// Design §13.4's exact owned-to-borrowed pairs; produced by roadmap 9.1.3.
pub enum ExactBorrowKind { Str, Slice, Path, OsStr }

pub struct OwnedParamSummary {
    pub pressure: OwnedParamPressure,
    pub usage: UseProfile,
    pub exemptions: CalleeExemptions,
    pub exact_borrow: Option<ExactBorrowKind>,
}
pub struct OwnedParamCausesClonePolicy {
    pub min_local_callsites: core::num::NonZeroUsize, // default 1
    pub avoid_breaking_exported_api: bool,          // default true
    pub ignore_async_fns: bool,                     // default true
    pub representative_callsite_note: bool,         // default true
}

pub enum OwnedParamSuppression {
    InsufficientCallsitePressure,
    NoObservedUse,
    UsesNotReadOnly(NotReadOnly),
    CalleeExempt(CalleeExemption),
}

pub struct OwnedParamCausesCloneFinding { /* private */ }
impl OwnedParamCausesCloneFinding {
    pub const fn fix_confidence(self) -> FixConfidence;
    pub const fn callsite_count(self) -> core::num::NonZeroUsize;
    pub const fn use_shape(self) -> ReadOnlyUseShape;
    pub const fn exact_borrow(self) -> Option<ExactBorrowKind>;
    /// Whether to add a note at the first retained call site in source order.
    pub const fn emits_callsite_note(self) -> bool;
}

pub const fn evaluate_owned_param_causes_clone(
    summary: OwnedParamSummary,
    policy: OwnedParamCausesClonePolicy,
) -> Evaluation<OwnedParamCausesCloneFinding, OwnedParamSuppression>;
```

Check order (I8): call-site pressure, use verdict, exemptions in declaration
order.

In `common/src/ownership_shape/evaluation/local_shared_ownership.rs`:

```rust
pub enum OperationPattern { LocalSequential, Other }
pub enum StrongAliases { None, Classified, Unclassified }
pub enum WeakHandles { Absent, Present }
pub enum InnerHandle { Plain, FrameworkHandle }

pub struct SharedOwnershipSummary {
    pub wrapper: WrapperShape,
    pub escapes: EscapeSet,
    pub constraints: ApiConstraintSet,
    pub operations: OperationPattern,
    pub strong_aliases: StrongAliases,
    pub weak_handles: WeakHandles,
    pub inner: InnerHandle,
    pub origin: SpanOrigin,
}
pub struct LocalSharedOwnershipPolicy {
    pub ignore_static_callback_captures: bool,    // default true
    pub ignore_trait_signature_constraints: bool, // default true
    pub ignore_external_api_boundaries: bool,     // default true
}

pub enum SharedOwnershipSuppression {
    MacroExpansion,
    UnsupportedWrapperShape,
    Escapes(Escape),
    ApiConstraintPresent(Constraint),
    NonSequentialOperations,
    WeakHandles,
    UnclassifiedAliases,
    FrameworkHandle,
}

pub struct LocalSharedOwnershipFinding { /* private */ }
impl LocalSharedOwnershipFinding {
    pub const fn wrapper(self) -> WrapperShape;
    pub const fn wrapper_class(self) -> WrapperClass;
    /// Always `FixConfidence::DiagnosticOnly` (design §14.5).
    pub const fn fix_confidence(self) -> FixConfidence;
}

pub const fn evaluate_local_shared_ownership(
    summary: SharedOwnershipSummary,
    policy: LocalSharedOwnershipPolicy,
) -> Evaluation<LocalSharedOwnershipFinding, SharedOwnershipSuppression>;
```

Check order (I8): macro origin, wrapper shape, escapes (first unwaived escape
in declaration order), constraints (first unwaived constraint), operations,
weak handles, strong aliases, inner handle. Each stage is its own private
`const fn` returning `Option<SharedOwnershipSuppression>`, keeping every
function under the cognitive-complexity threshold.

In `common/src/ownership_shape/diagnostics/mod.rs` and `builders.rs`:

```rust
pub const CLONE_ONLY_USED_BY_BORROW: MessageKey<'static>;
pub const OWNED_PARAM_CAUSES_CLONE: MessageKey<'static>;
pub const LOCAL_SHARED_OWNERSHIP: MessageKey<'static>;

pub enum DiagnosticArgName {
    Variable, ReceiverType, CloneForm, Callee, Parameter, Wrapper, WrapperClass,
    CallsiteCount, UseSummary, ExactBorrowType, BorrowHint,
}
impl DiagnosticArgName { pub const fn as_str(self) -> &'static str; }

// Variable, ReceiverType, CloneForm, UseSummary
pub const CLONE_ONLY_USED_BY_BORROW_ARGS: &[DiagnosticArgName];
// Callee, Parameter, CallsiteCount, UseSummary, ExactBorrowType, BorrowHint
pub const OWNED_PARAM_CAUSES_CLONE_ARGS: &[DiagnosticArgName];
// Variable, Wrapper, WrapperClass
pub const LOCAL_SHARED_OWNERSHIP_ARGS: &[DiagnosticArgName];

pub enum BorrowHint { Exact, Generic } // as_str, fallback_text

pub enum ArgValue { Text(String), Count(usize) }
pub struct OwnershipDiagnosticArgs { /* BTreeMap<DiagnosticArgName, ArgValue> */ }
impl OwnershipDiagnosticArgs {
    pub fn get(&self, name: DiagnosticArgName) -> Option<&ArgValue>;
    pub fn iter(&self) -> impl Iterator<Item = (DiagnosticArgName, &ArgValue)> + '_;
    pub fn to_fluent_arguments(&self) -> crate::i18n::Arguments<'_>;
}

/// `variable` names the cloned receiver as written (for `foo(&name.clone())`,
/// `name`); `receiver_type` is its rendered type.
pub struct CloneSubject<'a> { pub variable: &'a str, pub receiver_type: &'a str }
/// `exact_borrow_type` is the rendered borrowed type (for example `&[u8]`);
/// `borrow_hint` is `exact` only when it and the finding's kind are both present.
pub struct OwnedParamSubject<'a> {
    pub callee: &'a str,
    pub parameter: &'a str,
    pub exact_borrow_type: Option<&'a str>,
}
pub struct SharedOwnershipSubject<'a> { pub variable: &'a str }

pub fn clone_only_used_by_borrow_args(
    finding: CloneOnlyUsedByBorrowFinding,
    subject: &CloneSubject<'_>,
) -> OwnershipDiagnosticArgs;
pub fn owned_param_causes_clone_args(
    finding: OwnedParamCausesCloneFinding,
    subject: &OwnedParamSubject<'_>,
) -> OwnershipDiagnosticArgs;
pub fn local_shared_ownership_args(
    finding: LocalSharedOwnershipFinding,
    subject: &SharedOwnershipSubject<'_>,
) -> OwnershipDiagnosticArgs;
```

In `common/src/ownership_shape/spec.rs` (`#[cfg(any(test, kani))]`): one
`emits_*`, `reason_holds_*`, and `finding_matches_*` function per lint, written
as unordered conjunctions over the summary fields and policy, plus
`verdict_from_support`. These are the single source of the specification for
both K harnesses and P2.

Re-use policy (recorded in the developers' guide in EP-M6): the
`ownership_shape` module is owned by Phase 9. Permitted call sites are the
three ownership-shape lint crates and their tests; other lints may reuse the
`model` vocabulary (`UseClass`, `UseProfile`, `EscapeKind`, `EscapeSet`,
`ApiConstraint`, `ApiConstraintSet`, `FixConfidence`) but must not call the
per-lint `evaluate_*` functions. Composition rule: lint crates build summaries
from compiler facts following decisions D12 to D16, call `evaluate_*`, and on
`Emit` call the matching builder; no lint crate re-implements a decision rule.
Any change to an `evaluate_*` function must update `spec.rs` (P2 fails
otherwise) and the Verus model in the same commit.

Dependencies: no new crates. `common/Cargo.toml` gains
`googletest.workspace = true`, `pretty_assertions.workspace = true`, and
`insta.workspace = true` under `[dev-dependencies]`. Kani and Verus are
external tools installed by `scripts/install-kani.sh` and
`scripts/install-verus.sh`.

## Revision note

2026-11-11, initial draft: written from three reconnaissance reports; awaiting
expert review and user approval.

2026-11-11, revision 1, after a three-member expert design review (API and
contracts; verification; pre-mortem, alternatives and process), verdict
"proceed with conditions". Changes:

- Evaluation is now fully string-free and `const`: `ExactBorrowMapping`
  became the closed `ExactBorrowKind`, and display names moved to subject
  structs.
- Each lint has its own suppression enum, with payloads that exclude `No` and
  `None`, and the read-only verdict carries its shape. The clone summary
  dropped its redundant escape set, and handle evidence is now two independent
  facts.
- The receiver type, clone form and representative call-site note were added
  (design §12.4, §13.5, §17), along with decisions D9 to D16 covering RFC 0008
  and RFC 0009 hooks, the never-waived trait exemptions, literal policy flags,
  call-site precedence, alias semantics, sequential-operation semantics, async
  capture mapping, and policy scope.
- Verus no longer re-proves finite decision tables. It now proves
  stream-level properties: fold homomorphism, multiset permutation invariance,
  evidence anti-monotonicity, merge closure, threshold under saturation and
  inhabitation.
- Kani gained step-refinement harnesses, specifications written as unordered
  conjunctions, reason-truthfulness checks, and expected-counterexample
  reachability and discrimination harnesses in place of `kani::cover!`. The
  `Arbitrary` derive is restricted to types where it is safe.
- The Kani specifications run in `make test` through P2.
- E1 checks Fluent errors at the bundle level, because `common::i18n` drops
  them.
- `make test-doc` was added to every gate.
- EP-M2 is split per lint, and the ADR and design notes are drafted in EP-M0.
- Size estimates and tolerances were restated, with a pre-agreed two-PR
  fallback.
- BDD scenarios now bind by name, quote type text, and cover unused clones,
  macro-only pressure and mixed escapes.

Remaining work is unchanged in kind: approval, then EP-M0 onward.
