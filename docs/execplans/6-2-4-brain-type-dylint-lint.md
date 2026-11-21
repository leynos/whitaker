# Create the `brain_type` Dylint lint crate (roadmap 6.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — awaiting approval. Do not begin implementation until the user
explicitly approves this plan, including the three decisions listed under
`Decisions requiring approval`.

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Whitaker already ships every compiler-free piece of the `brain_type` lint. The
`whitaker-common` crate measures a type (`TypeMetricsBuilder`), decides whether
it is a brain type (`evaluate_brain_type`), formats the measured values
(`format_primary_message`, `format_note`, `format_help`), and clusters its
methods into decomposition advice (`suggest_decomposition`,
`format_decomposition_note`). Nothing yet connects those pieces to the
compiler. No crate walks the high-level intermediate representation (HIR) of a
user's code, so no user can see a `brain_type` diagnostic.

Roadmap item 6.2.4 creates that connection: a Dylint lint crate at
`crates/brain_type/`. After this change, a repository that opts in with
`[whitaker] experimental = ["brain_type"]` in its `dylint.toml` sees a warning
on any struct, enum, or union whose methods, taken across every `impl` block in
the crate, are complex in aggregate, contain a "brain method", and fall apart
into unrelated groups. The diagnostic names the measured values, explains why
they matter, suggests a decomposition, and points at the type's declaration.
Running `cargo dylint` over the fixture
`crates/brain_type/ui/fail_brain_type_warns.rs` prints, in English:

```plaintext
warning: `Parser` has WMC=74, LCOM4=2, and a brain method `parse_all` (CC=31, LOC=96).
  --> fail_brain_type_warns.rs:LL:CC
   |
LL | pub struct Parser {
   |            ^^^^^^
   = note: <metric explanation from format_note>
   = note: Potential decomposition for `Parser`:
           - [grammar] helper struct for `parse_nodes`, `parse_tokens`
           - [std::fs] module for `load_from_disk`, `save_to_disk`
   = help: <help text from format_help>
```

The exact text is pinned by the fixture's `.stderr` file once milestone EP-M5
lands; the shape above is the contract.

Terms used throughout:

- **Brain type**: a nominal type (struct, enum, or union) that has accreted
  too much behaviour. See `docs/brain-trust-lints-design.md` §Background.
- **WMC** (weighted methods count): the sum of the cognitive complexity (CC)
  of the type's methods.
- **CC** (cognitive complexity): the Sonar metric defined in G. Ann Campbell's
  white paper[^sonar], fed into `CognitiveComplexityBuilder`.
- **Brain method**: a method with CC >= 25 and at least 80 lines of code
  (LOC).
- **LCOM4** (lack of cohesion in methods, variant 4): the number of connected
  components in the graph whose nodes are methods and whose edges are shared
  field accesses or direct calls.
- **Subject**: the unit the lint judges — one local nominal type plus all of
  its methods in the crate.
- **Phase one / phase two**: the two-phase capture mandated by ADR 0015.
  Phase one records cheap scalars during the compiler's callbacks; phase two
  re-walks only the subjects the gate admits, after the whole crate has been
  visited.
- **Gate**: the predicate run between the phases. It may exclude a subject
  only when no enabled rule could flag it.
- **Authored / generated**: the provenance classes from ADR 0013. A node is
  generated when a macro or compiler AST pass produced it; desugarings such as
  `for`, `while`, `?`, and `async fn` are authored.

## Decisions requiring approval

The plan cannot be approved without an answer to each of these. Each has a
recommendation; the plan is written assuming the recommendation.

1. **Provenance predicate (deviation from ADR 0015 capture rule 6).** ADR 0015
   says the driver passes `span.from_expansion()` to every builder. ADR 0013's
   reuse rules (accepted 2026-11-08) forbid exactly that ("Do not re-derive
   provenance in a lint with `from_expansion`"), and RFC 0010 §9 asks the brain
   trust design to pass `is_generated(span)` instead, because
   `from_expansion()` is true for desugarings and would silently drop every
   `for` loop and `?` from the score. **Recommendation:** pass
   `whitaker_lint_core::is_generated(span)` as the `is_from_expansion`
   argument. Rule 6's structure — classify once, in the driver; the domain
   never sees a `Span` — is kept. ADR 0015 receives a dated amendment recording
   the change of predicate.
2. **How a `Deny` disposition reaches the user.** `evaluate_brain_type` returns
   `Pass`, `Warn`, or `Deny`, but a Dylint lint has one level, which the user
   controls. Emitting a different level per finding is not possible through
   `TyCtxt::emit_node_span_lint`, and a second lint name would double the
   opt-in and configuration surface. **Recommendation:** one lint,
   `brain_type`, declared at `Warn`. A `Deny` finding adds one extra note,
   "This type also exceeds the escalation thresholds: …", naming each
   escalation condition that fired. The finding keeps its disposition as a
   value so roadmap item 6.5.1 can map it to SARIF `error` level. A user who
   wants escalation to fail the build sets the lint to `deny` in the normal way.
3. **What `?` costs in the Sonar mapping.** Sonar charges nothing for early
   exits; the maintainer ruled on 15 November 2026 that "`?` is a branch", and
   ADR 0016 applies that ruling to `bumpy_road_function`. **Recommendation:**
   charge `?` as the authored early-return `if` it abbreviates — one structural
   increment plus the current nesting increment — but do not raise the nesting
   level for the scrutinee. This keeps one estate-wide answer to the question.
   Phase 16 (harmonized complexity) may revisit it through a superseding ADR.

## Constraints

- Scope is roadmap item 6.2.4 only. Out of scope, each with its owner:
  location resolution to `RepoRelativePath` and `FindingLocation` and all SARIF
  output (6.5.1); Fluent catalogue entries (6.6.2); `whitaker.toml`
  configuration sections (6.6.1); the comprehensive positive and negative UI
  suite (6.6.3); the full users' guide chapter (6.7.1); the `brain_trait` crate
  (6.3.3).
- Do not change any existing public signature in `whitaker-common`; it is
  published (`.github/workflows/release.yml`). Additive public items are
  permitted only where this plan names them (EP-M1).
- Add no new production dependency to `whitaker-common` or `whitaker_sarif`
  (ADR 0015 technical requirements).
- `crates/brain_type` must not depend on `whitaker_sarif`,
  `fluent-templates`, or `unic-langid` directly. Localization goes through
  `whitaker_common::safe_resolve_message_set` with English fallbacks.
- Every compiler-facing item lives behind the crate's `dylint-driver` feature.
  The policy and capture layers compile and test without `rustc_private`.
- No capture callback (`check_item`, `check_impl_item`) evaluates, emits, or
  builds `MethodInfo`, `MethodProfile`, or any string set (ADR 0015 lifecycle
  table).
- Emission goes through `TyCtxt::emit_node_span_lint` with the subject's
  declaration `HirId`. `LateContext::emit_span_lint` is forbidden in this crate
  (ADR 0015 lifecycle rule 1).
- No data-dependent path may call `span_bug`, `span_delayed_bug`,
  `delayed_bug`, or `bug` (ADR 0015 location rule 8). Unresolvable input is
  skipped and, where a subject still exists, still diagnosed.
- Every accumulator is an ordered container. `DefId` is not `Ord` on the
  pinned toolchain; key by `LocalDefId::local_def_index` (a `u32`) or by
  rendered strings, never by hashing.
- No source file may exceed 400 lines (`AGENTS.md`). Split by feature before
  reaching 350.
- Follow ADR 0014: the lint ships experimental, is compiled into the suite, and
  runs only when a repository names it in `[whitaker] experimental`.
- No `assume`, `admit`, `external_body`, or trusted proof body in any Verus
  file. No `#[allow]`; use item-scoped `#[expect(..., reason = "...")]` only as
  a last resort.
- `make check-fmt`, `make lint`, and `make test` must pass at the end of every
  milestone. `make verus-brain-type` must pass at the end of EP-M3 and every
  later milestone.

## Tolerances (exception triggers)

ExecPlan files are excluded from every size count below, including this one.
Counts are measured against `main` at the merge base with
`git diff --numstat $(git merge-base origin/main HEAD)`, as net lines
(additions minus deletions) per category. Each category is enforced on its own;
spare capacity in one never offsets another.

- **Code** (production Rust, tests, UI fixtures and their `.stderr` files,
  Verus proofs, the `Makefile`, and shell scripts): budget 3,600 net lines.
  Stop and escalate above 4,000. The estimate is in `Context and orientation`.
- **Configuration** (`Cargo.toml` manifests, `Cargo.lock`, `.config/`,
  `dylint.toml` fixtures, CI workflows): budget 150 net lines, excluding
  `Cargo.lock` churn caused only by adding the new package. Stop above 220.
- **Documentation** (everything under `docs/` except execplans, plus
  `README.md`): budget 700 net lines. Stop above 900.
- **Files**: stop if more than 70 non-ExecPlan files change.
- **Interface**: stop if any existing public signature in `whitaker-common`,
  `whitaker_lint_core`, or `whitaker_sarif` would change, or if a new public
  item is needed in any crate other than `crates/brain_type` and the
  `whitaker_common::path_exclusion` module named in EP-M1.
- **Dependencies**: stop if any dependency outside `[workspace.dependencies]`
  is needed.
- **Proof**: stop if the Verus gate proof needs `assume`, `admit`, a trusted
  body, or a timeout above Verus's default rlimit; or if, after three modelling
  iterations, a lemma still fails to verify.
- **Iterations**: stop if any gate fails three consecutive fix attempts for
  the same root cause.
- **Toolchain**: stop if the pinned nightly's HIR shape for `for`, `while`,
  `?`, `async fn`, `let … else`, or derive output contradicts the mapping in
  `Interfaces and dependencies` in a way the EP-M0 probe did not anticipate.
- **Ambiguity**: stop if ADR 0015, ADR 0013, and the design document give
  incompatible instructions on a point not listed under
  `Decisions requiring approval`.

## Risks

- **Risk: derive output inflates LCOM4.** `#[derive(Debug, Clone, PartialEq)]`
  adds trait-impl methods whose bodies are generated. Generated field accesses
  are discarded, so each derived method becomes an isolated LCOM4 component,
  and every plain data type with three derives would report LCOM4 >= 3 — the
  escalation threshold. Severity: high. Likelihood: certain if unaddressed.
  Mitigation: an `impl` item whose own span is generated contributes no methods
  to the subject (decision D-4). A UI fixture with five derives and an
  otherwise cohesive type must pass.
- **Risk: desugaring scaffolding double-charges loops.** HIR lowers `for` into
  a `match` around a `loop` around a `match`, and `while` into a `loop` around
  an `if`. Desugarings are authored under ADR 0013, so a naive mapping charges
  the scaffold as well as the loop. Severity: medium. Likelihood: high.
  Mitigation: the Sonar classifier recognizes the loop through `LoopSource` and
  treats nodes whose span carries the matching `DesugaringKind` as transparent
  scaffolding (Interfaces, table 1). UI fixtures pin the scores.
- **Risk: the gate is unsound for degenerate thresholds.** A zero threshold
  makes the corresponding rule fire on a method-less type, which a naive "skip
  types with no methods" gate would hide. Severity: medium. Likelihood: low in
  practice, certain under fuzzing. Mitigation: the gate is derived from the
  configuration and proved sound and maximal in Verus (EP-M3).
- **Risk: `emit_node_span_lint`'s decorator type differs from the current
  `DiagDecorator` usage.** The pinned signature takes
  `impl for<'a> Diagnostic<'a, ()>`. Severity: medium. Likelihood: low; the
  existing `cx.emit_span_lint` call sites already pass
  `rustc_lint::errors::DiagDecorator`. Mitigation: EP-M0 probe compiles the
  call before any other driver code is written.
- **Risk: brain_type UI tests are slow and flaky under compile admission.**
  They join the `serial-dylint-ui` nextest group automatically. Mitigation:
  keep the 6.2.4 fixture set small (about twelve files) and leave breadth to
  6.6.3.
- **Risk: `PathExclusions` extraction changes `no_std_fs_operations`
  behaviour.** Severity: medium. Likelihood: low. Mitigation: EP-M1 is a
  separate refactor commit whose acceptance is the unchanged
  `no_std_fs_operations` test suite and UI fixtures.
- **Risk: method-name collisions confuse cohesion.** Two methods named `new`
  from different impls share an LCOM4 node name. Severity: low. The shipped
  `cohesion_components` already unions every same-named callee by design
  (`common/src/lcom4/mod.rs:244-248`); the driver must not deduplicate by name.
- **Risk: large brain types make clustering slow.** `suggest_decomposition` is
  quadratic in edges and up to cubic overall. Mitigation:
  `max_methods_for_advice` (ADR 0015 capture rule 8) with a measured default
  (EP-M2 decision gate).

## Progress

- [ ] (YYYY-MM-DD HH:MMZ) Plan approved by the user, with answers to the three
  decisions.
- [ ] EP-M0 — HIR probe and walking skeleton.
- [ ] EP-M1 — `PathExclusions` extracted to `whitaker_common::path_exclusion`.
- [ ] EP-M2 — compiler-free policy core (configuration, gate, findings,
  finalize-once accumulator, ordering, advice cap).
- [ ] EP-M3 — Verus proof of gate soundness and maximality.
- [ ] EP-M4 — compiler-free capture sinks and nesting guard.
- [ ] EP-M5 — HIR driver, emission, and UI fixtures.
- [ ] EP-M6 — suite, installer, and experimental opt-in registration.
- [ ] EP-M7 — documentation, ADR amendment, roadmap entry marked done.

## Surprises & discoveries

Planning-time discoveries, recorded so the implementer does not rediscover them:

- `whitaker_lint_core` is in the `make test` `TEST_EXCLUDES` list (`Makefile`
  lines 37-41), so tests placed there never run in the gate. This is why the
  four-sink dispatcher lives in `crates/brain_type`, not in the shared crate.
- ADR 0015 places `resolve_subject_location` in the root `whitaker` crate under
  `src/location/`. Compiler-facing shared helpers moved to `whitaker_lint_core`
  under ADR 0012; the root crate's `src/` now holds the CLI (`src/cli`,
  `src/adapters`). 6.2.4 does not implement the resolver, so this is recorded
  for 6.5.1 rather than acted on.
- `BrainTypeThresholds` has no "disabled" setting for any rule, and WMC is
  unbounded by any phase-one scalar. With every configuration, the sound gate
  admits every subject with at least one method. The gate therefore saves only
  the work for method-less types; the proof in EP-M3 shows no sound gate over
  phase-one scalars can do better.
- `TypeMetricsBuilder::build` silently defaults LCOM4 and foreign reach to
  zero when unset. The driver must always set both, and a unit test must catch
  an omission (VP-9 type half).
- The workspace uses `rstest-bdd` 0.6.0. Several earlier plans cite 0.5.0;
  do not copy that.
- No Fluent catalogue may be added piecemeal: the i18n parity gate requires
  every one of 35 locale directories to carry the same file set
  (`common/tests/i18n_catalogue_parity/rules.rs:19-23`). 6.2.4 therefore ships
  no `.ftl` file and relies on English fallbacks until 6.6.2.

## Decision log

- **D-1 (planning): the four-sink dispatcher is compiler-free and lives in
  `crates/brain_type/src/capture/`.** ADR 0015 capture rule 4 asks for one
  visitor type owning all four builders with one method per HIR event. Split
  that into a compiler-free `MethodSinks` that accepts domain events, and a
  thin HIR adapter that maps nodes to events. The sinks are testable in
  `make test`; the adapter is covered by UI fixtures. `brain_trait` (6.3.3) is
  a concrete second consumer; 6.3.3 decides whether to lift the module into a
  shared crate, and this plan records that as the intended reuse path rather
  than pre-building it.
- **D-2 (planning): the gate is a pure function of configuration and the
  method count.** See `Verification plan`, VP-G1 and VP-G2.
- **D-3 (planning): phase two reuses the line count measured in phase one.**
  LOC is measured once per method from the method's full span (`fn` keyword
  through closing brace) through the source map, as physical lines, matching
  `module_max_lines`. Carrying the value avoids a representation drift between
  the gate's input and the evaluation's input.
- **D-4 (planning): generated `impl` items contribute no methods.** An `impl`
  whose item span `is_generated` (derive output, `macro_rules!`-generated
  impls) is skipped during phase one. Generated nodes inside an authored method
  are still descended into, but charge nothing and raise no nesting (ADR 0013
  consumer rule). This states, as ADR 0013 requires of a new consumer, which
  decisions are withheld from generated nodes: method membership, CC
  increments, nesting depth, field accesses, method calls, foreign references,
  local types, and signature types.
- **D-5 (planning): foreign reach and external domains share one
  definition.** A reference is foreign when a resolved path's `DefId` is not
  local. The recorded key is the item's def path for types and traits, and its
  parent module's def path otherwise. The external domain fed to
  `MethodProfileBuilder::record_external_domain` is the first two segments of
  that key (`std::fs`, `serde::json`), matching the design document's example
  note.
- **D-6 (planning): signature and local types are recorded as the last path
  segment of every resolved nominal type in the written type**, peeling
  references, slices, arrays, tuples, and generic arguments; primitives,
  `Self`, and type parameters are not recorded. `FnRetTy::DefaultReturn`
  records nothing (ADR 0015 capture rule 4).
- **D-7 (planning): suppression through `dylint.toml`.** `[brain_type]`
  accepts `excluded_crates` and `excluded_paths` with the same semantics as
  `no_std_fs_operations` (ADR 0015 lifecycle rule 1, `Goals and non-goals`).
  The matcher is extracted rather than copied (EP-M1).
- **D-8 (planning): `max_methods_for_advice` defaults to 128**, subject to the
  EP-M2 measurement gate: if `suggest_decomposition` over 128 methods with a
  dense similarity graph takes longer than 100 ms in a release build, the
  default drops to 64 and this entry is revised. Above the cap the advice note
  is replaced by "Decomposition advice omitted: `T` has N methods, more than
  `max_methods_for_advice` (M)." Zero disables advice.
- **D-9 (planning): no Kani harness.** The gate is a few comparisons over
  `usize`, and its interesting property is unbounded over method sequences, so
  Verus fits it. The remaining invariants are over heap-heavy containers
  (`BTreeMap`, `String`), where this repository has already measured CBMC
  exhausting an 8 GiB cap (design document §6.4.5 and §6.4.6). Property tests
  cover them at lower cost.
- **D-10 (planning): `brain_type` is added to `DOCTEST_EXCLUDES`.** It links
  `rustc_private` under `--all-features`, like
  `macro_aware_cognitive_complexity`. Policy examples are therefore exercised
  by unit tests, not doctests; the doc comments keep their examples for readers.

## Outcomes & retrospective

Not started.

## Conformance basis

Upstream artefacts, at the revision of `main` this plan was written against
(`9d4c5490`):

- `docs/roadmap.md` item 6.2.4 (`RM-6.2.4`), which requires 6.1.3, 6.2.3, and
  6.4.2, all complete.
- `docs/brain-trust-lints-design.md`: §`brain_type` signals (`DES-SIG`),
  §`brain_type` rule set (`DES-RULE`), §Implementation decisions 6.2.1 to 6.2.3
  and 6.4.2 (`DES-6.2.x`, `DES-6.4.2`), §Implementation approach (`DES-IMPL`),
  §Configuration, localization, and testing (`DES-CFG`).
- `docs/adrs/adr-0015-brain-trust-lint-driver-interfaces.md`, accepted
  2026-09-27: requirements `BTD-REQ-02`, `BTD-REQ-03`, `BTD-REQ-04`,
  `BTD-REQ-05`; capture rules 1-8 (`ADR15-CAP-n`); suggestion rules 1-2 and 4
  (`ADR15-SUG-n`); lifecycle rules 1-4 and 10 (`ADR15-LIFE-n`); language rules
  1, 3, 5 (`ADR15-LANG-n`); delegated obligations `VP-4` and the `brain_type`
  half of `VP-9` from `docs/execplans/6-1-3-...md`.
- `docs/adrs/adr-0013-node-origin-provenance-classifier.md` §Reuse rules
  (`ADR13-REUSE`).
- `docs/adrs/adr-0014-per-lint-experimental-opt-in.md` (`ADR14`).
- `docs/adrs/adr-0016-question-mark-is-a-branch.md` (`ADR16`), by analogy.
- `docs/rfcs/rfc-0010-macro-aware-cognitive-complexity.md` §9 (`RFC10-9`),
  status Proposed.
- Sonar Cognitive Complexity white paper, version 1.7, 29 August 2023
  (`SONAR`).

Deliberately not implemented here, and why: `BTD-REQ-01` (location resolution),
`ADR15-LIFE-5` to `-9` (artefact handoff, merge, fingerprints, SARIF zero-cost),
`ADR15-SUG-3` and `-5` (SARIF property bag), and `ADR15-LANG-2` (mapping
crate) all belong to 6.5.1, which needs a SARIF emitter that does not exist
yet. The finding type is shaped so 6.5.1 can add a location field without
changing anything else.

Trace links:

```plaintext
RM-6.2.4 + DES-RULE + ADR15-CAP-3 -> EP-M2 gate -> EP-M3 Verus -> VP-G1, VP-G2
RM-6.2.4 + ADR15-LIFE-2/3/4 -> EP-M2 accumulator -> VP-4
RM-6.2.4 + BTD-REQ-02 + ADR15-CAP-4/5/6 -> EP-M4 sinks -> VP-N, VP-9(type)
RM-6.2.4 + ADR15-CAP-1/2/7 + DES-SIG -> EP-M5 driver -> VP-K, VP-P, VP-9(type)
RM-6.2.4 + ADR15-LIFE-1 -> EP-M5 emission -> VP-E
RM-6.2.4 + BTD-REQ-03 + ADR15-SUG-1/2/4 + ADR15-CAP-8 -> EP-M2/M5 -> VP-C
BTD-REQ-05 + ADR15-LANG-1/3/5 -> EP-M5 rendering -> UI fixtures (English)
ADR14 -> EP-M6 registration -> suite registration tests
ADR13-REUSE + RFC10-9 -> Decision 1 -> EP-M5 -> VP-P
```

## Context and orientation

The repository is a Cargo workspace (root `Cargo.toml`, members include
`crates/*`, so a new crate joins automatically). The pinned toolchain is
`nightly-2026-05-28` (`rust-toolchain.toml`).

Read these before starting, in this order:

1. `docs/brain-trust-lints-design.md` — what the lint measures.
2. `docs/adrs/adr-0015-brain-trust-lint-driver-interfaces.md` §HIR capture,
   §Suggestion rendering, §Lint-pass lifecycle, §Language boundary — how the
   driver must be shaped.
3. `docs/adrs/adr-0013-node-origin-provenance-classifier.md` §Reuse rules.
4. `docs/adrs/adr-0014-per-lint-experimental-opt-in.md`.
5. `docs/developers-guide.md` §Brain trust lint driver boundary (line 4221),
   §Standard vs Experimental Lints (5391), §Adding a new lint (5518), §Module
   layout and test placement (1939).
6. `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`.
7. `docs/complexity-antipatterns-and-refactoring-strategies.md`, for the
   tone of the diagnostic prose.
8. `docs/whitaker-dylint-suite-design.md` §Per-lint crate scaffolding (stale:
   prefer the live crates below) and `docs/whitaker-clone-detector-design.md`
   for how the repository records proofs beside a pipeline.

Skills to load: `leta` (navigation), `rust-router` then `rust-types-and-apis`,
`rust-state-machines` (the accumulator typestate), `rust-unit-testing`,
`proptest`, `verus`, `hexagonal-architecture`, `nextest`, `execplans`.

Existing code the lint consumes (all in `common/src/`):

- `brain_type_metrics/mod.rs`: `MethodMetrics::new(name, cc, loc)`,
  `TypeMetricsBuilder::new(type_name, cc_threshold, loc_threshold)`,
  `add_method(name, cc, loc)` (pushes unconditionally), `set_lcom4`,
  `set_foreign_reach`, `build() -> TypeMetrics`.
- `brain_type_metrics/cognitive_complexity.rs`: `CognitiveComplexityBuilder`
  with `record_structural_increment`, `record_nesting_increment`,
  `record_fundamental_increment`, `push_nesting`, `pop_nesting` (panics when
  empty), and `build` (panics when unbalanced). Each takes
  `is_from_expansion: bool`.
- `brain_type_metrics/foreign_reach.rs`: `ForeignReferenceSet`.
- `brain_type_metrics/evaluation.rs`: `BrainTypeThresholds` and its builder
  (defaults: `wmc_warn` 60, `wmc_deny` 100, `lcom4_warn` 2, `lcom4_deny` 3,
  `brain_method_deny_count` 2), `evaluate_brain_type`, `BrainTypeDisposition`.
- `brain_type_metrics/diagnostic.rs`: `BrainTypeDiagnostic`,
  `format_primary_message`, `format_note`, `format_help`,
  `format_decomposition_note`.
- `lcom4/`: `MethodInfoBuilder::record_field_access`, `record_method_call`,
  `cohesion_components`.
- `decomposition_advice/`: `MethodProfileBuilder` (`record_accessed_field`,
  `record_signature_type`, `record_local_type`, `record_external_domain`),
  `DecompositionContext`, `SubjectKind::Type`, `suggest_decomposition`
  (fallible), `DecompositionSuggestion`.
- `i18n/helpers.rs`: `safe_resolve_message_set`, `MessageResolution`,
  `DiagnosticMessageSet`, `noop_reporter`; `get_localizer_for_lint`.

Live crates to imitate:

- `crates/macro_aware_cognitive_complexity/` — the closest shape: a
  compiler-free `pub mod policy` beside a
  `#[cfg(feature = "dylint-driver")] mod driver`, `whitaker-common` as a
  non-optional dependency, UI lanes in `tests/ui.rs`, `driver/diagnostic.rs`
  for English fallbacks.
- `crates/bumpy_road_function/` — `SharedConfig::load()` and
  `get_localizer_for_lint` in `check_crate`; `rstest-bdd` behaviour tests in
  `tests/analysis_behaviour.rs` with `tests/features/*.feature`.
- `crates/no_std_fs_operations/src/{config.rs,exclusion.rs}` — the exclusion
  semantics EP-M1 extracts.
- `crates/rstest_helper_should_be_fixture/` — `trybuild` usage.
- `suite/src/{lints.rs,driver.rs}`, `common/src/experimental.rs`,
  `installer/src/resolution.rs` — experimental registration.

Survey for the size budget. Comparable lint crates: `bumpy_road_function` has
1,882 `src/` lines over 10 files and about 340 test lines;
`macro_aware_cognitive_complexity` has 1,628 `src/` lines over 15 files and
about 1,956 test lines. `brain_type` has more moving parts (two phases, four
sinks, gate, typestate, emission) but reuses all arithmetic from `common`.
Estimate: policy 450 lines, capture 400, driver 900, unit and property tests
900, behaviour tests 250, UI fixtures and `.stderr` 450, Verus 250, EP-M1 net
60, registration 60, `Makefile` and scripts 30 — about 3,750 gross, about 3,500
net. Budget 3,600 with escalation at 4,000.

Past budget history: none of the comparable completed plans raised a size
tolerance during execution (`8-2-1`: 650 lines / 14 files; `2-2-5`: 550 / 16;
`6-4-2`: 900 / 10; `6-2-2`: 1,200 / 12). `6-1-3` and `issue-504` exceeded the
400-line file cap repeatedly and resolved it by splitting files, not by raising
limits. This plan's budget is larger than any of them because it is the first
brain trust consumer; the per-file cap, not the total, is the historically
binding constraint, so every module plan below names its split points.

## Plan of work

The work is organized as eight milestones. Each ends at a coherent, gated state
and in at least one commit.

### EP-M0 — HIR probe and walking skeleton (prototyping)

Purpose: retire the toolchain risks before any design is built on them.

Create the crate skeleton (`crates/brain_type/Cargo.toml`, `src/lib.rs`,
`src/driver/mod.rs`) modelled on `macro_aware_cognitive_complexity`, with a
pass whose `check_crate_post` emits one fixed English diagnostic at each local
struct's declaration through
`cx.tcx.emit_node_span_lint(BRAIN_TYPE, hir_id, span, DiagDecorator(…))`. Add
one UI fixture, `ui/probe.rs`, with two structs, one of them under
`#[allow(brain_type)]`.

In a scratch file outside the repository (`/tmp/brain-type-probe/`), dump with
`-Zunpretty=hir-tree` the lowering of `for`, `while`, `while let`, `?`,
`async fn`, `if let`, `let … else`, labelled `break`/`continue`, a closure, and
a `#[derive(Debug, Clone)]` struct, and record in `Artefacts and notes` the
`ExprKind`, `LoopSource`, `MatchSource`, and `DesugaringKind` at each node,
together with `is_generated` for each span.

Acceptance: the fixture shows exactly one warning, at the unannotated struct
(the `#[allow]` proves the level resolved at the declaration). The HIR table is
recorded. Decision gate: if any shape contradicts table 1 in
`Interfaces and dependencies`, revise the table and record a decision before
EP-M4. The skeleton is kept and grown; the fixed message is replaced in EP-M5.
`make check-fmt`, `make lint`, `make test` pass.

### EP-M1 — extract the module-path exclusion matcher (refactor)

Move the pure matcher from `crates/no_std_fs_operations/src/exclusion.rs` into
a new `common/src/path_exclusion.rs`, exposing
`whitaker_common::path_exclusion::PathExclusions`. The constructor returns the
matcher plus the list of rejected entries
(`PathExclusions::new(entries) -> (PathExclusions, Vec<RejectedEntry>)`)
instead of logging, because `whitaker-common` takes no new logging behaviour and
`log` is inert in a lint library anyway (ADR 0015 location rule 9). The
`no_std_fs_operations` driver logs the rejected entries exactly as before. Move
the unit and behaviour tests with the code.

Acceptance: `no_std_fs_operations` tests and UI fixtures pass unchanged;
`common` gains the moved tests; no `no_std_fs_operations` `.stderr` changes.
This is a separate refactor commit before any brain type code uses it.

### EP-M2 — compiler-free policy core

All in `crates/brain_type/src/policy/`, compiled without `dylint-driver`:

- `config.rs`: `BrainTypeConfig` (serde, `deny_unknown_fields`, defaults
  matching `BrainTypeThresholds::default()` plus `brain_method_min_cc = 25`,
  `brain_method_min_lines = 80`, `max_methods_for_advice = 128`,
  `excluded_crates`, `excluded_paths`) and `BrainTypeSettings`, built by
  `BrainTypeConfig::into_settings()`, holding `BrainTypeThresholds`, the two
  brain-method thresholds, the advice cap, and `PathExclusions`.
- `gate.rs`:
  `pub fn admits(settings: &GateInputs, method_count: usize) -> bool`, where
  `GateInputs` is the `Copy` projection of the three deny thresholds the gate
  reads. The body is exactly the Verus `spec_admits`.
- `finding.rs`: `BrainTypeFinding<H>` — the subject handle `H` (the driver
  supplies its `HirId` and `Span`; tests use `u32`), the `OrderKey`, the
  `TypeMetrics`, the `BrainTypeDisposition`, and an `Advice` value:
  `Suggestions(Vec<DecompositionSuggestion>)`,
  `OmittedTooManyMethods { methods, cap }`, `Unavailable(DecompositionError)`,
  or `Disabled`. No rendered prose (ADR 0015 language rule 1).
- `accumulator.rs`: `SubjectAccumulator<H>` (phase-one state: a `BTreeMap`
  keyed by `u32` subject index, holding the handle, the order key, and the
  per-method scalars) and
  `SubjectAccumulator::finalize(self, settings,
  deep: &mut impl DeepCapture<H>) -> FinalizedFindings<H>`.
  `finalize` takes `self` by value; there is no other way to obtain
  `FinalizedFindings`. `DeepCapture<H>` is the port the driver implements:
  given a handle and the admitted method records, it returns the `TypeMetrics`
  inputs (per-method CC, LCOM4, foreign reach) and the `MethodProfile`s.
- `order.rs`: `OrderKey { def_path, subject_name, line, column }` with derived
  `Ord`. `FinalizedFindings::into_sorted()` returns findings sorted by key.
- `advice.rs`: applies `max_methods_for_advice` and calls
  `suggest_decomposition` once per finding, on the complete method set (ADR
  0015 suggestion rule 1).
- `render.rs`: builds the `DiagnosticMessageSet` (primary, note, help) and the
  extra notes (decomposition or omission, escalation) from a finding, using the
  shipped `format_*` functions as English fallbacks via
  `safe_resolve_message_set`. Fluent keys are reserved now (`brain_type`,
  attributes `.note`, `.help`) so 6.6.2 only adds catalogues.

Measure the D-8 decision gate here: a `#[test] #[ignore]` timing probe,
`policy::advice::tests::dense_clustering_budget`, run once with
`cargo test --release -p brain_type -- --ignored dense_clustering_budget`,
records the time for 128 methods sharing one field (a complete similarity
graph). Record the measurement in `Artefacts and notes`.

Tests (red first): unit tests with `rstest` tables for configuration defaults,
unknown-key rejection, settings conversion, the gate truth table, advice cap
boundaries (cap − 1, cap, cap + 1, cap = 0), escalation-note selection, and
finding order. Property tests (VP-G1 bridge, VP-4). A `trybuild` compile-fail
case `tests/compile_fail/finalize_twice.rs` (VP-4). Behaviour tests in
`tests/policy_behaviour.rs` with `tests/features/brain_type_policy.feature`
(below). Snapshot tests with `insta` for the three rendered message shapes (no
brain method, one, many) plus the escalation and omission notes, each paired
with a semantic assertion that the measured numbers appear verbatim (ADR 0015
language rule 5).

Acceptance: all of the above pass; `make check-fmt lint test` pass.

### EP-M3 — Verus proof of the gate (proof milestone)

Add `verus/brain_type_gate.rs`, a group `brain-type` to `scripts/run-verus.sh`
(and to its `all` list), and a `Makefile` target `verus-brain-type` beside
`verus-node-origin`. The model and lemmas are in `Verification plan`. Add the
production/model correspondence test
(`policy::gate::tests::admits_matches_model_truth_table`) that enumerates every
combination of zero/non-zero for the three deny thresholds and `method_count` in
`{0, 1, 2}` against a transliteration of `spec_admits`.

Acceptance: `make verus-brain-type` reports every function verified and zero
errors; the seeded-fault controls in VP-G1 and VP-G2 each fail as predicted and
are reverted; `make check-fmt lint test` pass.

### EP-M4 — compiler-free capture sinks

All in `crates/brain_type/src/capture/`, compiled without `dylint-driver`:

- `events.rs`: the domain event vocabulary: `Origin` (re-using
  `whitaker_common::node_origin::NodeOrigin`), `CcEvent` (`Structural`,
  `StructuralNested`, `Hybrid`, `Fundamental`, `NestingOnly`), and
  `MethodDeclaration { name, has_body, line_count, signature_types }`.
- `sinks.rs`: `MethodSinks`, owning one `CognitiveComplexityBuilder`, one
  `MethodInfoBuilder`, one `MethodProfileBuilder`, and a borrow of the
  subject's shared `ForeignReferenceSet`. It is created only through
  `MethodSinks::declare(declaration, foreign)`, which records the signature
  types before any body event can arrive, so the declaration entry point is the
  only constructor (ADR 0015 capture rule 4). Its methods are one per event:
  `on_cc(event, origin) -> Option<NestingScope>`,
  `on_field_access(name, origin)`, `on_method_call(name, origin)`,
  `on_foreign_reference(key, origin)`, `on_local_type(name, origin)`.
  `finish(self) -> CapturedMethod` returns CC (zero for a bodyless method), the
  `MethodInfo`, and the `MethodProfile`.
- `nesting.rs`: `NestingScope<'a>`, a guard holding
  `&mut CognitiveComplexityBuilder`; `Drop` pops. Nested scopes are obtained
  from the outer scope (`scope.nest(event, origin)`), so the borrow checker
  forces last-in-first-out release.

Tests: unit tests per event; the VP-9 sink-level case (a bodyless declaration
and a bodied one, each with parameter and return types, assert
`MethodProfile::signature_types` and the zero CC); the VP-N property test.

Acceptance: as listed; `make check-fmt lint test` pass.

### EP-M5 — HIR driver, emission, and UI fixtures

All in `crates/brain_type/src/driver/`, behind `dylint-driver`:

- `mod.rs`: lint declaration (`impl_late_lint!`, level `Warn`), the pass
  struct (`Default`, not parameterized by `'tcx`), and the four callbacks.
  `check_crate` clears all state as its first statement, then loads
  `SharedConfig`, the localizer, and `BrainTypeConfig` through
  `dylint_linting::config`. `check_item` dispatches to `phase_one.rs`.
  `check_crate_post` calls `SubjectAccumulator::finalize` with the
  `HirDeepCapture` adapter and emits.
- `phase_one.rs`: for `ItemKind::{Struct, Enum, Union}`, registers the subject
  (local def index, `HirId`, ident span, def path, name, source position). For
  `ItemKind::Impl`, skips generated impls (D-4), normalizes the self type
  (capture rule 7: `tcx.type_of`, peel references, require a local `AdtDef`),
  skips crate- and path-excluded subjects, and records each associated `fn`'s
  `LocalDefId`, name, `BodyId`, and line count (D-3).
- `walker.rs`: `HirDeepCapture`, implementing `DeepCapture<HirId>`. For each
  admitted method it creates `MethodSinks::declare` from the `FnDecl` (via
  `signature.rs`), then walks the body with a `rustc_hir::intravisit::Visitor`
  that classifies each node's origin once with `is_generated(span)` and
  forwards events. After all methods: `cohesion_components` for LCOM4, the
  foreign set's count, and the profiles.
- `sonar.rs`: `classify_expr(&Expr) -> Option<CcEvent>` and the scaffolding
  test, per table 1.
- `signature.rs`: written-type flattening (D-6), foreign key and domain
  rendering (D-5).
- `emit.rs`: renders through `policy::render` and emits through
  `tcx.emit_node_span_lint(BRAIN_TYPE, hir_id, ident_span, DiagDecorator(..))`.

UI fixtures under `crates/brain_type/ui/` (each `fail_*` with `.stderr`, each
optional `*.dylint.toml`), driven by `tests/ui.rs` through
`whitaker_lint_core::testing::ui::run_ui_test`:

1. `fail_brain_type_warns.rs` — one warned type, with decomposition advice.
2. `fail_brain_type_escalates.rs` — WMC >= 100; shows the escalation note.
3. `fail_methods_across_impls.rs` — methods in an inherent impl, a trait
   impl, and an `impl Trait for &T`; LCOM4 and the advice name methods from all
   three (VP-9 type half, VP-K).
4. `pass_cohesive_large_type.rs` — high WMC, one brain method, LCOM4 = 1.
5. `pass_derives_do_not_count.rs` — five derives on a cohesive type (D-4).
6. `pass_generated_branching_not_charged.rs` — macro-generated branching that
   would cross the threshold if charged (VP-P, generated class).
7. `fail_desugarings_are_charged.rs` — `for`, `while`, `?`, `if let`,
   `let … else` written by hand reach the threshold only when charged (VP-P,
   authored class).
8. `pass_generic_impl_counted_once.rs` and
   `fail_concrete_impls_counted_twice.rs` — capture rule 7.
9. `fail_allow_on_other_type.rs` — two brain types, one under
   `#[allow(brain_type)]`; exactly one warning (VP-E).
10. `pass_excluded_path.rs` with `pass_excluded_path.dylint.toml` (D-7).
11. `fail_advice_omitted.rs` with `max_methods_for_advice = 3` (VP-C).

Acceptance: `make test` runs the `ui` harness green; every fixture's negative
control in `Verification plan` has been run once and recorded;
`make check-fmt lint test` pass.

### EP-M6 — experimental registration

Register `brain_type` as experimental, following ADR 0014 and the
`macro_aware_cognitive_complexity` precedent: `suite/Cargo.toml` (optional
dependency with `dylint-driver` and `constituent`), `suite/src/lints.rs`
(`LintDescriptor`, `SUITE_LINT_DECLS`, doc-example counts),
`suite/src/driver.rs` (`experimental_passes()`, count assertions),
`common/src/experimental.rs` (`EXPERIMENTAL_LINTS`),
`installer/src/resolution.rs` (`EXPERIMENTAL_LINT_CRATES` and the name tables),
`Makefile` `DOCTEST_EXCLUDES` (D-10), and any snapshot listing lint names
(`tests/cli_list_snapshot.rs`). Do not touch `LINT_CRATES` in the `Makefile`,
`installer/src/resolution.rs`, or `.github/workflows/rolling-release.yml`;
experimental crates never appear there.

Tests: extend `suite/tests/registration.rs` so `brain_type` registers only when
`[whitaker] experimental` names it, and the installer test that pins
`EXPERIMENTAL_LINT_CRATES == EXPERIMENTAL_LINTS`.

Acceptance: registration tests pass; `make test` with `NEXTEST_PROFILE=ci` also
passes once locally (the default profile filters installer behaviour tests);
`make check-fmt lint test` pass.

### EP-M7 — documentation and closure

- `docs/brain-trust-lints-design.md`: add
  `### Implementation decisions (6.2.4)` recording D-1 to D-10, the Sonar
  mapping table, the gate theorem in prose, and the three approved decisions.
- `docs/adrs/adr-0015-...md`: append a dated `## Amendments` entry recording
  the provenance predicate change (Decision 1) and the stale resolver home.
  Accepted decisions are preserved, not rewritten.
- `docs/developers-guide.md`: a `brain_type` driver section (two phases,
  `DeepCapture` port, `MethodSinks` reuse policy for 6.3.3, why the dispatcher
  is not in `whitaker_lint_core`), and the Verus proof inventory entry for
  `verus/brain_type_gate.rs` (what is and is not proved).
- `docs/users-guide.md`: a `### \`brain_type\`` section under
  §Available Lints marked experimental, with purpose, opt-in, the
  `[brain_type]` keys and defaults, what is allowed, what is reported, and
  how to fix. 6.7.1 extends this with SARIF usage later.
- `docs/contents.md` and `docs/repository-layout.md` if they list crates.
- `docs/roadmap.md`: mark 6.2.4 done only after every acceptance item above
  holds.

Acceptance: `make fmt`, `make markdownlint`, `make nixie`, `make spelling`,
`make check-fmt lint test`, `make verus-brain-type` pass.

## Concrete steps

Run every command from the worktree root. Capture gate output with `tee`:

```bash
BRANCH=$(git branch --show-current)
make fmt 2>&1 | tee /tmp/fmt-whitaker-${BRANCH}.out
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-${BRANCH}.out
make lint 2>&1 | tee /tmp/lint-whitaker-${BRANCH}.out
make test 2>&1 | tee /tmp/test-whitaker-${BRANCH}.out
make verus-brain-type 2>&1 | tee /tmp/verus-whitaker-${BRANCH}.out
```

Focused iteration commands (not gates):

```bash
cargo nextest run -p brain_type --all-features -E 'not binary(ui)'
cargo nextest run -p brain_type --all-features -E 'binary(ui)'
./scripts/run-verus.sh verus/brain_type_gate.rs
```

To re-bless UI expectations after an intended change, follow
`docs/developers-guide.md` §UI test fixtures; never re-bless to make a red test
green without recording why.

Commit at least once per milestone, gating each commit. Expected
`make verus-brain-type` tail:

```plaintext
verification results:: N verified, 0 errors
```

## Validation and acceptance

The feature is accepted when all of the following hold:

1. With `dylint.toml` containing `[whitaker] experimental = ["brain_type"]`,
   linting `crates/brain_type/ui/fail_brain_type_warns.rs` produces one warning
   at the type's identifier, carrying the measured WMC, LCOM4, brain method,
   the metric note, the decomposition note, and the help text.
2. Without the opt-in, the suite registers no `brain_type` pass
   (`suite/tests/registration.rs`).
3. Every UI fixture in EP-M5 passes; each `pass_*` fixture emits nothing.
4. `make verus-brain-type` verifies the gate lemmas with no trusted
   assumptions.
5. All tests named in `Verification plan` pass, and each recorded negative
   control failed when applied.
6. `make check-fmt`, `make lint`, and `make test` pass.

Behaviour specification,
`crates/brain_type/tests/features/brain_type_policy.feature` (traced to
RM-6.2.4, ADR15-CAP-3, ADR15-CAP-8, ADR15-LIFE-2/3, DES-RULE; milestone EP-M2):

```gherkin
Feature: brain_type policy decides, orders and advises

  Scenario: A complex, incohesive type with a brain method is warned
    Given the default brain_type configuration
    And a subject "Parser" with methods of complexity 31, 20 and 23
    And the method "parse_all" spans 96 lines
    And the methods form 2 cohesion components
    When the subject is finalized
    Then the finding for "Parser" has disposition "warn"
    And the primary message contains "WMC=74" and "LCOM4=2"

  Scenario: Weighted complexity alone escalates the type
    Given the default brain_type configuration
    And a subject "Hub" with twelve methods of complexity 9
    And no method spans 80 lines
    When the subject is finalized
    Then the finding for "Hub" has disposition "deny"
    And the escalation note names "WMC >= 100"

  Scenario: A type with no methods is not analysed under positive thresholds
    Given the default brain_type configuration
    And a subject "Plain" with no methods
    When the subject is finalized
    Then the gate excludes "Plain"
    And no finding is produced

  Scenario: Zero thresholds admit even a type with no methods
    Given a brain_type configuration with lcom4_deny 0
    And a subject "Plain" with no methods
    When the subject is finalized
    Then the gate admits "Plain"
    And the finding for "Plain" has disposition "deny"

  Scenario: Advice is omitted above the method cap
    Given a brain_type configuration with max_methods_for_advice 3
    And a warned subject "Wide" with 4 methods
    When the subject is finalized
    Then the advice for "Wide" is omitted with 4 methods and cap 3

  Scenario: Findings are emitted in definition-path order
    Given warned subjects "crate::b::Beta" and "crate::a::Alpha" captured in that order
    When the subjects are finalized
    Then the findings are ordered "crate::a::Alpha", "crate::b::Beta"
```

## Verification plan

Each obligation states its method, artefact, command, evidence, discharge
condition, and non-vacuity control. Kani is deliberately not used (D-9). Verus
is used only for the gate, which is contractual policy from ADR 0015 capture
rule 3 and holds over unbounded method sequences. Everything else is executable
behaviour, where tests are the proportionate tool.

### Axioms (trusted external and in-repository facts)

- A-1: `cohesion_components(ms) <= ms.len()`, and
  `cohesion_components(&[]) == 0`. Shipped behaviour of `whitaker-common`; the
  second is documented (`docs/brain-trust-lints-design.md` §6.1.1). The first
  is checked by a property test added in EP-M3
  (`policy::gate::tests::lcom4_never_exceeds_method_count`) over the shipped
  function, so the axiom the proof relies on is itself tested.
- A-2: `TypeMetrics::wmc()` is the sum of the added methods' CC, and
  `brain_method_count()` counts methods with
  `cc >= cc_threshold && loc >= loc_threshold`. Covered by the shipped `common`
  tests; the proof's `spec_wmc` and `spec_brain_count` mirror them, and the
  VP-G1 bridge property checks the mirror against the shipped builder.
- A-3: `evaluate_brain_type` implements "deny if any of `wmc >= wmc_deny`,
  `brain_count >= brain_method_deny_count`, `lcom4 >= lcom4_deny`; else warn if
  all of `wmc >= wmc_warn`, `brain_count >= 1`, `lcom4 >= lcom4_warn`; else
  pass" (`common/src/brain_type_metrics/evaluation.rs:250`). Mirrored as
  `spec_evaluate`; the bridge property checks the mirror.
- A-4: `TyCtxt::emit_node_span_lint` resolves the lint level at the supplied
  `HirId` (`rustc_middle/src/ty/context.rs:2461-2470` on the pin). Exercised by
  VP-E against the real compiler.
- A-5: `whitaker_lint_core::is_generated` classifies desugarings as authored
  and macro or AST-pass output as generated (ADR 0013, proved in
  `verus/node_origin.rs`). Exercised by VP-P against the real compiler.
- A-6: rustc's HIR holds one `ImplItem` per source `impl` block regardless of
  generic instantiation (ADR 0015 §Counting a method once). Exercised by VP-K.

### Proof sketch for the gate (VP-G1, VP-G2)

Abstract model, independent of the implementation (`verus/brain_type_gate.rs`):

```plaintext
Thresholds  = { wmc_warn, wmc_deny, lcom4_warn, lcom4_deny,
                bm_deny, bm_min_cc, bm_min_loc : nat }
Method      = { cc : nat, loc : nat }
PhaseOne    = { locs : Seq<nat> }                 -- what phase one knows
Deep        = { methods : Seq<Method>, lcom4 : nat }

consistent(p, d) :=  d.methods.len() == p.locs.len()
                  && forall i. d.methods[i].loc == p.locs[i]       (D-3)
                  && d.lcom4 <= d.methods.len()                    (A-1)
                  && (d.methods.len() == 0 ==> d.lcom4 == 0)       (A-1)

spec_wmc(ms)            := sum of ms[i].cc                         (A-2)
spec_brain_count(t, ms) := count of i with ms[i].cc >= t.bm_min_cc
                                       && ms[i].loc >= t.bm_min_loc (A-2)
spec_evaluate(t, d)     := Deny | Warn | Pass as in A-3

spec_admits(t, n) := n > 0 || t.wmc_deny == 0 || t.bm_deny == 0
                           || t.lcom4_deny == 0
```

Preconditions: none beyond the types; thresholds range over all naturals,
including zero. Postconditions, as theorems:

- L1 (soundness, VP-G1): for all `t`, `p`, `d`, if `consistent(p, d)` and
  `!spec_admits(t, p.locs.len())` then `spec_evaluate(t, d) == Pass`.
- L2 (maximality, VP-G2): for all `t`, `p`, if `spec_admits(t, p.locs.len())`
  then there exists `d` with `consistent(p, d)` and
  `spec_evaluate(t, d) != Pass`. This is the formal form of ADR 0015's claim
  that no phase-one scalar bounds WMC: the gate cannot exclude any more
  subjects without information phase one does not have.
- L3 (exec refinement): the executable `admits(inputs, n)` in the proof file
  satisfies `ensures result == spec_admits(t, n)`.

Provisional supporting lemmas, to be confirmed or replaced when the proof is
written: `spec_wmc` and `spec_brain_count` of an empty sequence are zero; for a
non-empty `p`, the witness `d` that copies `p.locs`, sets the first method's
`cc` to `t.wmc_deny` and the rest to zero, and sets `lcom4 = 1`, is consistent
and has `spec_wmc(d.methods) >= t.wmc_deny` (needs a lemma that a sum over
naturals is at least any one term, by induction on length).

Non-vacuity: L1's antecedent is satisfiable (exhibit `t = default`,
`p.locs = []`, `d = empty`); L2's antecedent is satisfiable for both `n > 0`
and the zero-threshold case (exhibit both witnesses as `proof fn` examples).
Seeded faults, each of which must make Verus fail and then be reverted: (a)
drop the `t.lcom4_deny == 0` disjunct from `spec_admits` — L1 fails; (b) replace
`n > 0` with `n > 1` — L1 fails (a one-method type with high CC is excluded);
(c) add a disjunct-free "LOC tripwire" gate `exists i. locs[i] >= bm_min_loc` —
L1 fails. Record each failure transcript in `Artefacts and notes`.

Production bridge: `crates/brain_type/src/policy/gate.rs` implements the same
four-disjunct predicate. The correspondence is checked two ways. (i) The
exhaustive truth-table test in EP-M3. (ii) A property test,
`policy::gate::tests::excluded_subjects_always_pass` (VP-G1 bridge), generating
thresholds in `0..=4`, method counts in `0..=6`, and per-method CC and LOC in
`0..=40` and `0..=100`; it builds `TypeMetrics` through the shipped
`TypeMetricsBuilder` with LCOM4 from the shipped `cohesion_components` over
generated field-access sets, and asserts that whenever `admits` is false, the
shipped `evaluate_brain_type` returns `Pass`. Classification: the run must
record at least 5% excluded cases and at least 5% admitted-and-flagged cases
(`prop_assume!` is not used; classification is asserted at the end of the
test). Negative control: change `admits` to `n > 1` and confirm the property
finds a counterexample.

Command: `make verus-brain-type`;
`cargo nextest run -p brain_type --all-features -E 'test(gate)'`. Discharge:
all three lemmas verified with no trusted items, bridge tests green, seeded
faults recorded.

### VP-4 — deterministic order and finalize-once (delegated from 6.1.3)

- Obligation: the ordered finding sequence is independent of the order in
  which subjects and methods were captured; a finalized accumulator cannot be
  finalized again.
- Method: `proptest` over permutations of a fixed multiset of synthetic
  subjects (with duplicate method names and two subjects sharing a source
  line), asserting identical `into_sorted()` output; a `trybuild` compile-fail
  case that calls `finalize` twice and expects `E0382` (use of moved value).
- Artefact: `crates/brain_type/src/policy/accumulator/tests.rs`,
  `crates/brain_type/tests/compile_fail/finalize_twice.rs` and its `.stderr`.
- Non-vacuity: negative control — temporarily key the accumulator on a
  `HashMap` with `RandomState` and sort nothing; the property must fail. Record
  the failing seed. Second control: make `finalize` take `&self`; the
  `trybuild` case must start compiling and therefore fail.
- Evidence: green property and compile-fail tests; both control transcripts.

### VP-9 (type half) — phase two reaches every method phase one counted

- Obligation: every method phase one recorded for a subject reaches all four
  sinks in phase two, including methods from different `impl` blocks, and its
  `MethodProfile` carries its parameter and written return types; a bodyless
  declaration contributes zero CC (sink level, since a type has no bodyless
  methods; the HIR-level bodyless case belongs to 6.3.3).
- Method: sink-level unit test (EP-M4) asserting `signature_types` for a
  bodied and a bodyless declaration and the zero CC; driver-level UI fixture
  `fail_methods_across_impls.rs`, whose LCOM4 equals the number of disconnected
  methods across three impls and whose decomposition note names methods from
  each impl; unit test that `HirDeepCapture` always calls `set_lcom4` and
  `set_foreign_reach` (a recording fake `DeepCapture` asserts both values are
  reported, so a forgotten call cannot default silently).
- Non-vacuity: (a) skip the trait-impl branch in `phase_one.rs` — the fixture's
  LCOM4 and advice change and the test fails; (b) remove the
  `record_signature_type` calls — the sink test fails rather than passing on
  empty sets; (c) build `TypeMetrics` without `set_lcom4` — the fake-port test
  fails.

### VP-N — nesting is balanced by construction

- Obligation: for any tree of nesting and increment events, delivering it
  through `NestingScope` never panics in `CognitiveComplexityBuilder::build` or
  `pop_nesting`, and the score equals a reference recursive evaluator of the
  Sonar rules over the same tree (with generated levels pushed but not
  deepening).
- Method: `proptest` over generated event trees (depth up to 6, fan-out up to
  4, mixed origins), via the public `MethodSinks` API.
- Non-vacuity: classify cases by maximum authored depth and by presence of a
  generated level; require at least one case at depth >= 3 and one with a
  generated level between two authored levels. Negative control: make
  `NestingScope::drop` skip the pop when the event was generated; the property
  must find a panic or a score mismatch.

### VP-P — provenance: generated charges nothing, desugarings charge

- Obligation: macro-generated control flow neither charges nor raises nesting;
  authored desugarings (`for`, `while`, `?`, `if let`, `let … else`,
  `async fn`) charge exactly per table 1.
- Method: UI fixtures 6 and 7 (EP-M5), each sized so that the type crosses
  the warn threshold only if the obligation is violated (fixture 6) or only if
  it holds (fixture 7); unit tests on `sonar::classify_expr` cannot run without
  HIR, so the fixtures are the evidence.
- Non-vacuity: replace `is_generated(span)` with `span.from_expansion()` —
  fixture 7 must lose its warning; replace it with constant `false` — fixture 6
  must gain one. Record both.

### VP-E — emission resolves the level at the subject

- Obligation: the lint level for a finding is resolved at the subject's
  declaration, not at the crate root.
- Method: UI fixture 9: two brain types, one under `#[allow(brain_type)]`;
  exactly one warning. (ADR 0015 does not make `#[allow]` the supported
  suppression mechanism; the fixture uses it only to observe where the level is
  resolved.)
- Non-vacuity: switch `emit.rs` to `cx.emit_span_lint`; both types must warn
  (the level resolves at the crate root). Record the transcript.

### VP-K — subject keying counts each source method once

- Obligation: `impl<T> Foo<T>` contributes each method once; `impl Foo<u8>`
  and `impl Foo<String>` each contribute; `impl Trait for &Foo` merges into
  `Foo`; impls on primitives, foreign types, and blanket impls contribute to no
  subject; derive output contributes nothing (D-4).
- Method: UI fixtures 3, 5, and 8.
- Non-vacuity: remove reference peeling — fixture 3 loses the `&Foo`
  methods; remove the generated-impl skip — fixture 5 starts warning.

### VP-C — advice cap

- Obligation: with `n > max_methods_for_advice`, no clustering runs and the
  omission note names `n` and the cap; with `n <= cap`, advice is computed.
- Method: `rstest` table at cap − 1, cap, cap + 1, and cap = 0 using a
  counting fake for the clustering call; UI fixture 11.
- Non-vacuity: change `>` to `>=` in the cap test — the `n == cap` row fails.

### Residual gaps

- Sonar fidelity beyond table 1 (recursion increments, `.await`) is not
  verified; recursion is not charged (it needs a call graph), and `.await` is
  not charged (RFC 0010 open question 2). Phase 16 revisits both.
- The HIR adapter has no in-process unit tests; it is covered by UI fixtures
  only, as for every other lint in the repository.
- Location, SARIF, and merge obligations (`VP-3`, `VP-5`, `VP-8`, `VP-10`)
  belong to 6.5.1.

## Idempotence and recovery

Every step is re-runnable. UI `.stderr` files are regenerated only through the
documented bless path. If EP-M1 has to be abandoned, revert its single commit;
nothing else depends on it until EP-M2's configuration, which can fall back to
a private copy of the matcher behind a recorded decision. If the Verus proof
stalls past the proof tolerance, stop and escalate; do not weaken the model.
Generated directories (`target/`, `.uv-cache/`) are not committed.

## Artefacts and notes

To be filled during implementation: the EP-M0 HIR table, the D-8 timing, Verus
seeded-fault transcripts, and each negative-control transcript.

## Interfaces and dependencies

New crate `crates/brain_type` (package `brain_type`, `publish = false`,
`crate-type = ["cdylib", "rlib"]`). Dependencies, all from
`[workspace.dependencies]`: `whitaker-common` (non-optional), `serde`; under
`dylint-driver`: `dylint_linting`, `rustc_hir`, `rustc_lint`, `rustc_middle`,
`rustc_session`, `rustc_span`, `whitaker_lint_core` (feature `dylint-driver`);
`constituent = ["dylint-driver", "dylint_linting/constituent"]`.
Dev-dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `proptest`,
`googletest`, `pretty_assertions`, `insta`, `trybuild`, `dylint_testing`,
`toml`, `camino`, `tempfile`.

Public compiler-free interface (signatures provisional until EP-M2):

```rust
pub mod policy {
    pub struct BrainTypeConfig { /* serde, deny_unknown_fields */ }
    pub struct BrainTypeSettings { /* thresholds, brain-method thresholds, cap, exclusions */ }
    #[derive(Clone, Copy)]
    pub struct GateInputs { pub wmc_deny: usize, pub lcom4_deny: usize, pub brain_method_deny_count: usize }
    pub fn admits(inputs: GateInputs, method_count: usize) -> bool;
    pub struct OrderKey { pub def_path: String, pub subject_name: String, pub line: usize, pub column: usize }
    pub enum Advice {
        Suggestions(Vec<DecompositionSuggestion>),
        OmittedTooManyMethods { methods: usize, cap: usize },
        Unavailable(DecompositionError),
        Disabled,
    }
    pub struct BrainTypeFinding<H> { /* handle, key, metrics, disposition, advice */ }
    pub trait DeepCapture<H> { fn capture(&mut self, subject: &H, methods: &[PhaseOneMethod]) -> DeepMetrics; }
    pub struct SubjectAccumulator<H> { /* BTreeMap<u32, SubjectRecord<H>> */ }
    impl<H> SubjectAccumulator<H> {
        pub fn finalize(self, settings: &BrainTypeSettings, deep: &mut impl DeepCapture<H>) -> FinalizedFindings<H>;
    }
}
pub mod capture {
    pub struct MethodSinks<'f> { /* four builders */ }
    pub struct NestingScope<'b> { /* &mut CognitiveComplexityBuilder; Drop pops */ }
}
```

Table 1 — provisional Sonar mapping (confirmed or revised by EP-M0):

| HIR construct (authored)                                | Event                                                               |
| ------------------------------------------------------- | ------------------------------------------------------------------- |
| `ExprKind::If` not in loop scaffolding (incl. `if let`) | `StructuralNested` (+1 + depth), nests                              |
| `else if` branch                                        | `Hybrid` (+1), nests                                                |
| `else` branch                                           | `Hybrid` (+1), nests                                                |
| `ExprKind::Match` with `MatchSource::Normal`            | `StructuralNested`, arms nest                                       |
| `ExprKind::Loop` (`LoopSource::{Loop, While, ForLoop}`) | `StructuralNested`, body nests; matching scaffolding is transparent |
| `?` (`MatchSource::TryDesugar`)                         | `StructuralNested`, no nesting (Decision 3)                         |
| `let … else`                                            | `StructuralNested`, else block nests                                |
| sequence of like `&&` / `\|\|` operators                | `Fundamental` (+1 per sequence)                                     |
| labelled `break` / `continue`                           | `Fundamental` (+1)                                                  |
| closure, `async` block (authored)                       | `NestingOnly`                                                       |
| `async fn` body wrapper                                 | transparent                                                         |
| generated node (any)                                    | no charge, no depth; children visited                               |
| `.await`, recursion, early `return`                     | no charge                                                           |

_Table 1: Provisional mapping of HIR constructs to Sonar increments._

## Revision note

- 2026-11-21: initial draft for review.

[^sonar]: G. Ann Campbell, _Cognitive Complexity: A new way of measuring
    understandability_, SonarSource, version 1.7, 29 August 2023,
    <https://www.sonarsource.com/docs/CognitiveComplexity.pdf>.
