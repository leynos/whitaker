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
   wrapper using small, deterministic, compiler-independent summary values;
2. call one pure function per lint and receive either a typed finding or a
   typed suppression reason; and
3. turn a finding plus the names of the things involved into a fixed,
   deterministic set of Fluent diagnostic arguments that the localization layer
   (`common::i18n`) can format.

Success is observable without any lint crate existing.
`cargo test -p whitaker-common` runs new unit, property, snapshot and behaviour
tests that exercise every emission and suppression rule in design §12 to §14.
`make kani-ownership-shape` exhaustively checks the production decision
functions against independently written specifications, and
`make verus-ownership-shape` proves the emission contracts over observation
sequences of unbounded length. A Fluent integration test shows that every
argument the builders produce resolves through the real
`common::i18n::resolve_message_set` without a missing-variable error.

There is no end-user-visible behaviour change in this item: no lint is
registered, no configuration is read, and no message appears in any build.

## Constraints

- `common` must remain free of `rustc_private` dependencies. No type from
  `rustc_*` crates (`DefId`, `Ty`, `Span`, `Applicability` from `rustc_errors`)
  may appear in `common/src/ownership_shape/`. Identity (which `DefId` a
  summary belongs to) stays in the lint crates.
- The roadmap scope boundary must hold. Resolved-path classifiers
  (`path_kinds`, roadmap 9.1.2), the exact borrow-mapping table (roadmap
  9.1.3), MIR summary helpers (roadmap 9.1.4), lint crates (9.2.1, 9.3.1,
  9.4.1), configuration loading from `dylint.toml` or `whitaker.toml`, and
  Fluent `.ftl` locale entries (roadmap 9.5.1) are out of scope. This plan
  defines the value type `ExactBorrowMapping` that 9.1.3's table will produce,
  but not the table.
- No `.ftl` file may be added under `common/locales/` by this item; Fluent
  entries for the three lints belong to roadmap 9.5.1. Integration tests use an
  inline, test-only Fluent resource instead.
- The workspace Clippy policy in the root `Cargo.toml` applies unchanged:
  among others `needless_pass_by_value`, `indexing_slicing`, `expect_used`,
  `unwrap_used`, `missing_const_for_fn`, `must_use_candidate`, `shadow_reuse`,
  `allow_attributes`, and `cognitive_complexity` are denied, and `pedantic` is
  warned (and therefore denied under `-D warnings`). Lint suppressions are a
  last resort and must be `#[expect(..., reason = "...")]` scoped to one item.
- Every source file stays under 400 lines (`AGENTS.md`). Modules use
  `mod.rs`-rooted directories and test modules wired with
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
  `assume_specification`, or any trusted body to discharge an obligation.
  Unproved lemma statements stay in this plan until the proof milestone.
- Kani harnesses may use `kani::assume` only to restate genuine production
  preconditions (none are expected; see `Verification plan`).
- No new external crates. `googletest`, `pretty_assertions`, and `insta` are
  already workspace dependencies (root `Cargo.toml` lines 22, 23 and 32) and
  may be added to `common/Cargo.toml` `[dev-dependencies]` with
  `.workspace = true`; this is explicitly authorized by the task brief.
- Behaviour-driven development (BDD) `given` and `when` steps must not
  assert; assertions belong in `then` steps (`panic_in_result_fn` is denied).

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

The size limits below exclude this ExecPlan file. Count only non-ExecPlan files
towards the file-count limit, and count code and other documentation changes
towards the line limit, including when this ExecPlan is updated in the same
change.

- Scope: if the implementation needs more than 45 non-ExecPlan files touched or
  more than 4,500 net added lines of code and other documentation, stop and
  escalate. The estimate is about 38 files and 3,300 lines.
- Interface: if any existing public item in `common` (outside the new
  `ownership_shape` module) must change signature, stop and escalate. Adding
  the new module and one `pub mod` line to `common/src/lib.rs` is in scope.
- Dependencies: if any crate not already declared in the root
  `[workspace.dependencies]` is required, stop and escalate.
- Design: if a design decision listed under "Proposed design clarifications"
  in `Decision log` is rejected at approval, or implementation evidence shows
  any other departure from [the design](../ownership-shape-lints-design.md)
  §11, §15 or §16 is needed, record it, set the status to `BLOCKED`, and
  escalate.
- Verification: if a Kani harness does not reach a verdict within the
  repository's 10-minute, 8 GiB `scripts/run-kani-limited.sh` budget, or a
  Verus lemma cannot be proved without a trusted assumption after three
  materially different attempts, stop and escalate with the open verification
  condition rather than weakening the statement.
- Iterations: if a gate (`make check-fmt`, `make lint`, `make test`) still
  fails after three fix attempts for the same root cause, stop and escalate.
- Time: if any single milestone takes more than one working day of elapsed
  effort, stop and escalate.
- Ambiguity: if a suppression or emission rule in design §12 to §14 admits two
  readings that produce different outcomes for the same summary, and the plan's
  `Decision log` does not already settle it, stop and present the options.

## Risks

- Risk: Kani cannot derive `kani::Arbitrary` for a field type used in the
  summaries (for example `core::num::NonZeroUsize` or `Option<String>`), or
  symbolic `String` values blow up CBMC. Severity: medium. Likelihood: medium.
  Mitigation: keep strings out of evaluation inputs except the
  `Option<ExactBorrowMapping>` payload; in harnesses, generate that option as
  either `None` or one fixed concrete mapping chosen by a symbolic boolean; for
  `NonZeroUsize`, generate a symbolic `usize`, and construct with
  `NonZeroUsize::new`, discarding zero by `kani::assume` (this restates the
  type's own invariant, not a production precondition). Confirm in Stage A.
- Risk: `fluent_bundle::FluentValue` lacks `From<usize>`, so numeric arguments
  need conversion. Severity: low. Likelihood: low. Mitigation: confirm in Stage
  A; if absent, convert through `u64` or `f64` explicitly and record it.
- Risk: The Fluent selector tokens chosen here (`use_summary`, `borrow_hint`,
  `wrapper_class`) constrain the translations written in roadmap 9.5.1, and a
  translator may need a token not provided. Severity: medium. Likelihood: low.
  Mitigation: tokens are a closed, documented set exposed as constants; adding
  a token later is additive. The Fluent integration test pins the contract.
- Risk: The `owned_param_causes_clone` and `local_shared_ownership` policy
  defaults diverge from what the configuration work later reads. Severity: low.
  Likelihood: medium. Mitigation: policy defaults are taken verbatim from
  design §17 and asserted by unit tests that cite §17.
- Risk: The Verus model drifts from the production code, so the proof shows a
  property of a function nobody runs. Severity: high. Likelihood: medium.
  Mitigation: Kani checks the production functions against specifications that
  are written in the same terms as the Verus model's predicates, and the Verus
  file names the production functions it mirrors line by line. Any production
  change to a decision function must update both (recorded in the developers'
  guide re-use policy).
- Risk: `scripts/run-kani.sh` and `scripts/run-verus.sh` hard-code harness and
  proof lists, and `tests/workflow_contracts/kani_selection_contract_test.py`
  and `kani_runner_contract_test.py` assert runner behaviour. Adding a group
  may break a contract test. Severity: low. Likelihood: medium. Mitigation: run
  `make test-workflow-contracts` in the proof-wiring milestones and update the
  contract tests in the same commit.
- Risk: The 400-line limit is exceeded by evaluation or test files because the
  rule tables are larger than estimated (the 6.3.2 brain-trait plan estimated
  200 lines for a test file that reached 278). Severity: low. Likelihood:
  medium. Mitigation: the layout below already splits evaluation and tests per
  lint; split further by rule family if a file passes 350 lines.
- Risk: An ADR number collides with one allocated on an unmerged branch.
  Severity: low. Likelihood: medium (the repository already has two `0004` and
  two `0011` files). Mitigation: fetch all remote branches and scan for
  `docs/adrs/adr-*` before allocating, as recorded in the project memory.

## Progress

- [x] (2026-11-11) Renamed branch to
  `9-1-1-common-ownership-shape-model-type`.
- [x] (2026-11-11) Reconnaissance of `common` precedent, verification wiring,
  and documentation conventions (three Wyvern agents).
- [x] (2026-11-11) Drafted this ExecPlan.
- [ ] Expert design review and plan revision.
- [ ] Plan approved by the user.
- [ ] EP-M0: Stage A confirmations and baseline gates.
- [ ] EP-M1: shared vocabulary (`model/`).
- [ ] EP-M2: per-lint summaries, policies and evaluation (`evaluation/`).
- [ ] EP-M3: diagnostic argument builders (`diagnostics/`).
- [ ] EP-M4: Kani harnesses and runner wiring.
- [ ] EP-M5: Verus model and proofs and runner wiring.
- [ ] EP-M6: documentation, ADR, and roadmap update.

## Surprises & discoveries

- Observation: The brain-trust modules, the stated precedent for this work,
  do not build Fluent arguments at all; they format English strings directly.
  Evidence: `common/src/brain_trait_metrics/diagnostic.rs`
  (`format_primary_message`, `format_note`, `format_help` return `String`); no
  brain-trust `.ftl` files exist under `common/locales/`. Impact:
  `ownership_shape::diagnostics` is the first argument builder in `common`; it
  sets its own convention, documented in the developers' guide.
- Observation: Design §16 proposes slugs `<lint>.primary`, `<lint>.note`, and
  `<lint>.help`, but `common::i18n::resolve_message_set` resolves a message
  `<lint>` plus its `.note` and `.help` attributes, and every existing `.ftl`
  file uses that shape. Evidence: `common/src/i18n/diagnostics.rs`;
  `common/locales/en-GB/module_max_lines.ftl`. Impact: see decision D4.
- Observation: `common` already defines a compiler-independent
  `Applicability` mirror with the four rustc variants. Evidence:
  `common/src/diagnostics.rs` lines 7 to 16; rustc's
  `rustc_errors::Applicability` documents `MachineApplicable`, `MaybeIncorrect`,
  `HasPlaceholders` and `Unspecified`. Impact: `FixConfidence` maps onto it
  rather than inventing a parallel enum.
- Observation: `common/Cargo.toml` lacks `insta`, `googletest` and
  `pretty_assertions` dev-dependencies although the workspace declares them.
  Evidence: `common/Cargo.toml` lines 30 to 41. Impact: EP-M1 adds them.

## Decision log

### Proposed design clarifications (require approval with this plan)

These decisions refine or depart from the letter of
[the design](../ownership-shape-lints-design.md). Each is recorded as an
implementation decision in design §11.6 (new) during EP-M6, and D1 to D3 are
captured in a new ADR because they fix the contract all three lint crates
consume.

- Decision D1: Evaluation returns `Evaluation<F>`, an enum of
  `Emit(F)` or `Suppress(SuppressionReason)`, instead of the design's
  `EvaluationResult { emit: bool, level, fix_confidence, diagnostic_args }`.
  Rationale: the design's record allows impossible states (for example
  `emit == false` with a fix confidence and arguments), and the suppression
  reason is the most useful fact for tests and for future "why was this quiet?"
  tracing. Per-finding emission level is not modelled: no rule in design §12 to
  §14 varies the level per finding, and the rustc lint level (`declare_lint!`
  default, `#[allow]`, configuration) already owns it. Design §6.3's
  "downgrade" is expressed through `FixConfidence` and suppression. Affected
  identifiers: DS-11.3, DS-15.3. Date/Author: 2026-11-11, planning agent;
  awaiting approval.
- Decision D2: Evaluation functions take only compiler-independent facts and
  never handle display names. A finding carries the semantic facts the
  diagnostic needs (fix confidence, use-shape token, call-site count, borrow
  mapping, wrapper class). Separate builders in `diagnostics/` combine a
  finding with a small "subject" struct of names (variable, callee, parameter)
  to produce Fluent arguments. Rationale: keeps the decision kernel string-free,
  `const`-friendly, and tractable for Kani and Verus; keeps display concerns
  in one place. The design's requirement that evaluation yields "the diagnostic
  argument map needed for Fluent" (§15.3) is met by the finding-to-arguments
  builder, which is total. Affected identifiers: DS-15.3, DS-16. Date/Author:
  2026-11-11, planning agent; awaiting approval.
- Decision D3: Escape and API-constraint evidence is aggregated as sets
  (`EscapeSet`, `ApiConstraintSet`), not reduced to a single representative
  kind. `EscapeKind::No` and `ApiConstraint::None` are kept, as the design
  specifies, and act as identities: inserting them changes nothing, and "no
  escape" is the empty set. Rationale: the `local_shared_ownership` policy
  waives some escape and constraint kinds but not others; reducing several
  observations to one representative would lose a non-waivable kind behind a
  waivable one and emit a false positive. Set union is order-independent, so
  "escape-kind reduction" (design §18.1) is well defined. Affected identifiers:
  DS-11.3, DS-14.3, DS-14.6, DS-18.1. Date/Author: 2026-11-11, planning agent;
  awaiting approval.
- Decision D4: The Fluent message identifier for each lint is the lint name
  (`clone_only_used_by_borrow`), with `.note` and `.help` attributes, matching
  `resolve_message_set` and every existing locale file. The design's
  `<lint>.primary` slug denotes the message value. This plan exposes the three
  identifiers as `MessageKey<'static>` constants and documents the mapping in
  design §16. Affected identifiers: DS-16. Date/Author: 2026-11-11, planning
  agent; awaiting approval.
- Decision D5: Each lint has a fixed argument-name set, always fully
  populated, so a Fluent message never meets a missing variable. Values that
  would otherwise be prose are closed selector tokens for Fluent select
  expressions: `use_summary` is one of `shared_borrow`, `shared_method`,
  `copy_read`, or `mixed_read`; `borrow_hint` is `exact` or `generic`, with
  `exact_borrow_type` set to the empty string when generic; `wrapper_class` is
  `interior_mutability`, `shared_handle`, or `shared_mutable` (design §14.4).
  `wrapper` is rendered from the classified wrapper shape (for example
  `Rc<RefCell<_>>`), not passed in as free text. Rationale: English prose
  inside an argument cannot be translated, and design §16 lists argument names
  as examples ("such as"), not as a closed set. Affected identifiers: DS-16.
  Date/Author: 2026-11-11, planning agent; awaiting approval.
- Decision D6: Module layout groups shared vocabulary under `model/`,
  per-lint summary, policy, finding, and evaluation function together under
  `evaluation/<lint>.rs`, and argument builders under `diagnostics/`. The
  design's `path_kinds.rs` and `exact_borrow_mappings.rs` are not created here
  (roadmap 9.1.2 and 9.1.3). Nothing is re-exported at the `common` crate root;
  callers write `common::ownership_shape::...`. Rationale: honours the design's
  top-level names while following `AGENTS.md` "group by feature" and the
  400-line limit, and avoids the root-level name collisions the 6.3.2
  brain-trait work hit with `format_help` and friends. Affected identifiers:
  DS-11.2. Date/Author: 2026-11-11, planning agent; awaiting approval.
- Decision D7: An empty use profile (no observed use of the clone or the
  parameter) is suppressed with `SuppressionReason::NoObservedUse` rather than
  treated as vacuously read-only. Rationale: "every observed use is read-only"
  is vacuously true of nothing; an unused clone or parameter is Clippy's and
  rustc's territory (`redundant_clone`, `unused_variables`), and Whitaker's
  false-positive budget (design §6.2) favours silence. Affected identifiers:
  DS-12.2, DS-13.2.2. Date/Author: 2026-11-11, planning agent; awaiting
  approval.
- Decision D8: Summaries carry no compiler identity (`DefId`, `TypeId`). The
  design's class diagram shows `callee_id: DefId` and `source_type: TypeId`;
  those stay in the lint crates, which group pressure by callee before building
  one `OwnedParamSummary` per parameter. Rationale: Constraint 1 (`common` is
  rustc-free). Affected identifiers: DS-11.3. Date/Author: 2026-11-11, planning
  agent; awaiting approval.

### Implementation decisions

- Decision: Use `make kani-ownership-shape` and `make verus-ownership-shape`
  as new Makefile targets backed by an `ownership-shape` group in each runner
  script, and include the group in the default `make kani` and `make verus`
  runs. Rationale: mirrors `kani-clone-detector` and `verus-clone-detector`;
  the nightly `proofs.yml` workflow already triggers on `common/**` and
  `verus/**`. Date/Author: 2026-11-11, planning agent.
- Decision: Verus proves model-level contracts over unbounded observation
  sequences; Kani checks the production decision functions exhaustively over
  their (finite after abstraction) input space; proptest bridges the production
  accumulation code over generated sequences. This follows the division of
  labour in
  [ADR 0003](../adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md).
  Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. To be completed at each milestone and at plan completion.

## Context and orientation

The repository is a Cargo workspace of Dylint lint crates. A Dylint lint is a
`cdylib` that the Rust compiler loads to run extra checks. The shared support
crate is `common/` (package `whitaker-common`, imported as `common`). It has no
compiler dependencies; everything compiler-specific lives in the lint crates
under `crates/`.

Key existing files a novice should read first:

- `docs/ownership-shape-lints-design.md` — the design. §11 (shared
  implementation approach), §12 to §14 (the three lints' rules), §15 (common
  helper requirements), §16 (diagnostics and localization), §17
  (configuration), §18 (testing strategy).
- `common/src/lib.rs` — declares `pub mod` modules and root re-exports.
- `common/src/brain_trait_metrics/` — the nearest precedent: a pure
  `evaluation.rs` with a `const fn` decision function and a `diagnostic.rs`.
- `common/src/diagnostics.rs` — defines `common::diagnostics::Applicability`
  (`MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders`, `Unspecified`).
- `common/src/i18n/` — Fluent localization. `Arguments<'a>` is
  `HashMap<Cow<'a, str>, FluentValue<'a>>` (`loader.rs`); `MessageKey::new` is
  `const`; `resolve_message_set(lookup, key, args)` resolves a message and its
  `.note` and `.help` attributes through any `BundleLookup`.
- `common/tests/brain_trait_evaluation_behaviour.rs` and
  `common/tests/features/brain_trait_evaluation.feature` — the `rstest-bdd`
  pattern: a `world` fixture declared with `#[allow_fixture_expansion_lints]`
  and `#[fixture]`, steps with `#[given]`, `#[when]`, `#[then]`, scenarios
  bound with `#[scenario(path = ..., index = N)]`.
- `verus/` — Verus proof sidecar, one file per proof, each a mirrored model of
  production code; run by `scripts/run-verus.sh` (groups listed in
  `proof_files_for_group`) and `make verus`. `verus/whitaker_cli_dispatch.rs`
  is the closest analogue: it defines predicates from the input alone and adds
  `lemma_*_inhabited` lemmas to rule out vacuity.
- `scripts/run-kani.sh` — Kani runner with hard-coded harness groups; each
  harness runs through `scripts/run-kani-limited.sh` (8 GiB, 10 minutes).
  `common/src/decomposition_advice/community/mod.rs` shows `#[cfg(kani)]`
  harness wiring inside `common`.
- `docs/developers-guide.md` — "Shared diagnostic localization", "Shared
  fingerprint helpers" (a model for documenting a shared helper and its re-use
  policy), "Verus scope and trust boundary", and the Kani harness guidance.

Terms used in this plan:

- *Use class*: how one observed use of a value treats it (shared borrow,
  `&self` method call, copy read, mutable borrow, `&mut self` method call,
  move, return, store, or call into unknown code). Design §11.3 `UseClass`.
- *Use profile*: the count of observed uses per use class for one value.
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

Relevant skills and documents for the implementer: the `rust-router` skill (then
`rust-types-and-apis` for the type surface, `rust-unit-testing` for `rstest`,
`googletest`, `pretty_assertions` and `insta`, `proptest`, `kani`, `verus`, and
`rust-verification`); the `execplans` skill for maintaining this plan;
`codegraph-mcp` for navigation; and the documents
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
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
- ADRs:
  [ADR 0003](../adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md)
  (Verus for semantic invariants over models, Kani for bounded checks of the
  real implementation, tests for regression) and
  [ADR 0006](../adrs/adr-0006-directory-based-module-layout.md) (module layout
  and 400-line budget).
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`, root
  `Cargo.toml` `[workspace.lints]`.

Traced design elements (identifier, then source):

- DS-11.2: module layout (§11.2).
- DS-11.3: shared domain model enums and summaries (§11.3).
- DS-12: `clone_only_used_by_borrow` firing rule (§12.2), suggestion policy
  (§12.5), false-positive controls (§12.6).
- DS-13: `owned_param_causes_clone` pass A and B (§13.2), exemptions (§13.3),
  exact-mapping hint policy (§13.4), false-positive controls (§13.7).
- DS-14: `local_shared_ownership` scope (§14.2), detection (§14.3), classes
  (§14.4), diagnostic-only policy (§14.5), Servo-style exemptions (§14.6),
  false-positive controls (§14.7), and §10 (Servo calibration).
- DS-15.3: pure evaluation helpers (§15.3).
- DS-16: Fluent slugs, arguments and English fallback (§16).
- DS-17: configuration defaults (§17).
- DS-18.1 and DS-18.2: unit and behaviour test expectations (§18.1, §18.2).

Trace links:

```plaintext
DS-11.3 -> EP-M1 -> ownership_shape::model tests (use_profile, escape_sets,
           fix_confidence, wrapper_shape) + K1 + V1 + V2
DS-12   -> EP-M2 -> evaluation::clone_only_used_by_borrow tests + BDD scenarios 1-4 + K2 + V3
DS-13   -> EP-M2 -> evaluation::owned_param_causes_clone tests + BDD scenarios 5-7 + K3 + V4
DS-14   -> EP-M2 -> evaluation::local_shared_ownership tests + BDD scenarios 8-9 + K4 + V5
DS-15.3 -> EP-M2 -> evaluation tests; D1, D2
DS-16   -> EP-M3 -> diagnostics tests + insta snapshots + Fluent contract test; D4, D5
DS-17   -> EP-M2 -> policy default tests
DS-18.1 -> EP-M1..EP-M3 -> unit and property suites
DS-18.2 -> EP-M2 -> BDD feature (public-API suppression, callback-boundary suppression, machine-suggestion cases)
DS-11.2 -> EP-M1..EP-M3 -> module tree; D6
```

Design §18.2's "configuration precedence" and "localization fallback" behaviour
belong to configuration loading and to the lint crates' fallback wiring, both
out of scope; this plan covers the policy-value half of configuration
precedence (an explicit policy overrides defaults) and the argument half of
localization (arguments always resolve).

## Verification plan

### What is being verified, and why it matters

The three evaluation functions are contractual business logic: they decide
whether Whitaker accuses code of an ownership smell. A wrong "emit" is a false
positive, which the design treats as the dominant risk (§6.2); a wrong
"suppress" silently loses the lint's value. The accumulation helpers
(`UseProfile`, `EscapeSet`, `ApiConstraintSet`) turn an arbitrary-length,
arbitrary-order stream of MIR observations into the facts the decisions read,
so their semantics must not depend on the order or number of observations.
Diagnostic argument shaping is plumbing: it needs determinism and a stable key
set, which tests establish adequately; it does not warrant a proof.

### Abstract model (independent of implementation)

Let `U` be the finite set of nine use classes and `R ⊂ U` the read-only classes
`{SharedBorrow, MethodBySharedRef, CopyRead}`. An observation stream is a
finite sequence `s` over `U`. The profile of `s` is the function
`count_s : U → ℕ`, where `count_s(c)` is the number of positions with value
`c`. The support of `s` is `{c | count_s(c) > 0}`.

The use verdict is a function of the support alone:

- `NoUses` iff the support is empty;
- `ReadOnly` iff the support is non-empty and a subset of `R`;
- otherwise `Consumed` if the support meets `{Move, Return, Store}`, else
  `MutablyBorrowed` if it meets `{MutableBorrow, MethodByMutableRef}`, else
  `Opaque` (the support contains `UnknownCall` and no consuming or mutable
  class).

Let `E` be the eight escape kinds and `E* = E \ {No}`. An escape stream `t` is
a finite sequence over `E`; its escape set is `{e ∈ E* | e occurs in t}`.
Constraints are modelled the same way with `ApiConstraint::None` as identity.

Each lint's emission predicate is a conjunction over these abstractions and a
few finite facts (receiver place kind, span origin, conflict status, rewrite
shape, call-site counts, callee exemptions, wrapper shape, policy flags). The
predicates are written out in full in the Verus file and in the Kani
specification functions; the English statements are:

- `clone_only_used_by_borrow` emits iff the clone is not from a macro
  expansion; the receiver is a local or field place; the form is `clone`, or
  the form is `to_owned` and the policy includes `to_owned`; the use verdict is
  `ReadOnly`; the escape set is empty; and the original place is not moved or
  mutably borrowed before the clone's last use (design §12.2 and §12.6; D7).
- `owned_param_causes_clone` emits iff the number of retained, non-macro
  clone-pressure call sites is at least the policy minimum (which is at least
  one); the parameter's use verdict is `ReadOnly`; and every callee exemption
  present is waived by the policy, where `Exported` is waived only when
  `avoid_breaking_exported_api` is false, `Async` only when `ignore_async_fns`
  is false, and `TraitDeclaration`, `TraitImpl`, `Extern`, and
  `RequiresLifetimeIntroduction` are never waived (design §13.2, §13.3, §13.7,
  §17).
- `local_shared_ownership` emits iff the wrapper is not from a macro
  expansion; the wrapper shape is classifiable (§14.2); every escape kind in
  the escape set is waived (only `StaticCallback` when
  `ignore_static_callback_captures` is false, and only `ExternalApiBoundary`
  when `ignore_external_api_boundaries` is false); every constraint in the
  constraint set is waived (`StaticCallbackRequired` by the static-callback
  flag, `TraitSignatureRequired` by the trait-signature flag,
  `SharedHandleRequired` by the external-API flag; `ThreadSafeSharedRequired`
  never); operations are local and sequential; no `Weak` handle or unclassified
  strong alias is observed; and the inner type is not a framework handle
  (design §14.3, §14.6, §14.7, §17).

Fix confidence:

- `clone_only_used_by_borrow`: `MachineApplicable` iff the policy enables
  machine suggestions and the rewrite shape is a same-expression form or a
  simple `let` binding whose every use rewrites directly; otherwise
  `DiagnosticOnly` (§12.5).
- `owned_param_causes_clone`: `MaybeIncorrect` when an exact borrow mapping
  is present, else `DiagnosticOnly`; never `MachineApplicable`, because a
  signature change also requires caller edits (§13.4).
- `local_shared_ownership`: always `DiagnosticOnly` (§14.5). This is enforced
  by construction: the finding type has no fix-confidence field and its
  accessor returns the constant.

### Invariants, lemmas and axioms

Invariants introduced or preserved:

- I1 (verdict depends only on support): for profiles `p` and `q` with the same
  support, `verdict(p) == verdict(q)`. Consequence: saturating counters cannot
  change a verdict.
- I2 (order independence): the profile, escape set and constraint set of a
  stream are invariant under permutation of the stream.
- I3 (identity elements): inserting `EscapeKind::No` or `ApiConstraint::None`
  leaves a set unchanged.
- I4 (emission soundness and completeness): for each lint, `evaluate` returns
  `Emit` iff that lint's emission predicate above holds.
- I5 (fix-confidence ceiling): `MachineApplicable` is only ever produced by
  `clone_only_used_by_borrow` with machine suggestions enabled and a rewritable
  shape; `local_shared_ownership` findings are always `DiagnosticOnly`.
- I6 (pressure floor): every `owned_param_causes_clone` finding reports a
  call-site count of at least one and at least the policy minimum.
- I7 (suppression precedence): when several suppression conditions hold, the
  reported reason is the first in the documented check order for that lint.
  This is a determinism property for tests and tracing; it carries no
  correctness weight and is verified by tests only.
- I8 (argument key-set stability): for each lint, the set of argument names
  produced by its builder is exactly the lint's documented constant set, for
  every finding and subject; every value is non-empty except
  `exact_borrow_type` when `borrow_hint` is `generic`.
- I9 (argument determinism): equal findings and subjects produce equal
  argument maps, iterated in a fixed order.

Lemmas (hypotheses about the eventual proof structure; implementation may
merge, split or discard them):

- L1 (profile counts): for any stream `s` and class `c`, the model profile
  built by folding `s` has `count(c)` equal to the number of indices `i` with
  `s[i] == c`.
- L2 (read-only characterization): `verdict(profile(s)) == ReadOnly` iff
  `|s| > 0` and every `s[i] ∈ R`.
- L3 (escape-set characterization): `e ∈ escapes(t)` iff `e ≠ No` and some
  `t[j] == e`; consequently `escapes(t)` is empty iff every `t[j] == No`.
- L4 (permutation invariance): if `s'` is a permutation of `s`, the profiles
  agree; likewise for escape and constraint sets. Expected to follow from L1
  and L3 without a separate induction.
- L5 (clone-lint headline theorem): for any use stream `s`, escape stream `t`
  and finite facts, if `evaluate_clone(summary(s, t, facts))` emits then
  `|s| > 0`, every `s[i] ∈ R`, every `t[j] == No`, the origin is user code, the
  receiver is a simple place, and the original is unconflicted; and conversely.
- L6 (owned-parameter headline theorem): emission implies the parameter use
  stream is non-empty and read-only, the retained call-site count is at least
  `max(1, policy minimum)`, and no unwaived exemption is present; and
  conversely.
- L7 (shared-ownership headline theorem): emission implies every escape and
  constraint observed is waived by the policy and the finding is
  diagnostic-only; and conversely.
- L8 (inhabitation): for each lint there exist a stream and facts for which it
  emits and, for each suppression family, inputs for which it suppresses. This
  is the non-vacuity guard for L5 to L7.

Axioms (trusted, not verified here):

- A1: Verus, Kani (and CBMC), and the Rust compiler are sound for the
  fragments used.
- A2: The lint crates (roadmap 9.2 to 9.4, using 9.1.2 and 9.1.4) map MIR
  facts onto `UseClass`, `EscapeKind`, `ApiConstraint`, and the finite fact
  enums faithfully. This is the largest residual gap: the proofs show the
  decisions are right given faithful summaries, not that summaries are
  faithful. Those items carry their own UI tests.
- A3: `fluent_bundle` reports a missing variable as a formatting error and
  formats a present string or number variable as its value; `common::i18n`
  surfaces such errors as `I18nError`. Exercised, not proved, by the Fluent
  contract test.
- A4: Use counts never approach `usize::MAX` in practice. The production
  profile saturates rather than overflows; I1 makes saturation harmless to
  every verdict, so A4 only affects the reported counts, never a decision.

### Obligations, methods and evidence

- Obligation K1 (I1, production verdict): the production
  `UseProfile::verdict` agrees with an independently written support-based
  specification for every profile. Method: Kani proof harness over a symbolic
  profile (nine symbolic `usize` counts; no loop depends on the counts, so the
  check is exhaustive, not bounded). Rationale: the function is small and
  finite-branching; Kani checks the real code. Artefact:
  `common/src/ownership_shape/kani.rs`, harness
  `verify_use_profile_verdict_matches_support_spec`. Evidence:
  `make kani-ownership-shape` prints `VERIFICATION:- SUCCESSFUL` for the
  harness. Non-vacuity: `kani::cover!` for each of the five verdicts must be
  reported `SATISFIED`; a seeded mutation (classifying `CopyRead` as
  non-read-only in a scratch branch) must make the harness fail.
- Obligation K2 (I4, I5 for `clone_only_used_by_borrow`): production
  `evaluate_clone_only_used_by_borrow` returns `Emit` iff the emission
  predicate holds, with the fix confidence specified above. Method: Kani over a
  symbolic `CloneUseSummary` and policy (all fields are fieldless enums,
  booleans, and the profile from K1; the escape set is a symbolic bitset).
  Artefact: harness `verify_clone_only_used_by_borrow_matches_spec`. Evidence:
  as K1. Non-vacuity: `kani::cover!` on `Emit` with each fix confidence and on
  every `SuppressionReason` the lint can return; seeded mutation: drop the
  macro-origin check, harness must fail.
- Obligation K3 (I4, I5, I6 for `owned_param_causes_clone`). Method: Kani
  over symbolic pressure counts, profile, exemption bitset, policy, and an
  `Option<ExactBorrowMapping>` chosen between `None` and one fixed value.
  Artefact: harness `verify_owned_param_causes_clone_matches_spec`.
  Non-vacuity: covers on `Emit` with and without a mapping and on each
  suppression reason; seeded mutation: compare with `>` instead of `>=` against
  the minimum, harness must fail.
- Obligation K4 (I4, I5 for `local_shared_ownership`). Method: Kani over a
  symbolic summary and policy. Artefact: harness
  `verify_local_shared_ownership_matches_spec`. Non-vacuity: covers on `Emit`
  for each wrapper class and on each suppression reason, including emission
  with a waived `StaticCallback` escape; seeded mutation: treat
  `ThreadSafeSharedRequired` as waivable, harness must fail.
- Obligations V1 to V5 (L1 to L8, I2, I3): proved in Verus over the model.
  Method: Verus deductive proof. Rationale: these properties quantify over
  streams of every length and order, which neither Kani (bounded unwinding) nor
  sampling can establish; the kernel is small and stable. Artefact:
  `verus/ownership_shape_evaluation.rs`, a mirrored model naming the production
  functions it mirrors, with `spec fn` predicates defined from the inputs alone
  (not from the decision functions, which would make completeness true by
  construction), `proof fn` lemmas for L1 to L7, and `lemma_*_inhabited` lemmas
  for L8. Evidence: `make verus-ownership-shape` reports
  `verification results:: N verified, 0 errors` with `N` equal to the number of
  `proof fn` and `fn` items in the file. Non-vacuity: the inhabitation lemmas
  construct explicit witnesses (for example the one-element stream
  `[SharedBorrow]` with no escapes for L5); seeded fault: deleting the
  non-empty conjunct from the model's `ReadOnly` verdict must make L2 fail to
  verify; inspection:
  `rg -n
  'assume|admit|external_body|assume_specification'
  verus/ownership_shape_evaluation.rs`
  must print nothing.
- Obligation P1 (I2, I3 on production accumulation): `UseProfile`,
  `EscapeSet`, and `ApiConstraintSet` built by `FromIterator` from a generated
  stream equal those built from any shuffle of it, and agree with a
  `BTreeMap`/`BTreeSet` oracle. Method: `proptest` (default 256 cases; streams
  of length 0 to 64). Artefact:
  `common/src/ownership_shape/tests/accumulation_props.rs`. Non-vacuity: the
  generator emits each class with positive probability and the empty stream; a
  dedicated `#[test]` asserts the strategy produced at least one empty stream
  and one stream containing every class over a fixed seed, so a degenerate
  generator is caught; seeded mutation: making `EscapeSet::insert` record `No`
  must fail the identity property.
- Obligation T1 (I4 examples, I7, design rule tables, DS-17 defaults):
  `rstest` parameterized tables per lint, one row per rule in design §12.6,
  §13.3, §13.7, §14.6, and §14.7, each asserting the exact `SuppressionReason`,
  plus happy-path rows asserting the exact finding. Policy-default tests assert
  design §17 values. Artefacts:
  `common/src/ownership_shape/tests/clone_evaluation.rs`,
  `owned_param_evaluation.rs`, `shared_ownership_evaluation.rs`,
  `policy_defaults.rs`. These remain even though Kani covers I4, because they
  are the readable specification and pin I7, which Kani does not.
- Obligation T2 (wrapper classification, applicability mapping): exhaustive
  `rstest` over all outer and inner wrapper combinations (six outer kinds times
  seven inner options) against a hand-written table, and over all three
  `FixConfidence` values for `applicability()`. Finite and small, so a table is
  exhaustive and no prover is warranted.
- Obligation T3 (I8, I9): `proptest` over generated findings and subjects
  asserting key-set equality with the per-lint constant and repeated-build
  equality; `insta` snapshots of the rendered argument maps for each lint and
  each selector token (deterministic `BTreeMap` order makes snapshots stable).
  Artefacts: `common/src/ownership_shape/tests/diagnostics.rs`,
  `diagnostics_props.rs`, snapshots under
  `common/src/ownership_shape/tests/snapshots/`. Non-vacuity: a seeded fault
  that omits `exact_borrow_type` for generic hints must fail the key-set
  property.
- Obligation E1 (A3 boundary, DS-16): an integration test builds a real
  `FluentBundle` from an inline test-only resource that references every
  argument and selector token for each lint, implements `BundleLookup` over it,
  converts builder output with `to_fluent_arguments`, and calls
  `common::i18n::resolve_message_set`. It asserts `Ok` with no Fluent errors
  and that each placeable's value appears in the output. Artefact:
  `common/tests/ownership_shape_fluent_contract.rs`. Non-vacuity: a negative
  control removes one argument and asserts `resolve_message_set` returns an
  error, proving the test would notice a missing argument.
- Obligation B1 (DS-18.2): `rstest-bdd` scenarios in
  `common/tests/features/ownership_shape_evaluation.feature` exercise the
  public API end to end from a contributor's perspective (see
  `Validation and acceptance`).

Where Verus is not warranted: diagnostic argument shaping (I8, I9),
wrapper-shape classification and applicability mapping (finite tables), policy
defaults, and suppression precedence (I7). These have finite or purely
structural behaviour that tests discharge completely or adequately.

## Plan of work

### Stage A: confirm and baseline (EP-M0, no production code)

Confirm, by reading dependency sources in `~/.cargo/registry` or with a scratch
test outside the tree, that `FluentValue` implements `From<usize>`, that
`kani::Arbitrary` is available for `NonZeroUsize` (or that the fallback in
`Risks` is needed), and that `MessageKey::new` is `const` (it is,
`common/src/i18n/diagnostics.rs` line 28). Run the baseline gates
(`make check-fmt`, `make lint`, `make test`) and record their results so later
failures can be attributed. Fetch all remote branches and record the next free
ADR number. Go/no-go: proceed when every confirmation is recorded in
`Surprises & discoveries` or `Decision log`.

### Stage B to D per milestone

Each of EP-M1 to EP-M3 follows Red-Green-Refactor. Red: add the tests (and the
module skeleton with `todo!`-free stubs that return a deliberately wrong value,
because `todo!` would panic rather than fail an assertion) and run the focused
command, observing assertion failures for the intended reason. Green: implement
until the focused command passes. Refactor: tidy, then run `make fmt`,
`make check-fmt`, `make lint`, and `make test`. EP-M4 and EP-M5 follow the
verification analogue: write the harness or lemma statements first and observe
the expected failure (a failing harness against the stub, or an unproved goal),
then complete.

### Module tree delivered

```plaintext
common/src/ownership_shape/
├── mod.rs                       # //! overview, pub mod model/evaluation/diagnostics, cfg(kani) mod kani
├── model/
│   ├── mod.rs                   # re-exports the shared vocabulary
│   ├── use_profile.rs           # UseClass, UseProfile, UseVerdict, ReadOnlyUseShape
│   ├── escape.rs                # EscapeKind, EscapeSet, ApiConstraint, ApiConstraintSet
│   ├── fix.rs                   # FixConfidence -> crate::diagnostics::Applicability
│   └── wrapper.rs               # WrapperKind, WrapperShape, WrapperClass
├── evaluation/
│   ├── mod.rs                   # Evaluation<F>, SuppressionReason, SpanOrigin
│   ├── clone_only_used_by_borrow.rs
│   ├── owned_param_causes_clone.rs   # includes ExactBorrowMapping value type
│   └── local_shared_ownership.rs
├── diagnostics/
│   ├── mod.rs                   # DiagnosticArgName, ArgValue, OwnershipDiagnosticArgs, MessageKey consts
│   └── builders.rs              # per-lint builders and subject structs
├── kani.rs                      # #[cfg(kani)] harnesses
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
    ├── diagnostics.rs
    ├── diagnostics_props.rs
    └── snapshots/
```

Behaviour and integration tests:

```plaintext
common/tests/features/ownership_shape_evaluation.feature
common/tests/ownership_shape_evaluation_behaviour.rs
common/tests/ownership_shape_fluent_contract.rs
```

Proof sidecar: `verus/ownership_shape_evaluation.rs`.

Wiring: `common/src/lib.rs` (one `pub mod ownership_shape;` line and a
crate-doc mention), `common/Cargo.toml` (dev-dependencies `googletest`,
`pretty_assertions`, `insta`), `Cargo.lock`, `scripts/run-kani.sh`,
`scripts/run-verus.sh`, `Makefile`, possibly
`tests/workflow_contracts/kani_*_contract_test.py`, and
`.github/workflows/proofs.yml` (prose harness count only, if present).

## Milestones and plateaus

### EP-M0: confirmations and baseline

- Outcome: Stage A facts recorded; baseline gate logs exist under `/tmp`.
- Requirements and gaps: none discharged; de-risks K1 to K4 and E1.
- Acceptance evidence: `Surprises & discoveries` entries for each
  confirmation; three baseline logs.
- Conformance check: no code changed; design unchanged.
- Recovery: repeatable; nothing to revert.
- Remaining gaps: everything else.
- Compatibility decision: none.

### EP-M1: shared vocabulary

- Outcome: `common::ownership_shape::model` compiles, is documented with
  doctests, and passes unit and property tests. `common/src/lib.rs` declares
  `pub mod ownership_shape;`.
- Requirements and gaps: DS-11.3 (enums), DS-18.1 (escape-kind reduction),
  I1 to I3 by test.
- Acceptance evidence: `cargo test -p whitaker-common ownership_shape::tests`
  passes, including `tests::use_profile`, `tests::escape_sets`,
  `tests::wrapper_shape`, `tests::fix_confidence`, and
  `tests::accumulation_props`; each was observed failing against stubs first.
- Conformance check:
  - DS-11.3 enum variants match the design names exactly.
  - D3 and D6 followed.
  - No rustc type introduced.
  - Only the authorized dev-dependencies added.
  - No persisted or wire format.
  - Trace links current.
- Recovery: `git revert` the milestone commit; nothing depends on it yet.
- Remaining gaps: evaluation, diagnostics, proofs.
- Compatibility decision: none (new, pre-release internal API).

### EP-M2: per-lint summaries, policies and evaluation

- Outcome: the three `evaluate_*` functions, their summary, policy and
  finding types, `Evaluation<F>`, and `SuppressionReason` exist and pass the
  rule tables and BDD scenarios.
- Requirements and gaps: DS-12, DS-13, DS-14, DS-15.3, DS-17, DS-18.2; D1,
  D2, D7, D8.
- Acceptance evidence: the rule-table tests and
  `cargo test -p whitaker-common --test ownership_shape_evaluation_behaviour`
  pass; the feature's nine scenarios were observed failing first.
- Conformance check:
  - Every bullet of design §12.6, §13.3, §13.7, §14.6, §14.7 has a table row.
  - Policy defaults equal design §17.
  - No rustc type; no string in evaluation inputs other than the
    `ExactBorrowMapping` payload.
  - Trace links current.
- Recovery: revert the milestone commit; EP-M1 remains a coherent plateau.
- Remaining gaps: diagnostics, proofs, documentation.
- Compatibility decision: none.

### EP-M3: diagnostic argument builders

- Outcome: builders produce deterministic, fixed-key argument maps convertible
  to `common::i18n::Arguments`; message-key constants exist.
- Requirements and gaps: DS-16; D4, D5; I8, I9; E1.
- Acceptance evidence: `tests::diagnostics`, `tests::diagnostics_props`, the
  `insta` snapshots, and
  `cargo test -p whitaker-common --test ownership_shape_fluent_contract` pass;
  the negative control in E1 passes by observing an error.
- Conformance check:
  - Argument names include every design §16 name that applies to each lint.
  - No `.ftl` file added under `common/locales/`.
  - Trace links current.
- Recovery: revert the milestone commit; EP-M2 remains coherent.
- Remaining gaps: proofs and documentation.
- Compatibility decision: none.

### EP-M4: Kani harnesses and runner wiring

- Outcome: four harnesses K1 to K4 verify; `make kani-ownership-shape`
  exists; the default `make kani` runs them.
- Requirements and gaps: I1, I4, I5, I6 on production code.
- Acceptance evidence: `make kani-ownership-shape` shows
  `VERIFICATION:- SUCCESSFUL` for each harness and every `cover!` property
  `SATISFIED`; the seeded mutations from the `Verification plan` were each
  observed failing and then reverted (record the transcript excerpts in
  `Artefacts and notes`); `make test-workflow-contracts` passes.
- Conformance check: harnesses check production functions, not copies; any
  `kani::assume` restates only a type invariant; runner lists updated.
- Recovery: revert the milestone commit.
- Remaining gaps: Verus and documentation.
- Compatibility decision: none.

### EP-M5: Verus model and proofs

- Outcome: `verus/ownership_shape_evaluation.rs` proves L1 to L8;
  `make verus-ownership-shape` exists; the default `make verus` includes it.
- Requirements and gaps: I2, I3, L1 to L8.
- Acceptance evidence: `make verus-ownership-shape` reports all items
  verified and 0 errors; the trusted-construct search prints nothing; the
  seeded fault was observed failing and reverted.
- Conformance check: model mirrors the production check order and names the
  mirrored functions; predicates defined from inputs only; no trusted
  constructs.
- Recovery: revert the milestone commit.
- Remaining gaps: documentation.
- Compatibility decision: none.

### EP-M6: documentation, ADR, and roadmap

- Outcome: design §11.6 "Implementation decisions (9.1.1)" records D1 to D8;
  design §16 notes the message-key mapping (D4) and the selector tokens (D5); a
  new ADR records the evaluation-outcome and argument contract (D1, D2, D5) and
  the proof split; `docs/developers-guide.md` gains "Shared ownership-shape
  helpers" with the re-use policy; `docs/repository-layout.md` mentions
  ownership-shape models in the `common/` paragraph; `docs/contents.md` lists
  the ADR; the Verus and Kani sections of the developers' guide list the new
  groups and targets; `docs/roadmap.md` marks 9.1.1 `[x]`.
- Requirements and gaps: AGENTS.md documentation duties; the abstraction
  policy (sweep, scope, record).
- Acceptance evidence: `make markdownlint`, `make nixie`, `make spelling`,
  `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check: every accepted deviation (D1 to D8) appears in the
  design; no upstream change unrecorded.
- Recovery: revert the documentation commit.
- Remaining gaps: none for 9.1.1; 9.1.2 onward.
- Compatibility decision: none.

`docs/users-guide.md` is not changed: it documents installation, configuration
and shipped lints only, and this item ships no lint and no configuration.
Roadmap item 9.5.3 updates it when the lints land.

## Concrete steps

All commands run from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/e706400c-e65a-4cdc-93f9-68dfd2930576`.
Log every gate with `tee`, using the template
`/tmp/$ACTION-$(get-project)-$(git branch --show-current).out`, and run gates
sequentially (never in parallel for the same worktree).

Baseline and per-milestone gates:

```sh
set -o pipefail
make fmt 2>&1 | tee "/tmp/fmt-$(get-project)-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-$(get-project)-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-$(get-project)-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-$(get-project)-$(git branch --show-current).out"
```

Expected tail of a passing `make test` includes a nextest summary line such as
`Summary [...] NNNN tests run: NNNN passed, M skipped` followed by doctest
results `test result: ok.` for each package.

Focused loops during Red-Green-Refactor:

```sh
set -o pipefail
cargo test -p whitaker-common ownership_shape 2>&1 | tee "/tmp/unit-$(get-project)-$(git branch --show-current).out"
cargo test -p whitaker-common --test ownership_shape_evaluation_behaviour 2>&1 \
  | tee "/tmp/bdd-$(get-project)-$(git branch --show-current).out"
cargo test -p whitaker-common --test ownership_shape_fluent_contract 2>&1 \
  | tee "/tmp/fluent-$(get-project)-$(git branch --show-current).out"
cargo insta test -p whitaker-common --review   # only when snapshots change; accept deliberately
```

Proof runs (EP-M4 and EP-M5):

```sh
set -o pipefail
make kani-ownership-shape 2>&1 | tee "/tmp/kani-$(get-project)-$(git branch --show-current).out"
make verus-ownership-shape 2>&1 | tee "/tmp/verus-$(get-project)-$(git branch --show-current).out"
make test-workflow-contracts 2>&1 | tee "/tmp/contracts-$(get-project)-$(git branch --show-current).out"
rg -n 'assume|admit|external_body|assume_specification' verus/ownership_shape_evaluation.rs
```

Expected Kani excerpt per harness:

```plaintext
SUMMARY:
 ** 0 of N failed
VERIFICATION:- SUCCESSFUL
```

Expected Verus excerpt:

```plaintext
verification results:: N verified, 0 errors
```

Documentation gates (EP-M6, in addition to the three gates above):

```sh
set -o pipefail
make markdownlint 2>&1 | tee "/tmp/markdownlint-$(get-project)-$(git branch --show-current).out"
make nixie 2>&1 | tee "/tmp/nixie-$(get-project)-$(git branch --show-current).out"
make spelling 2>&1 | tee "/tmp/spelling-$(get-project)-$(git branch --show-current).out"
```

Commit after each milestone once its gates pass, using the `commit-message`
conventions in `AGENTS.md` (imperative subject of at most about 50 characters,
wrapped body explaining what and why).

## Validation and acceptance

Behaviour-driven development (BDD) feature specification, to be written in
`common/tests/features/ownership_shape_evaluation.feature` during EP-M2 and
kept synchronized with the implementation:

```gherkin
Feature: Ownership-shape evaluation
  The shared evaluation helpers decide whether an ownership-shape lint
  reports a finding, using only compiler-independent summaries.

  Scenario: A clone only used by shared borrows is reported
    Given a clone of a local receiver
    And the clone is observed in 2 shared borrows
    And the clone is used in a same-expression rewrite shape
    When clone-only-used-by-borrow is evaluated with the default policy
    Then a finding is emitted with machine-applicable fix confidence

  Scenario: A moved clone is suppressed
    Given a clone of a local receiver
    And the clone is observed in 1 move
    When clone-only-used-by-borrow is evaluated with the default policy
    Then the evaluation is suppressed because the uses are consumed

  Scenario: A clone produced by a macro expansion is suppressed
    Given a clone of a local receiver inside a macro expansion
    And the clone is observed in 1 shared borrow
    When clone-only-used-by-borrow is evaluated with the default policy
    Then the evaluation is suppressed because of a macro expansion

  Scenario: Disabling machine suggestions keeps the finding but not the fix
    Given a clone of a local receiver
    And the clone is observed in 1 shared borrow
    And the clone is used in a same-expression rewrite shape
    When clone-only-used-by-borrow is evaluated without machine suggestions
    Then a finding is emitted with diagnostic-only fix confidence

  Scenario: A read-only owned parameter under clone pressure is reported
    Given a private callee parameter observed in 2 shared borrows
    And 2 retained clone-pressure call sites
    And an exact borrow mapping from String to &str
    When owned-param-causes-clone is evaluated with the default policy
    Then a finding is emitted with maybe-incorrect fix confidence
    And the finding reports 2 call sites

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
    Given a local Rc<Cell<_>> wrapper
    And the wrapper escapes into a static callback
    When local-shared-ownership is evaluated with the default policy
    Then the evaluation is suppressed because the wrapper escapes

  Scenario: A non-escaping shared mutable wrapper is reported diagnostic-only
    Given a local Rc<RefCell<_>> wrapper
    And the wrapper is only used by local sequential operations
    When local-shared-ownership is evaluated with the default policy
    Then a finding is emitted for a shared mutable wrapper
    And the finding is diagnostic-only
```

Red-Green-Refactor evidence to record per milestone in `Artefacts and notes`:
the focused command failing against stubs with assertion failures (for example
`assertion failed` or a `googletest` `Value of: ... Expected: ...` mismatch
naming the expected `SuppressionReason`); the same command passing after
implementation; then the full gates passing.

Quality criteria (what "done" means):

- Tests: `make test` passes, including the new unit, property, snapshot,
  behaviour, Fluent contract, and doctest suites.
- Verification: K1 to K4 verified with all covers satisfied;
  `verus/ownership_shape_evaluation.rs` verified with 0 errors and no trusted
  constructs; seeded mutations observed failing.
- Lint and formatting: `make check-fmt` and `make lint` pass (Clippy with
  `-D warnings` and Whitaker's own suite over `whitaker-common`).
- Documentation: `make markdownlint`, `make nixie`, and `make spelling` pass.
- Performance and security: not applicable; pure functions with no I/O.

## Idempotence and recovery

All steps are re-runnable. Tests, Kani and Verus have no side effects beyond
`target/` and the Verus and Kani caches under `~/.cache/whitaker/`. Each
milestone is a separate commit, so `git revert <sha>` restores the previous
plateau. If a gate fails half-way, fix and re-run that gate only; read the
`/tmp/...out` log rather than re-running to inspect output. Never use bare
`git stash` (the stash is shared between worktrees); use a temporary work in
progress commit instead.

## Artefacts and notes

To be filled during implementation: Red and Green transcript excerpts per
milestone, Kani and Verus summaries, seeded-mutation failure excerpts, and the
ADR number allocated.

## Interfaces and dependencies

The signatures below are the target shape. Names may be refined during
implementation only if recorded in `Decision log`; variants of design enums
must keep the design's names. All types derive `Clone`, `Copy` where every
field is `Copy`, `Debug`, `PartialEq`, `Eq`, and (for set elements and map keys)
`PartialOrd`, `Ord`, `Hash`; Kani builds add
`#[cfg_attr(kani, derive(kani::Arbitrary))]`.

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

/// Saturating per-class counts; order-independent by construction.
pub struct UseProfile { /* private: one counter per class */ }
impl UseProfile {
    pub const fn new() -> Self;
    #[must_use]
    pub const fn with_use(self, class: UseClass) -> Self;
    pub const fn count(&self, class: UseClass) -> usize;
    pub const fn total(&self) -> usize;
    pub const fn verdict(&self) -> UseVerdict;
    pub const fn read_only_shape(&self) -> Option<ReadOnlyUseShape>;
}
impl FromIterator<UseClass> for UseProfile { /* ... */ }

pub enum UseVerdict { NoUses, ReadOnly, MutablyBorrowed, Consumed, Opaque }
pub enum ReadOnlyUseShape { SharedBorrow, SharedMethod, CopyRead, Mixed }
```

In `common/src/ownership_shape/model/escape.rs`:

```rust
pub enum EscapeKind {
    No, Return, Store, ClosureCapture, StaticCallback,
    ThreadBoundary, ExternalApiBoundary, Unknown,
}
pub struct EscapeSet { /* private bitset over the seven non-`No` kinds */ }
impl EscapeSet {
    pub const fn new() -> Self;
    #[must_use]
    pub const fn with(self, kind: EscapeKind) -> Self; // `No` is the identity
    pub const fn contains(&self, kind: EscapeKind) -> bool;
    pub const fn is_empty(&self) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = EscapeKind> + '_;
}
impl FromIterator<EscapeKind> for EscapeSet { /* ... */ }

pub enum ApiConstraint {
    None, SharedHandleRequired, TraitSignatureRequired,
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
pub struct WrapperShape { outer: WrapperKind, inner: Option<WrapperKind> }
impl WrapperShape {
    pub const fn new(outer: WrapperKind, inner: Option<WrapperKind>) -> Self;
    /// `None` for shapes outside design §14.2 (for example `RefCell<Rc<_>>`).
    pub const fn class(self) -> Option<WrapperClass>;
    /// Locale-neutral code rendering such as `Rc<RefCell<_>>`.
    pub fn render(self) -> String;
}
pub enum WrapperClass { InteriorMutability, SharedHandle, SharedMutable }
```

In `common/src/ownership_shape/evaluation/mod.rs`:

```rust
#[must_use]
pub enum Evaluation<F> { Emit(F), Suppress(SuppressionReason) }

pub enum SpanOrigin { UserCode, MacroExpansion }

pub enum SuppressionReason {
    MacroExpansion,
    ReceiverNotSimplePlace,
    ToOwnedExcluded,
    NoObservedUse,
    UsesNotReadOnly(UseVerdict),
    Escapes(EscapeKind),
    OriginalPlaceConflict,
    InsufficientCallsitePressure,
    CalleeExempt(CalleeExemption),
    UnsupportedWrapperShape,
    ApiConstraintPresent(ApiConstraint),
    NonSequentialOperations,
    WeakHandles,
    UnclassifiedAliases,
    FrameworkHandle,
}
```

In `common/src/ownership_shape/evaluation/clone_only_used_by_borrow.rs`:

```rust
pub enum CloneForm { Clone, ToOwned }
pub enum ReceiverPlace { Local, Field, Other }
pub enum OriginalPlaceAccess { Unconflicted, MovedBeforeLastUse, MutablyBorrowedBeforeLastUse }
pub enum RewriteShape { SameExpression, SimpleLetBinding, Other }

pub struct CloneUseSummary {
    pub form: CloneForm,
    pub receiver: ReceiverPlace,
    pub uses: UseProfile,
    pub escapes: EscapeSet,
    pub original: OriginalPlaceAccess,
    pub origin: SpanOrigin,
    pub rewrite: RewriteShape,
}
pub struct CloneOnlyUsedByBorrowPolicy { pub include_to_owned: bool, pub emit_machine_suggestions: bool }
impl Default for CloneOnlyUsedByBorrowPolicy { /* both true, design §17 */ }

pub struct CloneOnlyUsedByBorrowFinding { /* private */ }
impl CloneOnlyUsedByBorrowFinding {
    pub const fn fix_confidence(&self) -> FixConfidence;
    pub const fn use_shape(&self) -> ReadOnlyUseShape;
}

pub const fn evaluate_clone_only_used_by_borrow(
    summary: &CloneUseSummary,
    policy: &CloneOnlyUsedByBorrowPolicy,
) -> Evaluation<CloneOnlyUsedByBorrowFinding>;
```

Check order (I7): macro origin, receiver place, `to_owned` policy, use verdict
(`NoObservedUse` then `UsesNotReadOnly`), escapes (reporting the lowest escape
kind present in declaration order), original-place conflict.

In `common/src/ownership_shape/evaluation/owned_param_causes_clone.rs`:

```rust
pub enum CallsiteEvidence { Retained, MacroExpanded, TemporarySource }
pub struct OwnedParamPressure { /* private saturating counts per evidence kind */ }
impl OwnedParamPressure {
    pub const fn new() -> Self;
    #[must_use]
    pub const fn with_callsite(self, evidence: CallsiteEvidence) -> Self;
    pub const fn retained(&self) -> usize;
}
impl FromIterator<CallsiteEvidence> for OwnedParamPressure { /* ... */ }

pub enum CalleeExemption {
    Exported, TraitDeclaration, TraitImpl, Extern, Async, RequiresLifetimeIntroduction,
}
pub struct CalleeExemptions { /* private bitset */ }

/// Owned-to-borrowed display pair, produced by roadmap 9.1.3's table.
pub struct ExactBorrowMapping { owned_type: String, borrowed_type: String }
impl ExactBorrowMapping {
    pub fn new(owned_type: impl Into<String>, borrowed_type: impl Into<String>) -> Self;
    pub fn owned_type(&self) -> &str;
    pub fn borrowed_type(&self) -> &str;
}

pub struct OwnedParamSummary {
    pub pressure: OwnedParamPressure,
    pub usage: UseProfile,
    pub exemptions: CalleeExemptions,
    pub exact_borrow: Option<ExactBorrowMapping>,
}
pub struct OwnedParamCausesClonePolicy {
    pub min_local_callsites: core::num::NonZeroUsize, // default 1
    pub avoid_breaking_exported_api: bool,          // default true
    pub ignore_async_fns: bool,                     // default true
}

pub struct OwnedParamCausesCloneFinding { /* private */ }
impl OwnedParamCausesCloneFinding {
    pub const fn fix_confidence(&self) -> FixConfidence;
    pub const fn callsite_count(&self) -> core::num::NonZeroUsize;
    pub const fn use_shape(&self) -> ReadOnlyUseShape;
    pub fn exact_borrow(&self) -> Option<&ExactBorrowMapping>;
}

pub fn evaluate_owned_param_causes_clone(
    summary: &OwnedParamSummary,
    policy: &OwnedParamCausesClonePolicy,
) -> Evaluation<OwnedParamCausesCloneFinding>;
```

Check order (I7): call-site pressure (macro-only and temporary-only evidence
both yield `InsufficientCallsitePressure`), use verdict, exemptions in
declaration order.

In `common/src/ownership_shape/evaluation/local_shared_ownership.rs`:

```rust
pub enum OperationPattern { LocalSequential, Other }
pub enum HandleEvidence { SingleOwner, ClassifiedAliases, WeakHandles, UnclassifiedAliases }
pub enum InnerHandle { Plain, FrameworkHandle }

pub struct SharedOwnershipSummary {
    pub wrapper: WrapperShape,
    pub escapes: EscapeSet,
    pub constraints: ApiConstraintSet,
    pub operations: OperationPattern,
    pub handles: HandleEvidence,
    pub inner: InnerHandle,
    pub origin: SpanOrigin,
}
pub struct LocalSharedOwnershipPolicy {
    pub ignore_static_callback_captures: bool,    // default true
    pub ignore_trait_signature_constraints: bool, // default true
    pub ignore_external_api_boundaries: bool,     // default true
}

pub struct LocalSharedOwnershipFinding { /* private */ }
impl LocalSharedOwnershipFinding {
    pub const fn wrapper(&self) -> WrapperShape;
    pub const fn wrapper_class(&self) -> WrapperClass;
    /// Always `FixConfidence::DiagnosticOnly` (design §14.5).
    pub const fn fix_confidence(&self) -> FixConfidence;
}

pub const fn evaluate_local_shared_ownership(
    summary: &SharedOwnershipSummary,
    policy: &LocalSharedOwnershipPolicy,
) -> Evaluation<LocalSharedOwnershipFinding>;
```

Check order (I7): macro origin, wrapper shape, escapes, constraints,
operations, handles, inner handle.

In `common/src/ownership_shape/diagnostics/mod.rs` and `builders.rs`:

```rust
pub const CLONE_ONLY_USED_BY_BORROW: MessageKey<'static>;
pub const OWNED_PARAM_CAUSES_CLONE: MessageKey<'static>;
pub const LOCAL_SHARED_OWNERSHIP: MessageKey<'static>;

pub enum DiagnosticArgName {
    Variable, Callee, Parameter, Wrapper, WrapperClass,
    CallsiteCount, UseSummary, ExactBorrowType, BorrowHint,
}
impl DiagnosticArgName { pub const fn as_str(self) -> &'static str; }

pub const CLONE_ONLY_USED_BY_BORROW_ARGS: &[DiagnosticArgName];  // Variable, UseSummary
// Callee, Parameter, CallsiteCount, UseSummary, ExactBorrowType, BorrowHint
pub const OWNED_PARAM_CAUSES_CLONE_ARGS: &[DiagnosticArgName];
pub const LOCAL_SHARED_OWNERSHIP_ARGS: &[DiagnosticArgName];     // Variable, Wrapper, WrapperClass

pub enum ArgValue { Text(String), Count(usize) }
pub struct OwnershipDiagnosticArgs { /* BTreeMap<DiagnosticArgName, ArgValue> */ }
impl OwnershipDiagnosticArgs {
    pub fn get(&self, name: DiagnosticArgName) -> Option<&ArgValue>;
    pub fn iter(&self) -> impl Iterator<Item = (DiagnosticArgName, &ArgValue)> + '_;
    pub fn to_fluent_arguments(&self) -> crate::i18n::Arguments<'_>;
}

pub struct OwnedParamSubject<'a> { pub callee: &'a str, pub parameter: &'a str }
pub struct SharedOwnershipSubject<'a> { pub variable: &'a str }

pub fn clone_only_used_by_borrow_args(
    finding: &CloneOnlyUsedByBorrowFinding,
    variable: &str,
) -> OwnershipDiagnosticArgs;
pub fn owned_param_causes_clone_args(
    finding: &OwnedParamCausesCloneFinding,
    subject: &OwnedParamSubject<'_>,
) -> OwnershipDiagnosticArgs;
pub fn local_shared_ownership_args(
    finding: &LocalSharedOwnershipFinding,
    subject: &SharedOwnershipSubject<'_>,
) -> OwnershipDiagnosticArgs;
```

Re-use policy (recorded in the developers' guide in EP-M6): the
`ownership_shape` module is owned by Phase 9. Permitted call sites are the
three ownership-shape lint crates and their tests; other lints may reuse
`model` vocabulary (`UseClass`, `EscapeKind`, and their accumulators) but must
not call the per-lint `evaluate_*` functions. Composition rule: lint crates
build summaries from compiler facts, call `evaluate_*`, and on `Emit` call the
matching builder; no lint crate re-implements a decision rule. Any change to an
`evaluate_*` function must update its Kani specification and the Verus model in
the same commit.

Dependencies: no new crates. `common/Cargo.toml` gains
`googletest.workspace = true`, `pretty_assertions.workspace = true`, and
`insta.workspace = true` under `[dev-dependencies]`. Kani and Verus are
external tools already installed by `scripts/install-kani.sh` and
`scripts/install-verus.sh`.

## Revision note

2026-11-11: initial draft from reconnaissance; awaiting expert review and user
approval.
