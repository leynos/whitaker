# Implement crate-post aggregation thresholds and actionable diagnostics for `rstest_helper_should_be_fixture` (8.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

Roadmap item: `docs/roadmap.md` §8.2, item 8.2.3. Requires 8.2.2 and 8.1.2
(both marked complete). Implementation must not begin until this plan is
explicitly approved. Several decisions below amend the governing design
document; they are listed together under `Decisions requiring approval` so that
approving the plan approves them explicitly.

## Purpose / big picture

`rstest_helper_should_be_fixture` (lint A) is an experimental Whitaker lint.
Today it walks `#[rstest]` test bodies, records every call to a local helper
function, and fingerprints each call's arguments, but it never reports
anything. This plan makes the lint useful. After `check_crate_post` has seen
every test in the crate being compiled, the lint groups the recorded calls by
helper, decides which helpers behave like fixtures, and emits one actionable
diagnostic per such helper. The diagnostic is attributed to the helper's own
High-level Intermediate Representation (HIR) node, so lint-level attributes on
the helper control it.

After this change, a user who enables the experimental lint and compiles their
tests (`--test`) with:

```rust
#[cfg(test)]
mod tests {
    fn make_db() -> Db { Db::in_memory() }

    #[rstest]
    fn inserts() { let db = make_db(); /* ... */ }

    #[rstest]
    fn deletes() { let db = make_db(); /* ... */ }
}
```

sees a warning of this shape (exact text is fixed in EP-M4 and pinned by a
byte-exact `.stderr` fixture):

```plaintext
warning: Helper `make_db` behaves like a fixture in `rstest` tests.
  --> src/lib.rs:3:8
   |
 3 |     fn make_db() -> Db { Db::in_memory() }
   |        ^^^^^^^
   |
note: Found 2 calls across 2 tests, all without arguments.
  --> src/lib.rs:6:31
   |
 6 |     fn inserts() { let db = make_db(); /* ... */ }
   |                             ^^^^^^^^^
...
   = help: Provide `make_db` through a `#[fixture]` and inject it as a test parameter.
```

`#[allow(rstest_helper_should_be_fixture)]` on `make_db` silences it. Because
counts exist only in compilations that include the tests, the supported way to
*expect* the lint is
`#[cfg_attr(test, expect(rstest_helper_should_be_fixture))]` or an `#[expect]`
placed inside `#[cfg(test)]` code; a bare `#[expect]` on a helper that is also
compiled without `--test` is reported as unfulfilled in that build. Counts are
per compilation unit: a helper in `tests/common/mod.rs` is evaluated separately
in each integration-test binary that includes it.

The plan also repairs two defects in the 8.2.2 evidence that this item depends
on (see `Surprises & discoveries`): plain `#[rstest]` tests are currently not
collected at all, and `#[case]` and other provider parameters are classified as
fixtures after macro expansion. Without those repairs, diagnostics would miss
the most common test shape and would recommend fixtures for helpers that
receive per-case data.

## Decisions requiring approval

Approving this plan approves the following, each detailed in `Decision log` and
recorded in ADR `0014` and the design document during EP-M6:

1. Precision rules beyond the design document's trigger list (design
   amendment `LA-TRIG+`): a helper qualifies only if it is defined in test-only
   code, is not generic, `async`, or `unsafe`, does not return `()` or `!`, is
   not a trait or trait-impl method, and is not an rstest fixture or a
   fixture's generated item; and every call is in setup position (the
   initializer of a `let` binding) and is not a method call whose receiver is a
   fixture local. Every rule can only suppress diagnostics.
2. Fixture-local classification by rstest binding provenance (design
   amendment `LA-FIX+`): a test parameter is a fixture local only if no visible
   provider attribute marks it *and* every rstest-generated caller binds it with
   `<fixture>::default()`. Parameters bound any other way (case data,
   `#[with(...)]` arguments via `get`/`partial_N`, unknown shapes) lower to
   `Unsupported`. `provider_param_attributes` continues to apply to attributes
   that are visible in HIR.
3. Emission anchoring at the helper definition, with the narrowed `#[expect]`
   contract described above.
4. All-or-nothing qualification per helper, with finer rejection reasons for
   measurement in 8.2.4.
5. Delivery: the evidence repair (EP-M3) is developed in this plan and may be
   raised as its own pull request stacked beneath the 8.2.3 implementation pull
   request; in either case EP-M4 must not merge before EP-M3. The user may
   instead direct that EP-M3 becomes a separate roadmap item.

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

- The lint remains experimental, behind the
  `experimental-rstest-helper-should-be-fixture` suite feature and the
  installer's `--experimental` flag. Promotion is out of scope.
- Do not add the upstream `clippy_utils` crate. The workspace `clippy_utils`
  is an unpublished local stub (`crates/clippy_utils/src/lib.rs`) that mirrors
  a subset of Clippy's modules; `span_lint_hir_and_then` is added to that stub
  at Clippy's own path (see `Interfaces and dependencies`).
- No new external dependency. Dev-dependencies already pinned in the
  workspace (`googletest`, `pretty_assertions`, `insta`, `proptest`, `rstest`,
  `rstest-bdd`, `rstest-bdd-macros`) may be added where missing; the user has
  authorized these. The published crate `whitaker-common` must not gain a
  runtime dependency.
- No source file may exceed 400 lines (`AGENTS.md`). Already near the cap:
  `crates/rstest_helper_should_be_fixture/src/driver.rs` (395 lines),
  `crates/rstest_helper_should_be_fixture/src/collector.rs` (383),
  `crates/whitaker_lint_core/src/hir/mod.rs` (374). New logic goes in new
  modules; a behaviour-preserving extraction from `driver.rs` precedes any
  growth there.
- Production code must not call `.expect()`, `.unwrap()`, or panic. Anything
  the lint cannot establish degrades to "no diagnostic" with a `debug!` record
  naming the reason.
- Fail closed: every uncertainty in evidence, classification, or callee scope
  must suppress a diagnostic, never cause one.
- Determinism: diagnostics, their order, their notes, and every snapshot are
  identical across runs and independent of HIR traversal or hash iteration
  order.
- `ArgAtom` and `ArgFingerprint` in `whitaker_common::rstest` keep their public
  shape. Changes to how arguments lower to atoms are limited to the EP-M3
  repairs recorded in `Decision log`.
- Source-span deduplication from 8.2.2 is preserved. "Call" in every threshold
  and message means a distinct source call site, not a runtime invocation per
  generated case.
- `collect_rstest_companion_test_functions` keeps its semantics; it is also
  used by `no_expect_outside_tests` and `no_unwrap_or_else_panic`.
- EP-M4 (diagnostics) must not merge before EP-M3 (evidence repair) is green.
- Fluent `.ftl` resources for this lint belong to roadmap item 8.5.2. This plan
  fixes the message key, attribute names, argument names, and selector values,
  resolves through `safe_resolve_message_set`, and supplies English fallback
  text. It adds no `.ftl` files.
- The full UI pass/fail matrix belongs to roadmap item 8.2.4. This plan adds
  only the fixtures needed to prove emission, every message variant, every
  precision rule, and lint-level attribution end to end.
- Comments and documentation use en-GB Oxford spelling. Markdown wraps at 80
  columns; code blocks at 120.
- Verus sidecars follow `docs/developers-guide.md` §Verus scope and trust
  boundary: Verus proves an implementation-shaped model; executable tests tie
  production code to the same specification. No `assume`, `admit`,
  `#[verifier::external_body]`, or equivalent may appear in a committed proof.

## Tolerances (exception triggers)

ExecPlan files are excluded from every size count. Count only non-ExecPlan
files towards file-count limits; count code and other documentation (including
Verus sidecars and `.stderr` fixtures) towards line limits.

- Scope: more than 45 non-ExecPlan files or more than 3,500 net added lines
  for the whole plan triggers escalation. EP-M3 alone above 1,000 net lines
  triggers escalation with the options "split EP-M3 into its own roadmap item"
  or "fall back to the P0 rule" (only plain-test parameters bound by
  `<fixture>::default()` count as fixture locals; everything else is
  `Unsupported`).
- Interface: any change to the signature or semantics of an existing public
  item in `whitaker_common`, `whitaker_lint_core`, `clippy_utils`, or
  `whitaker_suite` (as opposed to new items) triggers escalation.
- Dependencies: any dependency outside `[workspace.dependencies]`, or any new
  runtime dependency of `whitaker-common`, triggers escalation.
- Iterations: a gate that still fails after three fix attempts for the same
  root cause triggers escalation with the logs.
- Verification: if the Verus obligations are not discharged after three
  working sessions, adopt the pre-approved fallback (spec-level lemmas plus the
  proptest differential, with the undischarged refinement recorded as a
  residual gap in `Verification plan` and the developers' guide). Weakening a
  theorem statement silently is never permitted.
- Precision: if the EP-M5 dogfood run finds any false positive that the
  precision rules cannot exclude without a new rule, stop and escalate with the
  triage table.
- Time: a milestone taking more than two working days triggers escalation.
- Ambiguity: if EP-M0 shows rstest expansion shapes differing from `A-EXPAND`
  in a way that the provenance rule cannot classify fail-closed, stop and
  present options before EP-M3.

## Risks

- Risk: the lint flags the function under test (for example `validate(cfg)`
  or `parser.parse()` called with a fixture in several tests). Severity: high.
  Likelihood: high without mitigation. Mitigation: the precision rules
  (`LA-TRIG+`): test-only definition scope, setup position, no fixture-receiver
  method calls, no `()`-returning callees; plus the EP-M5 dogfood gate over
  Whitaker's own `rstest`-heavy crates.
- Risk: `#[expect]` placed on a helper is unfulfilled in builds without
  `--test`, and helpers shared across integration-test binaries warn once per
  binary. Severity: medium. Likelihood: high. Mitigation: the narrowed contract
  in `Purpose / big picture`; a UI case compiling the same example without
  `--test`; documentation in the users' guide and ADR `0014`.
- Risk: rstest expansion differs across versions or settings (`#[timeout]`,
  the `RSTEST_TIMEOUT` environment variable read at expansion time, `async`
  tests, compact `#[rstest(arg, case(...))]` syntax). Severity: medium.
  Likelihood: medium. Mitigation: EP-M0 characterizes rstest 0.25 to 0.28 (the
  versions available offline in the Cargo registry); inner-body discovery walks
  nested bodies and closures; provenance classification fails closed on any
  unknown binding shape; the exact-summary harness test acts as a canary for
  the workspace's rstest version.
- Risk: the new UI test misses the nextest override that grants the
  long timeout and the `serial-dylint-ui` group, then times out or races the
  example harness lock. Severity: medium. Likelihood: high if forgotten.
  Mitigation: EP-M4 updates both profiles in `.config/nextest.toml` and the
  pinned filter strings in `tests/workflow_contracts/nextest_overrides.py`.
- Risk: the Verus model is the repository's first proof of a fold over
  per-position state; effort may exceed the budget. Severity: medium.
  Likelihood: medium. Mitigation: model the algorithm as a recursive spec fold
  (as `verus/clone_detector_ast_features.rs` does with `fold_count_from`);
  three-session budget; pre-approved fallback in `Tolerances`.
- Risk: lint-crate unit tests under `src/*_tests.rs` do not run because the
  library target has `test = false`. Severity: medium. Likelihood: confirmed by
  metadata; EP-M0 confirms with `cargo nextest list`. Mitigation: keep all pure
  logic in `whitaker_common` or in non-driver modules of `whitaker_lint_core`,
  whose tests run; verify rustc-dependent code through `tests/ui.rs`. Report
  the pre-existing gap; do not repair unrelated crates.
- Risk: `dylint-driver` code in `whitaker_lint_core` and `clippy_utils` has no
  unit tests under `make test`. Severity: low. Likelihood: certain. Mitigation:
  keep those adapters thin and cover them through UI fixtures.
- Risk: the shared compile admission queue slows Cargo-backed UI tests.
  Severity: low. Likelihood: high. Mitigation: start gates once; check
  `build-limits-status` before blaming the candidate; EP-M0 measures the added
  harness time.
- Risk: ADR serial collision (`0012` exists twice across branches; `0013` is
  taken on `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`).
  Severity: low. Likelihood: medium. Mitigation: re-fetch every remote branch
  and re-scan `docs/adrs/` before allocating; `0014` is currently free.

## Progress

- [x] (2026-11-11) Renamed branch to `8-2-3-crate-post-aggregation-thresholds`.
- [x] (2026-11-11) Reconnaissance with a Wyvern agent team: lint crate state,
  shared helpers and diagnostics, verification tooling, documentation.
- [x] (2026-11-11) Confirmed on the pinned nightly that
  `TyCtxt::emit_node_span_lint` exists and accepts `DiagDecorator`; confirmed
  upstream Clippy's `span_lint_hir_and_then` body and the `unused_async`
  crate-post precedent.
- [x] (2026-11-11) Dumped expanded HIR for the collection example and confirmed
  both 8.2.2 evidence defects.
- [x] (2026-11-11) Drafted the ExecPlan (commit `1de2158a`).
- [x] (2026-11-11) Community-of-experts review (structure and contracts,
  failure modes and pre-mortem, alternatives and cost, verification and
  viability); plan revised (see `Revision note`).
- [ ] Plan approval by the user.
- [ ] EP-M0: baseline and expansion-shape characterization.
- [ ] EP-M1: pure helper-group qualification kernel.
- [ ] EP-M2: Verus proof of the qualification model.
- [ ] EP-M3: repair `rstest` expansion evidence.
- [ ] EP-M4: emission helper and crate-post diagnostics.
- [ ] EP-M5: dogfood precision gate.
- [ ] EP-M6: documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: plain `#[rstest]` tests contribute no evidence. Evidence: the
  `--test -Zunpretty=hir` dump of
  `crates/rstest_helper_should_be_fixture/examples/collection_zero_diagnostic.rs`
  shows rstest emitting an outer harness function whose body declares a nested
  same-named function holding the original body, binds fixtures with
  `let fixture = fixture::default();`, and calls the nested function. Neither
  function is a collection root, and the harness summary agrees
  (`callee=helper;records=2`, both from the case-driven test). Impact: lint A
  would be silent for the most common shape. Repaired in EP-M3.
- Observation: provider parameters are classified as fixture locals. Evidence:
  the case-driven original
  `fn case_generated_collection_stays_silent(input: &str, fixture: &str)` has
  no parameter attributes after expansion; the harness expects
  `fingerprint=fixture-local,fixture-local,const-path,const-path` for
  `helper(fixture, input, PREFIX, SUFFIX)`. The companion `case_1` binds
  `input` with `magic_conversion("first")` and `fixture` with
  `fixture::default()`. The compact `#[rstest(input, case(...))]` syntax, still
  supported by rstest 0.28, never puts an attribute on the parameter at all.
  Impact: false positives for case-fed helpers; source-text attribute recovery
  alone cannot fix it. Repaired in EP-M3 by binding provenance.
- Observation: `span_lint_hir_and_then` is not available. Evidence: the
  `clippy_utils` stub exposes only `macros::is_panic`; every lint emits through
  `cx.emit_span_lint(LINT, span, DiagDecorator(...))`, which attributes the
  level to the node being checked. `MultiSpan` and `DiagMessage` are reachable
  only through `rustc_errors` (re-exporting `rustc_error_messages`), not
  through the `rustc_span` shim. Impact: EP-M4 adds the helper to the stub with
  an `extern crate rustc_errors;`.
- Observation: `DefId` and `LocalDefId` are `impl !Ord` on the pinned nightly
  (`rustc_span/src/def_id.rs`). Impact: the kernel bounds test identity on
  `Eq + Hash`, and the adapter orders helpers by `(def_path_str, DefIndex)`.
- Observation: callee buckets are keyed by `def_path_str`, which can collide
  for block-scoped items (two `fn h` in different blocks print the same path).
  Impact: the adapter regroups each bucket by `callee_def_id` before evaluation.
- Observation: `Type::default()` and `x.clone()` resolve to non-local trait
  items, so `is_local_function` already drops them; only local traits reach the
  trait-method exclusion. Impact: the trait-exclusion UI row must use a local
  trait.
- Observation: the lint crate's own unit tests are probably dead. Evidence:
  `cargo metadata` reports the library target with `test = false`; the only
  `test = true` target is `tests/ui.rs`. Impact: the 8.2.2 proptests in
  `src/collector_tests.rs` and `src/driver_tests.rs` do not run under
  `make test`. EP-M0 confirms; reported to the user as a follow-up.
- Observation: `.config/nextest.toml` grants this package's UI tests their
  timeout and serial group by exact test name (lines 137 and 292), pinned by
  `tests/workflow_contracts/nextest_overrides.py` (lines 127 to 161). Impact:
  any new UI test must be added to both.
- Observation: `collect_companion_in_group` in
  `crates/whitaker_lint_core/src/hir/mod.rs` compares every function with every
  sibling item, which is quadratic per module. Impact: the new root discovery
  must be linear; refactoring the existing scan is a follow-up, not part of
  this item.
- Observation: ADR `0013` is already used on an unmerged branch. Impact: the
  ADR for this item takes `0014` unless a re-scan finds otherwise.

## Decision log

- Decision: emit one diagnostic per qualifying helper, anchored at the helper
  definition: lint-level `HirId` from `cx.tcx.local_def_id_to_hir_id`, primary
  span from `cx.tcx.def_ident_span` recovered with
  `recover_user_editable_hir_span`, and call sites in one multi-span note.
  Rationale: the remedy acts on the helper; one diagnostic per helper avoids N
  duplicates; Clippy's `unused_async` uses the same collect-in-`check_fn`,
  emit-in-`check_crate_post` pattern at `local_def_id_to_hir_id(def_id)`.
  Alternatives rejected: per-call-site emission (N warnings, `#[expect]`
  placement ambiguous and order-sensitive); first call site (anchor moves when
  tests are reordered); once per test (duplicates). Date/Author: 2026-11-11,
  planning agent; endorsed by the review panel.
- Decision: narrow the lint-expectation contract. `#[allow]` on the helper is
  the supported suppression; `#[expect]` is supported inside `#[cfg(test)]`
  code or as `#[cfg_attr(test, expect(...))]`. Counts are per compilation unit.
  Rationale: crate-post aggregation sees only the compilation's tests; a
  library compiled without `--test` has none, so rustc reports an unfulfilled
  expectation there. Raised independently by three reviewers. Date/Author:
  2026-11-11.
- Decision: per-test suppression is a non-goal. An `#[allow]` on a test does
  not remove that test's calls from the counts. Rationale: excluding allowed
  tests would make `#[expect]` on a test permanently unfulfilled and complicate
  the counting contract. Revisit after 8.2.4 tuning. Date/Author: 2026-11-11.
- Decision: precision rules (`LA-TRIG+`). Callee scope: defined in test-only
  code (the definition or an ancestor carries `cfg(test)` provenance, or the
  crate is a test-harness-only target such as an integration test; EP-M0 fixes
  the exact detection and reuses `whitaker_common::context` test-context
  helpers where possible); not generic over types or consts; not `async`; not
  `unsafe`; return type not `()` or `!`; not a trait method or trait-impl
  method; not an rstest fixture function or an associated function of a
  fixture's generated struct (detected as a same-named struct in the same
  module with inherent `get` and `default`). Call shape: every call is the
  initializer of a `let` binding, optionally wrapped in `?`, and no call is a
  method call whose receiver lowers to `FixtureLocal`. Callee-scope failures
  exclude the helper; call-shape failures reject the whole group (fail closed).
  Rationale: the pre-mortem's most likely incident is a warning flood on the
  function under test; these rules target that without touching the kernel's
  contract. Each rule can only suppress diagnostics. Date/Author: 2026-11-11,
  from the failure-modes review.
- Decision: classify fixture locals by binding provenance (`LA-FIX+`). For
  each collection root, find the rstest-generated callers (for plain tests, the
  outer harness function; for case and matrix tests, the companion `case_N`
  functions, however deeply nested) and mark a parameter as a fixture local
  only if no HIR-visible provider attribute marks it and every generated caller
  binds the corresponding argument with `<path>::default()`, where `<path>`
  resolves to a fixture's generated struct. Bindings through `get(...)` or
  `partial_N(...)` (that is, `#[with(...)]`), `magic_conversion`, literals, or
  any unknown shape lower to `Unsupported`. Rationale: this handles plain,
  case, matrix, and compact syntax uniformly, cannot misclassify case data as a
  fixture, removes the source lexer and its `rustc_lexer` dependency, and gives
  the same evidence needed to recognize existing fixtures. Alternatives:
  source-text lexing of parameter attributes (P1) was rejected because it
  misses compact syntax, would add a 2019 lexer to a published crate, and fails
  open on unknown attributes; a pre-expansion lint pass was rejected as softly
  deprecated in rustc. Date/Author: 2026-11-11, from the alternatives and
  failure-modes reviews.
- Decision: `#[with(...)]`-bound fixtures are `Unsupported` in this version.
  Rationale: two tests passing `#[with(1)] db` and `#[with(2)] db` share a
  parameter name but not a value; treating them as the same fixture would give
  wrong advice. Date/Author: 2026-11-11.
- Decision: all-or-nothing qualification per helper, as the design document
  specifies ("Every argument list satisfies one of …"). Rejection reasons are
  split finely (`MixedKinds`, `DifferingConstants`, `DifferingFixtureNames`)
  and counted in the test-only collection summary so 8.2.4 can measure recall
  loss. Deferred alternatives, recorded in ADR `0014`: qualify the largest
  consistent class above a ratio; treat differing constants as a parameterized
  fixture (`#[default]` plus `#[with]`). Rejected: one diagnostic per
  fingerprint class (several diagnostics per helper and ambiguous `#[expect]`).
  Date/Author: 2026-11-11.
- Decision: `const`, associated `const`, and immutable `static` paths remain
  `ArgAtom::ConstPath`; a path resolving to a `static mut` lowers to
  `Unsupported`. Rationale: the design document delegated the distinction to
  8.2.3; a mutable static is shared mutable state, not a fixture value.
  Date/Author: 2026-11-11.
- Decision: the qualification kernel is pure and lives in
  `whitaker_common::rstest::helper_group`. Rationale: testability (the lint
  crate's library tests do not run), the existing placement of pure `rstest`
  policy (8.1.1 to 8.1.3), and the proofs workflow path filter on `common/**`.
  Reuse by lint C is not claimed; 8.4.3 decides. Date/Author: 2026-11-11.
- Decision: test identity in the kernel is bounded `T: Eq + Hash`, and the
  adapter passes `DefId` directly. Rationale: only the cardinality of the
  identity set is observed, and `DefId` is `!Ord`. Date/Author: 2026-11-11.
- Decision: `span_lint_hir_and_then` is added to the `clippy_utils` stub as
  `clippy_utils::diagnostics::span_lint_hir_and_then`, faithful to upstream
  Clippy except for the Clippy documentation link and debug suggestion
  validation. Rationale: the stub exists to mirror Clippy's paths, so call
  sites stay source-compatible if the stub is ever replaced; the stub is
  `publish = false` and already depends on the `rustc_hir` and `rustc_lint`
  shims. Sweep (per `AGENTS.md` abstraction policy): no existing equivalent in
  `whitaker_lint_core`, `whitaker_common`, or any lint crate; the alternative
  home `whitaker_lint_core::diagnostics` was rejected for path parity.
  Date/Author: 2026-11-11, from the structure review.
- Decision: verify the qualification contract with Verus, scoped to observable
  behaviour. The spec defines the full verdict payload for qualifying groups
  (`Qualifies` iff the predicate holds, and the summary's fields); rejection
  reasons and their precedence are best-effort diagnostics for `debug!` logs
  and are pinned by unit tables and a whole-verdict permutation property, not
  by proof. Rationale: rejection policy is expected to change during 8.2.4
  tuning and is never user-visible; proving it would freeze it in five places.
  Date/Author: 2026-11-11, from the verification review.
- Decision: the help text says "Provide `<helper>` through a `#[fixture]`"
  and each argument-shape variant has a whole sentence, so translators can
  reorder freely. Date/Author: 2026-11-11.
- Decision: the EP-M4 insta snapshot of message sets is dropped; the
  byte-exact `.stderr` is the snapshot of rendered output, and the
  shape-to-selector mapping is a table-tested pure function in `common`.
  Date/Author: 2026-11-11.
- Decision: ADR `0014` (serial confirmed by re-scan) records anchoring, the
  expectation contract, `LA-TRIG+`, `LA-FIX+`, and all-or-nothing
  qualification, because users will write attributes against them. Date/Author:
  2026-11-11.

## Outcomes & retrospective

Not started. Complete at each milestone boundary and at completion, comparing
results against `Purpose / big picture` and reconciling every discovery with
the documents in `Conformance basis`.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint libraries into a `rustc`
driver. Each lint here is a late lint pass, meaning it runs after name
resolution and type checking and can see the HIR (the compiler's desugared
syntax tree) and resolved definitions. A `HirId` names one HIR node; a `DefId`
names one definition; a `LocalDefId` is a `DefId` in the current crate. Lint
levels (`allow`, `warn`, `deny`, `expect`) attach to HIR nodes through
attributes, and rustc resolves a diagnostic's level from the `HirId` it is
emitted at. A "compilation unit" is one invocation of the compiler: the
library, its unit-test build (`--test`), and each integration-test binary are
separate units.

`rstest` is a test framework whose `#[rstest]` attribute macro rewrites a test
function before the compiler sees it. A fixture is a function marked
`#[fixture]`; rstest generates a same-named struct with `get` and `default`
associated functions and injects `<fixture>::default()` into test parameters of
the same name. Parameters marked `#[case]`, `#[values]`, `#[files]`,
`#[future]`, or `#[context]` are providers: they carry per-case data.
`docs/rust-testing-with-rstest-fixtures.md` explains fixtures, `#[with(...)]`
(line 497), and `#[from(...)]` (line 563).

Lint A's design is `docs/lints-for-rstest-fixtures-and-test-hygiene.md` §Lint
A: call-site fixture extraction (lines 40 to 130), with implementation
decisions for 8.1.1 to 8.2.2 at lines 383 to 467.

The lint crate is `crates/rstest_helper_should_be_fixture/`:

- `src/lib.rs` declares `collector`, `driver`, and `visitor` behind the
  `dylint-driver` feature.
- `src/driver.rs` holds `Config` (`min_calls`, `min_distinct_tests`,
  `require_identical_fixture_arg_names`, `provider_param_attributes`,
  `use_source_callee_fallback`; the first two normalized to at least 2), the
  pass struct `RstestHelperShouldBeFixture`, and the hooks `check_crate`,
  `check_fn`, and `check_crate_post`. Today `check_crate_post` only finalizes
  the collector, logs, and writes an environment-gated summary
  (`WHITAKER_RSTEST_HELPER_COLLECTION_SUMMARY`) for the UI harness.
- `src/collector.rs` holds `CallSiteRecord` (fields `callee_def_id`,
  `fingerprint`, `test_source_def_id`, `span`, and `hir_local_id`), the
  deduplication key `CallSiteLocation`, `CallSiteCollector` (a
  `BTreeMap<String, Vec<CallSiteRecord>>` keyed by callee path, plus a `seen`
  set), `lower_arg_atom`, and `resolve_local_callee`.
- `src/visitor.rs` holds `CallSiteVisitor` and fixture-local classification.
- `tests/ui.rs` compiles `examples/*.rs` through
  `dylint_testing::ui::Test::example` with `--test`, under a cross-process
  lock, and compares diagnostics with an adjacent `.stderr` file.

Shared crates:

- `common/` (published package `whitaker-common`) owns pure policy:
  `common/src/rstest/` and localization in `common/src/i18n/`
  (`get_localizer_for_lint`, `safe_resolve_message_set`, `DiagnosticMessageSet`,
  `Arguments`).
- `crates/whitaker_lint_core/` (unpublished) owns rustc-aware helpers behind
  the `dylint-driver` feature, including `hir::recover_user_editable_hir_span`
  and `hir::collect_rstest_companion_test_functions`. `make test` runs its
  tests with `--no-default-features`.
- `crates/clippy_utils/` is an unpublished stub mirroring part of Clippy.
- `suite/src/driver.rs` combines every lint with
  `late_lint_methods!(declare_combined_late_lint_pass, ...)`, which forwards
  `check_crate_post` to every constituent pass.

Verification tooling: Verus sidecars in `verus/*.rs` run through
`scripts/run-verus.sh` (groups selected by name) and `make verus*`;
`.github/workflows/proofs.yml` runs them on pull requests touching `verus/**`,
`common/**`, the scripts, or the `Makefile`. See `docs/developers-guide.md`
§Proof workflows (line 2336) and §Verus scope and trust boundary (line 2592).

## Conformance basis

Upstream artefacts at the plan's base commit `d9eb8a09`:

- Terms of Reference: none exists for this lint family; the roadmap entry is
  the governing commitment.
- Roadmap: `docs/roadmap.md` §8.2, item 8.2.3 (lines 634 to 637); neighbours
  8.2.4 (UI matrix), 8.5.2 (Fluent), 8.5.3 (user documentation).
- Technical design: `docs/lints-for-rstest-fixtures-and-test-hygiene.md`.
  Traced elements:
  - `LA-TRIG`: trigger conditions (lines 51 to 63), amended by `LA-TRIG+`.
  - `LA-DET`: conservative `#[rstest]` detection (lines 65 to 71).
  - `LA-FIX`: fixture-local classification (lines 73 to 82), amended by
    `LA-FIX+`.
  - `LA-FP`: argument fingerprint rules (lines 84 to 107).
  - `LA-DIAG`: `span_lint_hir_and_then` with the best user-editable span,
    primary, note, and help (lines 109 to 119).
  - `LA-CFG`: configuration keys (lines 121 to 130).
  - `INT-SPAN`: user-editable spans, no diagnostics on macro-only glue (line
    36). `INT-EXP`: experimental rollout (line 37).
- Suite design: `docs/whitaker-dylint-suite-design.md` §Localization
  infrastructure (lines 171 to 254) as `SUITE-L10N`.
- ADRs: none govern this lint; EP-M6 adds ADR `0014`.
- Standards: `AGENTS.md`; `docs/documentation-style-guide.md`;
  `docs/developers-guide.md` §Shared diagnostic localization (lines 143 to
  152), §Proof workflows, §Assertions in fallible tests (line 290).

Trace links:

```plaintext
LA-TRIG   -> EP-M1 evaluate_helper_group          -> helper_group unit table, oracle proptest, BDD feature
LA-TRIG   -> EP-M2 verus/rstest_helper_group.rs   -> make verus-rstest-helper
LA-TRIG+  -> EP-M4 aggregate.rs precision rules   -> aggregation_diagnostics.rs rows + .stderr; EP-M5 triage table
LA-DET    -> EP-M3 inner-body roots               -> exact collection summary (plain-test records)
LA-FIX+   -> EP-M3 binding provenance             -> exact collection summary (case input unsupported); compact-syntax row
LA-FP     -> EP-M3 static mut lowering            -> exact collection summary
LA-DIAG   -> EP-M4 span_lint_hir_and_then + messages -> .stderr fixture; allow/expect and non --test cases
LA-CFG    -> EP-M1 HelperThresholds               -> threshold rows in the unit table; config variants deferred to 8.2.4
SUITE-L10N -> EP-M4 key, arguments, fallback      -> .stderr fixture; selector table test
INT-SPAN  -> EP-M4 macro-only helper exclusion    -> macro-generated helper row
INT-EXP   -> unchanged feature gating             -> existing suite registration tests
```

## Verification plan

### Abstract model

The contract is specified independently of rustc. A group `G` is a finite
sequence of calls to one helper. A call has a test identity and an argument
list of atoms; an atom is `Fixture(name)`, `Const(value)`, or `Unsupported`.
Literal text and constant paths are `Const` values in disjoint namespaces, so a
literal never equals a path. Thresholds are natural numbers `m` (minimum calls)
and `k` (minimum distinct tests) and a flag `s`
(`require_identical_fixture_arg_names`).

```plaintext
eligible(c)          = every atom of c is Fixture(_) or Const(_)
same_arity(G)        = every call in G has the same argument count n
fixture_pos(G, p)    = every call has Fixture(_) at p and (s implies all those names are equal)
const_pos(G, p, v)   = every call has Const(v) at p
consistent(G)        = for every p < n: fixture_pos(G, p) or there is v with const_pos(G, p, v)
tests(G)             = the set of test identities in G
qualifies(G, m, k, s) = G non-empty and every call eligible and same_arity(G)
                        and consistent(G) and |G| >= m and |tests(G)| >= k

shape(G)[p]          = Fixture { shared_name: the common name if all names at p are equal, else none }
                         when fixture_pos(G, p)
                     = Constant(v) when const_pos(G, p, v)
verdict(G, m, k, s)  = Qualifies { call_count: |G|, distinct_test_count: |tests(G)|, positions: shape(G) }
                         when qualifies(G, m, k, s)
                     = Rejected otherwise
conforms(c, shape)   = |c| = |shape| and for every p: shape[p] = Constant(v) implies c[p] = Const(v),
                       and shape[p] = Fixture { shared_name } implies c[p] = Fixture(x)
                       with (s implies shared_name = x)
```

Every quantity in `verdict` is a function of the multiset of calls, so the
verdict is order-independent by construction. The production algorithm is a
left fold: `observe(state, call)` per call, then `finish(state, thresholds)`.
The Verus sidecar models exactly that fold.

Representation relation: production `ArgAtom::FixtureLocal { name }` maps to
`Fixture(intern(name))`; `ConstLit { text }` maps to
`Const(intern("lit:" + text))`; `ConstPath { def_path }` maps to
`Const(intern("path:" + def_path))`; `Unsupported` maps to `Unsupported`.
`intern` is injective, so string equality coincides with model equality. Test
identities map injectively to naturals.

### Obligations

- Obligation `L-REF` (refinement): for every group and thresholds, the fold
  model's result equals `verdict(G, m, k, s)`: it qualifies exactly when
  `qualifies` holds, and then its summary equals the spec's summary. Method:
  Verus proof by induction over the fold, with an invariant relating the
  partial state after a prefix to the spec quantities over that prefix.
  Production link: a `proptest` differential test comparing
  `evaluate_helper_group` with a reference oracle that transliterates `verdict`
  directly (quantifier style, quadratic). Rationale: this is the lint's
  contractual trigger; a false `Qualifies` is a false positive. Domain: all
  finite groups (Verus); generated groups (proptest; see generator design).
  Artefacts: `verus/rstest_helper_group.rs`;
  `common/src/rstest/tests/helper_group_properties.rs`. Evidence:
  `make verus-rstest-helper` reports the file verified with zero errors; the
  differential test passes under `ProptestConfig { cases: 1024, .. }`.
  Non-vacuity: witness lemmas `L-WITNESS-YES` (two calls, two tests, no
  arguments, `m = k = 2`, qualifies) and `L-WITNESS-NO` (three calls, one test,
  `k = 2`, rejected) verify, so both branches are inhabited; the Verus negative
  control (fold counts calls as tests) must make Verus report `L-REF` as
  failing; seeded faults in production ("count calls instead of distinct
  tests", "ignore `Unsupported`", "`>` instead of `>=`", "compare fixture names
  when `s` is false") must each fail the differential test or a unit-table row.
- Obligation `INV-ORDER` (order independence of the observable verdict):
  permutations of a group yield equal verdicts. Method: corollary of `L-REF`
  because `verdict` is defined over the multiset (Verus states it as a lemma
  that cites `L-REF`); production property `verdict_is_permutation_invariant`
  over the *whole* production verdict, including rejection reasons.
  Non-vacuity: the seeded fault "report the first call's unsupported position
  instead of the least" must fail the production property.
- Obligation `L-MONO` (threshold monotonicity), spec sanity lemma: if
  `qualifies(G, m, k, s)` and `m' <= m` and `k' <= k`, then
  `qualifies(G, m', k', s)`; and `qualifies(G, m, k, true)` implies
  `qualifies(G, m, k, false)`. Method: Verus lemma over the spec; production
  property on the kernel. Rationale: lowering a threshold or relaxing
  strictness must never hide a diagnostic. Non-vacuity: generator reaches
  groups that qualify at the higher thresholds (classification counter).
- Obligation `L-EXT` (stable growth): if `qualifies(G, m, k, s)` and
  `conforms(c, shape(G))`, then `qualifies(G + [c], m, k, s)`. Method: Verus
  lemma; production property generating `c` from the summary shape (not by
  filtering). Rationale: adding another conforming test must not make a
  diagnostic disappear.
- Obligation `L-BOUND`, spec sanity lemma: `|tests(G)| <= |G|`. Method: Verus
  lemma used inside `L-REF`.
- Obligation `ADAPT-SAFE` (adapter and deduplication never inflate counts):
  for each helper, the evidence passed to the kernel contains at most one entry
  per distinct recovered call site, and its test identities are the `DefId`s of
  collection roots that contain those call sites. Method: exact
  collection-summary assertions in `tests/ui.rs` covering a multi-`#[case]`
  test (one test, one call), a `#[values]` matrix (one test), two same-named
  tests in different modules (two tests), and the absence of any record from
  rstest's outer harness (`fixture::default()` calls). Rationale: integration
  behaviour of rustc and rstest; the real driver is the only faithful boundary.
- Obligation `EVID-PLAIN` and `EVID-PROVIDER` (EP-M3): plain `#[rstest]`
  bodies are collected once per call site; every non-fixture-provenance
  parameter lowers to `Unsupported`. Method: exact collection-summary
  comparison (not substring checks) for the existing example plus new rows
  (compact syntax, `#[with]`, `#[from]`, `#[timeout]`, `async`, handwritten
  nested same-name function as a negative). Non-vacuity: the red step shows the
  current wrong summary.
- Obligation `EMIT-ATTR` (lint-level attribution): a diagnostic for helper `h`
  follows lint attributes on `h` and its ancestors; `#[expect]` inside
  `#[cfg(test)]` code is fulfilled. Method: UI fixture with byte-exact
  `.stderr`, compiled with `--test`, plus the same example compiled without
  `--test` (expected: no diagnostics and no `unfulfilled_lint_expectations`,
  because its expectations are `cfg_attr(test, ...)`). Non-vacuity: an
  unannotated twin helper in the same fixture must emit.
- Obligation `EMIT-ONCE`: exactly one diagnostic per qualifying helper, in
  ascending `(def_path_str, DefIndex)` order, with at most three call-site
  spans in ascending source order and a "further call sites" note when more
  exist. Method: UI fixture with two qualifying helpers and one helper with
  four call sites.
- Obligation `PREC-RULES` (each `LA-TRIG+` rule suppresses): Method: one UI
  row per rule (non-test-only helper, generic, `async`, `unsafe`, `()` return,
  local trait method, fixture function, fixture struct `get`, call not in setup
  position, method on a fixture receiver), each otherwise qualifying, so a rule
  that does not fire causes a diagnostic diff.
- Message wording and Fluent argument mapping carry no proof obligation. They
  are pinned by the `.stderr` fixture and by a table test of the
  `ArgumentShape`-to-selector function in `common`.

### Generator design (proptest)

Generate a target class first, then construct a group in that class:
`prop_oneof!` over "qualifying" (draw arity 0 to 4, a shape per position, then
calls that conform, with test identities from a pool of 1 to 4), and one
perturbation per rejection reason (insert an `Unsupported` atom, change one
call's arity, mix kinds at one position, change one constant, rename one
fixture). Draw `m` and `k` from `0..=6` so boundary faults (`>` versus `>=`)
are reachable. Classification evidence comes from a deterministic `TestRunner`
sampling test that tallies the oracle's verdict class over 2,000 draws and
asserts each class appears at least 5% of the time; proptest has no built-in
classification.

### Where Verus is and is not warranted

Verus is warranted for the qualification contract: it is the lint's contractual
business logic, `L-REF`, `L-EXT`, and `INV-ORDER` quantify over unbounded
groups, and a small pure kernel exists. Kani is not used: the kernel manipulates
`String` and `Vec` values that the repository records as prone to time-outs
under Kani, and a bounded check would add little beyond the Verus model plus
the differential property. Verus is not used for expansion evidence, binding
provenance, precision rules, or message rendering: those depend on rustc and
rstest behaviour, so real-driver UI fixtures are the appropriate assurance.
Rejection reasons are best-effort and covered by unit tables and the
permutation property only.

The proof and the oracle are two hand translations of one contract. Each Verus
lemma and each property carries its obligation identifier and the path of its
counterpart in its doc comment, and `helper_group.rs` opens with a module
comment stating that any change to the contract must update
`verus/rstest_helper_group.rs`, the oracle, and ADR `0014`.

### Axioms

- `A-LEVEL`: `TyCtxt::emit_node_span_lint(lint, hir_id, span, decorator)` on
  `nightly-2026-05-28` resolves the level with `lint_level_spec_at_node` and
  records `#[expect]` fulfilment; rustc checks expectations after all lint
  passes. Evidence: pinned rustc source
  `compiler/rustc_middle/src/ty/context.rs:2461`; exercised by `EMIT-ATTR`.
- `A-COMBINED`: `declare_combined_late_lint_pass` forwards `check_crate_post`
  to every constituent pass. Evidence: `suite/src/driver.rs`; exercised by the
  existing suite registration tests.
- `A-EXPAND`: rstest 0.25 to 0.28 expand a plain test into an outer harness
  function containing a same-named nested function and fixture bindings
  `let <name> = <fixture>::default();`, and a case or matrix test into a
  same-named function plus companion modules whose `case_N` functions bind case
  data by expressions other than `<fixture>::default()`; parameter attributes
  are stripped. `#[timeout]` and `RSTEST_TIMEOUT` wrap the inner call in a
  closure; `async` tests call it with `.await`. Evidence: EP-M0 shape table.
- `A-SPAN`: `SourceMap::span_to_snippet` returns user-written text for a
  user-editable span. Evidence: existing `literal_atom` in `collector.rs`.
- `A-INTERN`: the model's natural-number identities stand for strings and
  `DefId`s under injective maps; exercised by the differential test over real
  strings.

## Plan of work

Stage A (EP-M0) establishes facts without production changes. Stages B to D
then run per milestone: a red test or proof obligation first, the smallest
implementation, a cleanup pass, and the three gates. Milestones EP-M1 and EP-M2
are pure and independent of the evidence repair, so they come first; EP-M3
repairs evidence; EP-M4 wires diagnostics and depends on EP-M1 and EP-M3; EP-M5
measures precision on real code; EP-M6 closes documentation.

## Milestones and plateaus

### EP-M0: baseline and characterization

- Outcome: recorded facts, no tracked source changes except this plan.
- Work:
  1. Run `make check-fmt`, `make lint`, `make test` on the untouched branch
     and keep the logs.
  2. Run `cargo nextest list -p rstest_helper_should_be_fixture
     --all-features` and record whether `src/*_tests.rs` tests are listed.
  3. Write a throwaway example (not committed) containing: plain `#[rstest]`;
     `#[case]`; compact `#[rstest(input, case(1))]`; `#[values]` matrix;
     `#[with(...)]` and `#[from(...)]` fixtures; `#[timeout(...)]`; an `async`
     `#[rstest]`; a `#[rstest]` in a nested module sharing its name with
     another test; a `#[fixture]` function called directly and through
     `fixture::get()`; a handwritten `#[test] fn f() { fn f() {} f() }`; and a
     plain `#[test]`. Dump HIR with `cargo rustc -p
     rstest_helper_should_be_fixture --example <name> -- --test
     -Zunpretty=hir`, once with `RSTEST_TIMEOUT` unset and once set. Repeat
     against rstest 0.25, 0.26, and 0.27 from the offline registry in a
     scratch copy of the example (not committed; use the workspace target
     directory, not `/tmp`). Record per shape which functions hold the user
     body, how parameters are bound, the binding expression per parameter,
     and how rstest provenance can be established (span expansion data).
  4. Decide the test-only scope detection: how a `cfg(test)` ancestor appears
     in HIR on the pinned nightly (the dump shows `CfgTrace` attributes), how
     an integration-test crate is recognized, and whether
     `whitaker_common::context::in_test_like_context` can be reused.
  5. Measure the current wall time of the lint crate's UI tests and record the
     budget for one more Cargo-backed example.
  6. Identify the dogfood command for EP-M5 (the experimental lint run over
     `whitaker-common --all-targets` and the lint crates' tests).
- Go/no-go: proceed to EP-M3 only if every shape can be classified
  fail-closed by the provenance rule; otherwise escalate per `Tolerances`.
  EP-M1 and EP-M2 may proceed regardless.
- Acceptance evidence: the shape table, the scope decision, and the timings
  recorded in `Artefacts and notes` and `Surprises & discoveries`.
- Conformance check: evidence only; `A-EXPAND` confirmed or escalated; no
  interface, dependency, or format change; trace links unchanged.
- Recovery: re-run; nothing to revert.
- Compatibility decision: none.

### EP-M1: pure helper-group qualification kernel

- Outcome: `whitaker_common::rstest::helper_group` implements the abstract
  model, fully tested, unused by the lint yet.
- Work:
  1. Skeleton. Add the types and an `evaluate_helper_group` that returns
     `Rejected(Empty)` for every input, so tests compile and fail on their
     assertions rather than on missing items.
  2. Red. Add `common/src/rstest/tests/helper_group_tests.rs`: a parameterized
     `rstest` table using `pretty_assertions::assert_eq` for whole-verdict
     equality, covering the empty group; no-argument helpers at and below each
     threshold (including `m` or `k` of 0 and 1); three calls in one test;
     fixture positions with equal and differing names under both `s` values;
     equal and differing `ConstLit`; `ConstLit` against `ConstPath` with the
     same text; mixed kinds at one position; `Unsupported` at several
     positions (least position reported); arity mismatch; and the precedence
     when several rules fail. Add a table test for
     `ArgumentShape::fluent_selector`. Add
     `common/tests/features/rstest_helper_group.feature` (see
     `Validation and acceptance`) bound in
     `common/tests/rstest_helper_group_behaviour.rs`. Run and observe the
     assertion failures.
  3. Green. Implement `observe` and `finish` as described in `Abstract model`,
     using a `HashSet` of test identities and per-position state.
  4. Properties. Add `common/src/rstest/tests/helper_group_properties.rs` with
     the oracle, `L-REF` differential, whole-verdict `INV-ORDER`, `L-MONO`,
     `L-EXT`, and the classification sampling test.
  5. Snapshot. Add one `insta` snapshot of verdicts for a fixed corpus of
     named groups, one line per group, as a readable contract summary.
  6. Seeded faults. Apply each seeded fault named in `Verification plan`,
     confirm the expected test fails, revert, and record the observation. Do
     not commit faults.
- Requirements and gaps: `LA-TRIG`, `LA-CFG` (threshold semantics).
- Acceptance evidence (non-empty filter, record the count):
  `cargo nextest run -p whitaker-common -E 'test(helper_group) |
  binary(rstest_helper_group_behaviour)'`
  passes with at least the table rows, six BDD scenarios, and five properties;
  three gates pass.
- Conformance check: `LA-TRIG` implemented as specified; new public module
  only, with its reuse policy recorded in EP-M6; no format change; trace links
  current.
- Recovery: additive; revert the milestone commits.
- Compatibility decision: none.

### EP-M2: Verus proof of the qualification model

- Outcome: `verus/rstest_helper_group.rs` proves `L-REF`, `INV-ORDER` (as a
  corollary), `L-MONO`, `L-EXT`, `L-BOUND`, and both witness lemmas for the
  fold model, with no trusted shortcuts.
- Work:
  1. Write the spec functions of `Abstract model`, then the recursive spec
     fold mirroring `observe` and `finish` in production branch order. State
     the lemmas first; let open verification conditions drive auxiliary
     lemmas (set cardinality and sequence-prefix helpers from `vstd`).
  2. Register group `rstest-helper` in `scripts/run-verus.sh` (the `all`
     case, `proof_files_for_group`, and the accepted-group case), add a
     `verus-rstest-helper` Makefile target and list it in `.PHONY`, and list
     the group in `docs/developers-guide.md` §Tooling scripts.
     `.github/workflows/proofs.yml` already triggers on the touched paths.
  3. Negative control: locally make the fold count calls as tests; confirm
     Verus reports `L-REF` failing; revert; record the output.
  4. Update `docs/developers-guide.md` §Verus scope and trust boundary with
     what the sidecar does and does not cover.
- Requirements and gaps: `LA-TRIG` (formal assurance).
- Acceptance evidence: `make verus-rstest-helper` reports the file verified
  with zero errors; `make verus` passes;
  `grep -nE 'assume|admit|external_body' verus/rstest_helper_group.rs` prints
  nothing; three gates pass.
- Conformance check: obligations discharged as stated, none weakened; model
  reviewed side by side with the production `observe`/`finish`; no interface or
  dependency change.
- Recovery: additive; revert. If the budget is exhausted, apply the
  pre-approved fallback in `Tolerances` and record the residual gap.
- Compatibility decision: none.

### EP-M3: repair `rstest` expansion evidence

- Outcome: the collector sees plain, case, matrix, `#[timeout]`, and `async`
  rstest bodies once each; fixture locals are recognized only by binding
  provenance; `static mut` paths lower to `Unsupported`; call-shape facts
  needed by EP-M4 are recorded. The lint is still diagnostic-silent.
- Work:
  1. Red. Replace the substring assertions in
     `example_harness_collects_call_site_evidence` with an exact comparison of
     the whole summary (`pretty_assertions::assert_eq` against a literal), and
     update the expected text to the repaired evidence: the plain test's
     `helper` and `Builder::build` records appear; the `#[case] input`
     position becomes `unsupported`, so both existing `Builder::build`
     fingerprints become `unsupported,unsupported`; and the standalone
     `fingerprint=unsupported,fixture-local` line disappears. Add an
     `unsafe { helper(fixture, "literal", MUTABLE_PREFIX, SUFFIX) }` call (the
     `unsafe` block around the whole call, so the argument itself is a path)
     and expect `unsupported` at that position. Add the new rows listed under
     `EVID-PLAIN` and `ADAPT-SAFE` to a second example,
     `examples/collection_expansion_shapes.rs`, with its own exact summary.
     Run and observe the expected failure.
  2. Green, roots. Add `crates/whitaker_lint_core/src/hir/rstest_roots.rs`
     with `collect_rstest_inner_test_bodies(cx)`: for each function, inspect
     only its own statement items, and accept a same-named nested function
     item only with rstest provenance evidence fixed in EP-M0 (for example
     the outer function's generated statements carry an expansion of the
     `rstest` attribute macro) and a call to it anywhere in the outer body,
     including inside closures and `.await`. Resolve paths with `path.res`
     only; never read ambient typeck results from `check_crate`. The scan is
     linear in the number of items. Union with the existing roots in
     `check_crate`.
  3. Green, provenance. Add
     `crates/whitaker_lint_core/src/hir/rstest_bindings.rs` with
     `rstest_fixture_items(cx)` (fixture function and generated-struct pairs,
     shared with EP-M4's exclusion) and
     `fixture_bound_parameters(cx, root) -> Option<FixtureBindings>`, which
     returns, per parameter index, whether every generated caller binds it
     with `<fixture>::default()`. `None` (no generated caller found, or an
     unknown shape) means "no parameter is a fixture local". Replace the
     classification in `visitor.rs::fixture_local_ids` with the conjunction of
     visible-attribute classification and provenance. Pure decision rules
     (for example "all callers agree") live in `common/src/rstest/` so their
     tests run.
  4. Green, lowering and facts. In `collector.rs::lower_path_arg`, lower
     `DefKind::Static { mutability: Mut, .. }` to `Unsupported`. Extend
     `CallSiteRecord` with `call_shape: CallShape { is_setup_position: bool,
     has_fixture_receiver: bool }` computed in the visitor from the parent
     HIR node (a `let` initializer, optionally through `?`) and the lowered
     receiver atom. This adds facts; it does not change fingerprints or
     deduplication. Keep `collector.rs` under 400 lines by placing the shape
     computation in `visitor.rs` or a new `call_shape.rs`.
  5. Add `test(example_harness_collects_expansion_shapes)` (or the chosen
     name) to the `.config/nextest.toml` overrides in both profiles and to
     `tests/workflow_contracts/nextest_overrides.py`.
  6. Refactor and gate.
- Requirements and gaps: `LA-DET`, `LA-FIX+`, `LA-FP`, `ADAPT-SAFE`.
- Acceptance evidence: both exact-summary tests pass; three gates pass;
  `make test-workflow-contracts` passes.
- Conformance check: `LA-DET` and `LA-FIX+` satisfied for every EP-M0 shape;
  `collect_rstest_companion_test_functions` unchanged; new public items only
  (`collect_rstest_inner_test_bodies`, `rstest_fixture_items`,
  `fixture_bound_parameters`); no persisted-format change.
- Recovery: revert the milestone commits; the lint returns to 8.2.2
  behaviour. If the tolerance is breached, offer the P0 fallback or a split.
- Compatibility decision: none (private and pre-1.0 surfaces).

### EP-M4: emission helper and crate-post diagnostics

- Outcome: the lint emits one localizable diagnostic per qualifying helper,
  attributed to the helper's `HirId`, honouring every precision rule.
- Work:
  1. Refactor first (separate commit): move the collection-summary support
     from `src/driver.rs` into `src/summary.rs` with no behaviour change.
  2. Red. Add `examples/aggregation_diagnostics.rs` and its expected
     `examples/aggregation_diagnostics.stderr`. Contents: a no-argument helper
     (`none`); a fixtures-only helper with four call sites (`fixtures`, note
     cap); a constants-only helper (`constants`); a fixture-plus-constant
     helper (`mixed`); a helper with `#[allow(...)]`; a helper with
     `#[cfg_attr(test, expect(...))]`; an unannotated twin; and one otherwise
     qualifying row per `PREC-RULES` rule; a helper called three times from
     one test; a case-fed helper; and two block-scoped helpers with the same
     printed path. Scope `#![allow(unknown_lints)]` narrowly. Add test
     `example_reports_expected_diagnostics` (compiled with `--test`) and
     `example_without_test_harness_is_silent` (the same example compiled
     without `--test`, expecting no output). Add both names to the nextest
     overrides and the workflow contract. The expected `.stderr` is written
     by hand before green, so the red comparison is about meaning; it becomes
     byte-exact at green. Run and observe the empty actual output.
  3. Green, emitter. Add `crates/clippy_utils/src/diagnostics.rs` with
     `span_lint_hir_and_then` (see `Interfaces and dependencies`), adding
     `extern crate rustc_errors;` to the stub for `MultiSpan` and
     `DiagMessage`.
  4. Green, aggregation. Add `src/aggregate.rs`: after `finalize()`, regroup
     each bucket by `callee_def_id`; order helpers by `(def_path_str,
     DefIndex)`; exclude helpers failing callee scope (logging the rule);
     reject groups with any call failing call shape (logging the rule); build
     `HelperCallEvidence` with `test = test_source_def_id`; evaluate with
     `HelperThresholds` from `Config`. Add rejection-reason counts to the
     test-only collection summary.
  5. Green, diagnostics. Add `src/diagnostic.rs`: build `Arguments`
     (`helper`, `calls`, `tests`, `shown_calls`, `argument_shape`), resolve
     with `safe_resolve_message_set` and the English fallbacks, and emit
     through `span_lint_hir_and_then`: primary at the recovered helper
     identifier span (skip with `debug!` if macro-only), one `span_note` over
     the first three call sites carrying the counts note, a plain note for
     further call sites when there are more than three, and one `help`. Call
     aggregation and emission from `check_crate_post` after the summary step.
  6. Confirm `bootstrap_zero_diagnostic` and `collection_zero_diagnostic`
     remain silent. If `collection_zero_diagnostic` now emits, record why and
     either restructure the example so each helper still fails a rule, or
     rename it and bless the diagnostic. Never change thresholds to hide it.
  7. Refactor and gate.
- Requirements and gaps: `LA-DIAG`, `LA-TRIG` wiring, `LA-TRIG+`,
  `SUITE-L10N`, `INT-SPAN`.
- Acceptance evidence: both new UI tests pass (byte-exact `.stderr`; empty
  output without `--test`); `EMIT-ATTR`, `EMIT-ONCE`, and `PREC-RULES` observed
  in the fixture; three gates pass; `make test-workflow-contracts` passes.
- Conformance check: `LA-DIAG` satisfied with `span_lint_hir_and_then` at the
  helper `HirId`; `A-LEVEL` and `A-COMBINED` confirmed; new public item
  `clippy_utils::diagnostics::span_lint_hir_and_then` only.
- Recovery: revert the milestone commits; the summary extraction commit can
  stay.
- Remaining gaps: full UI matrix and configuration variants (8.2.4), Fluent
  files (8.5.2), remediation guide (8.5.3).
- Compatibility decision: none.

### EP-M5: dogfood precision gate

- Outcome: measured precision on real `rstest` code.
- Work: run the experimental lint, using the command fixed in EP-M0, over
  `whitaker-common --all-targets` and the `rstest`-using test targets of the
  workspace. Triage every diagnostic as true or false positive in a table in
  `Artefacts and notes`. A false positive that an existing rule should have
  excluded is a defect to fix with a new UI row; a false positive that needs a
  new rule triggers escalation per `Tolerances`. Do not change Whitaker's own
  code in response to true positives in this item; list them as follow-ups.
- Acceptance evidence: the triage table with zero untriaged and zero
  unexplained false positives.
- Conformance check: `LA-TRIG+` holds on real code; no interface change.
- Recovery: findings feed back into EP-M4 as new rows.
- Compatibility decision: none.

### EP-M6: documentation, ADR, and roadmap closure

- Outcome: every document reflects the shipped behaviour; roadmap entry done.
- Work:
  1. `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: add
     `## Implementation decisions (8.2.3)` summarizing the `Decision log`,
     amend §Trigger conditions for lint A and §Fixture-local classification
     with `LA-TRIG+` and `LA-FIX+`, and reference the ADR from §Diagnostic
     strategy. State that lint A has a single anchor and no
     `emit_once_per_group` option, unlike lint C.
  2. `docs/adrs/adr-0014-rstest-helper-aggregation-and-emission-anchoring.md`
     (serial re-confirmed) following `docs/documentation-style-guide.md`
     (lines 524 to 648), including the deferred alternatives; add it to
     `docs/contents.md`.
  3. `docs/users-guide.md` §`rstest_helper_should_be_fixture` (line 943):
     replace the "Bootstraps" and "diagnostic-silent" text with the trigger
     contract, the meaning of "call" and "test", per-compilation-unit counts,
     the diagnostic shape, suppression (`#[allow]`, and
     `#[cfg_attr(test, expect(...))]`), the excluded callee kinds and call
     shapes, the rstest versions whose expansion was verified, and that
     unknown expansions produce no diagnostics.
  4. `docs/developers-guide.md`: document
     `clippy_utils::diagnostics::span_lint_hir_and_then` (scope: emitting a
     lint whose level must follow a node other than the one being checked;
     permitted callers: any Dylint lint crate; composition: wrap it with
     lint-specific message resolution, do not re-implement it), the
     `helper_group` kernel (scope, the rule that contract changes update the
     Verus sidecar and oracle), `collect_rstest_inner_test_bodies`,
     `rstest_fixture_items`, and `fixture_bound_parameters` (reuse by lint C
     permitted; semantics fail closed). Update §Standard vs Experimental Lints
     (line 5056). Record the dead lint-crate unit tests as a known gap.
  5. `docs/roadmap.md`: mark 8.2.3 `[x]` with a status paragraph in the style
     of 8.2.2.
  6. Run `make fmt`, `make markdownlint`, and `make nixie`.
- Acceptance evidence: Markdown gates and the three code gates pass.
- Conformance check: design document and ADR updated with every decision; no
  unrecorded deviation; `Outcomes & retrospective` completed.
- Recovery: documentation-only reverts.
- Compatibility decision: none.

## Concrete steps

Run every command from the worktree root
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/9f4c7960-0994-4190-a861-391a6cfd4f40`.
Capture gate output with `tee`:

```bash
BR=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BR}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BR}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BR}.out"
```

Run gates sequentially, never in parallel for this worktree. Prefer delegating
full gate runs to the `scrutineer` agent and reading its cited logs rather than
re-running a gate. A build printing
`[build-limits] ... waiting for a compile slot` is queued, not hung; inspect
admission with `build-limits-status`.

Focused commands while iterating:

```bash
cargo nextest run -p whitaker-common -E 'test(helper_group) | binary(rstest_helper_group_behaviour)' 2>&1 | tee "/tmp/helper-group-whitaker-${BR}.out"
cargo nextest run -p rstest_helper_should_be_fixture --all-features --test ui 2>&1 | tee "/tmp/ui-whitaker-${BR}.out"
make verus-rstest-helper 2>&1 | tee "/tmp/verus-whitaker-${BR}.out"
make test-workflow-contracts 2>&1 | tee "/tmp/workflow-contracts-whitaker-${BR}.out"
cargo rustc -p rstest_helper_should_be_fixture --example collection_zero_diagnostic -- --test -Zunpretty=hir > "/tmp/hir-whitaker-${BR}.out"
```

Documentation milestones also run:

```bash
make fmt 2>&1 | tee "/tmp/fmt-whitaker-${BR}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BR}.out"
make nixie 2>&1 | tee "/tmp/nixie-whitaker-${BR}.out"
```

Commit after each green step with an imperative subject naming the milestone,
for example `Add helper-group qualification kernel (8.2.3 EP-M1)`.

## Validation and acceptance

The feature is accepted when all of the following hold.

Behaviour through the real driver: compiling
`crates/rstest_helper_should_be_fixture/examples/aggregation_diagnostics.rs`
with `--test` yields exactly `aggregation_diagnostics.stderr`, namely one
warning per qualifying helper (one per `argument_shape` variant, with the note
cap exercised) and none for the allowed, expected, single-test, case-fed, and
every `PREC-RULES` row, with no `unfulfilled_lint_expectations`. Compiling the
same example without `--test` produces no output. The EP-M5 triage table shows
no unexplained false positives.

Red-Green-Refactor evidence to record in `Artefacts and notes`:

- EP-M1 red: the skeleton returns `Rejected(Empty)`, so every qualifying row
  fails on its assertion. Green: every row, scenario, and property passes.
- EP-M2: the negative control makes Verus report `L-REF` failing; the
  committed sidecar verifies.
- EP-M3 red: the exact-summary test fails, showing `callee=helper;records=2`
  and `fixture-local` for the case input. Green: the repaired summary matches.
- EP-M4 red: `example_reports_expected_diagnostics` fails with empty actual
  output. Green: byte-exact match; the non-`--test` test passes.

Behaviour specification, `common/tests/features/rstest_helper_group.feature`:

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
    Then the helper is rejected for differing constants at position 0

  Scenario: Strict fixture names reject differently named fixtures
    Given a helper group where test "a" passes fixture "db" and test "b" passes fixture "database"
    And thresholds of 2 calls and 2 distinct tests with identical fixture names required
    When the helper group is evaluated
    Then the helper is rejected for differing fixture names at position 0

  Scenario: Lenient fixture names accept differently named fixtures
    Given a helper group where test "a" passes fixture "db" and test "b" passes fixture "database"
    And thresholds of 2 calls and 2 distinct tests
    When the helper group is evaluated
    Then the helper qualifies with 2 calls across 2 tests
    And the argument shape is "fixtures"
```

Step definitions follow `docs/rstest-bdd-users-guide.md` and
`docs/developers-guide.md` §Assertions in fallible tests: `given` and `when`
steps return `Result` and never assert; assertions live in `then` steps.

Quality criteria:

- Tests: `make test` passes, including the new `whitaker-common` unit,
  property, snapshot, and BDD tests and the lint crate UI tests;
  `make test-workflow-contracts` passes.
- Verification: `make verus-rstest-helper` and `make verus` pass with the
  obligations discharged (or the pre-approved fallback recorded); every seeded
  fault and the Verus negative control were observed to fail.
- Lint and format: `make check-fmt` and `make lint` pass with no new
  suppressions beyond the narrowly scoped `unknown_lints` allowance in the
  example fixture.
- Documentation: `make markdownlint` and `make nixie` pass.

## Idempotence and recovery

Every step is re-runnable. HIR dumps and logs go to `/tmp` and are overwritten;
scratch builds use the workspace target directory. UI fixtures are compared,
never regenerated automatically; to re-bless a `.stderr` file, review the
harness's actual output, copy it, and record why in the `Decision log`. If a
milestone fails part-way, commit the partial work as a clearly labelled
work-in-progress commit on this branch or reset to the last green commit after
confirming nothing else is uncommitted. Never use bare `git stash`.

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

Upstream Clippy `span_lint_hir_and_then`, the shape the stub mirrors:

```rust
pub fn span_lint_hir_and_then(
    cx: &LateContext<'_>,
    lint: &'static Lint,
    hir_id: HirId,
    sp: impl Into<MultiSpan>,
    msg: impl Into<DiagMessage>,
    f: impl FnOnce(&mut Diag<'_, ()>),
) {
    cx.tcx.emit_node_span_lint(lint, hir_id, sp, rustc_errors::DiagDecorator(|diag| {
        diag.primary_message(msg);
        f(diag);
        docs_link(diag, lint);
    }));
}
```

## Interfaces and dependencies

In `crates/clippy_utils/src/diagnostics.rs` (new; the stub already enables
`rustc_private`):

```rust
/// Emit `lint` at `span`, resolving its level and expectations from `hir_id`.
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
`whitaker_common::rstest` and the crate root like the other `rstest` models):

```rust
pub struct HelperThresholds { /* min_calls, min_distinct_tests, require_identical_fixture_arg_names */ }
impl HelperThresholds {
    pub const fn new(min_calls: usize, min_distinct_tests: usize, require_identical_fixture_arg_names: bool) -> Self;
}

pub struct HelperCallEvidence<'a, T> {
    pub test: T,
    pub fingerprint: &'a ArgFingerprint,
}

pub enum ConstantValue { Literal(String), Path(String) }

pub enum PositionShape {
    Fixture { shared_name: Option<String> },
    Constant(ConstantValue),
}

pub enum ArgumentShape { NoArguments, FixturesOnly, ConstantsOnly, FixturesAndConstants }
impl ArgumentShape {
    /// Fluent selector value: `none`, `fixtures`, `constants`, or `mixed`.
    pub const fn fluent_selector(self) -> &'static str;
}

pub struct HelperGroupSummary { /* call_count, distinct_test_count, positions */ }
impl HelperGroupSummary {
    pub const fn call_count(&self) -> usize;
    pub const fn distinct_test_count(&self) -> usize;
    pub fn positions(&self) -> &[PositionShape];
    pub fn argument_shape(&self) -> ArgumentShape;
}

pub enum HelperGroupRejection {
    Empty,
    UnsupportedArgument { position: usize },
    ArityMismatch { min_arity: usize, max_arity: usize },
    MixedKinds { position: usize },
    DifferingConstants { position: usize },
    DifferingFixtureNames { position: usize },
    BelowMinCalls { found: usize, required: usize },
    BelowMinDistinctTests { found: usize, required: usize },
}

pub enum HelperGroupVerdict {
    Qualifies(HelperGroupSummary),
    Rejected(HelperGroupRejection),
}

pub fn evaluate_helper_group<'a, T: Eq + Hash>(
    calls: impl IntoIterator<Item = HelperCallEvidence<'a, T>>,
    thresholds: HelperThresholds,
) -> HelperGroupVerdict;
```

Rejection precedence (best effort, pinned by tests only): `Empty`,
`UnsupportedArgument` (least position), `ArityMismatch`, then the least
inconsistent position reported as `MixedKinds`, `DifferingConstants`, or
`DifferingFixtureNames`, then `BelowMinCalls`, then `BelowMinDistinctTests`.

In `crates/whitaker_lint_core/src/hir/` (new modules; existing functions
unchanged):

```rust
// rstest_roots.rs
pub fn collect_rstest_inner_test_bodies(cx: &LateContext<'_>) -> HashSet<hir::HirId>;
// rstest_bindings.rs
pub fn rstest_fixture_items(cx: &LateContext<'_>) -> RstestFixtureItems;
pub fn fixture_bound_parameters(cx: &LateContext<'_>, root: LocalDefId, fixtures: &RstestFixtureItems)
    -> Option<FixtureBindings>;
```

In the lint crate, new private modules `src/summary.rs` (extracted),
`src/aggregate.rs`, `src/diagnostic.rs`, and possibly `src/call_shape.rs`.

Message contract for 8.5.2: key `rstest_helper_should_be_fixture`, attributes
`.note`, `.more_calls`, and `.help`; arguments `helper` (string), `calls`,
`tests`, and `shown_calls` (integers), and `argument_shape` (selector values
`none`, `fixtures`, `constants`, `mixed`; default variant `*[mixed]`). English
fallbacks:

```plaintext
primary:    Helper `{helper}` behaves like a fixture in `rstest` tests.
note:       none      Found {calls} calls across {tests} tests, all without arguments.
            fixtures  Found {calls} calls across {tests} tests with stable fixture arguments.
            constants Found {calls} calls across {tests} tests with stable constant arguments.
            mixed     Found {calls} calls across {tests} tests with stable fixture and constant arguments.
more_calls: Only the first {shown_calls} call sites are shown.
help:       none/fixtures  Provide `{helper}` through a `#[fixture]` and inject it as a test parameter.
            constants      Provide `{helper}` through a `#[fixture]` that supplies the repeated
                           constants, and inject it as a test parameter.
            mixed          Provide `{helper}` through a `#[fixture]` that depends on the same
                           fixtures and supplies the repeated constants, and inject it as a test
                           parameter.
```

Dev-dependencies added (all workspace-pinned): `googletest` (only if a test
file chooses matcher style; one assertion style per file), `pretty_assertions`,
and `insta` in `common/Cargo.toml`; `pretty_assertions` in the lint crate for
the exact-summary comparison. No runtime dependency is added to any crate.

Signposts: `docs/whitaker-dylint-suite-design.md` (localization, suite wiring),
`docs/whitaker-clone-detector-design.md` (precedent for deterministic grouping
and for Verus sidecars beside pure kernels),
`docs/rust-testing-with-rstest-fixtures.md` (fixture semantics, `#[with]`,
`#[from]`, `#[once]`), `docs/rust-doctest-dry-guide.md` (doc examples on new
public `whitaker_common` items; `ignore` examples for rustc-private items),
`docs/complexity-antipatterns-and-refactoring-strategies.md` (keep
`observe`/`finish` and the aggregation adapter free of the Bumpy Road shape),
and `docs/rstest-bdd-users-guide.md` (step definitions). Skills: `execplans`,
`rust-router`, `rust-unit-testing`, `proptest`, `verus`, `rust-verification`,
`nextest`, `arch-decision-records`, `codegraph-mcp`, `en-gb-oxendict-style`.

## Revision note

- 2026-11-11, initial draft after Wyvern reconnaissance and HIR
  characterization.
- 2026-11-11, revised after the community-of-experts review. Changes: added
  `Decisions requiring approval`; narrowed the `#[expect]` contract and added a
  non-`--test` UI case (all reviewers); added the `LA-TRIG+` precision rules
  and the EP-M5 dogfood gate (failure-modes pre-mortem); replaced source-text
  provider lexing with binding provenance (`LA-FIX+`), removing the
  `rustc_lexer` dependency and covering compact syntax (alternatives and
  failure-modes reviews); moved the emitter to `clippy_utils::diagnostics` and
  fixed its type imports and the `Diag<'_, ()>` signature (structure review);
  bounded test identity on `Eq + Hash` and regrouped buckets by `DefId`
  (structure review); respecified the Verus scope as the full qualifying
  verdict with `INV-ORDER` as a corollary of `L-REF`, a fold model, and no
  proof of rejection precedence, with a three-session budget and pre-approved
  fallback (verification review); fixed the acceptance filter, the nextest
  override and workflow contract updates, the generator design, and the red
  steps (verification review); split inconsistency rejections and added
  rejection counts to the summary (alternatives review); reordered milestones
  so the pure kernel and proof precede the evidence repair, with a hard
  constraint that diagnostics follow the repair. Effect on remaining work:
  EP-M3 and EP-M4 are larger; EP-M5 is new; the ADR covers more decisions.
