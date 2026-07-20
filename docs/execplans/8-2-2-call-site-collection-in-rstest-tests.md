# Call-site collection in `#[rstest]` tests

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 8.2.2 grows the `rstest_helper_should_be_fixture` lint from the
8.2.1 bootstrap into a passive, evidence-collecting late lint. After this
ExecPlan is implemented, the lint will walk every strict `#[rstest]` test in
the crate, recognize local helper calls inside those tests, classify each
positional argument as either a fixture-local binding, a stable literal, a
stable constant path, or unsupported, and record one
`whitaker_common::rstest::ArgFingerprint` per call. The lint pass keeps
records in a deterministic `BTreeMap<String, Vec<CallSiteRecord>>`, keyed by
callee definition path. Its Visitor traverses nested and deferred closure
bodies while preserving closure-span fallback context, ready for roadmap item
8.2.3 to apply thresholds and emit diagnostics.

The observable outcome of 8.2.2 is therefore structural rather than diagnostic.
A maintainer can:

- inspect the collected call-site map at the end of a crate-post hook via
  `debug!` logging targeted at the lint name,
- read unit and property tests proving source-span deduplication, fingerprint
  stability, and deterministic HIR ordering, and
- run the Cargo-backed `collection_zero_diagnostic` UI harness through the lint
  pipeline as the behavioural and end-to-end validation boundary.

The lint must remain diagnostic-silent. Aggregation thresholds, helper-class
filtering by trigger conditions, and `span_lint_hir_and_then` calls all belong
to roadmap item 8.2.3. UI pass/fail fixtures and the final `whitaker.toml`
schema notes belong to 8.2.4. The roadmap tick for 8.2.2 only lands after 8.2.2
has its own gates green and CodeRabbit clean.

This plan was approved before implementation started.

## Constraints

- Implement only roadmap item 8.2.2. Do not advance into 8.2.3 threshold
  evaluation, fingerprint-pattern filtering, or `span_lint_hir_and_then`
  diagnostics. Do not author UI `ui/fail_*.rs` fixtures that depend on
  diagnostic emission.
- Preserve the public contract of `crates/rstest_helper_should_be_fixture/`:
  keep `RSTEST_HELPER_SHOULD_BE_FIXTURE` exported as a `&'static Lint`, keep
  `RstestHelperShouldBeFixture` as the registered pass type, and keep the
  experimental suite feature `experimental-rstest-helper-should-be-fixture`
  unchanged.
- Reuse `whitaker_common::rstest` for detection, parameter classification,
  argument fingerprints, and span-recovery policy. Reuse the `whitaker::hir`
  adapter for any `rustc_span::Span` → recovery-frame conversion. Do not
  duplicate those policies in the lint crate.
- Argument lowering (HIR → `ArgAtom`) must live in a compiler-aware adapter
  layer — either inside `crates/rstest_helper_should_be_fixture/src/` if it
  stays single-use, or inside `src/hir/` if more than one downstream crate
  (8.3.x, 8.4.x) is expected to consume it. The pure `ArgAtom`/`ArgFingerprint`
  data model in `common/src/rstest/argument_fingerprint.rs` must not gain a
  rustc dependency.
- Conservative recall over precision: lower an argument to
  `ArgAtom::Unsupported` whenever resolution is uncertain (closure call, block
  expression, dereference, unknown path), and skip a whole call site whenever
  `whitaker::hir::recover_user_editable_hir_span` returns `None`. Never emit
  evidence based on a macro-only span.
- Restrict callee resolution to local-crate function definitions for v1, as
  required by the design document. External-crate or trait-method callees must
  be ignored at collection time.
- Keep every Rust source file at or below 400 lines. Each module must open
  with a `//!` module-level purpose comment. Public APIs need Rustdoc with at
  least one example.
- Run `make check-fmt`, `make lint`, and `make test` after each major
  milestone, sequentially (no parallel cargo invocations). Capture each run's
  output through `tee` into
  `/tmp/${ACTION}-whitaker-$(git branch --show-current).out`.
- Use the shared default Cargo cache. Do not introduce per-job caches.
- Localization: do not add new English Fluent messages for diagnostics that
  8.2.2 does not emit. Adding a placeholder fluent file in `common/locales/` is
  acceptable only if it documents future keys without affecting any resolution
  path.
- Use the repository's en-GB-oxendict spelling in prose: `-ize` (and `-yse`
  where applicable) and `-our`. Identifiers, external API names, and inline
  code may keep their existing American spelling.
- Do not mark roadmap item 8.2.2 done in `docs/roadmap.md` until
  implementation has landed, gates pass, CodeRabbit has no unresolved concerns,
  and the implementation PR is ready for review.

## Tolerances

- Scope: if implementation requires more than 18 changed files or more than
  900 net code lines (production plus tests), stop and ask whether the work
  should be split.
- Interface: if a public API in `whitaker_common`, `whitaker`, or
  `whitaker_suite` must change in an incompatible way (signature, item
  visibility, or trait bound), stop and present options.
- Dependencies: if a new third-party dependency is required (in particular,
  rustc-private crates beyond those already pinned in the workspace), stop and
  justify it before adding it.
- Test surface: if the Cargo-backed UI harness cannot demonstrate the
  collector's behaviour in user terms after one attempt, stop and choose
  between extending the observability surface and retaining only unit proof.
- Iterations: if `make test` still fails after two targeted fix attempts on
  the same milestone, stop and document the failing command, log path, and
  remaining options.
- Property test stability: if any `proptest` case in this work shrinks to a
  non-reproducible failure after two re-runs at fixed seeds, stop and document
  the seed and shrink trace before disabling the case.
- Span recovery edge cases: if `recover_user_editable_hir_span` returns
  `Some` for a call site that the design treats as macro-only, stop and decide
  whether to refine the span-recovery policy in `common::rstest::span` or relax
  the lint's filter.
- Review: if `coderabbit review --agent` reports concerns after a major
  milestone, address them or record why they are out of scope before moving on.
  Re-run until 0 findings.
- Documentation drift: if user-visible behaviour or configuration changes
  during implementation, stop, update `docs/users-guide.md` and
  `docs/developers-guide.md` in the same milestone, then resume.
- Ambiguity: if any of the items below have more than one plausible
  interpretation that materially affects test signals, stop and present the
  options:
  - what "fixture-local" means when the parameter is shadowed inside the test
    body,
  - how to attribute a call site that lies inside a `#[case]`-driven test
    function generated by the `rstest` macro,
  - whether `const`-vs-`static` paths should keep the same `ArgAtom::ConstPath`
    variant in v1.

## Risks

- Risk: macro-expansion-only call sites can pollute the collected map and
  inflate later thresholds. Severity: high. Likelihood: medium. Mitigation:
  drop every record whose `recover_user_editable_hir_span` is `None`. Add at
  least one regression test that constructs a macro-only call in source and
  asserts the collector ignores it.

- Risk: `#[case]`-driven `#[rstest]` expansion generates an outer module and
  one inner `#[test] fn case_N()` per case. The same helper call therefore
  appears `N` times even though the user wrote it once. Counting the generated
  sites separately would distort 8.2.3 thresholds. Severity: high. Likelihood:
  high. Mitigation: key the call-site map on the callee definition path and
  the user-editable call-site location, while keeping the source `DefId` on
  each record. Compare via the source span returned by recovery before
  deduplicating.

- Risk: callee resolution returns no `DefId` when the call goes through a
  trait method, a generic function whose instantiation is not yet known, or a
  closure invocation. Recording these as helper calls would create
  false-positive evidence at 8.2.3. Severity: medium. Likelihood: high.
  Mitigation: drop the record when `type_dependent_def_id` returns `None` for a
  method call and when `qpath_res` does not return `Res::Def(Fn | …, DefId)`
  for a direct call. Limit collection to callees whose `DefId.krate` is
  `LOCAL_CRATE` and whose `DefKind` is `Fn` or `AssocFn`.

- Risk: argument lowering treats macro-injected literals as `ConstLit` even
  though the user did not write them, producing spurious fingerprint collisions
  across tests. Severity: medium. Likelihood: medium. Mitigation: lower an
  argument to `ArgAtom::Unsupported` whenever its user-editable span is `None`
  or its expansion call-site differs from the enclosing call's call-site. Cover
  this with at least one regression test.

- Risk: `const`-vs-`static` semantics differ in mutability under unsafe code.
  Treating them with the same `ConstPath` atom understates risk. Severity: low.
  Likelihood: medium. Mitigation: keep the `ConstPath` atom for both in v1,
  document the decision under "Decision log", and flag follow-up refinement for
  8.2.3.

- Risk: collecting across the whole crate in a `&mut self` lint pass can
  conflict with rustc's traversal contract if the pass is shared across threads
  in the future. Severity: low. Likelihood: low. Mitigation: use plain owned
  `BTreeMap`/`Vec` fields on the pass. Do not introduce `RefCell` or
  `Arc<Mutex<_>>` unless rustc requires it. If future Dylint changes ever
  require shared state, escalate.

- Risk: full workspace gates are expensive and may surface unrelated
  failures on the nightly toolchain. Severity: medium. Likelihood: low.
  Mitigation: run focused `cargo nextest` first, then full `make test`
  sequentially. Tee output to `/tmp` so unrelated failures can be cited.

- Risk: documentation drift between
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md` and the in-tree
  behaviour can confuse downstream consumers. Severity: medium. Likelihood:
  medium. Mitigation: record any deviation under "Implementation decisions
  (8.2.2)" in the design document and link the ExecPlan section number from the
  design note.

## Progress

- [x] (2026-06-04T01:44:41Z) Loaded `leta`, `rust-router`,
  `rust-types-and-apis`, `arch-crate-design`, `execplans`, and
  `commit-message`, then confirmed the leta workspace was already registered
  for this checkout.
- [x] (2026-07-15T00:00:00Z) Resolved the remaining verification note by
  checking the implemented collector and harness docs directly with Wyvern
  agents; no Firecrawl pass was performed for this completed plan.
- [x] (2026-06-04T01:44:41Z) Treated the user's explicit request to
  "proceed with implementation" as approval to execute the existing ExecPlan.
- [x] (2026-06-04T01:51:51Z) Added the HIR-to-`ArgAtom` adapter and
  call-site collector under
  `crates/rstest_helper_should_be_fixture/src/collector.rs` (and any helper
  modules), wired into the existing driver.
- [x] (2026-06-04T01:51:51Z) Added the `check_fn`/`check_crate_post` hooks to
  `RstestHelperShouldBeFixture` so the collector populates state per test and
  the crate-post hook drains it into a `debug!` log without emitting
  diagnostics.
- [x] (2026-06-04T01:56:56Z) Added focused unit coverage for deterministic
  collector ordering and source-span deduplication. Property and end-to-end
  coverage remain to be added in the next stage.
- [x] (2026-06-04T02:46:39Z) Added parameterized unit coverage for duplicate
  and distinct source spans, a `proptest` insertion-order invariant for
  collector determinism, and the Cargo-backed `collection_zero_diagnostic` UI
  harness proving collection remains diagnostic-silent for fixture, literal,
  `const`, and `static` arguments.
- [x] (2026-06-04T04:44:51Z) Expanded the shared HIR span-recovery regression
  with an explicit `macro_only_hir_span_has_no_user_editable_recovery` test,
  covering the helper the collector uses to drop call sites whose spans cannot
  recover to user-editable source.
- [x] (2026-06-04T04:46:57Z) Validated the macro-only span regression with
  library-only `cargo nextest run` for
  `macro_only_hir_span_has_no_user_editable_recovery`, `make check-fmt`,
  `make lint`, `make test`, and `make markdownlint`; the full workspace test
  run executed 1459 tests with 1459 passed and 3 skipped.
- [x] (2026-06-04T05:20:01Z) Requested a full CodeRabbit review for the
  shared-source milestone; the full-scope CLI stalled after sandbox
  preparation, so it was terminated after verifying the PID belonged to this
  worktree. A scoped `coderabbit review --agent --dir src` then completed with
  0 findings after one rate-limit backoff.
- [x] (2026-06-04T01:56:56Z) Validated the first implementation milestone with
  `cargo check -p rstest_helper_should_be_fixture --all-targets --all-features`,
  `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`,
  `make check-fmt`, `make lint`, and `make test`, each captured through `tee`.
- [x] (2026-06-04T02:41:31Z) Ran `coderabbit review --agent`; two full-scope
  attempts reached `tools_completed` and then hung in the CLI without stored
  findings, with one intervening rate-limit wait handled by `vsleep`. A scoped
  `coderabbit review --agent --dir crates/rstest_helper_should_be_fixture`
  completed with 0 findings for the implemented lint-crate milestone.
- [x] (2026-06-04T02:46:39Z) Validated the second test-coverage milestone with
  `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`,
  `make check-fmt`, `make lint`, and `make test`; the full workspace test run
  executed 1459 tests with 1459 passed and 3 skipped.
- [x] (2026-06-04T03:37:16Z) Addressed two CodeRabbit findings from the
  second milestone by exercising helper symbols from the zero-diagnostic UI
  fixture's `main` function and strengthening the collector's direct storage
  assertions.
- [x] (2026-06-04T03:37:16Z) Revalidated after the CodeRabbit fixes with
  `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`,
  `make check-fmt`, `make lint`, `make test`, and `make markdownlint`; the full
  workspace test run again executed 1459 tests with 1459 passed and 3 skipped.
- [x] (2026-06-04T04:09:23Z) Addressed the valid trivial CodeRabbit style
  finding by collapsing the trybuild fixture function to a single-line simple
  return, rejected an equivalent macro fixture form because it produces an
  `unused_braces` compiler warning under the macro expansion, and revalidated
  with focused nextest, `make check-fmt`, `make lint`, and `make test`.
- [x] (2026-06-04T04:12:23Z) Documented the intentional `static` argument path
  in the zero-diagnostic UI fixture after CodeRabbit suggested normalizing it
  to `const`; the fixture keeps both forms so the collector exercises constant
  and static definition paths in one pass case.
- [x] (2026-06-04T04:43:04Z) Re-ran
  `coderabbit review --agent --dir crates/rstest_helper_should_be_fixture`
  after the static-path rationale and all deterministic gates; CodeRabbit
  completed with 0 findings for the test-coverage milestone.
- [x] (2026-06-04T05:21:45Z) Updated `docs/users-guide.md`,
  `docs/developers-guide.md`, and
  `docs/lints-for-rstest-fixtures-and-test-hygiene.md` to describe passive
  collection, diagnostic silence, the in-crate collector pattern, and 8.2.2
  implementation decisions.
- [x] (2026-06-04T05:21:45Z) Marked `docs/roadmap.md` 8.2.2 as complete after
  implementation, gates, and CodeRabbit review for the implementation and
  regression milestones were green.
- [x] (2026-06-04T05:25:13Z) Validated the documentation milestone with
  `make markdownlint` and `make nixie`. A scoped
  `coderabbit review --agent --dir docs` request stalled after sandbox
  preparation; after terminating only this worktree's PID,
  `coderabbit review findings --plain` reported no stored findings.
- [x] (2026-06-04T05:27:51Z) Confirmed the branch was already named
  `8-2-2-call-site-collection-in-rstest-tests`, pushed the implementation
  commits to origin, and updated draft PR
  [#235](https://github.com/leynos/whitaker/pull/235).
- [x] (2026-06-16T00:00:00Z) Consolidated duplicate-call and distinct-span
  storage outcomes in the parameterized `collector_records_calls_by_source_span`
  unit test, preserving the existing semantics.
- [x] (2026-06-16T00:00:00Z) Moved the `check_fn` call-site collection body
  into the private `collect_call_sites` helper so the `LateLintPass`
  implementation now delegates with one statement and the collection boundary
  is explicit in the driver.
- [x] (2026-06-16T00:00:00Z) Revalidated the focused lint crate after the
  refactor with
  `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`,
  which ran 23 tests with 23 passed. `make check-fmt` also passed before the
  refactor commit.
- [x] (2026-06-16T00:00:00Z) Fixed a Mermaid parser failure in
  `docs/whitaker-dylint-suite-design.md` by flattening wrapped node labels in
  the consumer adoption flow; `make --no-print-directory markdownlint nixie`
  then passed with all diagrams validated.
- [x] (2026-06-16T00:00:00Z) Updated `docs/roadmap.md` item 8.2.2 with the
  completed collector behaviour, final refactor note, validation coverage, and
  the explicit boundary that diagnostics remain 8.2.3 work.
- [x] (2026-06-16T00:00:00Z) Pushed the completed implementation, refactor,
  Mermaid fix, and roadmap update to
  `origin/8-2-2-call-site-collection-in-rstest-tests`.
- [x] (2026-07-20T00:00:00Z) Reclassified direct collector checks as unit and
  property coverage, removed the duplicate synthetic BDD seam and its
  crate-local dependencies, and retained the Cargo-backed UI harness as the
  behavioural and end-to-end boundary.

Each completed item should be timestamped, e.g.,
`- [x] (2026-05-28T12:30:00Z) Drafted this ExecPlan.`.

## Surprises & discoveries

- Discovery: `rustc_span::def_id::DefId` is not `Ord` on the pinned rustc
  toolchain, so it cannot be used directly as a `BTreeMap` or `BTreeSet` key.
  Impact: `CallSiteCollector` now keys deterministic maps and deduplication on
  `cx.tcx.def_path_str(callee_def_id)` while preserving the raw callee `DefId`
  inside each `CallSiteRecord` for downstream lookup.

- Discovery: full-scope `coderabbit review --agent` can reach
  `tools_completed` and then hang in CodeRabbit CLI v0.5.3 without writing
  stored findings. Impact: the first milestone used the same agent-mode review
  scoped to `crates/rstest_helper_should_be_fixture`, which completed with 0
  findings. Future milestones should retry full-scope review after
  documentation and roadmap changes land.

- Discovery: `make fmt` runs Markdown formatting and then Markdown lint across
  the whole repository; existing unrelated Markdown lint errors can make the
  target fail even when `cargo fmt --all` succeeds. Impact: unrelated formatter
  churn from that command was restored, and this milestone continues to use
  `make check-fmt` plus `make markdownlint` as the deterministic gates.

- Discovery: the macro-only call-site drop is enforced through
  `whitaker::hir::recover_user_editable_hir_span` before the collector records
  evidence. Impact: the most direct deterministic regression lives in
  `src/hir/tests.rs`, where the test can construct a macro-only `Span` without
  synthesizing a `rustc_lint::LateContext`; the lint crate continues to cover
  end-to-end silence through trybuild fixtures.

- Discovery: `cargo nextest run -p whitaker --all-targets --all-features` tries
  to link rustc-private integration-test binaries outside the Makefile wrapper
  and fails with duplicate `std`/`core` linkage errors. Impact: focused
  validation for the span regression uses the library-only target
  (`cargo nextest run -p whitaker --lib ... --all-features`), while full
  validation remains the standard `make test` target.

## Decision log

- Decision: Keep this ExecPlan to call-site collection and constant-aware
  argument fingerprinting only. Diagnostics, threshold filtering, and UI
  pass/fail cases stay assigned to 8.2.3 and 8.2.4 respectively. Rationale: the
  roadmap entry explicitly separates 8.2.2 from 8.2.3/8.2.4, and the 8.2.1
  bootstrap commits to a silent collector phase. Author/Date: Codex /
  2026-05-28.

- Decision: Place the HIR-to-`ArgAtom` adapter inside the lint crate
  (`crates/rstest_helper_should_be_fixture/src/collector.rs`) for v1, with
  small pure helpers tested independently. Rationale: only one lint consumes
  this lowering in 8.2.2; promoting it to `src/hir/` or to `common/src/hir/`
  adds an export surface that no other crate calls. If roadmap items 8.3.x or
  8.4.x need the adapter later, promote it through a separate refactor commit.
  Author/Date: Codex / 2026-05-28.

- Decision: Use `cx.qpath_res` and `cx.typeck_results().type_dependent_def_id`
  directly rather than wrapping them in a `clippy_utils`-style helper.
  Rationale: the workspace's local `crates/clippy_utils` is a small panic-only
  stub; pulling in upstream `clippy_utils::fn_def_id` would add a rustc-private
  dependency only to fold two cases that already match the pattern used in
  `no_std_fs_operations/src/driver.rs`. Author/Date: Codex / 2026-05-28.

- Decision: Treat `const`, associated `const`, and `static` paths with the
  same `ArgAtom::ConstPath` atom for v1. Rationale: matches the existing pure
  model in `common/src/rstest/argument_fingerprint.rs`, keeps fingerprint
  cardinality predictable, and defers the unsafe-mutability nuance to a
  documented follow-up. Author/Date: Codex / 2026-05-28.

- Decision: Do not introduce `check_crate_post` diagnostics in this milestone.
  Instead, the crate-post hook logs collected evidence at `debug!` level
  targeted at the lint name. Rationale: keeps the lint observable enough to
  validate behaviourally without claiming 8.2.3 emission semantics. The
  `debug!` line is gated by the existing logging setup, so it is invisible in
  normal cargo builds. Author/Date: Codex / 2026-05-28.

- Decision: Do not require Kani or Verus for 8.2.2. Rationale: the call-site
  collector and fingerprint adapter introduce data-structure invariants
  (determinism, deduplication) that fit within `proptest` coverage. Kani is
  reserved for later milestones (such as 8.2.3's threshold semantics) that
  might benefit from bounded state-space exploration. Author/Date: Codex /
  2026-05-28.

- Decision: Use `tcx.def_path_str(callee_def_id)` as the ordered collection
  key and keep `DefId` on `CallSiteRecord`. Rationale: the pinned rustc `DefId`
  implements equality and hashing but not ordering, while the roadmap requires
  deterministic per-callee collection. Definition paths provide the stable
  ordering surface already used elsewhere in Whitaker, and the raw `DefId`
  remains available for later compiler-side lookup. Author/Date: Codex /
  2026-06-04.

- Decision: Keep the final refactor private to the lint crate and avoid public
  API changes. Rationale: the duplicated setup lived entirely in collector
  unit and behaviour tests, and moving `check_fn` logic into
  `collect_call_sites` improves the internal driver shape without changing
  lint registration, configuration, or observable diagnostics. Author/Date:
  Codex / 2026-06-16.

## Outcomes & retrospective

Roadmap item 8.2.2 is complete. The lint now passively collects local helper
call evidence from `#[rstest]` tests, classifies fixture-local, literal,
`const`, `static`, and unsupported argument atoms, deduplicates generated
`#[case]` siblings by recovered source span, and reports the collected evidence
only through `debug!` logging at crate-post time. It remains diagnostic-silent,
so threshold evaluation, user-facing suggestions, and `span_lint_hir_and_then`
emission remain cleanly scoped to roadmap item 8.2.3.

The main design choices held: compiler-aware lowering stayed inside
`crates/rstest_helper_should_be_fixture`, the pure fingerprint model in
`whitaker_common::rstest` did not gain rustc dependencies, and conservative
unsupported handling avoids recording uncertain helper evidence. The final
test refactor reduced duplicated setup in the unit and BDD coverage without
altering semantics, and the driver now exposes a private collection helper
that 8.2.3 can extend without growing `check_fn`.

Validation covered the focused lint crate, full workspace gates during the
implementation milestones, documentation linting, Mermaid diagram validation,
and scoped CodeRabbit reviews with 0 unresolved findings. The one follow-up
for 8.2.3 is to keep using the existing collector records as passive evidence
and add aggregation thresholds and diagnostics without changing the 8.2.2
fingerprint or source-span deduplication semantics.

## Context and orientation

Whitaker is a Rust Cargo workspace. The root `Cargo.toml` declares workspace
members `common`, `crates/*`, `installer`, and `suite`. Each lint lives in its
own Dylint cdylib crate under `crates/`. Shared pure helpers (no `rustc_*`
dependency) live in `common/`. Shared compiler-aware adapters (gated behind the
`dylint-driver` feature) live in `src/hir/`. The `suite/` crate aggregates lint
constituents into a single Dylint library.

The relevant roadmap entry is in `docs/roadmap.md`:

```plaintext
8.2.2. Implement call-site collection in `#[rstest]` tests, including
fixture-local classification and constant-aware argument fingerprinting.
```

The design source of truth is
`docs/lints-for-rstest-fixtures-and-test-hygiene.md`, especially "Lint A:
call-site fixture extraction" (sections "Trigger conditions for lint A",
"Fixture-local classification", and "Argument fingerprint model"). It specifies:

- v1 fixture-local classification covers simple identifier bindings only;
  destructured parameters are reported as `Unsupported`;
- the default provider-driven parameter attributes are
  `case`, `values`, `files`, `future`, and `context`;
- the four `ArgAtom` variants are `FixtureLocal`, `ConstLit`, `ConstPath`, and
  `Unsupported`;
- `ConstLit` captures literal source text and `ConstPath` captures a stable
  definition path for `const`, associated `const`, or `static`;
- callee resolution must be conservative — default to local-crate function
  definitions only.

Roadmap prerequisite 8.2.1 already exists. The crate at
`crates/rstest_helper_should_be_fixture/` is registered with the suite and
installer, loads configuration through `dylint_linting::config`, and constructs
`RstestDetectionOptions` in `check_crate`. It does not yet hook `check_expr` or
`check_fn`. The lint is exposed through the suite feature
`experimental-rstest-helper-should-be-fixture` and through
`installer/src/resolution.rs` `EXPERIMENTAL_LINT_CRATES`.

Roadmap prerequisites 8.1.1, 8.1.2, and 8.1.3 provide:

- `whitaker_common::rstest::is_rstest_test` /
  `is_rstest_test_with(attrs, trace, options)` for strict detection;
- `whitaker_common::rstest::RstestDetectionOptions` /
  `RstestParameter` / `classify_rstest_parameter` / `fixture_local_ids`;
- `whitaker_common::rstest::ArgAtom` /
  `whitaker_common::rstest::ArgFingerprint`;
- `whitaker_common::rstest::recover_user_editable_span` plus
  `whitaker::hir::recover_user_editable_hir_span` and
  `whitaker::hir::span_recovery_frames` as the compiler-aware adapters.

Useful existing patterns to copy:

- `crates/no_std_fs_operations/src/driver.rs` and `src/usage.rs` show
  callee `DefId` resolution with `cx.qpath_res(qpath, hir_id)` /
  `cx.typeck_results().type_dependent_def_id(hir_id)` and
  `cx.tcx.def_path_str(def_id)`.
- `crates/bumpy_road_function/src/driver.rs` shows per-function HIR walks
  driven from `check_item`, `check_impl_item`, and `check_trait_item`.
- `crates/no_expect_outside_tests/src/driver/mod.rs` shows
  `cx.tcx.hir_parent_iter(hir_id)` for locating an enclosing `ItemKind::Fn`,
  plus `whitaker::hir::collect_harness_test_functions` and
  `collect_rstest_companion_test_functions` for `--test` harness recovery.
- `crates/function_attrs_follow_docs/src/driver.rs` shows the existing call
  site for `whitaker::hir::recover_user_editable_hir_span`.
- `crates/clippy_utils/src/lib.rs` shows the small "either method-call
  `type_dependent_def_id` or path `qpath_res(...).opt_def_id()`" idiom that
  this work will reuse.

Useful skills:

- `leta` for navigation, `rust-router` for further routing.
- `rust-async-and-concurrency` is not relevant; the lint runs synchronously
  inside the late pass.
- `rust-types-and-apis` and `arch-crate-design` are relevant when deciding
  where to place the HIR adapter.
- `rust-errors` is relevant when deciding whether the adapter should return
  `Result<ArgAtom, _>` or always return an atom (it always returns an atom,
  including `Unsupported`).
- `hexagonal-architecture` is relevant for keeping pure policy in `common/`
  and compiler-aware adapters out of it.
- `kani` and `verus` are not required for 8.2.2 (see Decision log).
- `nextest` and `en-gb-oxendict` are relevant operationally.

Documentation surfaces that may need updates:

- `docs/users-guide.md` `rstest_helper_should_be_fixture` section — add a
  short note that collection is now active but diagnostics still belong to the
  later milestones.
- `docs/developers-guide.md` — document the in-crate "collector" pattern if
  it differs from existing lint conventions.
- `docs/lints-for-rstest-fixtures-and-test-hygiene.md` "Implementation
  decisions" — append an "Implementation decisions (8.2.2)" subsection if the
  implementation deviates from the design.

## Plan of work

The plan progresses through six stages. Each ends with validation. Do not
advance to the next stage until the previous stage's gates pass.

### Stage A — Pre-implementation review (no code changes)

Read this ExecPlan, the roadmap entry, the Lint A design section, and the 8.2.1
ExecPlan. Confirm the plan aligns with the design and with prior decisions.
Obtain explicit user approval before editing production code.

### Stage B — Argument lowering adapter

Add `crates/rstest_helper_should_be_fixture/src/collector.rs` (new module,
re-exported from `lib.rs` as `pub(crate) mod collector;`). The module owns two
pure pieces and one adapter:

1. `pub(crate) struct CallSiteRecord` stores the callee `DefId`, the
   `ArgFingerprint`, the source test `DefId`, and the recovered user-editable
   call span.
2. `pub(crate) struct CallSiteCollector` stores records in a
   `BTreeMap<String, Vec<CallSiteRecord>>`, where the string key is the
   callee definition path from `tcx.def_path_str(callee_def_id)`. It
   deduplicates with a private `CallSiteLocation` stored in a `BTreeSet`, so
   `#[case]`-generated siblings collapse into one record.
3. `pub(crate) fn lower_arg_atom<'tcx>(...) -> ArgAtom` is the compiler-aware
   HIR-to-`ArgAtom` adapter. It uses `HashSet<rustc_hir::HirId>` fixture-local
   ID collection, matches `Res::Local(binding_id)` bindings against those IDs,
   and classifies expression shape plus source recoverability.

Adapter rules, applied in order:

- if `recover_user_editable_hir_span(expr.span).is_none()`, return
  `ArgAtom::Unsupported`;
- if the expression is `ExprKind::Path(QPath::Resolved(None, path))` whose
  resolution is `Res::Local(binding_id)` for a binding whose `HirId` is in the
  fixture-local ID set, return `ArgAtom::FixtureLocal { name }`;
- if the expression is `ExprKind::Lit(lit)`, return
  `ArgAtom::ConstLit { text: snippet }` where `snippet` is the user-editable
  source slice of the literal taken via
  `cx.tcx.sess.source_map().span_to_snippet` (fall back to
  `lit.node.to_string()` when the snippet is unavailable);
- if the expression is `ExprKind::Path(qpath)` and
  `cx.qpath_res(qpath, expr.hir_id)` returns a `const`, associated `const`, or
  `static` definition, return
  `ArgAtom::ConstPath { def_path: cx.tcx.def_path_str(def_id) }`;
- otherwise, return `ArgAtom::Unsupported`.

For the call-side, add `resolve_local_callee` with this signature:

```rust
pub(crate) fn resolve_local_callee<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx rustc_hir::Expr<'tcx>,
) -> Option<rustc_hir::def_id::DefId>;
```

Resolution rules:

- `ExprKind::Call(callee, _)` resolves with
  `cx.qpath_res(&qpath, callee.hir_id).opt_def_id()` when the callee is
  `ExprKind::Path`, then keeps only local function or associated-function
  definitions;
- `ExprKind::MethodCall(_, _, _, _)` →
  `cx.typeck_results().type_dependent_def_id(expr.hir_id)` filtered by the same
  local function or associated-function predicates;
- otherwise, `None`.

Unit tests in this stage cover pure helpers only:

- `BTreeMap` ordering — inserting helpers in non-sorted `DefId` order still
  yields a sorted iteration in `collector.iter()`;
- deduplication — recording the same `(callee, source-span)` twice keeps a
  single record and adds nothing on the second call;
- `lower_arg_atom` mapping tests — drive the adapter from synthetic HIR
  fixtures only where unavoidable, reserve pure model properties for
  `ArgAtom`/`ArgFingerprint`, and cover the adapter through compiled or UI
  tests that exercise `LateContext`.

The `lower_arg_atom` and `resolve_local_callee` adapters need a `LateContext`
to function, so they should be exercised by compiled or UI tests (Stage E)
rather than by traditional pure unit tests. Add only the unit coverage that
does not require `rustc` (the collector behaviour and any string helpers).

### Stage C — Wiring into the late pass

Extend `crates/rstest_helper_should_be_fixture/src/driver.rs`:

- Add `collector: collector::CallSiteCollector` to
  `RstestHelperShouldBeFixture` (and to `Default`).
- Implement `check_fn` with the late-pass signature:

  ```rust
  fn check_fn<'tcx>(
      &mut self,
      cx: &LateContext<'tcx>,
      kind: hir::intravisit::FnKind<'tcx>,
      _decl: &'tcx hir::FnDecl<'tcx>,
      body: &'tcx hir::Body<'tcx>,
      _span: rustc_span::Span,
      _def_id: hir::HirId,
  )
  ```

  - early-return when the function is not a strict `#[rstest]` test under
    the configured `RstestDetectionOptions` (use `whitaker::hir`-provided
    attribute access to feed `is_rstest_test_with`);
  - compute `fixture_local_ids = fixture_local_ids(&parameters,
    &self.detection_options)`, where `parameters` is built from the body's
    parameter patterns and attributes and the helper returns a
    `HashSet<rustc_hir::HirId>`;
  - drive a HIR walk over `body.value` that visits each `Call` /
    `MethodCall` expression once. Use a small `rustc_hir::intravisit::Visitor`
    that traverses nested and deferred closure bodies, preserves closure-span
    fallback context while entering and leaving each closure, and records
    `Call` and `MethodCall` expressions encountered within those bodies.
- Implement `fn check_crate_post(&mut self, _cx: &LateContext<'tcx>)`:
  - emit a single `debug!(target: LINT_NAME, "rstest helper call-site
    collection complete: {n} callees, {m} records", …)` line at the end
    of the crate post-pass;
  - do not call `span_lint*` from this hook.

For test-harness recovery, reuse
`whitaker::hir::collect_harness_test_functions` and
`collect_rstest_companion_test_functions` only if attribute-based detection
misses a generated `#[test]` companion. Behavioural evidence in Stage E will
decide whether to enable this fallback for collection (the default detection
options already mark this as `use_source_callee_fallback = false`).

### Stage D — Dedup and span-location policy

In the same module, use `CallSiteLocation` as the private dedup key:

- the source span used as the dedup key is
  `recover_user_editable_hir_span(call_expr.span)`,
- the dedup state keys on the callee definition path, source file, recovered
  `span_lo` / `span_hi`, and `expr.hir_id.local_id` via `CallSiteLocation`;
- when recovery returns `None`, the record is dropped — not stored as
  unsupported.

Confirm with direct unit and property tests that generated case siblings still
collapse into one record, that calls with recoverable user-editable spans
sharing an equal recovered span remain distinct via the `hir_local_id`
tie-breaker, and that calls whose span recovery returns `None` are dropped.

### Stage E — Tests

Test surfaces, in order of priority:

1. **Unit tests** in
   `crates/rstest_helper_should_be_fixture/src/collector_tests.rs`:
   - `BTreeMap` ordering and deduplication of `CallSiteCollector` using
     synthetic record values;
   - reserve pure property coverage for the `ArgAtom`/`ArgFingerprint` model;
     keep `lower_arg_atom` adapter coverage in compiled or UI tests that
     exercise the `LateContext` boundary.
   - property tests should live in
     `crates/rstest_helper_should_be_fixture/src/collector_tests.rs`
     gated on `cfg(test)`.
2. **End-to-end behavioural fixture** under
   `crates/rstest_helper_should_be_fixture/examples/` mirroring
   `no_unwrap_or_else_panic`'s `pass_unwrap_in_rstest_harness.rs` so the
   Cargo-backed `collection_zero_diagnostic` harness exercises the full lint
   path through `dylint_testing::ui::Test::example`.

Property tests should focus on these invariants:

- **Determinism**: given two HIR walks of the same body, the collector
  produces the same records (assert by re-running the same body twice).
- **Idempotence of dedup**: feeding the same record twice does not grow the
  collector beyond one entry.
- **Order-independence**: feeding the same set of records in any
  permutation produces the same `BTreeMap` ordering.

Kani is not required (Decision log).

### Stage F — Documentation, validation, review

Update documentation:

- `docs/lints-for-rstest-fixtures-and-test-hygiene.md`: append
  "Implementation decisions (8.2.2)" subsection capturing dedup key, adapter
  placement, and constant-vs-static handling;
- `docs/users-guide.md` `rstest_helper_should_be_fixture` section: note that
  collection is now active but the lint remains silent until 8.2.3;
- `docs/developers-guide.md`: add a short subsection describing the
  collector pattern if it differs from the per-expression model used elsewhere.
  Cross-reference this ExecPlan section number.

Validation order:

1. `cargo check -p rstest_helper_should_be_fixture --all-targets --all-features`.
2. `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`.
3. `cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets --all-features`.
4. `make markdownlint`.
5. `make check-fmt`.
6. `make lint`.
7. `make test`.

Run `coderabbit review --agent` after each major milestone and again before
final push. Address all relevant concerns until 0 findings.

Mark `docs/roadmap.md` 8.2.2 done only after all of the above succeed.

## Concrete steps

From the repository root, confirm branch and worktree state:

```sh
git branch --show-current
git status --short --branch
```

Expected branch after rename:

```plaintext
8-2-2-call-site-collection-in-rstest-tests
```

Inspect the existing patterns the implementation will reuse:

```sh
sed -n '1,160p' crates/no_std_fs_operations/src/driver.rs
sed -n '1,210p' crates/no_std_fs_operations/src/usage.rs
sed -n '1,210p' crates/rstest_helper_should_be_fixture/src/driver.rs
sed -n '1,210p' common/src/rstest/parameter.rs
sed -n '1,130p' src/hir/mod.rs
```

Run targeted checks after each milestone with `tee` for log capture:

```sh
ACTION=check-rstest-helper
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo check -p rstest_helper_should_be_fixture --all-targets --all-features 2>&1 | tee "$LOG"

ACTION=test-rstest-helper
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features 2>&1 | tee "$LOG"

ACTION=test-registration
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets --all-features 2>&1 | tee "$LOG"

ACTION=markdownlint
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "$LOG"
```

Final gates run sequentially:

```sh
ACTION=check-fmt
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "$LOG"

ACTION=lint
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make lint 2>&1 | tee "$LOG"

ACTION=test
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make test 2>&1 | tee "$LOG"
```

CodeRabbit review:

```sh
coderabbit review --agent
```

Commit using a file-based message (required by the repository commit hooks):

```sh
git status --short
git diff --stat
git add <changed files>
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Implement call-site collection in rstest helper lint

Add the HIR-to-ArgAtom adapter, a per-crate call-site collector, and a
late-pass walker for #[rstest] tests inside the
rstest_helper_should_be_fixture crate. Keep the lint diagnostic-silent;
threshold evaluation and diagnostics remain assigned to roadmap items
8.2.3 and 8.2.4.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Push and open or update the draft pull request only after validation succeeds:

```sh
git push -u origin 8-2-2-call-site-collection-in-rstest-tests
echo "${LODY_SESSION_ID}"
```

The PR title must include `(8.2.2)`. The PR body must mention this ExecPlan and
link to the Lody session URL
`https://lody.ai/leynos/sessions/${LODY_SESSION_ID}` under `## References`.

## Validation and acceptance

The plan is accepted for implementation when the user explicitly approves it.
Silence is not approval.

The implementation is accepted when all of the following are true:

- `crates/rstest_helper_should_be_fixture/src/collector.rs` exists with the
  documented APIs and at least the documented unit tests pass.
- `RstestHelperShouldBeFixture` accumulates a non-empty collection state
  when a synthetic test crate contains a `#[rstest]` test calling a local
  helper, demonstrated through the Cargo-backed
  `collection_zero_diagnostic` UI harness.
- Calling the same helper twice from the same source location (whether
  reached directly or via `#[case]`-generated companions) yields exactly one
  collected record.
- A call whose argument is a macro-expanded literal or whose call-site span
  recovers to `None` is excluded from the collected state.
- `cargo nextest run -p rstest_helper_should_be_fixture --all-targets --all-features`
  passes.
- `cargo nextest run -p whitaker_suite -p whitaker-installer --all-targets --all-features`
  passes, including the existing suite and installer registration scenarios.
- `make markdownlint`, `make check-fmt`, `make lint`, and `make test` all
  succeed.
- `coderabbit review --agent` has no unresolved relevant findings.
- `docs/users-guide.md`, `docs/developers-guide.md`, and the rstest lint
  design document are updated where behaviour or internal practice changed.
- `docs/roadmap.md` marks 8.2.2 done.

Expected final gate transcript shape:

```plaintext
$ make check-fmt
...
Finished

$ make lint
...
Finished

$ make test
...
test result: ok
```

The exact test count will change as new tests are added; success means each
command exits with status 0.

## Idempotence and recovery

All edit stages are ordinary file edits and can be retried from `git status`.
If a step fails part-way through, inspect `git diff` and either continue from
the partially edited file or revert only the files changed for this task. Do
not revert unrelated user changes.

If the collector or adapter scaffold is wrong, inspect only
`crates/rstest_helper_should_be_fixture/src/collector.rs`, its direct unit and
property tests, and the Cargo-backed example/UI fixture before changing files.
Recreate them from the patterns named in Stage B and Stage E without disturbing
unrelated tests.

After a commit, prefer a corrective follow-up commit over rewriting history
unless the user explicitly asks for history cleanup.

If validation fails, inspect the matching
`/tmp/*-whitaker-8-2-2-call-site-collection-in-rstest-tests.out` log before
changing code. Record persistent failures under "Surprises & discoveries" or
"Decision log" with the command and log path.

## Artifacts and notes

Wyvern repository-pattern findings (summary):

```plaintext
- No existing Whitaker lint uses check_crate_post. The current lints
  initialise per-crate config in check_crate and emit diagnostics
  expression-locally through check_expr / check_fn / check_item /
  check_impl_item.
- Callee resolution is consistently done via cx.qpath_res for Call and
  cx.typeck_results().type_dependent_def_id for MethodCall.
- whitaker::hir::recover_user_editable_hir_span is the canonical macro-aware
  span recovery used by lint drivers; span_recovery_frames is currently
  only exercised by unit tests in src/hir/tests.rs.
- span_lint_hir_and_then is referenced only in design documents — no lint
  uses it yet.
- UI tests use dylint_testing::Test plus the whitaker::testing::ui harness.
  This collector uses a Cargo-backed example as its behavioural boundary and
  keeps direct data-structure checks in unit and property tests.
- The local crates/clippy_utils crate is a panic-detection stub and does
  not expose fn_def_id; the small Call/MethodCall switch is best inlined.
```

Wyvern design-coherence findings (summary):

```plaintext
- 8.2.2 must deliver call-site collection plus fixture-local classification
  plus constant-aware argument fingerprinting, without emission.
- The fingerprint data model in common/src/rstest/argument_fingerprint.rs
  is sufficient; only an HIR → ArgAtom adapter is missing.
- Risks unique to 8.2.2 are macro-expanded glue, #[case] companion
  duplication, trait-method callees with no DefId, and const-vs-static
  semantics. Mitigations are documented in the Risks section.
- No new public API in common/ is required if the adapter stays inside
  the lint crate; promotion to src/hir/ becomes attractive only when
  8.3.x or 8.4.x consume the same adapter.
```

Firecrawl findings (summary):

The source-backed findings below rely on the rstest documentation[^1], Dylint
documentation[^2], rustc nightly documentation[^3], and the prior-art
research[^4].

```plaintext
- rstest 0.26 provider-driven parameter attributes:
  case, values, files, future, context. #[from] and #[with] are fixture-
  rename / partial-injection markers, still fixture-backed. #[by_ref],
  #[ignore], and #[notrace] are operational modifiers, not provider
  drivers.
- rstest macro expansion preserves the original function body as the inner
  item visited by `LateLintPass::check_fn`; `#[case]`-driven tests are
  emitted as siblings under an outer module named after the test
  function.[^1]
- Dylint 5.x `impl_late_lint!` registers a `LateLintPass`. `check_crate_post`
  is the canonical aggregation hook.[^2]
- rustc HIR: `cx.qpath_res(qpath, hir_id)` for direct calls and constants;
  `cx.typeck_results().type_dependent_def_id(expr.hir_id)` for method calls;
  `cx.tcx.def_path_str(def_id)` for stable definition paths.[^3]
- Prior art for "extract repeated helper as fixture" lints: nothing
  comparable in pytest, Ruff, or Clippy. The Whitaker rule is novel.[^4]
```

[^1]: `docs.rs/rstest` fixture and rstest attribute docs:
  <https://docs.rs/rstest/latest/rstest/attr.rstest.html>
  <https://docs.rs/rstest/latest/rstest/attr.fixture.html>
[^2]: `docs.rs/dylint_linting/5` - Dylint late-lint macros.
[^3]: rustc nightly docs for `LateLintPass` and `ExprKind`:
  <https://doc.rust-lang.org/nightly/nightly-rustc/rustc_lint/trait.LateLintPass.html>
  <https://doc.rust-lang.org/nightly/nightly-rustc/rustc_hir/hir/enum.ExprKind.html>
[^4]: Firecrawl findings checked during planning; no comparable prior
  art was identified in pytest, Ruff, or Clippy.

## Interfaces and dependencies

The lint crate exposes no new public API. The new in-crate module surfaces the
following `pub(crate)` items inside
`crates/rstest_helper_should_be_fixture/src/collector.rs`:

```rust
pub(crate) struct CallSiteRecord {
    pub(crate) callee_def_id: rustc_hir::def_id::DefId,
    pub(crate) fingerprint: whitaker_common::rstest::ArgFingerprint,
    pub(crate) test_source_def_id: rustc_hir::def_id::DefId,
    pub(crate) span: rustc_span::Span,
}

pub(crate) struct CallSiteLocation {
    callee_key: String,
    file_name: rustc_span::FileName,
    lo: rustc_span::BytePos,
    hi: rustc_span::BytePos,
}

pub(crate) struct CallSiteCollector {
    by_callee: std::collections::BTreeMap<
        String,
        Vec<CallSiteRecord>,
    >,
    seen: std::collections::BTreeSet<CallSiteLocation>,
}

pub(crate) fn lower_arg_atom<'tcx>(
    cx: &rustc_lint::LateContext<'tcx>,
    expr: &'tcx rustc_hir::Expr<'tcx>,
    fixture_local_ids: &std::collections::HashSet<rustc_hir::HirId>,
) -> whitaker_common::rstest::ArgAtom;

pub(crate) fn resolve_local_callee<'tcx>(
    cx: &rustc_lint::LateContext<'tcx>,
    expr: &'tcx rustc_hir::Expr<'tcx>,
) -> Option<rustc_hir::def_id::DefId>;
```

Dependencies in the landed contract:

- workspace-pinned `rustc_ast`, `rustc_hir`, `rustc_lint`, `rustc_session`,
  and `rustc_span` driver crates,
- `dylint_linting`, `serde`, `log`, `whitaker`, and `whitaker-common`,
- workspace dev-dependencies `rstest`, `proptest`, and `dylint_testing`,
- crate-local test-support dependencies `filetime` and `fs2`.

No additional third-party dependency is required for the implemented 8.2.2
contract.

## Revision note

Initial draft created on 2026-05-28. The draft captures pre-implementation
scope, repository orientation, external references, the in-crate adapter
placement decision, validation commands, and the approval gate for roadmap item
8.2.2.
