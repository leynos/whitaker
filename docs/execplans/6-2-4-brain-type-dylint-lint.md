# Create the `brain_type` Dylint lint crate (roadmap 6.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT — revised after design review, awaiting approval. Do not begin
implementation until the user explicitly approves this plan, including the four
decisions listed under `Decisions requiring approval`.

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
on any struct, enum, or union whose methods, taken across every authored `impl`
block in the crate, are complex in aggregate, contain a "brain method", and
fall apart into unrelated groups. The diagnostic names the measured values,
explains why they matter, suggests a decomposition, and points at the type's
declaration. Running the UI harness over the fixture
`crates/brain_type/ui/fail_brain_type_warns.rs` prints, in English, a
diagnostic of this shape:

```plaintext
warning: `Parser` has WMC=74, LCOM4=2, foreign reach=2, and a brain method `parse_all` (CC=31, LOC=96).
  --> $DIR/fail_brain_type_warns.rs:LL:CC
   |
LL | pub struct Parser {
   |            ^^^^^^
   = note: <metric explanation rendered by format_note>
   = note: Potential decomposition for `Parser`:
           - [std::fs] module for `load_from_disk`, `save_to_disk`
           - [grammar] helper struct for `parse_nodes`, `parse_tokens`
   = help: <help text rendered by format_help>
```

The exact text is pinned by the fixture's `.stderr` file once milestone EP-M6
lands; the shape above is the contract.

Terms used throughout:

- **Brain type**: a nominal type (struct, enum, or union) that has accreted
  too much behaviour. See `docs/brain-trust-lints-design.md` §Background.
- **WMC** (weighted methods count): the sum of the cognitive complexity (CC)
  of the type's methods.
- **CC** (cognitive complexity): the Sonar metric defined in G. Ann Campbell's
  white paper[^sonar], fed into `CognitiveComplexityBuilder`.
- **Brain method**: a method with CC >= 25 and at least 80 lines of code
  (LOC). Both thresholds are configurable.
- **LCOM4** (lack of cohesion in methods, variant 4): the number of connected
  components in the graph whose nodes are methods and whose edges are shared
  field accesses or direct calls.
- **Subject**: the unit the lint judges — one local nominal type plus all of
  its authored methods in the crate.
- **Phase one / phase two**: the two-phase capture mandated by ADR 0015.
  Phase one records cheap scalars during the compiler's callbacks; phase two
  re-walks only the subjects the gate admits, after the whole crate has been
  visited.
- **Gate**: the predicate run between the phases. It may exclude a subject
  only when no enabled rule could report it.
- **Reportable**: a subject the lint emits a diagnostic for (Decision 3).
- **Authored / generated**: the provenance classes from ADR 0013. A node is
  generated when a macro or compiler AST pass produced it; desugarings such as
  `for`, `while`, `?`, `.await`, and `async fn` are authored.
- **Port / adapter**: the hexagonal-architecture terms. A port is a trait the
  compiler-free core defines and calls (`DeepCapture`); an adapter is the
  compiler-facing implementation of it (`HirDeepCapture`).

## Decisions requiring approval

The plan cannot be approved without an answer to each of these. Each has a
recommendation; the rest of the plan is written assuming it.

1. **How a `Deny` disposition reaches the user.** `evaluate_brain_type`
   returns `Pass`, `Warn`, or `Deny`, but a Dylint lint has one level, which
   the user controls, and `TyCtxt::emit_node_span_lint` cannot override it per
   finding. A second lint name would double the opt-in and configuration
   surface. **Recommendation:** one lint, `brain_type`, declared at `Warn`. A
   `Deny` finding adds one extra note, "This type also exceeds the escalation
   thresholds: …", naming each escalation condition that fired with its
   configured value. The finding keeps its disposition as a value so roadmap
   item 6.5.1 can map it to SARIF `error`. A user who wants escalation to fail
   the build sets the lint to `deny` in the normal way. This matches the design
   document's "Escalate (configurable)".
2. **What `?` costs in the Sonar mapping.** Sonar charges nothing for early
   exits; the maintainer ruled on 15 November 2026 that "`?` is a branch", and
   ADR 0016 applies that ruling to `bumpy_road_function` only.
   **Recommendation:** charge `?` as the authored early-return `if` it
   abbreviates — one structural increment plus the current nesting increment —
   without raising the nesting level, behind a named policy constant
   (`QUESTION_MARK_CHARGE`) in the shared Sonar mapping so roadmap items 15.6.1
   and 15.6.2 can revisit it in one place. The EP-M6 calibration run measures
   this choice before it ships (see `Tolerances`).
3. **Whether low cohesion alone creates a finding.** The shipped
   `evaluate_brain_type` returns `Deny` when any one of `WMC >= wmc_deny`,
   `brain methods >= brain_method_deny_count`, or `LCOM4 >= lcom4_deny` holds.
   LCOM4 >= 3 alone is common in ordinary code: a handful of small, unrelated
   helpers on a data type. The design document's wording is "**Escalate** …
   when … cohesion is extremely low", which reads as strengthening a finding,
   not creating one. **Recommendation:** the lint reports a subject when `warn`
   holds, or `WMC >= wmc_deny`, or `brain methods >= brain_method_deny_count`;
   `LCOM4 >= lcom4_deny` only escalates a subject that is already reportable.
   This is applied in the lint's policy layer; `evaluate_brain_type` in the
   published `whitaker-common` is unchanged. If rejected, "reportable" becomes
   "`evaluate_brain_type` is not `Pass`", and the gate regains its
   `lcom4_deny == 0` disjunct (see `Verification plan`).
4. **Where the thresholds are configured.** Every shipped Whitaker lint reads
   its table from `dylint.toml` through `dylint_linting::config`; roadmap item
   6.6.1 adds `whitaker.toml` sections, and ADR 0014 records that the
   `whitaker` command supplies lint tables through `DYLINT_TOML`.
   **Recommendation:** 6.2.4 reads a `[brain_type]` table from `dylint.toml`
   with the threshold keys, `max_methods_for_advice`, `excluded_crates`, and
   `excluded_paths`. 6.6.1 then maps its `whitaker.toml` section onto this same
   table rather than inventing a second schema. The design document's example
   key `foreign_types_warn` is rejected (foreign reach has no threshold in
   `evaluate_brain_type`), and the example is amended in EP-M8.

Not a decision, but flagged for review: the provenance predicate. ADR 0015
capture rule 6 says the driver passes `span.from_expansion()` to the builders.
ADR 0013 (accepted 2026-11-08, after ADR 0015's 2026-09-27) forbids that in any
lint ("Do not re-derive provenance in a lint with `from_expansion`"), and
roadmap item 16.4.1 and RFC 0010 §9 already plan the switch to `is_generated`,
because `from_expansion()` is true for desugarings and would drop every `for`
loop and `?`. Because `brain_type` has never shipped, it adopts
`whitaker_lint_core::is_generated(span)` from the start, conforming to the
later ADR. ADR 0015 receives a dated amendment, and 16.4.1 is narrowed to the
threshold re-derivation it still owns.

## Constraints

- Scope is roadmap item 6.2.4 only. Out of scope, each with its owner:
  location resolution to `RepoRelativePath` and `FindingLocation`, the rule
  identifier `WHK004`, and all SARIF output (6.5.1); Fluent catalogue entries
  (6.6.2); `whitaker.toml` configuration sections (6.6.1); the comprehensive
  positive and negative UI suite (6.6.3); the full users' guide chapter
  (6.7.1); the `brain_trait` crate (6.3.3).
- Do not change any public item in `whitaker-common`; it is published
  (`.github/workflows/release.yml`). No item is added to it either, apart from
  the value of the `EXPERIMENTAL_LINTS` constant (EP-M7).
- Add no new production dependency to `whitaker-common` or `whitaker_sarif`
  (ADR 0015 technical requirements).
- `crates/brain_type` must not depend on `whitaker_sarif`,
  `fluent-templates`, or `unic-langid` directly. Localization goes through
  `whitaker_common::safe_resolve_message_set` with English fallbacks.
- Compiler-facing code lives behind a `dylint-driver` feature. The policy core
  in `crates/brain_type/src/policy/` and the capture core in
  `whitaker_lint_core::brain_trust` compile and test without `rustc_private`, so
  `make test` runs them (the feature-free `CORE_TEST_CARGO_FLAGS` pass covers
  `whitaker_lint_core`).
- No capture callback (`check_item`) evaluates, emits, or builds
  `MethodInfo`, `MethodProfile`, or any string set (ADR 0015 lifecycle table).
- Emission goes through `TyCtxt::emit_node_span_lint` with the subject's
  declaration `HirId`. `LateContext::emit_span_lint` is forbidden in this crate
  (ADR 0015 lifecycle rule 1).
- No data-dependent path may call `span_bug`, `span_delayed_bug`,
  `delayed_bug`, or `bug` (ADR 0015 location rule 8). No HIR lookup may use a
  panicking accessor on data that phase one did not itself validate.
- Paths are rendered from `tcx.def_path` plus `tcx.crate_name`, never with
  `def_path_str`: trimmed-path rendering is costly, renders the local crate as
  `crate`, and constructing trimmed paths without emitting a diagnostic is an
  internal compiler error at session end.
- Every accumulator is an ordered container. `DefId` is not `Ord` on the
  pinned toolchain; key by `LocalDefId::local_def_index` (a `u32`), never by
  hashing.
- No source file may exceed 400 lines (`AGENTS.md`). Split by feature before
  reaching 350. Every module, test module, and UI fixture begins with a `//!`
  comment.
- Follow ADR 0014: the lint ships experimental, is compiled into the suite, and
  runs only when a repository names it in `[whitaker] experimental`.
- No `assume`, `admit`, `external_body`, `assume_specification`, or trusted
  proof body in any Verus file. No `#[allow]`; use item-scoped
  `#[expect(..., reason = "...")]` only as a last resort.
- `make check-fmt`, `make lint`, and `make test` must pass at the end of every
  milestone. `make verus-brain-type` must pass at the end of EP-M3 and every
  later milestone.

## Tolerances (exception triggers)

ExecPlan files are excluded from every size count below, including this one.
The work is delivered as two stacked pull requests (decision D-13): PR A holds
EP-M1 to EP-M4; PR B holds EP-M5 to EP-M8. Counts are measured per PR against
its base with `git diff --numstat <base>...HEAD`, as net lines (additions minus
deletions) per category. Each category is enforced on its own; spare capacity
in one never offsets another.

- **Code** (production Rust, tests, UI fixtures and their `.stderr` files,
  Verus proofs, the `Makefile`, shell scripts, and Python workflow-contract
  tests): PR A budget 2,900 net lines, escalate above 3,300; PR B budget 2,200,
  escalate above 2,600. Moves of existing code (EP-M1) count at their net, not
  their churn; record the churn in `Artefacts and notes`.
- **Configuration** (`Cargo.toml` manifests, `Cargo.lock`, `.config/`,
  `dylint.toml` fixtures, CI workflows): 200 net lines across both PRs,
  excluding `Cargo.lock` lines caused only by adding the package. Escalate
  above 260.
- **Documentation** (everything under `docs/` except execplans, plus
  `README.md`): 800 net lines across both PRs. Escalate above 1,000.
- **Files**: escalate if PR A changes more than 45 non-ExecPlan files or PR B
  more than 50.
- **Interface**: escalate if any existing public signature in
  `whitaker-common`, `whitaker_lint_core`, or `whitaker_sarif` would change, or
  if a new public item is needed anywhere other than `crates/brain_type` and the
  `whitaker_lint_core` modules named in EP-M1, EP-M4, and EP-M6.
- **Dependencies**: escalate if any dependency outside
  `[workspace.dependencies]` is needed.
- **Proof**: escalate if the Verus gate proof needs any trusted construct or a
  raised rlimit, or if after three modelling iterations a lemma still fails.
- **Calibration**: escalate before EP-M7 if the EP-M6 calibration run reports
  more than two findings per 10,000 lines of code in either corpus, or if
  setting `QUESTION_MARK_CHARGE` to zero changes the finding count by more than
  half (evidence that Decision 2 dominates the results).
- **Iterations**: escalate if any gate fails three consecutive fix attempts
  for the same root cause.
- **Toolchain**: escalate if the pinned nightly's HIR shape for any construct
  in table 1 contradicts the EP-M5 probe's expectations in a way that changes a
  charge rather than a pattern.
- **Ambiguity**: escalate if ADR 0015, ADR 0013, and the design document give
  incompatible instructions on a point not settled in this plan.

## Risks

- **Derive and macro output inflate LCOM4.**
  `#[derive(Debug, Clone, PartialEq)]` adds trait-impl methods whose bodies are
  generated; a `macro_rules!` getter generator inside an authored impl does the
  same. Generated field accesses are discarded, so each such method would
  become an isolated LCOM4 component. Severity: high. Likelihood: certain if
  unaddressed. Mitigation: D-4 drops any impl whose item is generated, any
  `#[automatically_derived]` impl, and any associated function whose identifier
  span is generated. Fixtures `pass_derives_do_not_count.rs` and
  `pass_macro_generated_methods.rs` pin it.
- **Ordinary types trip LCOM4 without being brain types.** Enum methods that
  `match self`, constructors, and `From` impls touch no field. Severity: high.
  Mitigation: D-11 (whole-`self` use is a synthetic field access; associated
  functions without a `self` receiver stay out of the LCOM4 graph) and Decision
  3 (LCOM4 alone does not report). Fixtures `pass_enum_match_self_methods.rs`
  and `pass_many_from_impls.rs`.
- **Desugaring scaffolding double-charges.** `for` lowers to a `match` around
  a `loop` around a `match`; `while` to a `loop` around an `if`; `.await` to a
  `match` around a `loop` with `LoopSource::Loop`; `async fn` to a coroutine
  closure. Desugarings are authored under ADR 0013, so a naive mapping charges
  the scaffolding. Severity: high (async code would be reported wholesale).
  Mitigation: table 1 treats nodes whose span carries the construct's
  `DesugaringKind` as transparent and charges `LoopSource::Loop` only when its
  span has no desugaring; `async fn` bodies are unwrapped with the shared
  `async_fn_body`; fixture `pass_await_heavy_async_type.rs`.
- **The gate is unsound for degenerate thresholds.** A zero threshold makes a
  rule fire on a method-less type. Severity: medium. Mitigation: the gate is
  derived from configuration and proved sound and least in Verus (EP-M3).
- **Test-only code makes library and test builds disagree.** Under `--test`,
  `#[cfg(test)] impl Foo { … }` adds methods. Severity: medium. Mitigation:
  D-12 excludes methods under `cfg(test)`, and the UI harness runs a `--test`
  lane.
- **False positives in the wild.** The thresholds come from Sonar and Clippy
  scales and have never run over Rust HIR at type level. Severity: high.
  Mitigation: the EP-M6 calibration run over this workspace and
  `leynos/netsuke`, with an escalation tolerance.
- **UI lanes exceed nextest's base timeout.** Only a single top-level
  `fn ui()` joins the `serial-dylint-ui` group automatically
  (`.config/nextest.toml:95`). Mitigation: one `fn ui()` that runs both lanes
  in sequence (D-15), so no new override is needed; a case in
  `crates/whitaker_lint_core/tests/nextest_ui_filter.rs` records it.
- **EP-M1 refactors change existing lints.** Severity: medium. Likelihood:
  low. Mitigation: each lift is its own commit, accepted only when the source
  lint's tests and `.stderr` files are unchanged.
- **Large brain types make clustering slow.** `suggest_decomposition` is
  quadratic in edges and runs label propagation for up to `2n` passes.
  Mitigation: `max_methods_for_advice` (ADR 0015 capture rule 8) with a
  measured default (D-8); advice computed only for reportable subjects.
- **Memory retained for a large crate.** Mitigation: phase one keeps only
  scalars; phase two processes one subject at a time and drops its deep data
  before the next.

## Progress

- [ ] (YYYY-MM-DD HH:MMZ) Plan approved by the user, with answers to the four
  decisions.
- [ ] PR A, EP-M1 — shared compiler-facing helpers lifted into
  `whitaker_lint_core`.
- [ ] PR A, EP-M2 — compiler-free policy core in `crates/brain_type`.
- [ ] PR A, EP-M3 — Verus proof of the gate.
- [ ] PR A, EP-M4 — compiler-free capture core in
  `whitaker_lint_core::brain_trust`.
- [ ] PR B, EP-M5 — HIR probe and walking skeleton.
- [ ] PR B, EP-M6 — HIR driver, emission, UI fixtures, calibration.
- [ ] PR B, EP-M7 — experimental registration.
- [ ] PR B, EP-M8 — documentation, ADR amendment, roadmap entry marked done.

## Surprises & discoveries

Planning-time discoveries, recorded so the implementer does not rediscover them:

- `make test` runs `whitaker_lint_core` in a dedicated feature-free pass
  (`Makefile`, `CORE_TEST_CARGO_FLAGS`), so compiler-free modules there are
  gated; only `dylint-driver` code there is not. The first draft of this plan
  wrongly assumed otherwise.
- Two private copies of `PathExclusions` already exist:
  `crates/no_std_fs_operations/src/exclusion.rs` and
  `crates/string_continuation_style/src/exclusion.rs`.
- ADR 0015 places `resolve_subject_location` in the root `whitaker` crate
  under `src/location/`. Compiler-facing shared helpers live in
  `whitaker_lint_core` (ADR 0012); the root `src/` holds the CLI. 6.2.4 does
  not implement the resolver, so this is recorded for 6.5.1.
- ADR 0015 capture rule 5 assumes nesting is "entered and left across separate
  visitor callbacks", which describes `check_expr`/`check_expr_post`. With
  `rustc_hir::intravisit`, push, walk, and pop happen in one stack frame, and
  `walk_expr` needs the whole visitor mutably, so a guard that borrows only the
  builder does not compile (`E0499`). The guard must borrow the visitor.
- `BrainTypeThresholds` has no "disabled" setting, and per-method CC is
  unbounded by any phase-one scalar. Under every configuration the sound gate
  admits every subject with at least one method; EP-M3 proves no sound gate
  over phase-one scalars can do better. An earlier draft of ADR 0015 proposed a
  LOC tripwire gate that was unsound, which is why the proof is kept.
- `TypeMetricsBuilder::build` silently defaults LCOM4 and foreign reach to
  zero when unset; the policy core, not the adapter, must set both.
- `evaluate_brain_type` hard-codes `brain methods >= 1` in its warn rule, so
  warn can never fire for a method-less type whatever the thresholds.
- `macro_aware_cognitive_complexity`'s traversal produces a flat Clippy-model
  event stream with no nesting (`driver/traversal.rs`, `policy/mod.rs`), so it
  cannot supply Sonar CC; only its `async_fn_body` helper is reusable.
- The workspace uses `rstest-bdd` 0.6.0. Several earlier plans cite 0.5.0.
- No Fluent catalogue may be added piecemeal: the parity gate requires all 35
  locale directories to carry the same file set
  (`common/tests/i18n_catalogue_parity/rules.rs:19-23`). 6.2.4 ships no `.ftl`
  file.
- `trybuild` cases carry workflow-contract machinery (`Makefile`
  `TRYBUILD_CONTRACT_FILTER`, `.config/nextest.toml` overrides, and
  `tests/workflow_contracts/compile_contract_allowance_test.py`). A
  `compile_fail` doctest in the default-feature doctest lane is cheaper.

## Decision log

- **D-1: the four-sink capture core lives in `whitaker_lint_core::brain_trust`
  (compiler-free) and the Sonar classifier in `whitaker_lint_core::hir::sonar`
  (behind `dylint-driver`).** ADR 0015 capture rule 4 asks for one dispatch
  type owning all four builders. `brain_trait` (6.3.3) and the Sonar model
  (15.6.1) are concrete second and third consumers, and 16.4.1 requires
  brain-method CC to equal the function-level score, which one shared mapping
  guarantees and two would not. `whitaker_lint_core` is unpublished and its
  compiler-free modules run in `make test`. Alternatives rejected: keeping the
  core private to `brain_type` (forces 6.3.3 to move it, and leaves two
  mappings for 16.4.1); `whitaker-common` (published, and the constraints
  forbid new public items there); a new crate `brain_trust_capture` (one more
  crate for no extra isolation).
- **D-2: the gate is a pure function of configuration and the method
  count.** See `Verification plan`.
- **D-3: phase two reuses the line count measured in phase one.** LOC is the
  number of physical lines from the method's `fn` keyword through its closing
  brace, computed from `SourceMap::lookup_char_pos` on the span's `lo` and `hi`
  (`hi.line - lo.line + 1`). `span_to_lines` and `span_to_snippet` are not used
  (they allocate). A span whose ends fall in different files is recorded as
  zero lines. Carrying the value avoids drift between the gate's input and the
  evaluation's.
- **D-4: generated methods are not members of a subject.** Phase one skips an
  `impl` item whose span `is_generated`, an `impl` carrying
  `#[automatically_derived]`, and an associated function whose identifier span
  `is_generated`. Checking the identifier rather than the whole item keeps
  methods re-emitted by attribute macros such as `#[async_trait]`, whose
  identifiers stay authored (RFC 0010 §7.5). Inside an authored method,
  generated nodes are descended into but charge nothing, raise no nesting, and
  record no field access, method call, foreign reference, or local type (ADR
  0013 consumer rule). A subject whose declaration identifier is generated is
  skipped entirely: its user cannot edit it.
- **D-5: foreign reach and external domains share one definition.** A
  reference is foreign when a resolved path's `DefId` is not local. Its key is
  the crate-anchored def path (crate name plus named `def_path` segments) of
  the item for types and traits, and of its parent module otherwise. The
  external domain is the key's first two segments (`std::fs`, `serde::json`),
  or its single segment when the key has one; this is the pure function
  `domain_of`.
- **D-6: signature and local types are lowered to a compiler-free
  `TypeShape` and flattened.** The adapter lowers a written `hir::Ty` (or a
  `let` binding's written type) into `TypeShape` — `Named(segment, args)`,
  `Primitive`, `SelfType`, `Param`, `Reference(inner)`, `Slice(inner)`,
  `Array(inner)`, `Tuple(items)`, `Other` — and the pure `flatten` records the
  last segment of every `Named` node. `FnRetTy::DefaultReturn` records nothing
  (ADR 0015 capture rule 4). Local types use written annotations only; a
  binding without an annotation records nothing, avoiding typeck-result
  dependencies.
- **D-7: suppression through `dylint.toml`.** `[brain_type]` accepts
  `excluded_crates` and `excluded_paths` with `no_std_fs_operations` semantics.
  A subject is excluded when an entry is a segment-wise prefix of the
  crate-anchored def path of the type's *declaration* (never of an `impl`
  block's location). The same anchored path is `OrderKey::def_path`.
- **D-8: `max_methods_for_advice` defaults to 128**, subject to the EP-M2
  measurement: if `suggest_decomposition` over 128 methods takes more than 100
  ms in a release build for either a dense input (all methods share one field)
  or an oscillating input (two equal-sized method groups with cross-group edges
  of equal weight, which keeps label propagation running to its `2n` bound),
  the default drops to 64 and this entry is revised. Zero means
  `Advice::Disabled`, decided before the size check. Above the cap the advice
  note reads "Decomposition advice omitted: `T` has N methods, more than
  `max_methods_for_advice` (M)." Advice is computed only for reportable
  subjects.
- **D-9: no Kani harness.** The gate's interesting property is unbounded over
  method sequences, so Verus fits it. The remaining invariants are over
  heap-heavy containers (`BTreeMap`, `String`), where this repository has
  measured CBMC exhausting an 8 GiB cap (design document §6.4.5 and §6.4.6).
  Property tests cover them at lower cost.
- **D-10: doctests.** `brain_type` joins `DOCTEST_EXCLUDES` for the
  all-features doctest lane (it links `rustc_private` there), and gains a
  default-feature lane `cargo test --doc -p brain_type` in `make test-doc`,
  exactly as `macro_aware_cognitive_complexity` does. Policy examples, and the
  finalize-twice `compile_fail` doctest, run there.
- **D-11: LCOM4 graph membership.** Associated functions without a `self`
  receiver contribute to WMC but are not LCOM4 nodes (the usual treatment of
  static methods in LCOM). Any use of `self` other than as the base of a field
  projection or a method-call receiver — passing it, matching on it,
  dereferencing it — is recorded as an access to the synthetic field `self`, so
  methods that inspect the whole value cohere with each other.
- **D-12: methods under `cfg(test)` are excluded.** An `impl` item, or an
  associated function, carrying a `cfg(test)` trace (or nested in a module that
  does) is skipped in phase one, so library and `--test` builds measure the
  same subject. Detection reuses `no_expect_outside_tests`'s
  `is_cfg_test_attribute`, lifted in EP-M1.
- **D-13: two stacked pull requests within one roadmap item.** Design review
  measured the first draft at three to four times the largest precedent's
  budget, and the closest precedent (`macro_aware_cognitive_complexity`, one PR
  of 142 files) needed three forward fixes to its desugaring charges. PR A is
  compiler-free (helpers, policy, proof, capture) and reviewable on its own; PR
  B is the compiler adapter, fixtures, registration, and docs. The roadmap item
  is not split; it is marked done when PR B merges.
- **D-14: the Verus proof is kept, strengthened.** Two of four reviewers
  proposed dropping it as ceremony, since the gate only skips method-less
  types. It is kept because the user selects Verus for contractual business
  logic, the gate's soundness is a contract in ADR 0015 capture rule 3, and the
  least-gate theorem (L2′) is the formal form of ADR 0015's claim that no
  phase-one scalar bounds WMC — a claim an earlier ADR draft got wrong. Seeded
  faults are committed as counterexample proofs so the discrimination is
  re-checked on every run.
- **D-15: one `fn ui()` harness runs two lanes.** The default lane
  (`ui/`) and a `--test` lane (`ui-test/`) run in sequence inside the single
  `ui` test, so the harness inherits the `serial-dylint-ui` group and its
  timeout with no new nextest override.

## Outcomes & retrospective

Not started.

## Conformance basis

Upstream artefacts, at the revision of `main` this plan was written against
(`9d4c5490`):

- `docs/roadmap.md` item 6.2.4 (`RM-6.2.4`), which requires 6.1.3, 6.2.3, and
  6.4.2, all complete. Item 6.5.1 does not list 6.2.4 as a prerequisite but
  depends on it in practice; EP-M8 adds the edge.
- `docs/brain-trust-lints-design.md`: §`brain_type` signals (`DES-SIG`),
  §`brain_type` rule set (`DES-RULE`), §Implementation decisions 6.2.1 to 6.2.3
  and 6.4.2 (`DES-6.2.x`, `DES-6.4.2`), §Implementation approach (`DES-IMPL`),
  §Configuration, localization, and testing (`DES-CFG`).
- `docs/adrs/adr-0015-brain-trust-lint-driver-interfaces.md`, accepted
  2026-09-27: requirements `BTD-REQ-02`, `-03`, `-04`, `-05`; capture rules 1-8
  (`ADR15-CAP-n`); suggestion rules 1, 2, and 4 (`ADR15-SUG-n`); lifecycle
  rules 1-4 (`ADR15-LIFE-n`); language rules 1, 3, 5, and 6 (`ADR15-LANG-n`);
  delegated obligations `VP-4` and the `brain_type` half of `VP-9` from
  `docs/execplans/6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces.md`.
- `docs/adrs/adr-0013-node-origin-provenance-classifier.md` §Reuse rules
  (`ADR13-REUSE`), which supersedes `ADR15-CAP-6`'s predicate.
- `docs/adrs/adr-0014-per-lint-experimental-opt-in.md` (`ADR14`).
- `docs/adrs/adr-0016-question-mark-is-a-branch.md` (`ADR16`), by analogy.
- `docs/rfcs/rfc-0010-macro-aware-cognitive-complexity.md` §7.5 and §9
  (`RFC10`), status Proposed; roadmap 15.6.1 and 16.4.1.
- Sonar Cognitive Complexity white paper, version 1.7, 29 August 2023
  (`SONAR`).

Deliberately not implemented here: `BTD-REQ-01` (location resolution),
`ADR15-LIFE-5` to `-10` (artefact handoff, per-target duplication, incremental
staleness, fingerprints, SARIF zero cost, rule identifiers), `ADR15-SUG-3` and
`-5` (SARIF property bag), and `ADR15-LANG-2` and `-4` (mapping crate, rule
metadata) — all 6.5.1, which needs a SARIF emitter that does not exist yet. The
finding type is shaped so 6.5.1 adds a location field without changing anything
else.

Trace links:

```plaintext
RM-6.2.4 + DES-RULE + ADR15-CAP-3 -> EP-M2 gate -> EP-M3 Verus -> VP-G1, VP-G2
RM-6.2.4 + DES-RULE + Decision 3 -> EP-M2 reportable -> VP-G1 bridge, VP-X
RM-6.2.4 + ADR15-LIFE-2/3/4 -> EP-M2 accumulator -> VP-4
RM-6.2.4 + BTD-REQ-02 + ADR15-CAP-3/4 -> EP-M2 port -> VP-9(type), VP-R
RM-6.2.4 + ADR15-CAP-4/5 -> EP-M4 capture core -> VP-N, VP-9(type)
RM-6.2.4 + ADR15-CAP-1/2/7 + DES-SIG -> EP-M6 driver -> VP-K, VP-P, VP-L
RM-6.2.4 + ADR15-LIFE-1 -> EP-M5/M6 emission -> VP-E
RM-6.2.4 + BTD-REQ-03 + ADR15-SUG-1/2/4 + ADR15-CAP-8 -> EP-M2/M6 -> VP-C
BTD-REQ-05 + ADR15-LANG-1/3/5/6 -> EP-M2 render -> snapshot tests, UI fixtures
ADR14 -> EP-M7 registration -> suite registration tests
ADR13-REUSE + RFC10 -> provenance -> EP-M6 -> VP-P
DES-SIG (foreign reach) -> D-5, D-6 -> EP-M2/M6 -> VP-D, VP-T
```

## Context and orientation

The repository is a Cargo workspace (root `Cargo.toml`; members include
`crates/*`, so a new crate joins automatically). The pinned toolchain is
`nightly-2026-05-28` (`rust-toolchain.toml`).

Read these before starting, in this order:

1. `docs/brain-trust-lints-design.md` — what the lint measures.
2. `docs/adrs/adr-0015-brain-trust-lint-driver-interfaces.md` §HIR capture,
   §Suggestion rendering, §Lint-pass lifecycle, §Language boundary — how the
   driver must be shaped.
3. `docs/adrs/adr-0013-node-origin-provenance-classifier.md` §Reuse rules.
4. `docs/adrs/adr-0014-per-lint-experimental-opt-in.md`.
5. `docs/rfcs/rfc-0010-macro-aware-cognitive-complexity.md` §7.4-7.6.
6. `docs/developers-guide.md` §Brain trust lint driver boundary (line 4221),
   §Node-origin provenance facility (2708), §Standard vs Experimental Lints
   (5391), §Adding a new lint (5518), §Module layout and test placement (1939),
   §Test timeouts (5916).
7. `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/rstest-bdd-users-guide.md`, `docs/rust-doctest-dry-guide.md`.
8. `docs/complexity-antipatterns-and-refactoring-strategies.md`, for the tone
   of the diagnostic prose.
9. `docs/whitaker-dylint-suite-design.md` (its §Per-lint crate scaffolding is
   stale; prefer the live crates below) and
   `docs/whitaker-clone-detector-design.md` for how the repository records
   proofs beside a pipeline.

Skills to load: `leta` (navigation), `rust-router` then `rust-types-and-apis`,
`rust-state-machines` (the accumulator typestate), `hexagonal-architecture`
(port and adapter), `rust-unit-testing`, `proptest`, `verus`, `nextest`,
`execplans`.

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
  `format_primary_message` (appends `foreign reach=N` when non-zero),
  `format_note`, `format_help`, `format_decomposition_note`.
- `lcom4/`: `MethodInfoBuilder::record_field_access`, `record_method_call`,
  `cohesion_components` (zero for an empty slice).
- `decomposition_advice/`: `MethodProfileBuilder` (`record_accessed_field`,
  `record_signature_type`, `record_local_type`, `record_external_domain`),
  `DecompositionContext`, `SubjectKind::Type`, `suggest_decomposition`
  (fallible), `DecompositionSuggestion`.
- `node_origin`: `NodeOrigin`; adapter `whitaker_lint_core::is_generated`.
- `i18n/helpers.rs`: `safe_resolve_message_set`, `MessageResolution`,
  `DiagnosticMessageSet`, `noop_reporter`; `get_localizer_for_lint`.

Live crates to imitate:

- `crates/macro_aware_cognitive_complexity/` — compiler-free `pub mod policy`
  beside `#[cfg(feature = "dylint-driver")] mod driver`; `whitaker-common` as a
  non-optional dependency; `tests/ui.rs` lanes; `driver/diagnostic.rs` for
  English fallbacks; `driver/traversal.rs` for `async_fn_body`.
- `crates/bumpy_road_function/` — `SharedConfig::load()` and
  `get_localizer_for_lint` in `check_crate`; `rstest-bdd` tests in
  `tests/analysis_behaviour.rs` with `tests/features/*.feature`.
- `crates/no_std_fs_operations/src/{config.rs,exclusion.rs,driver.rs}` —
  exclusion semantics and `enclosing_item_path`.
- `crates/no_expect_outside_tests/src/context/mod.rs` —
  `is_cfg_test_attribute`.
- `suite/src/{lints.rs,driver.rs}`, `common/src/experimental.rs`,
  `installer/src/resolution.rs` — experimental registration.

Survey for the size budget. `bumpy_road_function` has 1,882 `src/` lines over
10 files and about 340 test lines; `macro_aware_cognitive_complexity` has 1,628
`src/` lines over 15 files and about 1,956 test lines. Estimate for PR A: EP-M1
lifts net +250 (moves plus new module wiring and tests), policy 600, policy
tests 900, Verus 350, capture core 350, capture tests 350, `Makefile`, script,
and contract test 60 — about 2,860. PR B: driver and Sonar classifier 1,100, UI
fixtures and `.stderr` 750 (low thresholds keep fixtures short; only two use
defaults), harness 80, registration 80, tests 150 — about 2,160.

Past budget history: none of the comparable completed plans raised a size
tolerance during execution (`8-2-1`: 650 lines / 14 files; `2-2-5`: 550 / 16;
`6-4-2`: 900 / 10; `6-2-2`: 1,200 / 12). `6-1-3` and `issue-504` repeatedly
breached the 400-line file cap and resolved it by splitting files. The
first-draft budget for this plan (one PR of 3,600 lines) was rejected in design
review as unreviewable, which produced D-13. The per-file cap remains the
historically binding constraint, so each module list below names its split
points.

## Plan of work

Each milestone ends at a coherent, gated state in at least one commit.

### PR A, EP-M1 — lift shared compiler-facing helpers (refactor)

Four separate refactor commits, each moving existing code into
`whitaker_lint_core` and switching its current users, with no behaviour change:

1. `whitaker_lint_core::path_exclusion::PathExclusions` (compiler-free),
   replacing both private copies in `no_std_fs_operations` and
   `string_continuation_style`. The constructor takes
   `impl IntoIterator<Item: AsRef<str>>` and returns a `#[must_use]`
   `PathExclusionsBuild { exclusions, rejected }`; the bounded-quoting helper
   for logging rejected entries moves with it. Each lint keeps logging the
   rejected entries exactly as before.
2. `whitaker_lint_core::hir::item_path::crate_anchored_path(tcx, def_id) -> SimplePath`,
   lifted from `no_std_fs_operations::driver::enclosing_item_path` (the caller
   keeps its `hir_get_parent_item` step).
3. `whitaker_lint_core::hir::cfg_test::is_cfg_test_attribute`, lifted from
   `no_expect_outside_tests`, which switches to it.
4. `whitaker_lint_core::hir::async_body::async_fn_body`, lifted from
   `macro_aware_cognitive_complexity`, which switches to it.

Acceptance: every moved test passes in its new home; the three source lints'
tests and UI `.stderr` files are byte-identical; `make check-fmt lint test`
pass after each commit.

### PR A, EP-M2 — compiler-free policy core

All in `crates/brain_type/src/policy/`, compiled without `dylint-driver`. The
crate's `Cargo.toml`, `src/lib.rs` (with `pub mod policy`), and an empty
`#[cfg(feature = "dylint-driver")] mod driver` stub are created here.

- `config.rs`: `BrainTypeConfig` (serde, `deny_unknown_fields`, defaults
  matching `BrainTypeThresholds::default()` plus `brain_method_min_cc = 25`,
  `brain_method_min_lines = 80`, `max_methods_for_advice = 128`, and empty
  `excluded_crates` and `excluded_paths`) and `BrainTypeSettings`, built by
  `BrainTypeConfig::into_settings()`. Any threshold values are accepted,
  including zero and `warn > deny`, matching the unvalidated
  `BrainTypeThresholdsBuilder` and the `macro_aware_cognitive_complexity`
  precedent of accepting zero.
- `gate.rs`: `admits(inputs: GateInputs, method_count: usize) -> bool`, where
  `GateInputs` is the `Copy` projection of the deny thresholds the gate reads.
  The body is exactly the Verus `spec_admits`.
- `report.rs`: `reportability(metrics, settings) -> Option<Disposition>`,
  implementing Decision 3 on top of the shipped `evaluate_brain_type`, and
  `escalations(metrics, thresholds) -> Vec<Escalation>`, listing exactly the
  deny conditions that fired.
- `finding.rs`: `BrainTypeFinding<H>` — the subject handle `H` (the driver
  supplies its `HirId` and ident `Span`; tests use `u32`), the `OrderKey`, the
  `TypeMetrics`, the disposition, the escalations, and an `Advice` value:
  `Suggestions(Vec<DecompositionSuggestion>)`,
  `OmittedTooManyMethods { methods, cap }`, `Unavailable(DecompositionError)`,
  or `Disabled`. No rendered prose (ADR 0015 language rule 1).
- `port.rs`: the driven port, `PhaseOneMethod` and `DeepCapture<H>` with
  its single method `capture_method(subject, method, foreign)` (signatures in
  `Interfaces and dependencies`). `CapturedMethod` carries CC, the
  `MethodInfo`, and the `MethodProfile`. The port is infallible: an adapter
  that cannot fetch a body returns the declaration-only capture (CC zero, no
  accesses), mirroring the bodyless rule, so the method count never diverges
  from phase one (VP-R).
- `accumulator.rs`: `SubjectAccumulator<H>` — a
  `BTreeMap<u32, SubjectRecord<H>>` keyed by the type's local def index. A
  record holds `Option<(H, OrderKey)>` (an `impl` may be visited before its
  declaration) and the phase-one methods.
  `finalize(self, settings, deep) -> FinalizedFindings<H>` consumes the
  accumulator; the pass calls it as
  `std::mem::take(&mut self.accumulator).finalize(..)`, and the empty
  accumulator left behind is cleared again by `check_crate` (lifecycle rule 4).
  Finalization, per subject in key order: drop records with no declaration;
  apply the gate; sort the methods by `def_index`; call the port once per
  method; build `TypeMetrics` (always calling `set_lcom4` with
  `cohesion_components` over the receiver methods, and `set_foreign_reach`);
  compute reportability; for reportable subjects only, compute advice; drop the
  subject's deep data.
- `order.rs`: `OrderKey { def_path, subject_name, line, column, def_index }`
  with derived `Ord`. `def_index` makes the order total; the file identifier of
  lifecycle rule 3 is omitted because the crate-anchored def path already
  determines the file within one compilation, and 6.5.1 adds it with the
  location.
- `advice.rs`: applies the cap (D-8) and calls `suggest_decomposition` once
  per reportable finding on the complete method set (ADR 0015 suggestion rule
  1), through a small `Cluster` seam so tests can count calls.
- `render.rs`: builds the `DiagnosticMessageSet` and the extra notes
  (decomposition, omission, escalation) from a finding, with the shipped
  `format_*` functions as English fallbacks through `safe_resolve_message_set`.
  Fluent keys are reserved now (`brain_type` with `.note` and `.help`) so 6.6.2
  only adds catalogues.
- `domain.rs` and `type_shape.rs`: the pure `domain_of` (D-5) and `TypeShape`
  with `flatten` (D-6).

Record the D-8 measurement with a one-off local `--release` run of a scratch
test (not committed); put the timings in `Artefacts and notes`.

Tests are written first (red), then the code (green), then refactored:

- `rstest` tables for configuration defaults, unknown-key rejection
  (including `foreign_types_warn`), settings conversion, the gate truth table
  with literal expected values, reportability (Decision 3 cases), escalation
  selection, advice cap rows (cap − 1, cap, cap + 1, cap = 0 → `Disabled`),
  `domain_of`, and `flatten`.
- Property tests: `evaluate_matches_model`, `excluded_subjects_never_report`,
  `l2_witness_is_reported`, `lcom4_bounds` (see `Verification plan`),
  `escalation_note_iff_deny`, `flatten_records_only_named_segments`, and VP-4's
  permutation property.
- A fake-port test for VP-9 and VP-R, and a counting fake for "no clustering
  for unreportable subjects".
- `compile_fail` doctest on `SubjectAccumulator::finalize` (VP-4).
- Behaviour tests in `crates/brain_type/tests/policy_behaviour.rs` with
  `tests/features/brain_type_policy.feature` (below). `Then` steps assert with
  `googletest` matchers (`contains_substring`, `elements_are!`).
- `insta` snapshots of `render.rs` output for each disposition crossed with
  each `Advice` variant, paired with semantic assertions that every measured
  number appears verbatim (ADR 0015 language rule 5). The three primary message
  shapes are not snapshotted again; `common` already covers them.
- `pretty_assertions::assert_eq` for sorted finding vectors and configuration
  structs.

Acceptance: all of the above pass; `make check-fmt lint test` pass.

### PR A, EP-M3 — Verus proof of the gate (proof milestone)

Add `verus/brain_type_gate.rs`; add the group `brain-type` to
`scripts/run-verus.sh` in `proof_files_for_group`, in the `all` list, and in
the argument-dispatch pattern; add a case to
`tests/workflow_contracts/verus_runner_contract_test.py`; add the `Makefile`
target `verus-brain-type` beside `verus-node-origin`. The model, lemmas,
counterexample proofs, and bridge are in `Verification plan`.

Acceptance: `make verus-brain-type` reports the pinned number of verified items
and zero errors; `make test-workflow-contracts` passes;
`make check-fmt lint test` pass.

### PR A, EP-M4 — compiler-free capture core

All in `crates/whitaker_lint_core/src/brain_trust/`, compiled without
`dylint-driver`:

- `events.rs`: `CcEvent` (`Structural`, `StructuralNested`, `Hybrid`,
  `Fundamental`, `NestingOnly`) and
  `MethodDeclaration { name, has_body, signature_types }`, plus the named
  policy constant `QUESTION_MARK_CHARGE` (Decision 2).
- `sinks.rs`: `MethodSinks`, owning one `CognitiveComplexityBuilder`, one
  `MethodInfoBuilder`, and one `MethodProfileBuilder`, and receiving the
  subject's `ForeignReferenceSet` per call. It is constructed only through
  `MethodSinks::declare(declaration)`, which records the signature types before
  any body event, so the declaration is the single entry point that bodied and
  bodyless methods share (ADR 0015 capture rule 4). One method per event:
  `on_cc(event, origin)`, `on_field_access(name, origin)`,
  `on_whole_self_use(origin)` (D-11), `on_method_call(name, origin)`,
  `on_foreign_reference(key, origin, foreign)`, `on_local_type(name, origin)`.
  `finish(self) -> CapturedMethod`.
- `nesting.rs`: `trait HasSinks { fn sinks(&mut self) -> &mut MethodSinks; }`
  and `NestingScope<'a, V: HasSinks>`, which holds `&'a mut V`, implements
  `DerefMut<Target = V>`, pushes on `enter(visitor, event, origin)`, and pops in
  `Drop`. A HIR visitor writes
  `let mut scope = NestingScope::enter(self,
  event, origin); walk_expr(&mut *scope, expr);`;
  every inner visit takes its own scope from the reborrow, so the borrow
  checker forces last-in, first-out release (ADR 0015 capture rule 5). It is
  the only path to `push_nesting`.

Tests: unit tests per event; VP-9 sink-level cases (a bodied and a bodyless
declaration with parameter and return types, asserting
`MethodProfile::signature_types` with `googletest`'s `unordered_elements_are!`
and the zero CC); the VP-N property test through a test visitor implementing
`HasSinks`.

Acceptance: as listed; `make check-fmt lint test` pass. PR A is opened for
review here.

### PR B, EP-M5 — HIR probe and walking skeleton (prototyping)

Purpose: retire the toolchain risks before the adapter is written.

Grow the `driver` stub into a pass whose `check_crate_post` emits one fixed
English diagnostic at each local struct's identifier through
`cx.tcx.emit_node_span_lint(BRAIN_TYPE, hir_id, span, DiagDecorator(..))`. Add
one UI fixture, `ui/fail_probe_emission.rs`, with two structs, one under
`#[allow(brain_type)]`.

Separately, in a scratch crate kept under the worktree's ignored `target/`
directory (never under `/tmp`, never committed), dump with
`-Zunpretty=hir-tree` the lowering of `for`, `while`, `while let`, `loop`, `?`,
`.await` (top level and inside a loop), `async fn`, an `async` block, `if let`,
`let … else`, labelled `break` and `continue`, `&&`/`||` chains, a closure, a
`#[derive(Debug, Clone)]` struct, a `macro_rules!`-generated method, and a
`#[cfg(test)] impl`. Record in `Artefacts and notes` the `ExprKind`,
`LoopSource`, `MatchSource`, and `DesugaringKind` at each node, `is_generated`
for each span, and the `cfg_trace` attribute shape.

Acceptance: the fixture shows exactly one warning, at the unannotated struct
(the level resolved at the declaration). The HIR table is recorded. Decision
gate: if any shape contradicts table 1, revise table 1 and record a decision
before EP-M6. The fixed message is replaced in EP-M6.
`make check-fmt lint test` pass.

### PR B, EP-M6 — HIR driver, emission, fixtures, and calibration

`crates/whitaker_lint_core/src/hir/sonar.rs` (behind `dylint-driver`):
`classify_expr(&Expr) -> Option<CcEvent>` and `is_scaffolding(&Expr) -> bool`
per table 1, using the lifted `async_fn_body`.

`crates/brain_type/src/driver/` (behind `dylint-driver`):

- `mod.rs`: lint declaration (`impl_late_lint!`, level `Warn`), the pass
  (`Default`, not parameterized by `'tcx`), and the callbacks. `check_crate`
  clears all state as its first statement, then loads `SharedConfig`, the
  localizer, and `BrainTypeConfig` through `dylint_linting::config`.
  `check_crate_post` finalizes and emits.
- `phase_one.rs`: for `ItemKind::{Struct, Enum, Union}`, registers the
  declaration (local def index, `HirId`, ident span, crate-anchored path, name,
  source position) unless excluded (D-7) or generated (D-4). For
  `ItemKind::Impl`, applies D-4 and D-12, normalizes the self type
  (`tcx.type_of(..).instantiate_identity()`, peel references, require a local
  `AdtDef`; projections, primitives, foreign types, and blanket impls are
  skipped), and records each authored associated `fn` as a `PhaseOneMethod`.
- `walker.rs`: `HirDeepCapture`, the `DeepCapture<HirId>` adapter. It
  rebuilds the `LocalDefId` from the method's def index, fetches the
  declaration with `tcx.hir_node_by_def_id`, lowers the `FnDecl` to
  `TypeShape`s, declares the sinks, and walks the body (unwrapped with
  `async_fn_body` when it is an `async fn`) with an `intravisit::Visitor` that
  implements `HasSinks`, classifies each node's origin once with
  `is_generated(span)`, and forwards events. This satisfies ADR 0015 capture
  rule 3's retrieval clause by recorded identity: phase two revisits exactly
  the declarations phase one counted.
- `paths.rs`: `TypeShape` lowering and foreign-key rendering with
  `crate_anchored_path` (D-5, D-6).
- `emit.rs`: renders through `policy::render` and emits through
  `tcx.emit_node_span_lint(BRAIN_TYPE, hir_id, ident_span, DiagDecorator(..))`.

UI fixtures. Every fixture starts with a `//!` comment. Only fixtures 1 and 2
use the default thresholds; the others carry a `*.dylint.toml` with low
thresholds (for example `brain_method_min_lines = 6`, `brain_method_min_cc = 4`,
`wmc_warn = 10`, `wmc_deny = 40`) so each stays short. `tests/ui.rs` has a
single `fn ui()` running `ui/` and then `ui-test/` with
`rustc_flags(["--test"])` (D-15).

1. `fail_brain_type_warns.rs` — one warned type with foreign reach and
   advice (defaults).
2. `fail_brain_type_escalates.rs` — WMC >= 100 with LCOM4 1; shows the
   escalation note naming only `WMC >= 100` (defaults).
3. `fail_methods_across_impls.rs` — methods in an inherent impl, a trait
   impl, and an `impl Trait for &T`; LCOM4 and the advice name methods from all
   three (VP-9 type half, VP-K).
4. `pass_cohesive_large_type.rs` — high WMC below `wmc_deny`, one brain
   method, LCOM4 1.
5. `pass_derives_do_not_count.rs` — five derives on a cohesive type (D-4).
6. `pass_macro_generated_methods.rs` — a `macro_rules!` generating getters
   inside an authored impl (D-4).
7. `pass_generated_branching_not_charged.rs` — macro-generated branching
   that would cross the threshold if charged (VP-P).
8. `fail_desugarings_are_charged.rs` — handwritten `for`, `while`, `?`,
   `if let`, `let … else`, and labelled `break`; the `.stderr` pins the brain
   method's exact `(CC=n, LOC=m)` (VP-P).
9. `pass_await_heavy_async_type.rs` — many `.await` points that must not be
   charged as loops.
10. `pass_enum_match_self_methods.rs` and `pass_many_from_impls.rs` — D-11
    and Decision 3.
11. `pass_generic_impl_counted_once.rs` and
    `fail_concrete_impls_counted_twice.rs` — capture rule 7.
12. `fail_allow_on_other_type.rs` — two brain types, one under
    `#[allow(brain_type)]`; exactly one warning (VP-E).
13. `pass_excluded_path.rs` with `pass_excluded_path.dylint.toml` (D-7).
14. `fail_advice_omitted.rs` with `max_methods_for_advice = 3` (VP-C).
15. `fail_brain_method_line_boundary.rs` — candidates at 79 and 80 lines with
    `brain_method_min_lines = 80`; only the 80-line method is named (VP-L).
16. `pass_clean_crate_no_ice.rs` — a crate with foreign references and no
    findings, proving no trimmed-path construction at session end.
17. `ui-test/pass_cfg_test_impl_not_counted.rs` — a `#[cfg(test)] impl`
    that would cross the threshold if counted (D-12).

Calibration (decision gate before EP-M7): run the lint, opted in, over this
workspace and over a checkout of `leynos/netsuke`, once as built and once with
`QUESTION_MARK_CHARGE` locally set to zero (not committed). Record, in
`Artefacts and notes`, the line counts, finding counts, dispositions, and the
five highest WMC values with their brain methods. Apply the calibration
tolerance.

Acceptance: `make test` runs the `ui` harness green; every negative control in
`Verification plan` has been run once and recorded; calibration recorded within
tolerance; `make check-fmt lint test` and `make verus-brain-type` pass.

### PR B, EP-M7 — experimental registration

Register `brain_type` as experimental, following ADR 0014 and the
`macro_aware_cognitive_complexity` precedent: `suite/Cargo.toml` (optional
dependency with `dylint-driver` and `constituent`), `suite/src/lints.rs`
(`LintDescriptor`, `SUITE_LINT_DECLS`, doc-example counts),
`suite/src/driver.rs` (`experimental_passes()`, count assertions),
`common/src/experimental.rs` (the `EXPERIMENTAL_LINTS` value),
`installer/src/resolution.rs` (`EXPERIMENTAL_LINT_CRATES` and the name tables),
the `Makefile` doctest lanes (D-10), any snapshot listing lint names
(`tests/cli_list_snapshot.rs`), and a case in
`crates/whitaker_lint_core/tests/nextest_ui_filter.rs`. Do not touch
`LINT_CRATES` in the `Makefile`, `installer/src/resolution.rs`, or
`.github/workflows/rolling-release.yml`; experimental crates never appear there.

Tests: extend `suite/tests/registration.rs` so `brain_type` registers only when
`[whitaker] experimental` names it — the end-to-end check of the opt-in
workflow — and keep the installer test that pins
`EXPERIMENTAL_LINT_CRATES == EXPERIMENTAL_LINTS` green.

Acceptance: registration tests pass; `NEXTEST_PROFILE=ci make test` passes once
locally (the default profile filters installer behaviour tests);
`make check-fmt lint test` pass.

### PR B, EP-M8 — documentation and closure

- `docs/brain-trust-lints-design.md`: add
  `### Implementation decisions (6.2.4)` recording D-1 to D-15, table 1, the
  gate theorem in prose, and the four approved decisions; amend §6.2.3's
  `from_expansion` wording per RFC 0010 §9; amend the configuration example
  (drop `foreign_types_warn`).
- `docs/adrs/adr-0015-...md`: append a dated `## Amendments` entry recording
  the provenance predicate (ADR 0013 conformance), the visitor-borrowing guard
  shape for capture rule 5, the per-method port satisfying capture rule 3's
  retrieval clause, and the stale resolver home. Accepted text is preserved.
- `docs/developers-guide.md`: a `brain_type` driver section (two phases, the
  `DeepCapture` port, `whitaker_lint_core::brain_trust` and `hir::sonar` scope
  and reuse policy for 6.3.3 and 15.6.1, the lifted helpers), and the Verus
  proof inventory entry for `verus/brain_type_gate.rs` (what is and is not
  proved).
- `docs/users-guide.md`: a `### \`brain_type\`` section under
  §Available Lints, marked experimental, with purpose, opt-in, the
  `[brain_type]` keys and defaults, what is allowed, what is reported, and
  how to fix. 6.7.1 later adds SARIF usage.
- `docs/roadmap.md`: add `Requires 6.2.4` to 6.5.1; note in 15.6.1 that it
  consumes `whitaker_lint_core::hir::sonar`; narrow 16.4.1 to threshold
  re-derivation and parameter renaming; update the phase 16 introduction's
  `from_expansion()` sentence; mark 6.2.4 done only after every acceptance item
  holds.
- `docs/contents.md` and `docs/repository-layout.md` if they list crates or
  proof groups.

Acceptance: `make fmt`, `make markdownlint`, `make nixie`, `make spelling`,
`make check-fmt lint test`, and `make verus-brain-type` pass. PR B is marked
ready for review.

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
cargo nextest run -p brain_type -E 'not binary(ui)'
cargo nextest run -p whitaker_lint_core --no-default-features -E 'test(brain_trust)'
cargo nextest run -p brain_type --all-features -E 'binary(ui)'
cargo test --doc -p brain_type
./scripts/run-verus.sh brain-type
```

To re-bless UI expectations after an intended change, follow
`docs/developers-guide.md` §UI test fixtures; never re-bless to make a red test
green without recording why.

Commit at least once per milestone, gating each commit. Expected
`make verus-brain-type` tail (the count is pinned in EP-M3):

```plaintext
verification results:: N verified, 0 errors
```

## Validation and acceptance

The feature is accepted when all of the following hold:

1. With `[whitaker] experimental = ["brain_type"]` in `dylint.toml`, the UI
   fixture `fail_brain_type_warns.rs` produces one warning at the type's
   identifier carrying the measured WMC, LCOM4, foreign reach, brain method,
   the metric note, the decomposition note, and the help text.
2. Without the opt-in, the suite registers no `brain_type` pass
   (`suite/tests/registration.rs`).
3. Every UI fixture in EP-M6 passes; each `pass_*` fixture emits nothing.
4. `make verus-brain-type` verifies every gate lemma and counterexample proof
   with no trusted construct.
5. Every test named in `Verification plan` passes, and each recorded negative
   control failed when applied.
6. The calibration run is recorded and within tolerance.
7. `make check-fmt`, `make lint`, and `make test` pass.

Behaviour specification,
`crates/brain_type/tests/features/brain_type_policy.feature` (traced to
RM-6.2.4, ADR15-CAP-3, ADR15-CAP-8, ADR15-LIFE-2/3, DES-RULE, Decision 3;
milestone EP-M2):

```gherkin
Feature: brain_type policy decides, orders and advises

  Scenario: A complex, incohesive type with a brain method is warned
    Given the default brain_type configuration
    And a subject "Parser" with methods "parse_all", "emit" and "reset"
    And the method "parse_all" has complexity 31 and spans 96 lines
    And the methods "emit" and "reset" have complexity 20 and 23
    And the methods form 2 cohesion components
    When the subject is finalized
    Then the finding for "Parser" has disposition "warn"
    And the primary message contains "WMC=74" and "LCOM4=2"

  Scenario: Weighted complexity alone escalates the type
    Given the default brain_type configuration
    And a subject "Hub" with twelve methods of complexity 9
    And no method spans 80 lines
    And the methods form 1 cohesion component
    When the subject is finalized
    Then the finding for "Hub" has disposition "deny"
    And the escalation note names only "WMC >= 100"

  Scenario: Low cohesion alone does not report a type
    Given the default brain_type configuration
    And a subject "Colour" with four methods of complexity 2
    And the methods form 4 cohesion components
    When the subject is finalized
    Then no finding is produced for "Colour"

  Scenario: A cohesive, complex type is analysed but not reported
    Given the default brain_type configuration
    And a subject "Engine" with methods of complexity 30, 30, 20 and 19
    And only the first method of complexity 30 spans 90 lines
    And the methods form 1 cohesion component
    When the subject is finalized
    Then the gate admits "Engine"
    And no finding is produced for "Engine"

  Scenario: A type with no methods is not analysed under positive thresholds
    Given the default brain_type configuration
    And a subject "Plain" with no methods
    When the subject is finalized
    Then the gate excludes "Plain"
    And no finding is produced for "Plain"

  Scenario: A zero escalation threshold admits even a type with no methods
    Given a brain_type configuration with wmc_deny 0
    And a subject "Plain" with no methods
    When the subject is finalized
    Then the gate admits "Plain"
    And the finding for "Plain" has disposition "deny"

  Scenario: Advice is omitted above the method cap
    Given a brain_type configuration with max_methods_for_advice 3
    And a subject "Wide" with methods of complexity 30, 15, 10 and 10
    And the method of complexity 30 spans 96 lines
    And the methods form 2 cohesion components
    When the subject is finalized
    Then the finding for "Wide" has disposition "warn"
    And the advice for "Wide" is omitted with 4 methods and cap 3

  Scenario: A zero advice cap disables advice
    Given a brain_type configuration with max_methods_for_advice 0
    And the warned subject "Parser" from the first scenario
    When the subject is finalized
    Then the advice for "Parser" is disabled

  Scenario: Findings are emitted in definition-path order
    Given warned subjects "demo::b::Beta" and "demo::a::Alpha" captured in that order
    When the subjects are finalized
    Then the findings are ordered "demo::a::Alpha", "demo::b::Beta"
```

Arithmetic under the shipped defaults: "Parser" has WMC 74 (>= 60), one brain
method (31 >= 25, 96 >= 80), LCOM4 2, and no deny condition (74 < 100, 1 < 2, 2
< 3): warn. "Hub" has WMC 108: deny. "Colour" has WMC 8 and no brain method;
only `LCOM4 >= 3` fires, which under Decision 3 reports nothing. "Engine" has
WMC 99, one brain method, LCOM4 1: warn fails on LCOM4 and no deny holds.
"Wide" has WMC 65, one brain method, LCOM4 2: warn.

## Verification plan

Each obligation states its method, artefact, command, evidence, discharge
condition, and non-vacuity control. Kani is deliberately not used (D-9). Verus
is used only for the gate, which is contractual policy from ADR 0015 capture
rule 3 and holds over unbounded method sequences (D-14). Everything else is
executable behaviour, where tests are the proportionate tool; the proof says
nothing about the compiler adapter, which the UI fixtures cover.

### Axioms

- A-1: `cohesion_components(ms)` is zero for an empty slice and lies in
  `1..=ms.len()` otherwise (union-find roots, `common/src/lcom4/mod.rs`).
  Checked by the property test `policy::gate::tests::lcom4_bounds` over the
  shipped function, so the axiom the proof's `consistent_full` relies on is
  itself tested.
- A-2: `TypeMetrics::wmc()` is the sum of the added methods' CC, and
  `brain_method_count()` counts methods with
  `cc >= cc_threshold && loc >= loc_threshold`. Covered by `common`'s tests;
  mirrored as `spec_wmc` and `spec_brain_count`; checked against the mirror by
  `evaluate_matches_model`.
- A-3: `evaluate_brain_type` is "deny if any of `wmc >= wmc_deny`,
  `brain_count >= brain_method_deny_count`, `lcom4 >= lcom4_deny`; else warn if
  all of `wmc >= wmc_warn`, `brain_count >= 1`, `lcom4 >= lcom4_warn`; else
  pass" (`common/src/brain_type_metrics/evaluation.rs`). Mirrored as
  `spec_evaluate`; checked by `evaluate_matches_model`, plus the tripwire test
  `warn_never_fires_without_methods` (`wmc_warn = lcom4_warn = 0`, every deny
  threshold >= 1, empty metrics → `Pass`), which fails if the hard-coded `>= 1`
  is ever removed.
- A-4: `TyCtxt::emit_node_span_lint` resolves the lint level at the supplied
  `HirId` (`rustc_middle/src/ty/context.rs:2461-2470` on the pin). Exercised by
  VP-E.
- A-5: `is_generated` classifies desugarings as authored and macro or
  AST-pass output as generated (ADR 0013; `verus/node_origin.rs`). Exercised by
  VP-P.
- A-6: rustc's HIR holds one `ImplItem` per source `impl` block regardless of
  generic instantiation (ADR 0015 §Counting a method once). Exercised by VP-K.
- A-7 (achievable complexity): for every line count `loc >= 1` and every
  complexity `c`, some authored method body of `loc` physical lines has CC
  `c` — for example, one line holding `c` alternating `&&`/`||` sequences.
  Production `usize` overflow is outside the model. L2 and L2′ depend on it; it
  is the reason no phase-one scalar bounds WMC.

### VP-G1, VP-G2 — the gate is sound and least (proof sketch)

Abstract model, independent of the implementation (`verus/brain_type_gate.rs`):

```plaintext
Thresholds = { wmc_warn, wmc_deny, lcom4_warn, lcom4_deny,
               bm_deny, bm_min_cc, bm_min_loc : nat }
Method     = { cc : nat, loc : nat }
PhaseOne   = { locs : Seq<nat> }                -- what phase one knows
Deep       = { methods : Seq<Method>, lcom4 : nat }

consistent_min(p, d)  := |d.methods| == |p.locs|
                      && (|d.methods| == 0 ==> d.lcom4 == 0)
consistent_full(p, d) := consistent_min(p, d)
                      && forall i. d.methods[i].loc == p.locs[i]        (D-3)
                      && d.lcom4 <= |d.methods|                         (A-1)
                      && (|d.methods| >= 1 ==> d.lcom4 >= 1)            (A-1)
                      && forall i. p.locs[i] >= 1

spec_wmc(ms)            := sum of ms[i].cc                              (A-2)
spec_brain_count(t, ms) := count of i with ms[i].cc >= t.bm_min_cc
                                       && ms[i].loc >= t.bm_min_loc     (A-2)
spec_warn(t, d)         := spec_wmc >= t.wmc_warn && spec_brain_count >= 1
                        && d.lcom4 >= t.lcom4_warn                      (A-3)
spec_reportable(t, d)   := spec_warn(t, d) || spec_wmc >= t.wmc_deny
                        || spec_brain_count >= t.bm_deny                (Decision 3)

spec_admits(t, n) := n > 0 || t.wmc_deny == 0 || t.bm_deny == 0
```

Under Decision 3 the gate has no `lcom4_deny` disjunct, because LCOM4 alone
never makes a subject reportable. If Decision 3 is rejected, `spec_reportable`
becomes "`spec_evaluate` is not `Pass`" and `spec_admits` gains
`|| t.lcom4_deny == 0`; the lemma statements are unchanged.

Theorems (hypotheses about the eventual proof structure; the decomposition may
change when the proof is written):

- L1 (soundness): for all `t`, `p`, `d`:
  `consistent_min(p, d) && !spec_admits(t, |p.locs|) ==> !spec_reportable(t, d)`.
  Stated over the weaker relation so it holds for every deep result phase one
  could pair with.
- L2 (witness): for all `t`, `p` with `forall i. p.locs[i] >= 1`:
  `spec_admits(t, |p.locs|) ==> exists d. consistent_full(p, d) &&
  spec_reportable(t, d)`.
  Stated over the stronger relation so the witness is realistic. A lemma shows
  `consistent_full ==> consistent_min`.
- L2′ (least sound gate): call a gate
  `g: spec_fn(Thresholds, PhaseOne) -> bool` sound when, for all `t`, `p`, `d`,
  `consistent_full(p, d)` and `!g(t, p)` imply `!spec_reportable(t, d)`. Then
  for every sound `g` and every `t`, `p`:
  `spec_admits(t, |p.locs|) ==> g(t, p)`. Proved from L2. Together, L1 and L2′
  say `spec_admits` is the strongest gate phase-one scalars allow.
- Exec mirror: an executable `admits` in the proof file with
  `ensures result == spec_admits(t, n)`. This is labelled a mirror; it is not
  counted as evidence about production code.

Provisional supporting lemmas: `spec_wmc` and `spec_brain_count` of an empty
sequence are zero; a sum over naturals is at least any one term (induction on
length); the witness for `n > 0` copies `p.locs`, sets the first method's `cc`
to `t.wmc_deny` and the others to zero, and sets `lcom4 = 1`; the witness for
`n = 0` is the empty deep result, reportable because a deny threshold is zero.

Non-vacuity, committed as proofs in the same file (the style of
`lemma_class_witnesses` in `verus/node_origin.rs`):

- Satisfiability witnesses as `proof fn`s: L1's antecedent at default
  thresholds with `p.locs = []`; L2's antecedent at `n = 1`, and at `n = 0` with
  `t.wmc_deny = 0`.
- Each seeded fault is kept as a named `spec fn`, with a `proof fn` whose
  `ensures` states the counterexample it admits: (a) drop `t.bm_deny == 0` from
  `spec_admits` — breaks L1 at `n = 0`, `bm_deny = 0`; (b) replace `n > 0` with
  `n > 1` — breaks L1 at `n = 1` with `cc = wmc_deny`; (c) replace `n > 0` with
  `exists i. locs[i] >= bm_min_loc` (the LOC tripwire) — breaks L1 with short
  methods summing past `wmc_deny`; (d) `spec_admits := true` — breaks L2′
  minimality at `n = 0` with every deny threshold >= 1 (the gate is sound but
  not least); (e) change `spec_warn`'s `brain_count >= 1` to `>= 0` — breaks L1
  with `wmc_warn = lcom4_warn = 0`; (f) drop
  `|d.methods| == 0 ==> d.lcom4 == 0` from `consistent_min` under the Decision
  3-rejected variant — breaks L1 with an empty `d` and `lcom4 = 5`.

The pinned count of verified items in the `make verus-brain-type` output
catches a deleted lemma.

Production bridge (Verus cannot import the production crate, and a `#[path]`
include would need a trusted specification, which the constraints forbid):

- `admits_truth_table`: an `rstest` with literal expected booleans over
  thresholds in `{0, 1, usize::MAX}` and `n` in `{0, 1, 2}`.
- `evaluate_matches_model`: a property comparing a handwritten
  `model_evaluate` with the shipped `evaluate_brain_type` and the shipped
  `TypeMetricsBuilder`, with values biased to `{t − 1, t, t + 1}` around each
  threshold.
- `excluded_subjects_never_report`: thresholds in `0..=4`, method counts from
  `prop_oneof![1 => Just(0), 2 => 1..=6usize]`, CC in `0..=40`, LOC in
  `1..=100`, LCOM4 from the shipped `cohesion_components` over generated
  field-access sets; asserts `!admits ==> reportability(..) == None`. At least
  one excluded case and one admitted-and-reported case are asserted per run.
- `l2_witness_is_reported`: for admitted `n > 0`, builds the L2 witness
  through the shipped builder and asserts it is reportable.

Negative control for the bridge: change `admits` to `n > 1`; both
`admits_truth_table` and `excluded_subjects_never_report` must fail.

Command: `make verus-brain-type`;
`cargo nextest run -p brain_type -E 'test(gate)'`. Discharge: every lemma and
counterexample verified with no trusted item; bridge tests green.

### VP-X — reportability and escalation

- Obligation: a finding exists iff `spec_reportable`; its disposition is
  `Deny` iff `evaluate_brain_type` returns `Deny`; the escalation note lists
  exactly the deny conditions that fired, with configured values; `Pass` never
  produces a finding.
- Method: `rstest` rows per condition and per pair; property
  `escalation_note_iff_deny`.
- Non-vacuity: make `reportability` return the shipped disposition unchanged —
  the "Colour" scenario and its row fail.

### VP-4 — deterministic order and finalize-once (delegated from 6.1.3)

- Obligation: the ordered finding sequence, and every finding's advice, are
  independent of capture order; a consumed accumulator cannot be finalized
  again.
- Method: a property over permutations of a fixed multiset of synthetic
  subjects (duplicate method names, two subjects sharing a source line,
  colliding key prefixes distinguished only by `def_index`), permuting both
  subjects and methods within subjects, comparing the full `FinalizedFindings`
  (including advice) with `pretty_assertions::assert_eq`; a `compile_fail`
  doctest calling `finalize` twice.
- Non-vacuity: key subjects with a `HashMap` and skip the method sort — the
  property must fail; record the seed. Make `finalize` take `&self` — the
  doctest starts compiling and therefore fails.

### VP-9 (type half) and VP-R — phase two reaches every counted method

- Obligation: every method phase one recorded reaches the port exactly once,
  in `def_index` order, and lands in all four sinks; `TypeMetrics` always has
  LCOM4 and foreign reach set; `metrics.method_count()` equals the phase-one
  count and each method's LOC equals its phase-one line count (D-3); each
  `MethodProfile` carries its parameter and written return types; a bodyless
  declaration contributes zero CC. The HIR-level bodyless case (required trait
  methods) belongs to 6.3.3.
- Method: a recording fake `DeepCapture` in the policy tests; sink-level unit
  tests (EP-M4); UI fixture 3, whose LCOM4 counts disconnected methods across
  three impls and whose advice names methods from each.
- Non-vacuity: (a) skip the trait-impl branch in `phase_one.rs` — fixture 3
  changes; (b) remove the `record_signature_type` calls — the sink test fails
  rather than passing on empty sets; (c) remove the `set_lcom4` call — the
  fake-port test fails; (d) recompute LOC in phase two from a different span —
  the representation test fails.

### VP-N — nesting is balanced by construction

- Obligation: for any tree of events delivered through `NestingScope`,
  `CognitiveComplexityBuilder::build` and `pop_nesting` never panic, and the
  score equals the reference
  `score = Σ over authored nodes v of base(v) +
  [nests(v)] · (number of authored nesting ancestors of v)`.
- Method: a property over generated event trees (depth up to 6, fan-out up to
  4, mixed origins) walked by a test visitor implementing `HasSinks`.
- Non-vacuity: assert at least one case of authored depth >= 3 and one with a
  generated level between two authored levels. Negative control: make
  `NestingScope::drop` skip the pop for generated events — the property must
  find a panic or a mismatch.

### VP-P — provenance: generated charges nothing, desugarings charge

- Obligation: macro-generated control flow neither charges nor raises
  nesting; authored desugarings charge per table 1; `.await` and `async fn`
  wrappers charge nothing.
- Method: UI fixtures 7, 8, and 9; fixture 8 pins exact CC and LOC.
- Non-vacuity: replace `is_generated(span)` with `span.from_expansion()` —
  fixture 8's CC drops; replace it with constant `false` — fixture 7 gains a
  warning; remove the `.await` scaffolding rule — fixture 9 gains one.

### VP-E — emission resolves the level at the subject

- Obligation: the lint level of a finding is resolved at the subject's
  declaration, not at the crate root.
- Method: fixture 12 (and the EP-M5 probe fixture). ADR 0015 does not make
  `#[allow]` the supported suppression mechanism; the fixture uses it only to
  observe where the level is resolved.
- Non-vacuity: switch `emit.rs` to `cx.emit_span_lint` — both types warn.

### VP-K — subject keying counts each source method once

- Obligation: `impl<T> Foo<T>` contributes each method once; `impl Foo<u8>`
  and `impl Foo<String>` each contribute; `impl Trait for &Foo` merges into
  `Foo`; impls on primitives, foreign types, projections, and blanket impls
  contribute to no subject; derive and macro output contribute nothing (D-4);
  `cfg(test)` methods contribute nothing (D-12).
- Method: UI fixtures 3, 5, 6, 11, and 17.
- Non-vacuity: remove reference peeling — fixture 3 loses the `&Foo`
  methods; remove the generated-impl skip — fixture 5 warns; remove the
  `cfg(test)` skip — fixture 17 warns.

### VP-C — advice cap

- Obligation: cap 0 yields `Disabled` with no clustering; `n > cap` yields
  the omission note naming `n` and the cap with no clustering; `n <= cap`
  clusters once; unreportable subjects never cluster.
- Method: `rstest` rows at cap − 1, cap, cap + 1, and cap 0 with a counting
  fake; fixture 14.
- Non-vacuity: change `>` to `>=` — the `n == cap` row fails; compute advice
  before reportability — the counting fake sees a call for a passing subject.

### VP-L, VP-D, VP-T — LOC, domains, and type shapes

- VP-L: the line-count function is a unit-tested pure function of
  `(lo.line, hi.line, same_file)`; fixture 15 pins the 79/80 boundary. Control:
  use `hi.line - lo.line` — fixture 15 names neither method.
- VP-D: `domain_of` `rstest` table (one, two, and three segments); property
  "a rendered domain implies `foreign_reach >= 1`" through the policy layer.
  Control: return the full key — the table fails.
- VP-T: `flatten` `rstest` table over every `TypeShape` variant; property
  "the output contains no primitive, `Self`, or type parameter name". Control:
  record `Param` names — the property fails.

### Residual gaps

- Sonar fidelity beyond table 1 is not verified: recursion is not charged (it
  needs a call graph), and `.await` is not charged (RFC 0010 open question 2).
  15.6.1 and phase 16 revisit both.
- The HIR adapter has no in-process unit tests; it is covered by UI fixtures,
  as for every other lint in the repository.
- Location, SARIF, and merge obligations (`VP-3`, `VP-5`, `VP-8`, `VP-10`)
  belong to 6.5.1.

## Idempotence and recovery

Every step is re-runnable. UI `.stderr` files are regenerated only through the
documented bless path. Each EP-M1 lift is one commit and can be reverted on its
own; nothing in `brain_type` uses a lift until EP-M2. If the Verus proof stalls
past the proof tolerance, stop and escalate; do not weaken the model. The EP-M5
scratch crate lives under the ignored `target/` directory and is deleted after
the table is recorded.

## Artefacts and notes

To be filled during implementation: the EP-M1 move churn, the EP-M5 HIR table,
the D-8 timings (dense and oscillating), the pinned Verus count, each
negative-control transcript, and the EP-M6 calibration results.

## Interfaces and dependencies

New crate `crates/brain_type` (package `brain_type`, `publish = false`,
`crate-type = ["cdylib", "rlib"]`). Dependencies, all from
`[workspace.dependencies]`: `whitaker-common` and `whitaker_lint_core`
(non-optional, default features), `serde`; under `dylint-driver`:
`dylint_linting`, `rustc_hir`, `rustc_lint`, `rustc_middle`, `rustc_session`,
`rustc_span`, and `whitaker_lint_core/dylint-driver`;
`constituent = ["dylint-driver", "dylint_linting/constituent"]`.
Dev-dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `proptest`,
`googletest`, `pretty_assertions`, `insta`, `dylint_testing`, `toml`, `camino`,
`tempfile`. `whitaker_lint_core` gains `rustc_middle` under its `dylint-driver`
feature only if `hir::sonar` needs it.

Compiler-free interface (signatures provisional until EP-M2 and EP-M4):

```rust
// crates/brain_type/src/policy
pub struct BrainTypeConfig { /* serde, deny_unknown_fields */ }
pub struct BrainTypeSettings { /* thresholds, brain-method thresholds, cap, exclusions */ }
#[derive(Clone, Copy)]
pub struct GateInputs { pub wmc_deny: usize, pub brain_method_deny_count: usize }
pub fn admits(inputs: GateInputs, method_count: usize) -> bool;
pub struct OrderKey {
    pub def_path: String,
    pub subject_name: String,
    pub line: usize,
    pub column: usize,
    pub def_index: u32,
}
pub enum Advice {
    Suggestions(Vec<DecompositionSuggestion>),
    OmittedTooManyMethods { methods: usize, cap: usize },
    Unavailable(DecompositionError),
    Disabled,
}
pub struct PhaseOneMethod {
    pub def_index: u32,
    pub name: String,
    pub line_count: usize,
    pub has_self_receiver: bool,
}
pub trait DeepCapture<H> {
    fn capture_method(
        &mut self,
        subject: &H,
        method: &PhaseOneMethod,
        foreign: &mut ForeignReferenceSet,
    ) -> CapturedMethod;
}
pub struct SubjectAccumulator<H> { /* BTreeMap<u32, SubjectRecord<H>> */ }
impl<H> SubjectAccumulator<H> {
    pub fn finalize(
        self,
        settings: &BrainTypeSettings,
        deep: &mut impl DeepCapture<H>,
    ) -> FinalizedFindings<H>;
}

// crates/whitaker_lint_core/src/brain_trust
pub enum CcEvent { Structural, StructuralNested, Hybrid, Fundamental, NestingOnly }
pub struct MethodSinks { /* three builders */ }
pub trait HasSinks { fn sinks(&mut self) -> &mut MethodSinks; }
pub struct NestingScope<'a, V: HasSinks> { /* &'a mut V; DerefMut; Drop pops */ }
```

Table 1 — provisional Sonar mapping (confirmed or revised by EP-M5):

| HIR construct (authored)                          | Event                                               |
| ------------------------------------------------- | --------------------------------------------------- |
| `ExprKind::If` outside scaffolding (`if let` too) | `StructuralNested` (+1 + depth), branches nest      |
| `else if` branch                                  | `Hybrid` (+1), nests                                |
| `else` branch                                     | `Hybrid` (+1), nests                                |
| `ExprKind::Match`, `MatchSource::Normal`          | `StructuralNested`, arms nest                       |
| `ExprKind::Loop`, `LoopSource::ForLoop`/`While`   | `StructuralNested`, body nests; scaffolding skipped |
| `ExprKind::Loop`, `LoopSource::Loop`, no desugar  | `StructuralNested`, body nests                      |
| `.await` (`DesugaringKind::Await` scaffolding)    | transparent, no charge                              |
| `?` (`MatchSource::TryDesugar`)                   | `QUESTION_MARK_CHARGE` (Decision 2), no nesting     |
| `let … else`                                      | `StructuralNested`, else block nests                |
| sequence of like `&&` or `\|\|` operators         | `Fundamental` (+1 per sequence)                     |
| labelled `break` or `continue`                    | `Fundamental` (+1)                                  |
| authored closure or `async` block                 | `NestingOnly`                                       |
| `async fn` body wrapper (`async_fn_body`)         | transparent                                         |
| generated node (any)                              | no charge, no depth; children visited               |
| recursion, early `return`                         | no charge                                           |

*Table 1: Provisional mapping of HIR constructs to Sonar increments.*

## Revision note

- 2026-11-21: initial draft for review.
- 2026-11-21: revised after a four-lens design review (structure and
  contracts; failure modes and cost; alternatives and viability; verification
  rigour). Changes: the nesting guard now borrows the visitor (the first design
  could not compile against `intravisit`); the port became per-method with
  policy-owned LCOM4; the capture core and Sonar classifier moved to
  `whitaker_lint_core` (the "never runs in `make test`" premise was false);
  four shared helpers are lifted rather than copied; Decisions 3 and 4 were
  added and the provenance predicate reframed as conformance to ADR 0013; D-11
  to D-15 were added for LCOM4 membership, `cfg(test)`, delivery as two stacked
  PRs, the kept Verus proof, and the UI harness; `.await` scaffolding, derive
  detection, path rendering, and LOC measurement were corrected; the Verus
  model was split into minimal and full consistency, gained the least-gate
  theorem L2′ and committed counterexample proofs; the bridge tests were
  corrected; `trybuild` was replaced with a `compile_fail` doctest; a
  calibration gate and its tolerance were added; the BDD scenarios' arithmetic
  was pinned. Remaining work is unchanged in kind: approval, then PR A.

[^sonar]: G. Ann Campbell, *Cognitive Complexity: A new way of measuring
    understandability*, SonarSource, version 1.7, 29 August 2023,
    <https://www.sonarsource.com/docs/CognitiveComplexity.pdf>.
