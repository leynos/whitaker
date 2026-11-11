# Implement crate-post aggregation thresholds and actionable diagnostics for `rstest_helper_should_be_fixture` (8.2.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

Roadmap item: `docs/roadmap.md` §8.2, item 8.2.3. Requires 8.2.2 and 8.1.2
(both complete). Implementation must not begin until this plan is explicitly
approved.

## Purpose / big picture

`rstest_helper_should_be_fixture` (lint A) is an experimental Whitaker lint.
Today it walks `#[rstest]` test bodies, records every call to a local helper
function, and fingerprints each call's arguments, but it never tells the user
anything. This plan makes the lint useful: after `check_crate_post` has seen
every test in the crate, it groups the recorded calls by helper, decides which
helpers behave like fixtures, and emits one actionable diagnostic per such
helper, attributed to the helper's own High-level Intermediate Representation
(HIR) node so that `#[allow]` and `#[expect]` on the helper behave as users
expect.

After this change, a user who enables the experimental lint and writes:

```rust
fn make_db() -> Db { Db::in_memory() }

#[rstest]
fn inserts(...) { let db = make_db(); /* ... */ }

#[rstest]
fn deletes(...) { let db = make_db(); /* ... */ }
```

sees a warning of this shape (exact text is fixed in milestone EP-M4 and
pinned by a byte-exact `.stderr` fixture):

```plaintext
warning: Helper `make_db` behaves like a fixture in `rstest` tests.
  --> src/lib.rs:1:4
   |
 1 | fn make_db() -> Db { Db::in_memory() }
   |    ^^^^^^^
   |
note: Found 2 calls across 2 tests, all without arguments.
  --> src/lib.rs:5:14
   |
 5 | fn inserts(...) { let db = make_db(); /* ... */ }
   |                            ^^^^^^^^^
...
   = help: Provide `make_db` through a `#[fixture]` and inject it as a test parameter.
```

Adding `#[allow(rstest_helper_should_be_fixture)]` (or `#[expect(...)]`) to
`make_db` silences that warning, and an `#[expect]` there is reported as
fulfilled rather than unfulfilled.

The plan also repairs two defects in the 8.2.2 evidence that this item depends
on (see `Surprises & discoveries`): plain `#[rstest]` tests are currently not
collected at all, and `#[case]`/`#[values]` parameters are misclassified as
fixtures after macro expansion. Without those repairs, the diagnostics would
miss most real tests and would recommend fixtures for helpers that receive
per-case data.

## Constraints

These are hard invariants. Violating one requires escalation, not a
workaround.

- The lint remains experimental. It stays behind the
  `experimental-rstest-helper-should-be-fixture` suite feature and the
  installer's `--experimental` flag. Promotion is out of scope.
- Do not add the upstream `clippy_utils` crate. The workspace `clippy_utils`
  is a local stub (`crates/clippy_utils/src/lib.rs`), and 8.2.2 explicitly
  declined a rustc-private upstream dependency. `span_lint_hir_and_then` is
  provided by a repository-owned helper (see `Interfaces and dependencies`).
- No new external dependency. Dev-dependencies already pinned in the
  workspace (`googletest`, `pretty_assertions`, `insta`, `proptest`,
  `rstest`, `rstest-bdd`, `rstest-bdd-macros`) may be added to crates that
  lack them; the user has authorized these. Any dependency not already in
  `[workspace.dependencies]` requires escalation.
- No source file may exceed 400 lines (`AGENTS.md`). Two files are already at
  the cap: `crates/rstest_helper_should_be_fixture/src/driver.rs` (395 lines)
  and `crates/rstest_helper_should_be_fixture/src/collector.rs` (383 lines).
  New logic must live in new modules, and a behaviour-preserving extraction
  from `driver.rs` must precede any growth there.
- Production code must not call `.expect()`, `.unwrap()`, or panic. Lint
  emission failures degrade to "no diagnostic" with a `debug!` record.
- Determinism: diagnostics, their order, their notes, and every test
  snapshot must be identical across runs and independent of HIR traversal
  order or hash-map iteration order.
- The fingerprint model in `whitaker_common::rstest` (`ArgAtom`,
  `ArgFingerprint`) keeps its public shape. The only permitted change in how
  arguments are lowered to atoms is the narrowly scoped `static mut` and
  provider-parameter repairs in EP-M1, each recorded in the `Decision log`.
- Source-span deduplication semantics from 8.2.2 (one record per distinct
  recovered call-site span, callee, and HIR-local id) are preserved. A "call"
  in every threshold and message means a distinct source call site, not a
  runtime invocation per generated case.
- Fluent `.ftl` resources for this lint are owned by roadmap item 8.5.2. This
  plan fixes the message key, attribute names, and argument names, resolves
  them through `safe_resolve_message_set`, and supplies English fallback text.
  It does not add `.ftl` files.
- The full UI pass/fail matrix is owned by roadmap item 8.2.4. This plan adds
  only the fixtures needed to prove emission, message variants, and lint-level
  attribution end to end.
- Comments and documentation use en-GB Oxford spelling (`-ize`, `-yse`,
  `-our`). Markdown wraps at 80 columns; code blocks at 120.
- Formal-verification sidecars follow the existing repository convention
  (`docs/developers-guide.md` §Verus scope and trust boundary): Verus proves an
  implementation-shaped model; executable tests tie the production code to the
  same specification. No `assume`, `admit`, `#[verifier::external_body]`, or
  equivalent shortcut may appear in a committed proof.

## Tolerances (exception triggers)

ExecPlan files are excluded from every size count below. Count only
non-ExecPlan files towards file-count limits, and count code plus other
documentation (including Verus sidecars and `.stderr` fixtures) towards line
limits.

- Scope: if the whole implementation needs more than 40 non-ExecPlan files or
  more than 3,000 net added lines, stop and escalate. If EP-M1 alone needs more
  than 900 net lines, stop and escalate, because that signals the expansion
  repair is larger than a prerequisite and should become its own roadmap item.
- Interface: if any existing public item in `whitaker_common`,
  `whitaker_lint_core`, or `whitaker_suite` must change signature or
  semantics (as opposed to gaining new items), stop and escalate. In
  particular, `collect_rstest_companion_test_functions` must keep its current
  semantics because other lints consume it.
- Dependencies: any dependency outside `[workspace.dependencies]` triggers
  escalation.
- Iterations: if a gate still fails after three fix attempts for the same root
  cause, stop and escalate with the logs.
- Verification: if a Verus obligation in `Verification plan` cannot be
  discharged after a bounded effort (two working sessions) without weakening
  the statement, stop and escalate with the open verification condition. Do
  not weaken a theorem statement silently.
- Time: if a milestone takes more than two working days, stop and escalate.
- Ambiguity: if EP-M0 shows that rstest expansion shapes differ materially from
  those recorded here (for example, a newer `rstest` emits parameter attributes
  or a different nesting), stop and present options before EP-M1.

## Risks

- Risk: provider-parameter recovery from source text (EP-M1) is fragile for
  tests generated by user macros, raw strings in attribute arguments, or
  comments between attributes. Severity: medium. Likelihood: medium.
  Mitigation: lex rather than pattern-match; treat any parameter whose source
  cannot be recovered as `Unsupported` (which can only suppress a diagnostic,
  never cause one); cover comments, raw strings, nested brackets, and
  multi-line attributes in the prototype corpus.
- Risk: `#[expect(rstest_helper_should_be_fixture)]` on a helper might be
  reported as unfulfilled when emission happens in `check_crate_post`.
  Severity: medium. Likelihood: low. Mitigation: the pinned rustc
  `TyCtxt::emit_node_span_lint` resolves the level through
  `lint_level_spec_at_node`, and Clippy's `unused_async` relies on exactly this
  path from `check_crate_post`; EP-M4 pins the behaviour with a UI fixture
  whose expected `.stderr` contains no `unfulfilled_lint_expectations`.
- Risk: the existing `collection_zero_diagnostic` example starts emitting once
  thresholds apply, especially after EP-M1 makes plain `#[rstest]` tests
  visible. Severity: low. Likelihood: high. Mitigation: EP-M1 updates the
  harness summary expectations deliberately; EP-M4 either keeps the example
  silent by construction (each helper must still fail a qualification rule)
  or renames the example to reflect a blessed diagnostic. Never silence it by
  configuration.
- Risk: helper callees that cannot carry `#[fixture]` (trait-impl methods such
  as `Default::default` for a local type, trait default methods, or functions
  that are already rstest fixtures) produce misleading advice. Severity:
  medium. Likelihood: high in real code. Mitigation: EP-M4 excludes these
  callees at aggregation time and records each exclusion with a `debug!`
  reason; see `Decision log`.
- Risk: lint-crate unit tests under `src/*_tests.rs` are not compiled by the
  gates because the library target has `test = false`. Severity: medium.
  Likelihood: confirmed by metadata, to be confirmed by `cargo nextest list`
  in EP-M0. Mitigation: place all pure logic with unit, property, and BDD
  tests in `whitaker_common`, and verify lint-crate behaviour through the
  `tests/ui.rs` integration harness, which does run. Report the pre-existing
  gap to the user; do not repair unrelated crates in this item.
- Risk: the shared compile admission queue slows Cargo-backed UI tests and
  Verus runs. Severity: low. Likelihood: high. Mitigation: start gates once,
  let `build-limits` queue them, and check `build-limits-status` before
  blaming the candidate.
- Risk: ADR serial collision. ADR `0012` already exists twice across branches
  and `0013` is taken on the unmerged branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`.
  Severity: low. Likelihood: medium. Mitigation: re-fetch all remote branches
  and re-scan `docs/adrs/` immediately before allocating; `0014` is the
  current next free serial.

## Progress

- [x] (2026-11-11) Renamed branch to `8-2-3-crate-post-aggregation-thresholds`.
- [x] (2026-11-11) Reconnaissance with a Wyvern agent team: lint crate state,
  shared helpers and diagnostics, verification tooling, and documentation.
- [x] (2026-11-11) Confirmed on the pinned nightly that
  `TyCtxt::emit_node_span_lint` exists and accepts `DiagDecorator`; confirmed
  upstream Clippy's `span_lint_hir_and_then` body and the `unused_async`
  crate-post precedent.
- [x] (2026-11-11) Dumped expanded HIR for the collection example and
  confirmed both 8.2.2 evidence defects.
- [x] (2026-11-11) Drafted this ExecPlan.
- [ ] Community-of-experts design review and revision.
- [ ] Plan approval by the user.
- [ ] EP-M0: baseline and expansion-shape characterization.
- [ ] EP-M1: repair `rstest` expansion evidence.
- [ ] EP-M2: pure helper-group qualification kernel.
- [ ] EP-M3: Verus proof of the qualification model.
- [ ] EP-M4: shared emission helper and crate-post diagnostics.
- [ ] EP-M5: documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: plain `#[rstest]` tests contribute no evidence. Evidence: the
  `--test -Zunpretty=hir` dump of
  `crates/rstest_helper_should_be_fixture/examples/collection_zero_diagnostic.rs`
  shows rstest emitting an outer harness function
  `fn rstest_helper_call_collection_stays_silent()` whose body declares a
  nested inner function of the same name holding the original body, then
  calls it with fixtures resolved by `fixture::default()`. Neither function is
  in `collect_rstest_companion_test_functions`, and the inner function has no
  attributes, so `check_fn` skips both. The harness summary agrees:
  `callee=helper;records=2`, both from the case-driven test. Impact: lint A
  would be silent for the most common test shape. Repaired in EP-M1.
- Observation: `#[case]` (and other provider) parameters are classified as
  fixture locals. Evidence: in the same dump, the case-driven original
  `fn case_generated_collection_stays_silent(input: &str, fixture: &str)` has
  no parameter attributes; rstest strips `#[case]` when it re-emits tokens.
  The harness expects
  `fingerprint=fixture-local,fixture-local,const-path,const-path` for
  `helper(fixture, input, PREFIX, SUFFIX)`. Impact: false positives for
  helpers that receive per-case data. Repaired in EP-M1.
- Observation: `span_lint_hir_and_then` is not available. Evidence:
  `crates/clippy_utils/src/lib.rs` exposes only `macros::is_panic`; every lint
  emits through `cx.emit_span_lint(LINT, span, DiagDecorator(...))`, which
  attributes lint levels to the node being checked, not to an arbitrary
  `HirId`. Impact: EP-M4 adds a repository-owned helper.
- Observation: the lint crate's own unit tests are probably dead. Evidence:
  `cargo metadata` reports the `rstest_helper_should_be_fixture` library target
  with `test = false`, and the only `test = true` target is `tests/ui.rs`; the
  Makefile has no `--lib` run for this package. Impact: the 8.2.2 proptests in
  `src/collector_tests.rs` and `src/driver_tests.rs` do not run under
  `make test`. EP-M0 confirms; this plan keeps new testable logic out of the
  lint crate's `src/` tests.
- Observation: ADR `0013` is already used on an unmerged branch. Evidence: a
  scan of every `refs/remotes/origin/*` tree. Impact: the ADR for this item
  takes `0014` unless a re-scan finds otherwise.

## Decision log

- Decision: emit one diagnostic per qualifying helper, anchored at the helper
  definition. The lint-level `HirId` is
  `cx.tcx.local_def_id_to_hir_id(callee_local_def_id)`; the primary span is
  the helper's identifier span (`cx.tcx.def_ident_span`), recovered with
  `whitaker_lint_core::hir::recover_user_editable_hir_span`; call sites appear
  in a single multi-span note. Rationale: the remedy acts on the helper, so
  one diagnostic per helper avoids N duplicate warnings, and attributing the
  level to the helper lets users suppress or expect the lint where they would
  fix it. This mirrors Clippy's `unused_async`, which collects in `check_fn`
  and emits in `check_crate_post` at `local_def_id_to_hir_id(def_id)`.
  Alternative rejected: emitting at each call site, which multiplies warnings
  and makes `#[expect]` placement ambiguous. Date/Author: 2026-11-11, planning
  agent.
- Decision: per-test suppression is a non-goal for this item. An
  `#[allow(rstest_helper_should_be_fixture)]` on a test does not remove that
  test's calls from the counts. Rationale: excluding allowed tests would make
  `#[expect]` on a test permanently unfulfilled (the lint never emits there)
  and would complicate the counting contract. Revisit after 8.2.4 tuning.
  Date/Author: 2026-11-11, planning agent.
- Decision: group qualification is all-or-nothing per callee, as the design
  document specifies ("Every argument list satisfies one of ..."). One call
  with an `Unsupported` atom, a mismatched arity, or a position that mixes
  fixture locals with constants, or that carries differing constants,
  rejects the whole helper. Rationale: precision is the stated priority for
  lint A (design document, Table 1). Alternative recorded for later tuning:
  qualify the largest consistent fingerprint class. Date/Author: 2026-11-11,
  planning agent.
- Decision: `const`, associated `const`, and immutable `static` paths remain
  `ArgAtom::ConstPath`; a path resolving to a `static mut` lowers to
  `ArgAtom::Unsupported`. Rationale: the design document delegated the
  const/static distinction to 8.2.3; immutable statics are stable values a
  fixture can capture, whereas a mutable static is shared mutable state that
  must not be baked into fixture semantics. The change can only suppress
  diagnostics. Date/Author: 2026-11-11, planning agent.
- Decision: the qualification kernel is pure and lives in
  `whitaker_common::rstest::helper_group`. Rationale: the lint crate's library
  tests do not run; `whitaker_common` already owns the pure `rstest` policy
  (8.1.1 to 8.1.3); `common/**` is in the proofs workflow path filter; and lint
  C (8.4.3) needs the same distinct-test thresholds. Date/Author: 2026-11-11,
  planning agent.
- Decision: provide `span_lint_hir_and_then` as a repository-owned helper in a
  new `whitaker_lint_core::diagnostics` module behind the `dylint-driver`
  feature, faithful to upstream Clippy's signature and body except for the
  Clippy documentation link. Rationale: the roadmap names this API; lints B
  and C will also emit from crate-level aggregation; the alternative of an
  upstream `clippy_utils` dependency was declined in 8.2.2. Date/Author:
  2026-11-11, planning agent.
- Decision: repair expansion evidence in this item (EP-M1) rather than leave
  8.2.3 to emit on defective evidence. Rationale: the defects only become
  user-visible once diagnostics exist, and 8.2.3 is the first consumer. If
  EP-M1 breaches its tolerance, it is split into its own roadmap item and
  stacked pull request. Open for the user to overrule at approval.
  Date/Author: 2026-11-11, planning agent.
- Decision: recover provider-parameter attributes from source text by lexing
  the parameter list of the user-written function (option P1), then classify
  with the existing `whitaker_common::rstest::classify_rstest_parameter` and
  the configured `provider_param_attributes`. Option P2, inferring providers
  from the `let` initializers in rstest's generated companion `case_N`
  functions, is the fallback if the P1 prototype fails its acceptance corpus.
  Rationale: P1 honours user configuration and the design document's
  attribute-based rule; P2 depends on rstest internals (`magic_conversion`,
  `fixture::default()`), ignores `provider_param_attributes`, and has no
  companion for plain tests. Date/Author: 2026-11-11, planning agent.
- Decision: the help text says "Provide `<helper>` through a `#[fixture]`"
  rather than "Convert `<helper>` to a `#[fixture]`". Rationale: a helper
  that production code also calls should be wrapped by a fixture, not
  converted; the neutral wording is correct for both cases. Date/Author:
  2026-11-11, planning agent.
- Decision: no new ADR is drafted in the plan phase; EP-M5 writes
  `docs/adrs/adr-0014-rstest-helper-aggregation-and-emission-anchoring.md`
  (serial confirmed by re-scan) covering anchoring, all-or-nothing
  qualification, and source-text provider recovery, because those choices are
  hard to reverse once users write `#[expect]` attributes against them.
  Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and at completion, comparing
results against `Purpose / big picture` and reconciling every discovery with
the documents in `Conformance basis`.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint libraries into a
`rustc` driver; each lint is a "late lint pass", meaning it runs after name
resolution and type checking, so it can see the HIR (the compiler's desugared
syntax tree) and resolved definitions. A `HirId` names one HIR node; a
`DefId` names one definition (a function, a constant, and so on);
`LocalDefId` is a `DefId` known to be in the current crate. Lint levels
(`allow`, `warn`, `deny`, `expect`) are attached to HIR nodes by attributes,
and rustc resolves the level for a diagnostic from the `HirId` it is emitted
at.

`rstest` is a test framework whose `#[rstest]` attribute macro rewrites a test
function before the compiler sees it. A "fixture" is a function marked
`#[fixture]` whose result rstest injects into a test parameter of the same
name. Parameters marked `#[case]`, `#[values]`, `#[files]`, `#[future]`, or
`#[context]` are "providers": they carry per-case data rather than fixtures.
`docs/rust-testing-with-rstest-fixtures.md` explains fixtures, `#[with(...)]`
(line 497) and `#[from(...)]` (line 563).

Lint A's design is `docs/lints-for-rstest-fixtures-and-test-hygiene.md`
§Lint A: call-site fixture extraction (lines 40 to 130), with implementation
decisions for 8.1.1, 8.1.2, 8.2.1, and 8.2.2 at lines 383 to 467.

The lint crate is `crates/rstest_helper_should_be_fixture/`:

- `src/lib.rs` (22 lines) declares `collector`, `driver`, and `visitor` behind
  the `dylint-driver` feature.
- `src/driver.rs` (395 lines) holds `Config` (fields `min_calls`,
  `min_distinct_tests`, `require_identical_fixture_arg_names`,
  `provider_param_attributes`, `use_source_callee_fallback`; the first two are
  normalized to at least 2), the pass struct `RstestHelperShouldBeFixture`
  (fields `config`, `detection_options`, `collector`,
  `rstest_collection_roots`, `localizer`), and the hooks `check_crate`,
  `check_fn`, and `check_crate_post`. `check_crate_post` currently only
  finalizes the collector, logs, and writes an environment-gated summary
  (`WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY`) used by the UI harness.
- `src/collector.rs` (383 lines) holds `CallSiteRecord { callee_def_id,
  fingerprint, test_source_def_id, span, hir_local_id }`,
  `CallSiteLocation` (the deduplication key), `CallSiteCollector
  { by_callee: BTreeMap<String, Vec<CallSiteRecord>>, seen }`,
  `lower_arg_atom`, and `resolve_local_callee`. `finalize()` sorts each bucket
  by `(span.lo, span.hi, hir_local_id)`.
- `src/visitor.rs` (240 lines) holds `CallSiteVisitor` and the fixture-local
  classification of test parameters.
- `tests/ui.rs` is the Cargo-backed UI harness. It compiles `examples/*.rs`
  through `dylint_testing::ui::Test::example` with `--test`, under a
  cross-process lock in `tests/ui/harness_lock.rs`, and compares any
  diagnostics with an adjacent `.stderr` file.

Shared crates:

- `common/` (package `whitaker-common`) owns pure policy:
  `common/src/rstest/` (`detection.rs`, `parameter.rs`, `span.rs`,
  `argument_fingerprint.rs`, `paragraph_fingerprint.rs`) and localization in
  `common/src/i18n/` (`Localizer`, `get_localizer_for_lint`,
  `safe_resolve_message_set`, `DiagnosticMessageSet`, `MessageKey`,
  `Arguments`).
- `crates/whitaker_lint_core/` owns rustc-aware helpers behind the
  `dylint-driver` feature: `hir::recover_user_editable_hir_span`,
  `hir::collect_rstest_companion_test_functions`, and others.
- `suite/src/driver.rs` combines every lint into one pass with
  `late_lint_methods!(declare_combined_late_lint_pass, ...)`, which forwards
  every `LateLintPass` method, including `check_crate_post`.

Verification tooling: Verus sidecars live in `verus/*.rs` and run through
`scripts/run-verus.sh` (groups selected by name) and `make verus*`;
`.github/workflows/proofs.yml` runs them on pull requests touching `verus/**`
or `common/**`. Kani harnesses live in `#[cfg(kani)]` modules and run through
`scripts/run-kani.sh`. See `docs/developers-guide.md` §Proof workflows (line
2336) and §Verus scope and trust boundary (line 2592).

## Conformance basis

Upstream artefacts at the plan's base commit `d9eb8a09`:

- Terms of Reference: none exists for this lint family; the roadmap entry is
  the governing commitment.
- Roadmap: `docs/roadmap.md` §8.2, item 8.2.3 (lines 634 to 637); neighbours
  8.2.4 (UI matrix), 8.5.2 (Fluent), and 8.5.3 (user documentation).
- Technical design: `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.
  Traced design elements:
  - `LA-TRIG`: trigger conditions (lines 51 to 63).
  - `LA-DET`: conservative `#[rstest]` detection (lines 65 to 71).
  - `LA-FIX`: fixture-local classification and provider attributes (lines 73
    to 82).
  - `LA-FP`: argument fingerprint model and rules (lines 84 to 107).
  - `LA-DIAG`: diagnostic strategy using `span_lint_hir_and_then` with the
    best user-editable span, primary, note, and help (lines 109 to 119).
  - `LA-CFG`: configuration keys (lines 121 to 130).
  - `INT-LATE` and `INT-SPAN`: late-lint and user-editable span constraints
    (lines 28 to 38).
  - `INT-EXP`: experimental-set rollout (line 37).
- Suite design: `docs/whitaker-dylint-suite-design.md` §Localization
  infrastructure (lines 171 to 254) as `SUITE-L10N`.
- ADRs: none govern this lint today. EP-M5 adds ADR `0014`.
- Standards: `AGENTS.md`; `docs/documentation-style-guide.md`;
  `docs/developers-guide.md` §Shared diagnostic localization (lines 143 to
  152), §Proof workflows, §Kani bounded model checking.

Trace links:

```plaintext
LA-TRIG  -> EP-M2 kernel evaluate_helper_group     -> common helper_group unit tables, proptest oracle, BDD; Verus L-REF
LA-TRIG  -> EP-M4 crate-post aggregation          -> examples/aggregation_diagnostics.rs + .stderr
LA-DET   -> EP-M1 plain-rstest inner-body roots   -> example_harness_collects_call_site_evidence (plain-test records)
LA-FIX   -> EP-M1 provider recovery               -> provider corpus tests; harness fingerprint expectations
LA-FP    -> EP-M1 static mut lowering             -> harness fingerprint expectations
LA-DIAG  -> EP-M4 span_lint_hir_and_then + messages -> .stderr fixture; allow/expect attribution fixture
LA-CFG   -> EP-M2 HelperThresholds; EP-M4 Config wiring -> threshold unit tables; config-variant UI case
SUITE-L10N -> EP-M4 message key/args + fallback   -> .stderr fixture (fallback English); 8.5.2 adds .ftl
INT-SPAN -> EP-M4 macro-only helper exclusion     -> unit/UI case for macro-generated helper
INT-EXP  -> unchanged feature gating              -> suite registration tests remain green
```

## Verification plan

### Abstract model

The qualification contract is specified independently of rustc. A group is a
finite sequence of calls to one helper. Each call has a test identity and an
argument list of atoms, where an atom is `Fixture(name)`, `Const(value)`, or
`Unsupported`. Literal text and constant definition paths are modelled as
`Const` values in disjoint namespaces, so a literal never equals a path. The
thresholds are natural numbers `m` (minimum calls) and `k` (minimum distinct
tests) and a flag `strict` (`require_identical_fixture_arg_names`).

```plaintext
eligible(c)            = every atom of c is Fixture(_) or Const(_)
same_arity(G)          = all calls in G have the same argument count n
consistent_at(G, p)    = (every call has Fixture(_) at p
                          and (strict implies all those names are equal))
                         or (there is v such that every call has Const(v) at p)
distinct_tests(G)      = cardinality of the set of test identities in G
qualifies(G, m, k, s)  = G is non-empty
                         and every call is eligible
                         and same_arity(G)
                         and consistent_at(G, p) for every p < n
                         and |G| >= m
                         and distinct_tests(G) >= k
```

The production verdict is `Qualifies(summary)` exactly when `qualifies`
holds, where the summary reports `call_count = |G|`,
`distinct_test_count = distinct_tests(G)`, and one `PositionShape` per
position (`Fixture { shared_name }` with `shared_name` present only when every
call uses the same name, or `Constant(atom)`). Otherwise it is `Rejected`
with the first failing rule in this fixed precedence: `Empty`,
`UnsupportedArgument { position }` (least such position over all calls),
`ArityMismatch { min, max }`, `InconsistentPosition { position }` (least such
position), `BelowMinCalls { found, required }`,
`BelowMinDistinctTests { found, required }`. Every rejection field is a
function of the multiset of calls, so the whole verdict is order-independent.

Representation relation: production `ArgAtom::FixtureLocal { name }` maps to
`Fixture(intern(name))`; `ConstLit { text }` maps to
`Const(intern("lit:" + text))`; `ConstPath { def_path }` maps to
`Const(intern("path:" + def_path))`; `Unsupported` maps to `Unsupported`.
`intern` is injective, so string equality coincides with model equality.

### Obligations

- Obligation `INV-ORDER` (order independence): for any two groups that are
  permutations of each other, the verdicts are equal.
  Method: Verus lemma `L-PERM` over the spec; `proptest` property on the
  production kernel. Rationale: the claim quantifies over unbounded
  sequences; the production `BTreeMap` iteration and `finalize()` sort already
  give a canonical order, but the kernel must not depend on it. Domain: all
  finite sequences (Verus); generated groups of up to 12 calls, arity 0 to 4,
  up to 4 tests, with atoms drawn from small name and value pools so
  collisions occur (proptest). Artefacts: `verus/rstest_helper_group.rs`,
  `common/src/rstest/tests/helper_group_properties.rs`. Evidence:
  `make verus-rstest-helper`; `cargo nextest run -p whitaker-common
  helper_group`. Non-vacuity: the generator's classification counters must
  show both `Qualifies` and every `Rejected` variant at least 5% of cases
  each; seeded fault "report the first call's unsupported position instead of
  the least position" must fail the property.
- Obligation `L-REF` (refinement and soundness): the executable algorithm
  returns `Qualifies(s)` if and only if `qualifies(G, m, k, strict)`, and then
  `s.call_count == |G|`, `s.distinct_test_count == distinct_tests(G)`, and
  every call conforms to `s.positions`. Method: Verus proof that the
  implementation-shaped single-pass model in the sidecar refines the spec;
  `proptest` differential test that the production kernel agrees with a
  quantifier-style reference oracle that transliterates the spec. Rationale:
  this is the lint's contractual trigger; a false `Qualifies` is a false
  positive shipped to users. Domain: as above. Evidence: Verus reports the
  file verified with zero errors; proptest passes 1,024 cases by default.
  Non-vacuity: Verus witness lemmas `L-WITNESS-YES` (two calls, two tests,
  no arguments, `m = k = 2`, qualifies) and `L-WITNESS-NO` (three calls, one
  test, `k = 2`, does not qualify) must verify, proving the antecedents are
  inhabited; seeded faults "count calls instead of distinct tests", "ignore
  `Unsupported`", and "`>` instead of `>=`" must each fail the differential
  test or a unit-table row.
- Obligation `L-MONO` (threshold monotonicity): if `qualifies(G, m, k, s)`
  and `m' <= m` and `k' <= k`, then `qualifies(G, m', k', s)`; and
  `qualifies(G, m, k, true)` implies `qualifies(G, m, k, false)`. Method:
  Verus lemma; `proptest` property on production. Rationale: users tune
  thresholds; lowering a threshold or relaxing strictness must never hide an
  existing diagnostic. Non-vacuity: generator must reach groups that qualify
  at the higher thresholds.
- Obligation `L-EXT` (stable growth): if `qualifies(G, m, k, s)` and call `c`
  conforms to the summary shape of `G`, then
  `qualifies(G + [c], m, k, s)`. Method: Verus lemma; `proptest` property.
  Rationale: adding another conforming test must not make an existing
  diagnostic disappear. Non-vacuity: generator constructs conforming
  extensions from the summary shape rather than filtering random calls.
- Obligation `L-BOUND`: `distinct_tests(G) <= |G|`. Method: Verus lemma
  (used by `L-REF`); no separate test. Rationale: explains why `k > m`
  behaves as `k` alone and keeps message arithmetic honest.
- Obligation `EMIT-ATTR` (lint-level attribution): a diagnostic for helper
  `h` is controlled by lint attributes on `h` and its enclosing items, and
  `#[expect]` on `h` is fulfilled. Method: Cargo-backed UI fixture with
  byte-exact `.stderr`. Rationale: this is integration behaviour of rustc's
  lint-level machinery, which cannot be modelled meaningfully; the real
  driver is the only faithful boundary. Non-vacuity: the same fixture
  contains an unannotated twin helper that must emit, so a silent driver
  cannot pass.
- Obligation `EMIT-ONCE`: exactly one diagnostic per qualifying helper, in
  ascending callee-key order, with call-site notes in ascending source order
  and at most `MAX_NOTED_CALL_SITES` (3) spans. Method: UI fixture with two
  qualifying helpers and one helper with four call sites. Non-vacuity: the
  fixture has more call sites than the cap.
- Obligation `EVID-PLAIN` and `EVID-PROVIDER` (EP-M1 repairs): plain
  `#[rstest]` bodies are collected exactly once per call site, and provider
  parameters lower to `Unsupported`. Method: harness summary assertions plus
  a parameterized corpus of parameter-list sources for the recovery lexer
  (whichever crate hosts it must run its tests; see EP-M1). Non-vacuity: the
  red step must show the current wrong summary (`records=2` for `helper`,
  `fixture-local` for `input`).
- Message wording, Fluent argument mapping, and fallback text carry no proof
  obligation. They are pinned by the `.stderr` fixture and by an `insta`
  snapshot of the resolved message sets for each argument-shape variant.

### Where Verus is and is not warranted

Verus is warranted for the qualification predicate: it is the lint's
contractual business logic, its key properties (`INV-ORDER`, `L-REF`, `L-MONO`,
`L-EXT`) quantify over unbounded groups, and a small pure kernel exists. Kani
is not used: the production kernel manipulates `String` and `Vec` values,
which `docs/developers-guide.md` and `scripts/run-kani.sh` record as prone to
time-outs, and the bounded guarantee would add little beyond the Verus
model plus the differential property test. Verus is not used for span
recovery, provider-attribute lexing, callee exclusion, or message rendering:
those are integration behaviours whose correctness depends on rustc and
source text, so example corpora and UI fixtures give the appropriate
assurance.

### Axioms

- `A-LEVEL`: `TyCtxt::emit_node_span_lint(lint, hir_id, span, decorator)` on
  `nightly-2026-05-28` resolves the level with
  `lint_level_spec_at_node(lint, hir_id)` and records `#[expect]`
  fulfilment. Evidence: pinned rustc source
  `compiler/rustc_middle/src/ty/context.rs:2461`; exercised by `EMIT-ATTR`.
- `A-COMBINED`: `declare_combined_late_lint_pass` forwards
  `check_crate_post` to every constituent pass. Evidence:
  `suite/src/driver.rs`; exercised by the existing suite registration tests
  and by running the example through the lint driver.
- `A-EXPAND`: rstest 0.28 expands a plain test into an outer harness function
  containing a same-named nested function, and a case-driven test into a
  same-named function plus a companion module, stripping parameter
  attributes in both. Evidence: EP-M0 HIR dumps, committed as excerpts in
  `Artefacts and notes`.
- `A-SPAN`: `SourceMap::span_to_snippet` returns the user-written text for a
  user-editable span. Evidence: existing `literal_atom` use in
  `collector.rs`.
- `A-INTERN`: the Verus model's `nat` identities stand for strings under an
  injective map; exercised by the differential test, which uses real strings.

## Plan of work

### Stage A: understand and propose (EP-M0, no production code)

Record the baseline. Run the three gates on the untouched branch and keep the
logs. Confirm whether the lint crate's `src/*_tests.rs` run with
`cargo nextest list -p rstest_helper_should_be_fixture --all-features` and
record the answer in `Surprises & discoveries`.

Characterize expansion shapes. Write a throwaway example (not committed)
containing a plain `#[rstest]`, a `#[case]` test, a `#[values]` matrix, a test
with `#[with(...)]` and `#[from(...)]` fixtures, an `async` `#[rstest]`, a
`#[rstest]` inside a nested module, and a plain `#[test]`. Dump its HIR with
`cargo rustc -p rstest_helper_should_be_fixture --example <name> -- --test
-Zunpretty=hir` and record, per shape, which functions hold the user body,
how parameters appear, and which existing roots match. Go/no-go: proceed to
EP-M1 only if the shapes match `A-EXPAND`; otherwise escalate per
`Tolerances`.

### Stage B, C, D per milestone

Each implementation milestone follows Red-Green-Refactor: the smallest
failing test first, then the smallest implementation, then a cleanup pass,
then the three gates. Milestones are listed in `Milestones and plateaus`.

## Milestones and plateaus

### EP-M0: baseline and characterization

- Outcome: recorded baseline gate logs, confirmed test-target facts, and the
  expansion-shape table in `Artefacts and notes`. No tracked source changes
  except this plan.
- Requirements and gaps: prerequisite evidence for `LA-DET` and `LA-FIX`.
- Acceptance evidence: plan updated with the shape table and the
  `cargo nextest list` result.
- Conformance check:
  - No requirement is discharged; evidence only.
  - Design unchanged.
  - `A-EXPAND` confirmed or escalated.
  - No interface or dependency change.
  - No trust-boundary or persisted-format change.
  - Trace links unchanged.
- Recovery: re-run the dumps; nothing to revert.
- Remaining gaps: all implementation.
- Compatibility decision: none.

### EP-M1: repair `rstest` expansion evidence

- Outcome: the collector sees plain `#[rstest]` bodies and classifies provider
  parameters as non-fixtures; `static mut` paths lower to `Unsupported`. The
  lint is still diagnostic-silent at the end of this milestone.
- Work:
  1. Red. Update `example_harness_collects_call_site_evidence` in
     `crates/rstest_helper_should_be_fixture/tests/ui.rs` to expect the plain
     test's three `helper` and `Builder::build` call sites, and
     `unsupported` (not `fixture-local`) in the position of `#[case] input`.
     Add a `static mut` argument call to the example and expect
     `unsupported` there. Run the test and observe the expected failure.
  2. Prototype (label: prototyping). Implement provider-attribute recovery
     option P1: given a function whose parameters have user-editable spans,
     take the source text from the opening parenthesis of the parameter list
     to each parameter pattern, lex it with the workspace `rustc_lexer`
     dependency, collect the outer attribute paths that precede each
     parameter, and pass them to `classify_rstest_parameter` with the
     configured `RstestDetectionOptions`. Acceptance corpus (parameterized
     `rstest` table): bare and qualified attributes, attributes with
     arguments containing `]`, `,` and string literals, raw strings, line and
     block comments between attributes, multi-line signatures, `mut`
     bindings, and a parameter with no recoverable source. Keep P1 if every
     row passes; otherwise prototype P2 and record the decision.
  3. Green. Add a new function
     `whitaker_lint_core::hir::collect_rstest_inner_test_bodies(cx) ->
     HashSet<hir::HirId>` that finds, for each harness-test function, a
     same-named nested function item that the outer body calls. Lints B and
     C need the same roots, which is why it is shared; the existing
     `collect_rstest_companion_test_functions` is not modified. Union the two
     sets in `check_crate`. Host the pure lexing policy in
     `whitaker_common::rstest` (for example `parameter_source.rs`) so its
     corpus runs under `make test`, with a thin rustc adapter in
     `whitaker_lint_core::hir`. Lower `static mut` paths to `Unsupported` in
     `collector.rs::lower_path_arg`.
  4. Refactor. Keep every touched file under 400 lines.
- Requirements and gaps: `LA-DET`, `LA-FIX`, `LA-FP`.
- Acceptance evidence: the updated harness test passes; the corpus table
  passes under `make test`; `make check-fmt`, `make lint`, `make test` pass.
- Conformance check:
  - `LA-DET`, `LA-FIX` satisfied for the shapes in the EP-M0 table.
  - Design followed; `classify_rstest_parameter` reused.
  - `A-EXPAND` still valid.
  - New public items only: `collect_rstest_inner_test_bodies`, the pure
    parameter-source helper; reuse policy documented in EP-M5.
  - No persisted-format change.
  - Trace links `LA-DET` and `LA-FIX` now point at real tests.
- Recovery: revert the milestone commits; the lint returns to 8.2.2
  behaviour.
- Remaining gaps: aggregation and diagnostics.
- Compatibility decision: none (private and pre-1.0 surfaces).

### EP-M2: pure helper-group qualification kernel

- Outcome: `whitaker_common::rstest::helper_group` implements the abstract
  model with production types, fully tested.
- Work:
  1. Red. Add `common/src/rstest/tests/helper_group_tests.rs` with a
     parameterized `rstest` table (googletest matchers, `pretty_assertions`
     for structural equality) covering: empty group; no-argument helper at
     and below each threshold; three calls in one test; fixture-only
     positions with equal and differing names under both `strict` values;
     equal and differing `ConstLit`; `ConstLit` against `ConstPath` with the
     same text; mixed fixture and constant at one position; `Unsupported` at
     several positions (least position reported); arity mismatch; the exact
     rejection precedence when several rules fail. Add
     `common/tests/features/rstest_helper_group.feature` with the scenarios
     in `Validation and acceptance`, bound in
     `common/tests/rstest_helper_group_behaviour.rs`. Run and observe
     compile failure because the module does not exist.
  2. Green. Implement the types and `evaluate_helper_group` (see
     `Interfaces and dependencies`) as a single pass that tracks the least
     unsupported position, minimum and maximum arity, per-position shape
     state, and a `BTreeSet` of test identities, then applies the precedence.
  3. Properties. Add `common/src/rstest/tests/helper_group_properties.rs`
     with the `proptest` properties for `INV-ORDER`, `L-REF` (against the
     reference oracle written in the same file as a direct transliteration of
     `qualifies`), `L-MONO`, and `L-EXT`, plus generator classification.
  4. Snapshot. Add an `insta` snapshot of the verdicts for a fixed corpus of
     named groups, rendered as one line per group, so reviewers can read the
     contract in one place.
  5. Seeded faults. Temporarily apply each seeded fault named in
     `Verification plan`, confirm the expected test fails, and revert. Record
     the observations in `Artefacts and notes`. Do not commit the faults.
- Requirements and gaps: `LA-TRIG`, `LA-CFG` (threshold semantics).
- Acceptance evidence: `cargo nextest run -p whitaker-common helper_group`
  passes; the BDD scenarios pass; three gates pass.
- Conformance check:
  - `LA-TRIG` semantics implemented as specified, including all-or-nothing
    qualification.
  - Design followed.
  - No upstream assumption falsified.
  - New public module only; reuse policy recorded in EP-M5.
  - No persisted-format change.
  - Trace links current.
- Recovery: the module is additive; revert its commits.
- Remaining gaps: proof, wiring, and diagnostics.
- Compatibility decision: none.

### EP-M3: Verus proof of the qualification model

- Outcome: `verus/rstest_helper_group.rs` proves `L-REF`, `L-PERM`
  (for `INV-ORDER`), `L-MONO`, `L-EXT`, `L-BOUND`, and the two witness lemmas
  for the implementation-shaped model, with no trusted shortcuts.
- Work:
  1. Write the spec functions of `Abstract model` and the executable model
     mirroring `evaluate_helper_group`'s branch order. State lemmas first;
     let the verifier's open conditions drive auxiliary lemmas (multiset and
     set cardinality helpers from `vstd`).
  2. Register a new group `rstest-helper` in `scripts/run-verus.sh` (in the
     `all` case, in `proof_files_for_group`, and in the accepted-group case)
     and a Makefile target `verus-rstest-helper`. `proofs.yml` already
     triggers on `verus/**`, `common/**`, the scripts, and the Makefile.
  3. Negative control. Locally break the executable model (count calls as
     tests) and confirm Verus rejects `L-REF`; revert and record the output.
  4. Update `docs/developers-guide.md` §Verus scope and trust boundary to
     list the new sidecar and what it does and does not cover.
- Requirements and gaps: `LA-TRIG` (formal assurance).
- Acceptance evidence: `make verus-rstest-helper` reports the file verified
  with zero errors; `make verus` passes; three gates pass.
- Conformance check:
  - Obligations discharged as stated; none weakened.
  - Model mirrors production branch order (reviewed side by side).
  - No `assume`, `admit`, or external bodies (`grep` returns nothing).
  - No interface or dependency change.
  - No persisted-format change.
  - Trace links current.
- Recovery: the sidecar is additive; revert its commits.
- Remaining gaps: wiring and diagnostics.
- Compatibility decision: none.

### EP-M4: shared emission helper and crate-post diagnostics

- Outcome: the lint emits one localizable diagnostic per qualifying helper,
  attributed to the helper's `HirId`.
- Work:
  1. Refactor first (separate commit). Move the collection-summary support
     (`collection_summary`, `write_collection_summary`, and their helpers)
     from `src/driver.rs` into `src/summary.rs` with no behaviour change, so
     `driver.rs` has room.
  2. Red. Add `examples/aggregation_diagnostics.rs` and its expected
     `examples/aggregation_diagnostics.stderr` to the lint crate, and a case
     for it in a new `example_reports_expected_diagnostics` test in
     `tests/ui.rs`. The example contains: a no-argument helper called from two
     plain `#[rstest]` tests; a helper called with one fixture local and one
     `const` from two tests; a helper with four call sites (exercising the
     note cap); a helper annotated `#[allow(rstest_helper_should_be_fixture)]`;
     a helper annotated `#[expect(rstest_helper_should_be_fixture)]`; a
     helper called three times from one test; a helper whose second call
     passes a `#[case]` value; a `Default::default` call on a local type; and
     a direct call to an existing `#[fixture]` function. Scope
     `#![allow(unknown_lints)]` narrowly as other lint fixtures do. Run and
     observe the missing diagnostics.
  3. Green. Add `whitaker_lint_core::diagnostics::span_lint_hir_and_then`.
     Add `src/aggregate.rs` in the lint crate: iterate `collector.iter()`
     after `finalize()`, exclude callees that are trait methods, trait-impl
     methods, existing rstest fixtures, or whose definition span is
     macro-only (each exclusion logged at `debug!` with its reason), map each
     record to `HelperCallEvidence` with a test identity derived from
     `test_source_def_id`, and evaluate with `HelperThresholds` built from
     `Config`. Add `src/diagnostic.rs`: build `Arguments` (`helper`, `calls`,
     `tests`, `argument_shape`), resolve with `safe_resolve_message_set` and
     English fallbacks, and emit with `span_lint_hir_and_then`: primary at the
     helper identifier, one `span_note` over the first three call-site spans
     carrying the counts note, and one `help`. Call aggregation and emission
     from `check_crate_post` after the existing summary step.
  4. Confirm `bootstrap_zero_diagnostic` and `collection_zero_diagnostic`
     remain silent. If `collection_zero_diagnostic` now emits, decide per
     `Risks` and record the decision; do not change thresholds to hide it.
  5. Add the message-set `insta` snapshot. Because the lint crate's library
     tests do not run, place the argument-shape-to-message mapping in a small
     pure module that `tests/ui.rs` (or a new integration test file) can
     exercise, or snapshot the rendered `.stderr` variants only; record the
     choice.
  6. Refactor and gate.
- Requirements and gaps: `LA-DIAG`, `LA-TRIG` (wiring), `LA-CFG`,
  `SUITE-L10N`, `INT-SPAN`.
- Acceptance evidence: the new UI test passes with byte-exact `.stderr`;
  `EMIT-ATTR` and `EMIT-ONCE` observed in that file; three gates pass.
- Conformance check:
  - `LA-DIAG` satisfied with `span_lint_hir_and_then` at the helper `HirId`.
  - Design followed, with the documented wording decisions.
  - `A-LEVEL` and `A-COMBINED` confirmed by the fixture.
  - New public item `whitaker_lint_core::diagnostics::span_lint_hir_and_then`
    only.
  - No persisted-format change.
  - Trace links current.
- Recovery: revert the milestone commits; the summary refactor commit can
  stay.
- Remaining gaps: full UI matrix (8.2.4), Fluent files (8.5.2), user-facing
  remediation guide (8.5.3).
- Compatibility decision: none.

### EP-M5: documentation, ADR, and roadmap closure

- Outcome: every document reflects the shipped behaviour and the roadmap entry
  is marked done.
- Work:
  1. `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: add
     `## Implementation decisions (8.2.3)` summarizing the `Decision log`, and
     reference the ADR from §Diagnostic strategy.
  2. `docs/adrs/adr-0014-rstest-helper-aggregation-and-emission-anchoring.md`
     (serial re-confirmed by scanning all remote branches) following the ADR
     template in `docs/documentation-style-guide.md` (lines 524 to 648);
     add it to `docs/contents.md`.
  3. `docs/users-guide.md` §`rstest_helper_should_be_fixture` (line 943):
     replace the "Bootstraps" and "diagnostic-silent" text with the trigger
     contract, the meaning of "call" and "test", the diagnostic shape,
     suppression at the helper, and the excluded callee kinds.
  4. `docs/developers-guide.md`: document
     `whitaker_lint_core::diagnostics::span_lint_hir_and_then` (scope:
     emitting a lint whose level must follow a node other than the one being
     checked; permitted callers: any Dylint lint crate; composition: wrap, do
     not re-implement, lint-specific message resolution), the
     `helper_group` kernel reuse policy (lint C may reuse thresholds; helper
     wording stays in the lint crate), `collect_rstest_inner_test_bodies`,
     and the parameter-source recovery helper. Update §Standard vs
     Experimental Lints (line 5056) and §Verus scope and trust boundary.
  5. `docs/roadmap.md`: mark 8.2.3 `[x]` with a status paragraph in the
     style of 8.2.2.
  6. Run `make fmt`, `make markdownlint`, and `make nixie`.
- Requirements and gaps: documentation requirements from `AGENTS.md`.
- Acceptance evidence: Markdown gates and the three code gates pass.
- Conformance check:
  - Upstream design updated with every decision; ADR referenced.
  - No deviation left unrecorded.
  - Trace links point at final file names.
  - No interface change.
  - No persisted-format change.
  - Plan `Outcomes & retrospective` completed.
- Recovery: documentation-only reverts.
- Remaining gaps: 8.2.4, 8.5.2, 8.5.3.
- Compatibility decision: none.

## Concrete steps

Run every command from the worktree root
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/9f4c7960-0994-4190-a861-391a6cfd4f40`.
Capture gate output with `tee` so truncation does not hide failures:

```bash
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BR}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BR}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BR}.out"
```

Run the gates sequentially, never in parallel for this worktree. Prefer
delegating full gate runs to the `scrutineer` agent and read its cited logs
rather than re-running a gate. If a build prints
`[build-limits] ... waiting for a compile slot`, it is queued, not hung; run
`build-limits-status` to inspect admission.

Focused commands while iterating:

```bash
cargo nextest run -p whitaker-common helper_group 2>&1 | tee /tmp/helper-group-whitaker-${BR}.out
cargo nextest run -p rstest_helper_should_be_fixture --all-features --test ui 2>&1 | tee /tmp/ui-whitaker-${BR}.out
make verus-rstest-helper 2>&1 | tee /tmp/verus-whitaker-${BR}.out
cargo rustc -p rstest_helper_should_be_fixture --example collection_zero_diagnostic -- --test -Zunpretty=hir > /tmp/hir-whitaker-${BR}.out
```

Documentation milestones additionally run:

```bash
make fmt 2>&1 | tee "/tmp/fmt-whitaker-${BR}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BR}.out"
make nixie 2>&1 | tee "/tmp/nixie-whitaker-${BR}.out"
```

Commit after each green step with an imperative subject naming the
milestone, for example `Repair rstest expansion evidence (8.2.3 EP-M1)`.

## Validation and acceptance

The feature is accepted when all of the following hold.

Behaviour, observed through the real driver: compiling
`crates/rstest_helper_should_be_fixture/examples/aggregation_diagnostics.rs`
under the UI harness yields exactly the diagnostics in
`aggregation_diagnostics.stderr`: one warning for each qualifying helper, none
for the allowed, expected, single-test, case-fed, trait-impl, and
existing-fixture helpers, and no `unfulfilled_lint_expectations` warning.

Red-Green-Refactor evidence to record per milestone in `Artefacts and notes`:

- EP-M1 red: `example_harness_collects_call_site_evidence` fails, showing
  `callee=helper;records=2` and `fixture-local` for the case input. Green: the
  same test passes with the plain-test records and `unsupported`.
- EP-M2 red: `cargo nextest run -p whitaker-common helper_group` fails to
  compile (module missing). Green: all table rows, properties, and BDD
  scenarios pass.
- EP-M3: the negative control is rejected by Verus; the committed sidecar
  verifies.
- EP-M4 red: `example_reports_expected_diagnostics` fails with an empty actual
  output against the expected `.stderr`. Green: byte-exact match.

Behaviour-driven specification, file
`common/tests/features/rstest_helper_group.feature`:

```gherkin
Feature: Qualifying repeated rstest helpers as fixture candidates

  Scenario: A no-argument helper called from two tests qualifies
    Given a helper group with calls from tests "a" and "b" without arguments
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper qualifies with 2 calls across 2 tests
    And the argument shape is "none"

  Scenario: Calls concentrated in one test do not qualify
    Given a helper group with 3 calls from test "a" without arguments
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper is rejected as below the distinct-test threshold with 1 of 2

  Scenario: An unsupported argument rejects the whole helper
    Given a helper group where test "a" passes fixture "db"
    And test "b" passes an unsupported argument
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper is rejected for an unsupported argument at position 0

  Scenario: Differing constants at one position do not qualify
    Given a helper group where test "a" passes literal "1" and test "b" passes literal "2"
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper is rejected as inconsistent at position 0

  Scenario: Strict fixture names reject differently named fixtures
    Given a helper group where test "a" passes fixture "db" and test "b" passes fixture "database"
    And thresholds of 2 calls and 2 distinct tests with identical fixture names required
    When the helper group is evaluated
    Then the helper is rejected as inconsistent at position 0

  Scenario: Lenient fixture names accept differently named fixtures
    Given a helper group where test "a" passes fixture "db" and test "b" passes fixture "database"
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper qualifies with 2 calls across 2 tests
    And the argument shape is "fixtures"
```

Step definitions follow `docs/rstest-bdd-users-guide.md`; `given` and `when`
steps return `Result` and never assert, and assertions live in `then` steps.

Quality criteria:

- Tests: `make test` passes, including the new `whitaker-common` unit,
  property, snapshot, and BDD tests and the lint crate UI tests.
- Verification: `make verus-rstest-helper` and `make verus` pass with the
  obligations in `Verification plan` discharged; each seeded fault and the
  Verus negative control were observed to fail.
- Lint and format: `make check-fmt` and `make lint` pass with no new
  suppressions beyond the narrowly scoped `unknown_lints` allowance in the
  example fixture.
- Documentation: `make markdownlint` and `make nixie` pass.

## Idempotence and recovery

Every step is re-runnable. HIR dumps and logs go to `/tmp` and are
overwritten. UI fixtures are compared, never regenerated automatically; to
re-bless a `.stderr` file, copy the actual output from the harness failure
message after reviewing it, and record why in the `Decision log`. If a
milestone fails part-way, `git reset --hard` to the last green milestone
commit only after confirming nothing else is uncommitted; otherwise commit the
partial work as a clearly labelled work-in-progress commit on this branch.
Never use bare `git stash`.

## Artefacts and notes

Expanded HIR excerpt for a plain `#[rstest]` test (rstest 0.28,
`nightly-2026-05-28`):

```plaintext
fn rstest_helper_call_collection_stays_silent() {
    fn rstest_helper_call_collection_stays_silent(fixture: &'_ str) {
        { let value = helper(fixture, "literal", PREFIX, SUFFIX); ... }
    }
    let fixture = fixture::default();
    rstest_helper_call_collection_stays_silent(fixture)
}
```

Expanded HIR excerpt for a case-driven test:

```plaintext
fn case_generated_collection_stays_silent(input: &'_ str, fixture: &'_ str) { ... }
mod case_generated_collection_stays_silent {
    fn case_1() {
        let input = { use rstest::magic_conversion::*; (...).magic_conversion("first") };
        let fixture = fixture::default();
        case_generated_collection_stays_silent(input, fixture)
    }
}
```

Upstream Clippy `span_lint_hir_and_then`, the shape the repository helper
mirrors:

```rust
pub fn span_lint_hir_and_then(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    hir_id: HirId,
    sp: impl Into<MultiSpan>,
    msg: impl Into<DiagMessage>,
    f: impl FnOnce(&mut Diag<'_>),
) {
    cx.tcx.emit_node_span_lint(lint, hir_id, sp, rustc_errors::DiagDecorator(|diag| {
        diag.primary_message(msg);
        f(diag);
        docs_link(diag, lint);
    }));
}
```

## Interfaces and dependencies

In `crates/whitaker_lint_core/src/diagnostics.rs` (new, behind
`dylint-driver`, re-exported from `lib.rs`):

```rust
/// Emit `lint` at `span`, resolving its level from `hir_id`.
#[track_caller]
pub fn span_lint_hir_and_then(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    hir_id: HirId,
    span: impl Into<MultiSpan>,
    msg: impl Into<DiagMessage>,
    decorate: impl FnOnce(&mut Diag<'_, ()>),
)
```

In `common/src/rstest/helper_group.rs` (new, re-exported from
`whitaker_common::rstest` and the crate root like the other `rstest`
models):

```rust
pub struct HelperThresholds { /* min_calls, min_distinct_tests, require_identical_fixture_arg_names */ }
impl HelperThresholds {
    pub const fn new(min_calls: usize, min_distinct_tests: usize, require_identical_fixture_arg_names: bool) -> Self;
}

pub struct HelperCallEvidence<'a, T> {
    pub test: T,
    pub fingerprint: &'a ArgFingerprint,
}

pub enum PositionShape {
    Fixture { shared_name: Option<String> },
    Constant(ArgAtom),
}

pub enum ArgumentShape { NoArguments, FixturesOnly, ConstantsOnly, FixturesAndConstants }

pub struct HelperGroupSummary { /* call_count, distinct_test_count, positions */ }
impl HelperGroupSummary {
    pub fn call_count(&self) -> usize;
    pub fn distinct_test_count(&self) -> usize;
    pub fn positions(&self) -> &[PositionShape];
    pub fn argument_shape(&self) -> ArgumentShape;
}

pub enum HelperGroupRejection {
    Empty,
    UnsupportedArgument { position: usize },
    ArityMismatch { min_arity: usize, max_arity: usize },
    InconsistentPosition { position: usize },
    BelowMinCalls { found: usize, required: usize },
    BelowMinDistinctTests { found: usize, required: usize },
}

pub enum HelperGroupVerdict {
    Qualifies(HelperGroupSummary),
    Rejected(HelperGroupRejection),
}

pub fn evaluate_helper_group<'a, T: Ord>(
    calls: impl IntoIterator<Item = HelperCallEvidence<'a, T>>,
    thresholds: HelperThresholds,
) -> HelperGroupVerdict;
```

In `crates/whitaker_lint_core/src/hir/` (new function; existing functions
unchanged):

```rust
pub fn collect_rstest_inner_test_bodies(cx: &LateContext<'_>) -> HashSet<hir::HirId>;
```

In the lint crate, new private modules `src/summary.rs` (extracted),
`src/aggregate.rs`, and `src/diagnostic.rs`. Message contract for 8.5.2:
message key `rstest_helper_should_be_fixture` with attributes `.note` and
`.help`; arguments `helper` (string), `calls` (integer), `tests` (integer),
and `argument_shape` (one of `none`, `fixtures`, `constants`, `mixed`, for
Fluent selectors). English fallbacks:

```plaintext
primary: Helper `{helper}` behaves like a fixture in `rstest` tests.
note:    Found {calls} calls across {tests} tests, all without arguments.         (none)
         Found {calls} calls across {tests} tests with stable fixture arguments.  (fixtures)
         Found {calls} calls across {tests} tests with stable constant arguments. (constants, mixed)
help:    Provide `{helper}` through a `#[fixture]` and inject it as a test parameter.
         ... and move the repeated constant arguments into the fixture or pass them with `#[with(...)]`. (constants, mixed)
```

Dev-dependencies added (all workspace-pinned): `googletest`,
`pretty_assertions`, and `insta` in `common/Cargo.toml`; `insta` in the lint
crate only if EP-M4 step 5 places snapshots there.

Signposts: `docs/whitaker-dylint-suite-design.md` (localization and suite
wiring), `docs/whitaker-clone-detector-design.md` (precedent for
deterministic grouping and for Verus or Kani sidecars beside pure kernels),
`docs/rust-testing-with-rstest-fixtures.md` (fixture semantics, `#[with]`,
`#[from]`), `docs/rust-doctest-dry-guide.md` (doc examples on new public
items in `whitaker_common`; `ignore` examples for rustc-private items),
`docs/complexity-antipatterns-and-refactoring-strategies.md` (keep
`evaluate_helper_group` and the aggregation adapter free of the Bumpy Road
shape), and `docs/rstest-bdd-users-guide.md` (step definitions). Skills:
`execplans`, `rust-router`, `rust-unit-testing`, `proptest`, `verus`,
`rust-verification`, `nextest`, `arch-decision-records`, `codegraph-mcp`, and
`en-gb-oxendict-style`.

## Revision note

- 2026-11-11: initial draft after Wyvern reconnaissance and HIR
  characterization. Next: community-of-experts review.
