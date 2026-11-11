# Deliver the `missing_docs_in_test_functions` pre-expansion pass (2.2.11)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan must be approved before implementation begins. Do not start milestone
`EP-M0` until the user has explicitly approved this document, including the
decisions marked "needs approval" in `Decision log`.

## Purpose / big picture

Roadmap item 2.2.11 delivers the first half of RFC 0003: a Whitaker lint named
`missing_docs_in_test_functions` that runs as a Dylint pre-expansion pass and
reports source-authored functions that lack documentation, but only when rustc
is compiling a test harness (`rustc --test`, which is what `cargo test` does
for unit and integration tests).

A pre-expansion pass is a lint pass that rustc runs over the abstract syntax
tree (AST) immediately after parsing and before any macro is expanded. That
timing matters for test code because procedural attribute macros such as
`#[rstest]`, `#[tokio::test]`, or `rstest-bdd`'s `#[given(...)]` rewrite the
function they decorate into wrappers and generated machinery. Before expansion,
the lint sees exactly the function the developer wrote, with its own attributes
and doc comments, and nothing that a macro generated.

After this change, a developer who builds the new lint library and runs it over
a test target observes:

- ``warning: test function `step_counter_starts_at_zero` has no documentation comment``
  with the caret under the function's name, for an undocumented `fn` in a
  test-harness compilation, including one decorated with a procedural attribute
  macro such as `#[given("...")]`;
- no diagnostic for the same function when it carries a `///` comment, a
  `/** ... */` comment, or a `#[doc = "..."]` attribute;
- no diagnostic for anything when the same source is compiled without
  `--test`;
- no diagnostic for functions that macros generate, for functions nested
  inside function bodies, or for methods in trait implementations.

This item delivers the pass and its candidate-selection predicate only. The
configurable include/exclude path policy is roadmap item 2.2.12, registration
in the Whitaker suite is 2.2.13, localization of the messages into Welsh and
Scottish Gaelic is 2.2.14, the SARIF-contract UI matrix is 2.2.15, and the
property-test suite for path configuration is 2.2.16. Until 2.2.13 lands, the
lint is buildable and testable inside this repository but is not shipped to
users through the suite, the installer, or the rolling release.

## Constraints

- The lint must be implemented as a Dylint pre-expansion pass, registered by
  `dylint_linting::impl_pre_expansion_lint!`. A late (HIR) pass, a
  post-expansion early pass, a Clippy pass, a rustdoc JSON consumer, or a text
  scanner is forbidden (RFC 0003 §7, technical requirements).
- The predicate must return "no diagnostic" unless every gate in RFC 0003 §6
  "Candidate selection" holds: test harness, source-authored function item,
  selected source path, and absence of documentation.
- Both line or block documentation comments (`///`, `/** */`, and the inner
  forms `//!` and `/*! */` placed inside a function body) and the
  `#[doc = ...]` attribute must count as documentation.
- The diagnostic must be attached to the function's identifier whenever the
  identifier span is reliable, falling back to the item span otherwise.
- Do not register the lint in `suite/`, the installer's lint lists
  (`installer/src/resolution.rs`, `installer/src/bin/package_lints.rs`), the
  `Makefile` `LINT_CRATES` list, `.github/workflows/rolling-release.yml`, or
  `EXPERIMENTAL_LINT_CRATES`. Those belong to 2.2.13, and partial registration
  breaks the lint-list contract test
  (`tests/workflow_contracts/lint_crate_list_contract_test.py`).
- Do not add `.ftl` locale resources. Adding an `en-GB` resource without `cy`
  and `gd` counterparts fails `common/tests/i18n_quality`, and the translations
  belong to 2.2.14. Messages must nevertheless flow through the shared
  `whitaker_common::i18n` resolution helpers with an English fallback, so that
  2.2.14 only adds resources.
- Do not implement `dylint.toml` include/exclude configuration, glob matching,
  or a shared path-glob helper. Those belong to 2.2.12 and RFC 0003 §6 requires
  their ownership to be documented before such a helper exists.
- The pure policy module must compile without the `dylint-driver` feature and
  without any `rustc_*` crate, so its tests, property tests, and proof model
  run without compiler-private plumbing.
- No source file may exceed 400 lines (`AGENTS.md`). Every module starts with
  a `//!` comment; every function carries a `///` comment.
- Use only dependencies already declared in the root `Cargo.toml`
  `[workspace.dependencies]`. No new external crate may be introduced.
- Do not unset or override `RUSTC_WRAPPER`, and do not bypass the shared
  compile-admission queue described in the operator instructions.
- Documentation follows `docs/documentation-style-guide.md`: en-GB Oxford
  spelling, 80-column wrapping for prose, dashes for bullets.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 45 files or more
  than 3,500 net lines of code and other documentation, stop and escalate. This
  ExecPlan file is excluded from both counts.
- Interface: if delivering the item requires changing a public signature in
  `whitaker-common`, `whitaker_lint_core`, or `suite`, stop and escalate.
  Adding new private or crate-internal items there is also out of scope; the
  new lint crate owns its helpers.
- Dependencies: if any new external dependency, or a workspace dependency
  version change, is required, stop and escalate.
- Toolchain: if the `EP-M0` spike shows that `impl_pre_expansion_lint!` from
  the locked `dylint_linting` 6.3.0 does not register a working pass on
  `nightly-2026-05-28`, stop and escalate rather than upgrading Dylint or the
  toolchain.
- Semantics: if the spike shows that pre-expansion hooks do not see
  functions in out-of-line module files (`mod helpers;`), or that item-level
  `#[allow(missing_docs_in_test_functions)]` is not honoured, stop and
  escalate, because both break RFC 0003 requirements that later items build on.
- Proof: if the Verus proofs in `EP-M2` cannot be discharged after three
  focused attempts per lemma, or would need `assume`, `admit`,
  `#[verifier::external_body]` on a proved function, or an axiom added only to
  prove the theorem, stop and escalate.
- Iterations: if a gate still fails after three fix attempts for the same
  root cause, stop and escalate.
- Time: if a milestone takes more than one working day of wall time,
  excluding time spent queued for compile admission, stop and escalate.
- Ambiguity: if a newly discovered source form (for example, a function
  shape not listed in `Decision log` D-5) materially changes which items are
  candidates, stop and present options.

## Risks

- Risk: the locked `dylint_linting` 6.3.0 pre-expansion registration macro
  may select a `LintStore` method that the pinned nightly does not provide.
  Severity: high. Likelihood: low. Mitigation: `EP-M0` builds the skeleton
  first; reconnaissance found that 6.3.2 picks `register_pre_expansion_pass`
  for toolchains before 2026-06-22 through `rustversion`, and the pinned
  `LintStore` has that method (`rustc_lint/src/context.rs:168`). The spike
  confirms that 6.3.0 behaves in the same way.

- Risk: rustc runs a pre-expansion pass once for the crate root and once for
  every out-of-line module file as it is loaded during expansion
  (`rustc_expand/src/expand.rs:1402`), and each run builds a fresh lint-level
  stack. Crate-level `#![allow(...)]` attributes in `lib.rs` or the test root
  may therefore not reach items in `mod helpers;` files. Severity: medium.
  Likelihood: medium. Mitigation: `EP-M0` records the observed behaviour in
  `Surprises & discoveries`; the developers' guide and the users' guide state
  it; item-level allows, which 2.2.15 relies on, are verified by a UI fixture in
  `EP-M3`.

- Risk: before expansion, `#[cfg(...)]` has not been evaluated, so items
  under `#[cfg(not(test))]` or a disabled feature are still visible, and
  `#[cfg_attr(..., doc = "...")]` has not yet produced a doc attribute.
  Severity: low. Likelihood: high. Mitigation: record decision D-6, cover it
  with a UI fixture, and document it as a pre-expansion limitation.

- Risk: `#[doc = include_str!("...")]` has a macro-call value before
  expansion, so `AttributeExt::doc_str()` returns `None` for it. Severity:
  medium. Likelihood: high. Mitigation: classify documentation from the
  attribute's syntactic shape (`doc` path with an `=` value), not from
  `doc_str()` alone; cover it with unit tests and a UI fixture.

- Risk: `rstest` expansion through the Dylint compiletest driver is known to
  hang on Windows
  (`crates/no_unwrap_or_else_panic/examples/pass_unwrap_in_rstest_harness.rs:31`).
  Severity: medium. Likelihood: low for this pass, because a pre-expansion
  diagnostic is emitted before the macro runs, but the macro still expands
  afterwards. Mitigation: prefer `rstest-bdd-macros` and `tokio` for the
  procedural-attribute fixture, and reuse the existing `cfg(windows)` fallback
  pattern if the hang reappears.

- Risk: doctests for the policy module may fail with `E0658` because
  `make test` runs doctests with `--all-features`, which enables
  `rustc_private`. Severity: low. Likelihood: medium. Mitigation: the policy
  module has no `rustc_private` dependency, and the crate root only enables the
  feature under `dylint-driver`; if a doctest still fails, follow the
  `bumpy_road_function` precedent of hidden setup lines before considering a
  `DOCTEST_EXCLUDES` entry, and record the choice.

- Risk: the Verus model drifts from the production policy code, so a proof
  describes code that no longer exists. Severity: medium. Likelihood: medium.
  Mitigation: an exhaustive conformance test runs the production predicate
  against the same specification the proof uses, and a drift-guard script
  checked in CI fails if the model's mirrored enums diverge from the production
  enums (see `Verification plan`).

- Risk: compile admission on the shared host makes gates slow. Severity:
  low. Likelihood: high. Mitigation: allow generous timeouts, check
  `build-limits-status` before blaming the candidate, and never start a second
  build to "help".

## Progress

- [x] (2026-11-11) Rename branch to
  `2-2-11-missing-docs-in-test-funcs-pre-expansion-ast-pass`.
- [x] (2026-11-11) Reconnaissance by a Wyvern team: lint crate template, AST
  and pre-expansion APIs on the pinned toolchain, verification infrastructure,
  and test-harness fixture precedents.
- [x] (2026-11-11) Draft this ExecPlan.
- [ ] Community-of-experts design review and revision of this ExecPlan.
- [ ] User approval of this ExecPlan and of the decisions marked "needs
  approval".
- [ ] `EP-M0` pre-expansion spike and crate skeleton.
- [ ] `EP-M1` pure policy kernel with unit, behavioural, snapshot, and
  property tests.
- [ ] `EP-M2` Verus proofs and drift guard.
- [ ] `EP-M3` pre-expansion driver, diagnostics, and UI fixtures.
- [ ] `EP-M4` documentation, ADR, and roadmap completion.

## Surprises & discoveries

- Observation: the repository has no pre-expansion lint yet; the only early
  lint, `string_continuation_style`, is a post-expansion early pass. Evidence:
  no use of `declare_pre_expansion_lint!` or `impl_pre_expansion_lint!` outside
  `docs/`. Impact: this crate sets the pattern for pre-expansion lints, so its
  conventions are written down in the developers' guide and in an ADR.

- Observation: pre-expansion `NodeId`s are all dummy values, so the pass
  cannot key state on node identifiers. Evidence: rustc assigns node
  identifiers during expansion; the pre-expansion run in
  `rustc_interface/src/passes.rs:87-109` happens before it. Impact: context
  such as "inside a trait implementation" must be tracked with an explicit
  container stack driven by paired `check_*`/`check_*_post` hooks.

- Observation: `EarlyLintPass` offers paired hooks `check_item` and
  `check_item_post`, `check_impl_item` and `check_impl_item_post`, and
  `check_trait_item` and `check_trait_item_post`, and the visitor calls each
  pair unconditionally around the walk of that node
  (`rustc_lint/src/early.rs:79-85, 208-228`). Impact: a stack that pushes
  exactly one frame in every pre-hook and pops exactly one in every post-hook
  is balanced by construction.

- Observation: ADR number 0013 is already used on the unmerged branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`. Evidence:
  scanning every fetched `refs/remotes/origin/*` tree for `adr-001[3-9]`.
  Impact: this plan allocates ADR 0014.

## Decision log

- Decision D-1: put the rustc-free policy in a `policy` module inside the new
  lint crate (`crates/missing_docs_in_test_functions/src/policy/`), not in a
  new support crate and not in `whitaker-common`. Rationale: the kernel is
  small and specific to this lint. The `string_continuation_style` precedent
  split its policy into `whitaker_string_literals` because that policy was
  large and shared by property and proof suites; nothing here is shared yet. A
  sweep of `common/` and `crates/whitaker_lint_core` found no AST documentation
  or container-context helper to reuse (the existing documentation and span
  helpers are HIR-bound or rstest-specific). If 2.2.12 needs a shared path-glob
  helper, that item records its ownership. Date/Author: 2026-11-11, planning
  agent.

- Decision D-2 (needs approval): before 2.2.12 exists, the path gate selects
  every item whose span resolves to a real local source file, and rejects items
  whose file is virtual, anonymous, or otherwise not recoverable. Rationale:
  RFC 0003 §6 lists the path gate as clause 3 of the predicate and requires
  matching "after the compiler source map has recovered a real file path", so
  the real-file recovery is permanent and belongs here. The include/exclude
  layer replaces the "every real file" default in 2.2.12. Because the lint is
  not registered anywhere users can reach until 2.2.13, which requires 2.2.12,
  no consumer sees the temporary breadth. Alternatives considered: (a) inert
  until configured: safe, but leaves no observable behaviour to test end to end
  in this item and would need a test-only configuration seam that 2.2.12 would
  discard; (b) a built-in default such as "paths containing a `tests/`
  component": invents a path grammar that 2.2.12 must own and may contradict.
  Date/Author: 2026-11-11, planning agent.

- Decision D-3: represent each gate as a two-variant enum rather than a
  `bool` (`HarnessStatus`, `Provenance`, `PathSelection`, `Documentation`) and
  the function's position as a `FunctionContext` enum. Rationale: `AGENTS.md`
  asks for newtypes over primitive soup; named variants make the truth-table
  snapshot and the Verus mirror readable and stop arguments being transposed.
  Date/Author: 2026-11-11, planning agent.

- Decision D-4: the predicate returns a `Verdict` that is either `Diagnose`
  or `Skip(SkipReason)`, where the reason names the first failing gate in the
  fixed order harness, provenance, function context, path, documentation.
  Rationale: the reason powers `tracing`/`log` debug events and makes property
  and proof statements about gate precedence precise. The order puts the
  cheapest session-wide gate first and the per-attribute scan last.
  Date/Author: 2026-11-11, planning agent.

- Decision D-5: function forms in scope for 2.2.11 are free functions whose
  every enclosing item is a module, inherent associated functions (in an
  `impl Type` block whose every enclosing item is a module), and trait methods
  with a default body. Out of scope: methods in trait implementations
  (`impl Trait for Type`), because their documentation conventionally lives on
  the trait; trait methods without a body (RFC 0003 §6); functions nested
  inside a function, constant, static, enum discriminant, or other non-module
  item; foreign functions in `extern` blocks; closures; and anything generated
  by a macro. Rationale: RFC 0003 §6 asks the UI matrix to state the included
  forms and leaves associated functions to "any equivalent associated-function
  AST shape supplied by the pinned compiler API" (§11 open question 2). The
  rule "every enclosing item is a module" is simple to state, to track, and to
  prove, and it matches rustc's own `missing_docs` lint, which ignores items
  inside bodies. Date/Author: 2026-11-11, planning agent.

- Decision D-6: documentation is present when any attribute on the function
  (outer, or inner within the body) is a sugared doc comment, or a `doc`
  attribute with an `=` value of any expression form. `#[doc(hidden)]` and
  other list-form `doc` attributes do not count. `#[cfg_attr(…, doc = …)]` does
  not count, because it is unexpanded before expansion. Empty or
  whitespace-only documentation counts as present. Rationale: the success
  criterion requires `///` and `#[doc = "..."]` to count. Classifying by shape
  rather than by `doc_str()` keeps `#[doc = include_str!("...")]` working
  before expansion. Counting empty documentation as present follows rustc's
  `missing_docs`; policing documentation quality is out of scope for the RFC.
  Date/Author: 2026-11-11, planning agent.

- Decision D-7: functions under `#[cfg(...)]` that would be removed later
  are still candidates. Rationale: before expansion, configuration predicates
  have not been evaluated, and the authored function is still authored test
  source. Diagnosing it is consistent with "source-authored" and avoids
  reimplementing cfg evaluation. Date/Author: 2026-11-11, planning agent.

- Decision D-8: use Verus for the candidate-selection contract (soundness,
  completeness, gate precedence, and suppression monotonicity) and for the
  container-stack invariant over unbounded, well-nested event sequences. Use
  exhaustive tests, not Verus, for the finite documentation-shape and
  span-choice tables, and do not use Kani. Rationale: see `Verification plan`.
  The predicate is the contractual business logic of RFC 0003 §7, and
  `AGENTS.md` requires Verus for it; the monotonicity lemma is the property
  that 2.2.12 extends when exclude patterns arrive. The container stack is the
  only unbounded state in the item. The documentation and span rules are finite
  case tables whose risk lies in rustc's attribute and span shapes, which only
  UI tests exercise. Kani would add a bounded check of a domain that exhaustive
  tests already cover completely. Date/Author: 2026-11-11, planning agent.

- Decision D-9: diagnostics in 2.2.11 resolve through
  `whitaker_common::i18n::safe_resolve_message_set` with `noop_reporter` and an
  English fallback message set, with no `.ftl` resources yet. Rationale: this
  satisfies the RFC's "Whitaker's localized diagnostic conventions"
  structurally while leaving the translated resources to 2.2.14 without
  tripping the locale-pairing gate. Date/Author: 2026-11-11, planning agent.

- Decision D-10: the lint's default level is `Warn`.
  Rationale: every other Whitaker lint declares `Warn`; consumers raise it with
  `-D` or `[lints]`. Date/Author: 2026-11-11, planning agent.

- Decision D-11: driver debug events use the workspace `log` crate rather
  than `tracing`. Rationale: `AGENTS.md` prefers `tracing`, but every existing
  lint driver (for example
  `crates/string_continuation_style/src/driver/facts.rs:10` and
  `crates/no_unwrap_or_else_panic/src/driver.rs:5`) uses `log::debug!`, and the
  shared i18n helpers do the same. Consistency inside the Dylint driver is
  worth more than a one-crate divergence; a suite-wide move to `tracing` would
  be its own change. No metrics are emitted, because a lint library must not
  install recorders and the item has no usage requirement. Date/Author:
  2026-11-11, planning agent.

## Outcomes & retrospective

Not started. Complete this section at each milestone boundary and at the end.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint libraries (`cdylib`
crates) into a rustc driver and lets them register lint passes. Lint crates
live in `crates/<lint_name>/`. Each one builds as a standalone library when its
`dylint-driver` feature is enabled, and as part of the combined suite library
(`suite/`) when its `constituent` feature is enabled. The workspace pins
`nightly-2026-05-28` in `rust-toolchain.toml` and `dylint_linting` 6.3.0 in
`Cargo.lock`. Compiler crates are reached through proxy crates such as
`crates/rustc_ast/src/lib.rs`, which re-export the toolchain's private crates.

The specification is `docs/rfcs/rfc-0003-missing-docs-in-test-functions.md`.
Sections 6 and 7 define the pass boundary, the four-clause candidate predicate,
the documentation forms, and the diagnostic placement. The roadmap entry is
`docs/roadmap.md` item 2.2.11; items 2.2.12 to 2.2.16 are the follow-on work
named in `Purpose / big picture`.

Useful precedents and where to look:

- `crates/string_continuation_style/` is the only early lint. Its `Cargo.toml`
  shows the feature layout (`dylint-driver`, `constituent`, optional `rustc_*`
  dependencies). `src/driver/mod.rs:34-57` shows the
  `mod declaration { #![expect(missing_docs, ...)] ... }` wrapper around the
  `impl_early_lint!` macro. `src/diagnostics.rs` shows localized emission
  through `cx.emit_span_lint(..., DiagDecorator(...))`.
- `crates/no_expect_outside_tests/src/lib_ui_tests.rs` shows UI tests that
  compile `examples/` fixtures with `Test::example(...)` and
  `test.rustc_flags(["--test", ...])`, which is what this lint needs.
- `crates/whitaker_clones_core/tests/candidate_pair_behaviour.rs` with
  `tests/features/candidate_pair.feature` shows the `rstest-bdd` world,
  fixture, and step conventions, including `#[allow_fixture_expansion_lints]`.
- `verus/clone_detector_candidate_pair.rs`, `scripts/run-verus.sh`,
  `scripts/check-verus-fragment-id-bridge.sh`, and
  `.github/workflows/proofs.yml` show how a Verus sidecar is added, run, and
  guarded against drift.
- `docs/developers-guide.md` §"Writing an early pass" (around line 3801) and
  `docs/whitaker-dylint-suite-design.md` §"Early-pass registration addendum"
  (around line 1068) document early-pass conventions this plan extends.

Terms used in this plan:

- Test harness compilation: a rustc invocation with `--test`. Inside a lint
  pass this is `cx.sess().opts.test`.
- Pre-expansion pass: an `EarlyLintPass` registered with
  `register_pre_expansion_pass`, run on the parsed AST before macro expansion,
  and run again for each out-of-line module file as rustc loads it.
- Source-authored: an item whose span has the root syntax context, that is,
  `!span.from_expansion()`.
- Container stack: the pass's record of the items currently being walked,
  from outermost to innermost, used to decide whether a function is
  module-level, an inherent or trait-default method, or nested elsewhere.
- Reliable identifier span: the function identifier's span when it is not
  the dummy span, not from an expansion, and lies inside the item's span.

## Conformance basis

Upstream artefacts and revisions:

- RFC 0003 `docs/rfcs/rfc-0003-missing-docs-in-test-functions.md` at commit
  `84eed837` (status: Proposed), §§6-7, with §9 and §11 for test and
  open-question context.
- `docs/roadmap.md` at commit `d9eb8a09`, items 2.2.11 (this plan) and
  2.2.12 to 2.2.16 (boundaries this plan must not cross).
- `docs/whitaker-dylint-suite-design.md` at commit `d9eb8a09`, §"Per-lint
  crate scaffolding" and §"Early-pass registration addendum".
- ADR 0002 `docs/adrs/adr-0002-attribute-macro-for-conditional-dylint-expect.md`
  for the pre-expansion `cfg_attr(dylint_lib = ...)` limitation.
- ADR 0003
  `docs/adrs/adr-0003-formal-proof-strategy-for-the-clone-detector-pipeline.md`
  for the repository's Verus-versus-Kani split.
- Governing standards: `AGENTS.md`, `docs/documentation-style-guide.md`,
  `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- No Terms of Reference document exists for this work.

Traced requirements (identifiers are local to this plan):

- `RFC3-FR-HARNESS` (only test-harness compilations) -> `EP-M1`
  `policy::selection` -> `EP-M2` lemma `L-SOUND` -> `EP-M3` fixture
  `pass_undocumented_without_test_harness` -> `EV-M3-ui`.
- `RFC3-FR-SOURCE` (only source-authored functions; generated items never
  diagnosed) -> `EP-M1` provenance gate -> `EP-M3` fixtures
  `pass_macro_generated_function` and `fail_proc_macro_attribute_function` ->
  `EV-M3-ui`.
- `RFC3-FR-DOCS` (`///` and `#[doc = "..."]` accepted) -> `EP-M1`
  `policy::documentation` -> `EP-M3` fixtures `pass_documented_forms` ->
  `EV-M1-unit`, `EV-M3-ui`.
- `RFC3-FR-SPAN` (identifier span when reliable) -> `EP-M1`
  `policy::span` -> `EP-M3` fixture `fail_undocumented_free_function` caret
  placement -> `EV-M3-ui`.
- `RFC3-FR-PROCMACRO` (procedural-attribute functions diagnosed at authored
  location) -> `EP-M3` fixture `fail_proc_macro_attribute_function` ->
  `EV-M3-ui`.
- `RFC3-TR-PREEXP` (pre-expansion pass) -> `EP-M0` skeleton -> `EV-M0`.
- `RFC3-TR-L10N` (localization conventions) -> `EP-M3`
  `diagnostics.rs` fallback resolution -> `EV-M3-ui` (English text); completed
  by 2.2.14.
- `RFC3-DES-SCOPE` (associated-function shapes stated explicitly) ->
  `Decision log` D-5 -> `EP-M1` `policy::context` -> `EP-M2` lemma `L-STACK` ->
  `EP-M3` fixtures `fail_inherent_method`, `pass_trait_impl_method`,
  `pass_nested_function` -> `EV-M3-ui`.
- `RFC3-FR-PATH` (configured path selection) -> partially advanced:
  real-file gate only (D-2); include/exclude deferred to 2.2.12.

## Verification plan

### Abstract model

The policy is specified independently of rustc as a function over facts.

`Facts` is a record of five fields:

- `harness ∈ {TestHarness, NotTestHarness}`;
- `provenance ∈ {Source, Expansion}`;
- `context`, one of `ModuleFunction`, `InherentMethod`, `TraitDefaultMethod`,
  `TraitImplMethod`, `BodylessTraitMethod`, or `Nested`;
- `path ∈ {Selected, NotSelected}`;
- `documentation ∈ {Documented, Undocumented}`.

The specification is `should_diagnose(f)`, true exactly when
`f.harness = TestHarness`, `f.provenance = Source`,
`f.context ∈ {ModuleFunction, InherentMethod, TraitDefaultMethod}`,
`f.path = Selected`, and `f.documentation = Undocumented`.

Gate order is `harness < provenance < context < path < documentation`. For a
fact record that fails at least one gate, `first_failing_gate(f)` is the least
failing gate in that order.

The suppression order `f ⊑ g` ("g is at least as suppressed as f") holds when g
is obtained from f by changing any subset of fields towards suppression:
harness to `NotTestHarness`, provenance to `Expansion`, context to any
out-of-scope variant, path to `NotSelected`, or documentation to `Documented`.

The container model is a sequence of events `Enter(k)` and `Exit`, where
`k ∈ {Module, InherentImpl, TraitImpl, Trait, Other}`. A sequence is well
nested when every prefix has at least as many `Enter` events as `Exit` events
and the whole sequence has equally many. `open(p)` is the sequence of container
kinds entered and not yet exited in prefix `p`, outermost first. The
specification of function context for a function item observed after prefix `p`
is `spec_context(open(p), shape)`, where `shape` is `Free` or
`Associated { has_body }`:

- a free function is `ModuleFunction` if every element of `open(p)` is
  `Module`, otherwise `Nested`;
- an associated function whose last open container is `InherentImpl` and
  all earlier containers are `Module` is `InherentMethod`;
- one whose last open container is `Trait`, with all earlier containers
  `Module`, is `TraitDefaultMethod` when it has a body and
  `BodylessTraitMethod` otherwise;
- one whose last open container is `TraitImpl`, with all earlier containers
  `Module`, is `TraitImplMethod`;
- any other associated function is `Nested`.

### Invariants and lemmas

- Obligation `L-SOUND`: for every `f`, `select(f) = Diagnose` implies
  `should_diagnose(f)`.
- Obligation `L-COMPLETE`: for every `f`, `should_diagnose(f)` implies
  `select(f) = Diagnose`.
- Obligation `L-PRECEDENCE`: for every `f` with `¬should_diagnose(f)`,
  `select(f) = Skip(r)` with `r = first_failing_gate(f)`.
- Obligation `L-MONOTONE` (anti-widening): for every `f ⊑ g`,
  `select(g) = Diagnose` implies `select(f) = Diagnose`. Equivalently, no
  change towards suppression can create a diagnostic. 2.2.12 extends this lemma
  when the path gate becomes a function of include and exclude patterns.
- Obligation `L-STACK`: for every well-nested event sequence `s` and every
  prefix `p` of `s`, the tracker's frame list after processing `p` equals
  `open(p)`, and the tracker's `classify(shape)` after `p` equals
  `spec_context(open(p), shape)`. After the whole of `s`, the frame list is
  empty.
- Obligation `T-DOCS`: for every attribute shape in the finite table of D-6
  (outer `///`, outer `/** */`, inner `//!`, inner `/*! */`, `#[doc = "lit"]`,
  `#[doc = include_str!(..)]`, `#[doc(hidden)]`, `#[doc(alias = "x")]`,
  `#[cfg_attr(p, doc = "x")]`, a non-doc attribute such as `#[given("...")]`),
  the classification matches D-6, and a function's documentation status is
  `Documented` exactly when at least one attribute classifies as documentation.
- Obligation `T-SPAN`: the diagnostic span is the identifier span when it is
  reliable, and the item span otherwise; the chosen span is never from an
  expansion when the item span is not.
- Obligation `T-HOOKS` (integration): on the pinned toolchain, the pass
  observes free functions in the crate root and in out-of-line module files,
  inherent and trait items, and honours an item-level
  `#[allow(missing_docs_in_test_functions)]`.

### Axioms (trusted, not verified here)

- `A-VISITOR`: rustc's early-lint visitor calls `check_item` before and
  `check_item_post` after walking every item, and likewise for impl and trait
  associated items (`rustc_lint/src/early.rs:79-85, 208-228`), with no early
  exit between the pair. This is what makes the hook stream a well-nested
  sequence; `T-HOOKS` exercises it.
- `A-OPTS-TEST`: `Session::opts.test` is true exactly for `--test`
  compilations (`rustc_session/src/options.rs:328`).
- `A-PROVENANCE`: `Span::from_expansion()` is true exactly when the span's
  syntax context is not the root context
  (`rustc_span/src/span_encoding.rs:308`).
- `A-PREEXP`: before expansion, macro invocations are `MacCall` nodes, not
  items, so generated items are not visited
  (`rustc_interface/src/passes.rs:87-109`).
- `A-DYLINT`: `impl_pre_expansion_lint!` in `dylint_linting` 6.3.0 registers
  the given pass value with `register_pre_expansion_pass`; `EP-M0` checks this
  empirically.

### Methods, artefacts, and evidence

`L-SOUND`, `L-COMPLETE`, `L-PRECEDENCE`, `L-MONOTONE`:

- Method: Verus proof over a model that mirrors the production enums and the
  production `select` match structure, plus an exhaustive conformance test that
  runs the production `select` over every element of the finite domain (2 x 2 x
  6 x 2 x 2 = 96 records) and compares it with an independently written
  `should_diagnose` and `first_failing_gate`.
- Rationale: the domain is finite, so the exhaustive test is itself a
  complete check of the production code, and it is the link between code and
  model. Verus states the contract and the monotonicity lemma in a form that
  2.2.12 can extend to unbounded pattern lists, where enumeration is no longer
  possible.
- Artefacts: `verus/missing_docs_in_test_functions_policy.rs`;
  `crates/missing_docs_in_test_functions/src/policy/selection_tests.rs`;
  `crates/missing_docs_in_test_functions/src/policy/snapshots/` (insta
  truth-table snapshot).
- Evidence: `make verus-missing-docs` reports all functions verified with
  zero errors;
  `cargo nextest run -p missing_docs_in_test_functions policy::selection`
  passes, and the snapshot is reviewed once and then stable.
- Non-vacuity: the conformance test asserts that the domain enumerator
  yields 96 distinct records and that at least one record diagnoses and at
  least one record fails each gate first. Seeded faults that must be rejected:
  deleting the documentation gate (Verus fails `L-SOUND`; the conformance test
  fails on the `Documented` witness), swapping the path and context checks
  (Verus fails `L-PRECEDENCE`), and treating `TraitImplMethod` as in scope
  (Verus fails `L-SOUND`). The proof file must contain no `assume`, `admit`, or
  `external_body` on proved items; inspect this with
  `rg -n 'assume|admit|external_body' verus/missing_docs_*`.

`L-STACK`:

- Method: Verus proof by induction over the event sequence, plus a
  `proptest` model test of the production `ContainerStack`.
- Rationale: nesting depth is unbounded, so neither exhaustive enumeration
  nor a bounded model checker covers it. The proof establishes the
  representation relationship (frames equal `open(p)`), which is a genuine
  inductive invariant, not a restatement. The property test runs the real type
  against a recursive reference over generated item trees, which ties the
  production code to the same specification.
- Domain: Verus, all well-nested sequences; proptest, generated item trees up
  to depth 6 and breadth 4, linearized into the hook order of `A-VISITOR`, 256
  cases.
- Artefacts: `verus/missing_docs_in_test_functions_policy.rs` (same file,
  separate section);
  `crates/missing_docs_in_test_functions/src/policy/context_props.rs`.
- Evidence: `make verus-missing-docs`;
  `cargo nextest run -p missing_docs_in_test_functions policy::context`.
- Non-vacuity: the generator's classification counters must show every
  `FunctionContext` variant reached at least once per run (asserted with
  `prop_assert!` on a collected histogram in a companion deterministic test
  over a fixed seed), and generated trees must include depth at least 3. Seeded
  faults that must be rejected: popping only for container frames (pushing
  conditionally), and classifying by the innermost frame alone without checking
  that outer frames are modules.

`T-DOCS` and `T-SPAN`:

- Method: `rstest` parameterized tables over the finite shape tables in the
  pure module, and UI fixtures that pass real rustc attributes and spans
  through the driver.
- Rationale: the risk is in how rustc represents attributes and spans before
  expansion, which only a real compilation exercises; proofs over an
  abstraction of those shapes would verify the abstraction, not the risk.
- Artefacts: `src/policy/documentation_tests.rs`, `src/policy/span_tests.rs`,
  and the UI fixtures listed under `EP-M3`.
- Non-vacuity: each table row is a distinct D-6 shape, the
  `#[doc = include_str!(..)]` row fails under a naive `doc_str()`
  implementation (seeded fault), and the UI fixture for the fallback span fails
  if the driver always uses the item span.

`T-HOOKS`:

- Method: UI tests compiled with `--test`, plus the `EP-M0` spike
  transcript.
- Evidence: `cargo nextest run -p missing_docs_in_test_functions tests::ui`
  passes with blessed `.stderr` files.
- Non-vacuity: the out-of-line module fixture's expected output contains a
  diagnostic located in the module file; the `--test`-less fixture shows that
  the same source otherwise diagnoses.

### Where Verus is not used

Diagnostic text, fallback messages, Fluent resolution, the AST-to-facts
adapter, and lint registration are integration plumbing. They are covered by UI
tests and unit tests and acquire no proof obligations. Kani is not used: the
only unbounded state is covered by Verus and proptest, and the finite tables
are enumerated exhaustively.

### Relationship between proof and tests

The Verus model proves the contract for the model. The exhaustive conformance
test and the container property test prove that the production code computes
the same functions as the model's specifications. The drift guard proves that
the model still mirrors the production enum variants. UI tests prove that rustc
supplies the facts the policy assumes. Each layer fails for a different class
of defect, and none of them replaces the others.

## Plan of work

### Stage A: understand and propose (this document)

No code changes. The draft is reviewed by a community-of-experts panel and then
by the user.

### Stage B: spike and red tests

`EP-M0` builds the crate skeleton and proves the pre-expansion mechanism.
`EP-M1` and `EP-M3` begin with failing tests; `EP-M2` begins with lemma
statements whose proofs are absent, so Verus reports them as unproved.

### Stage C: implementation with verification

The policy, the proofs, and the driver are developed milestone by milestone,
each closing its obligations before the next starts.

### Stage D: documentation and wider validation

`EP-M4` updates the design document, records ADR 0014, documents conventions,
and marks the roadmap item done.

## Milestones and plateaus

### `EP-M0`: pre-expansion spike and crate skeleton (prototyping)

Outcome: a new crate `crates/missing_docs_in_test_functions/` that builds as a
standalone Dylint library, registers a pre-expansion pass with
`impl_pre_expansion_lint!`, and, during the spike only, emits a
debug-placeholder diagnostic on every function it sees under `--test`.

Work:

1. Create `crates/missing_docs_in_test_functions/Cargo.toml` modelled on
   `crates/string_continuation_style/Cargo.toml`:
   `crate-type = ["cdylib", "rlib"]`; features `dylint-driver` (enabling
   optional `dylint_linting`, `log`, `rustc_ast`, `rustc_lint`, `rustc_session`,
   `rustc_span`, `whitaker-common`, `whitaker_lint_core`) and `constituent`; a
   non-optional `thiserror` dependency for the policy module's error type;
   dev-dependencies `dylint_testing`, `googletest`, `insta`,
   `pretty_assertions`, `proptest`, `rstest`, `rstest-bdd`, `rstest-bdd-macros`,
   `serial_test`, `tokio` (macros), `whitaker-common`, `whitaker_lint_core`,
   and `whitaker_test_macros`, all from the workspace.
   `[lints] workspace = true`.
2. Create `src/lib.rs` with
   `#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]`,
   `pub mod policy;`, and a feature-gated `mod driver;` that re-exports
   `MISSING_DOCS_IN_TEST_FUNCTIONS` and `MissingDocsInTestFunctions`.
3. Create `src/driver/mod.rs` declaring the lint inside a
   `mod declaration { #![expect(missing_docs, reason = "...")] ... }` with
   `dylint_linting::impl_pre_expansion_lint!`.
4. Add three throwaway fixtures under `examples/spike_*.rs`: a root file with
   free functions, `#[cfg(not(test))]` items, `#[doc = include_str!(..)]`, and
   `#[given(...)]`; a `#[path = "support/spike_helpers.rs"] mod helpers;`
   out-of-line module; and an item with
   `#[allow(unknown_lints)] #[allow(missing_docs_in_test_functions)]`, plus a
   crate-level `#![allow(missing_docs_in_test_functions)]` variant.
5. Run them through a temporary UI test with `--test` and record the output
   as `EV-M0` in `Artefacts and notes`.

Acceptance evidence `EV-M0`: the transcript shows placeholder diagnostics in
both the root file and the out-of-line module file; none on the item-allowed
function; the observed behaviour of the crate-level allow in the module file;
that `#[given]` functions are seen with the attribute intact; and the attribute
kind of `#[doc = include_str!(..)]`.

Conformance check:

- `RFC3-TR-PREEXP` satisfied by the registered pre-expansion pass.
- The design is still followed if the hooks behave as `A-VISITOR` says.
- If any `Tolerances` "Semantics" or "Toolchain" trigger fires, stop.
- No public interface outside the new crate; no new dependency.
- Trace links unchanged.

Recovery: the spike is additive. Delete the `spike_*` fixtures, the temporary
test, and the placeholder emission before `EP-M1` is committed; keep the
skeleton. The milestone commit contains the skeleton without the placeholder
diagnostic, and `make check-fmt`, `make lint`, and `make test` pass.

Remaining gaps: no policy, no diagnostics.

Compatibility decision: none.

### `EP-M1`: pure policy kernel

Outcome: `crates/missing_docs_in_test_functions/src/policy/` contains
rustc-free types and functions, fully tested, and unused by the driver yet. The
module is public so the BDD integration tests and doctests can reach it; it is
pre-1.0 and carries no compatibility promise.

Files (each under 400 lines):

- `policy/mod.rs`: module docs and re-exports.
- `policy/facts.rs`: `HarnessStatus`, `Provenance`, `PathSelection`,
  `Documentation`, `FunctionContext`, `CandidateFacts`, `Verdict`, `SkipReason`.
- `policy/selection.rs`: `pub fn select(facts: CandidateFacts) -> Verdict`.
- `policy/context.rs`: `ContainerKind`, `FunctionShape`, and
  `ContainerStack` with `enter`, `exit`, and `classify`.
- `policy/documentation.rs`: `DocAttributeShape` and
  `pub fn documentation_status(shapes: impl IntoIterator<Item =
  DocAttributeShape>) -> Documentation`.
- `policy/span.rs`: `SpanFacts` and `pub fn choose_diagnostic_span<S>(...)`.
- Test modules `selection_tests.rs`, `context_tests.rs`,
  `context_props.rs`, `documentation_tests.rs`, `span_tests.rs`, wired with
  `#[cfg(test)] #[path = ...]` as the precedents do.
- `tests/features/candidate_selection.feature` and
  `tests/candidate_selection_behaviour.rs`.

Red-green-refactor:

1. Red: write the tests and the feature file first against `todo!()`-free
   stubs that return `Verdict::Skip(SkipReason::NotTestHarness)`,
   `Documentation::Undocumented`, and `FunctionContext::Nested`. Run
   `cargo nextest run -p missing_docs_in_test_functions --no-fail-fast 2>&1 |
   tee /tmp/test-whitaker-2-2-11-m1-red.out`
   and expect failures in `selection_tests`, `context_tests`,
   `documentation_tests`, and the BDD scenarios for the reasons asserted, not
   compile errors.
2. Green: implement each function minimally; rerun the same command and
   expect all to pass.
3. Refactor: tidy names and docs, rerun, then run `make check-fmt`,
   `make lint`, and `make test`, each piped through `tee` to
   `/tmp/<action>-whitaker-2-2-11-missing-docs-in-test-funcs-pre-expansion-ast-pass.out`.

BDD feature (`tests/features/candidate_selection.feature`):

```gherkin
Feature: Candidate selection for missing_docs_in_test_functions

  Scenario: An undocumented module function in a test harness is diagnosed
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is diagnose

  Scenario: A line doc comment suppresses the diagnostic
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has a line doc comment
    When the candidate predicate is evaluated
    Then the verdict is skip because the function is documented

  Scenario: A doc attribute suppresses the diagnostic
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has a doc attribute with an include_str value
    When the candidate predicate is evaluated
    Then the verdict is skip because the function is documented

  Scenario: A normal compilation never diagnoses
    Given a module function in a selected source file
    And the compilation is not a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the compilation is not a test harness

  Scenario: Expansion-derived functions are never diagnosed
    Given a module function whose span comes from a macro expansion
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the function is not source-authored

  Scenario: Methods in trait implementations are out of scope
    Given an associated function inside a trait implementation
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the function context is out of scope

  Scenario: A function nested in a function body is out of scope
    Given a free function nested inside another function
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the function context is out of scope
```

Steps follow the repository conventions: a `world` fixture of `RefCell` fields
annotated with `#[allow_fixture_expansion_lints]`, `given` and `when` steps
returning `Result<(), String>` without assertions, and assertions only in
`then` steps (`googletest` matchers such as
`assert_that!(verdict, eq(Verdict::Diagnose))`).

Unit tests use `rstest` tables, `googletest` matchers, and
`pretty_assertions::assert_eq` for structured comparisons. The truth table over
all 96 fact records is rendered as a fixed-width text table and snapshotted with
`insta::assert_snapshot!`, which gives reviewers a single artefact to read
when gate precedence changes.

Acceptance evidence `EV-M1-unit`: the red transcript, the green transcript, and
the three gates passing.

Conformance check: D-3 to D-6 implemented as written; no rustc dependency in
`policy/` (`rg -n 'rustc_' crates/missing_docs_in_test_functions/src/policy`
returns nothing); no new dependency; trace links current.

Recovery: the module is additive and unused by the driver; revert the milestone
commit to retry.

Remaining gaps: proofs, driver, UI tests.

Compatibility decision: none.

### `EP-M2`: Verus proofs and drift guard

Outcome: `verus/missing_docs_in_test_functions_policy.rs` proves `L-SOUND`,
`L-COMPLETE`, `L-PRECEDENCE`, `L-MONOTONE`, and `L-STACK`; `make verus` and a
new `make verus-missing-docs` target run it; CI runs it and guards it against
drift.

Proof sketch (what must be shown, not how):

- Selection: define the spec functions `should_diagnose`,
  `first_failing_gate`, and `suppressed_at_least` over the mirrored enums.
  `select` in the model mirrors the production match. `L-SOUND` and
  `L-COMPLETE` follow by case analysis on the gate that `select` checks first.
  `L-PRECEDENCE` follows from the fixed order of the checks. `L-MONOTONE`
  follows from `L-SOUND`, `L-COMPLETE`, and a field-wise lemma that each gate
  predicate is antitone in `⊑`. These are hypotheses about the proof structure;
  implementation may show that some are unnecessary for the SMT solver or need
  splitting.
- Container stack: define `open(p)` recursively over prefixes. The
  invariant is "the model's frame sequence equals `open(p)`" after every
  processed prefix. `Enter(k)` appends and `Exit` drops the last element,
  matching the definition of `open`; well-nestedness guarantees that `Exit`
  never sees an empty stack. Classification then needs a lemma relating "every
  element is `Module`" for `open(p)` to the model's quantifier over its frame
  sequence. The empty final stack follows from the balanced count. Auxiliary
  sequence lemmas are deferred until Verus exposes the actual verification
  conditions.

Work:

1. Write the lemma statements with empty proof bodies and run
   `make verus-missing-docs 2>&1 | tee /tmp/verus-whitaker-2-2-11-missing-docs-in-test-funcs-pre-expansion-ast-pass.out`;
   expect verification failures for the unproved lemmas (red).
2. Add proofs until the file verifies.
3. Apply each seeded fault from the `Verification plan` to the model,
   confirm Verus rejects it, and revert. Record the transcript lines.
4. Add the file to the `missing-docs` and `all` groups in
   `scripts/run-verus.sh`, add a `verus-missing-docs` Makefile target and its
   `.PHONY` entry, and add `crates/missing_docs_in_test_functions/**` to the
   `pull_request.paths` of `.github/workflows/proofs.yml`.
5. Add `scripts/check-verus-missing-docs-mirror.sh`, which extracts the
   variant names of the mirrored enums from
   `crates/missing_docs_in_test_functions/src/policy/facts.rs` and
   `policy/context.rs` and from the Verus file and fails if they differ. Call
   it from the CI job that runs `check-verus-fragment-id-bridge.sh`
   (`.github/workflows/ci.yml` around line 411) and from
   `make verus-missing-docs`.

Acceptance evidence `EV-M2`: Verus reports every function verified with zero
errors; the seeded-fault transcripts show rejection;
`rg -n
'assume|admit|external_body' verus/missing_docs_in_test_functions_policy.rs`
returns nothing; the drift guard fails when a variant is renamed in the
production enum and passes after reverting; `make check-fmt`, `make lint`, and
`make test` pass.

Conformance check: D-8 holds; proofs depend only on the spec functions and
`vstd`; no new dependency; trace links current.

Recovery: proof files are additive. If a lemma cannot be discharged within
tolerance, stop and escalate rather than weakening it.

Remaining gaps: driver and UI tests.

Compatibility decision: none.

### `EP-M3`: pre-expansion driver, diagnostics, and UI fixtures

Outcome: the pass converts AST items into `CandidateFacts`, consults the
policy, and emits a diagnostic at the chosen span. UI tests compiled with
`--test` cover the behaviour matrix.

Driver files (under `src/driver/`, each under 400 lines):

- `mod.rs`: lint declaration and `EarlyLintPass` impl with `check_item`,
  `check_item_post`, `check_impl_item`, `check_impl_item_post`,
  `check_trait_item`, and `check_trait_item_post`. Every pre-hook first
  classifies the item (when it is a function), then pushes exactly one frame;
  every post-hook pops exactly one frame.
- `facts.rs`: builds `CandidateFacts` from `&EarlyContext` and the AST item:
  harness from `cx.sess().opts.test`, provenance from
  `item.span.from_expansion()`, path selection from
  `cx.sess().source_map().span_to_filename(item.span)` resolving to a real
  local path (D-2), documentation via `policy::documentation_status` over
  shapes mapped from `item.attrs`.
- `attributes.rs`: maps `rustc_ast::Attribute` to `DocAttributeShape` by
  inspecting `AttrKind::DocComment` and a `Normal` attribute whose path is
  `doc` with `AttrArgs::Eq` (D-6).
- `diagnostics.rs`: loads the localizer once per pass instance via
  `whitaker_lint_core::SharedConfig::load()` and `get_localizer_for_lint`,
  resolves the message set with an English fallback (D-9), and emits with
  `cx.emit_span_lint` and `DiagDecorator`. Primary message:
  ``test function `{name}` has no documentation comment``. Help:
  ``add a `///` comment directly above `fn {name}` that
  explains what the test exercises``.

UI fixtures under `examples/`, run by `src/tests/ui.rs` (module path chosen so
that the nextest serial-lane filter `test(tests::ui::)` in
`.config/nextest.toml` applies), with
`rustc_flags(["--test", "-D", "missing_docs_in_test_functions"])` unless stated:

- `fail_undocumented_free_function.rs`: caret on the identifier.
- `pass_documented_forms.rs`: `///`, `/** */`, inner `//!`,
  `#[doc = "..."]`, `#[doc = include_str!("support/doc.md")]`.
- `fail_doc_hidden_is_not_documentation.rs`: `#[doc(hidden)]` alone.
- `fail_proc_macro_attribute_function.rs`: an undocumented
  `#[given("...")]` step and an undocumented `#[tokio::test]` function, each
  diagnosed once at the identifier, with no diagnostic on generated items.
- `pass_macro_generated_function.rs`: a `macro_rules!` macro that expands
  to an undocumented function.
- `pass_undocumented_without_test_harness.rs`: the free-function source
  compiled without `--test`.
- `fail_inherent_method.rs` and `pass_trait_impl_method.rs`, plus a trait
  with an undocumented default method (fail) and an undocumented bodiless
  method (pass).
- `pass_nested_function.rs`: functions inside a function body, a `const`
  initializer, and an enum discriminant.
- `fail_out_of_line_module.rs` with `support/out_of_line_helpers.rs`: the
  diagnostic is reported in the module file.
- `pass_item_level_allow.rs`: the two-attribute workaround on one function
  and an undocumented control function that is still diagnosed. 2.2.15 extends
  this into the full suppression matrix.
- `fail_cfg_gated_function.rs`: an undocumented `#[cfg(not(test))]`
  function is diagnosed (D-7).

Red-green-refactor:

1. Red: add the fixtures and blessed `.stderr` files written by hand from the
   expected behaviour, then run
   `cargo nextest run -p missing_docs_in_test_functions tests::ui 2>&1 | tee /tmp/test-ui-whitaker-2-2-11-missing-docs-in-test-funcs-pre-expansion-ast-pass.out`
   with the driver still emitting nothing; expect failures on every `fail_*`
   fixture.
2. Green: wire the driver; rerun and expect all to pass. Re-bless only after
   reading each diff and confirming it matches the handwritten expectation.
3. Refactor; run `make check-fmt`, `make lint`, and `make test`.

Acceptance evidence `EV-M3-ui`: the red and green transcripts, and the blessed
`.stderr` for `fail_undocumented_free_function.rs` and
`fail_out_of_line_module.rs` quoted in `Artefacts and notes`.

Conformance check: every traced requirement except `RFC3-FR-PATH` (partial by
design) is now observable; the driver contains no policy decisions beyond
mapping facts; no suite or installer registration; trace links current.

Recovery: the driver is confined to the new crate; revert the milestone commit
to retry.

Remaining gaps: documentation.

Compatibility decision: none.

### `EP-M4`: documentation, ADR, and roadmap

Outcome: documentation describes the lint and its conventions; the roadmap
marks 2.2.11 done.

Work:

1. Write
   `docs/adrs/adr-0014-pre-expansion-pass-for-missing-docs-in-test-functions.md`
   recording the pre-expansion pass shape, the container-stack design, the
   in-scope function forms (D-5), the documentation rule (D-6), and the interim
   path gate (D-2), following the documentation style guide's ADR format.
   Re-scan fetched remote branches for ADR 0014 immediately before committing,
   and renumber if it has been taken.
2. Add a `missing_docs_in_test_functions` section to
   `docs/whitaker-dylint-suite-design.md` that summarizes the design, records
   that 2.2.13 will need a `PassKind::PreExpansion` descriptor and a
   `register_pre_expansion_pass` call, and links ADR 0014.
3. Add "Writing a pre-expansion pass" to `docs/developers-guide.md` beside
   "Writing an early pass": per-module pass instantiation, dummy `NodeId`s,
   unevaluated `cfg`, lint-level scope in out-of-line modules (as observed in
   `EV-M0`), the container stack and its balanced-hook rule, `--test` UI
   harnesses, and the Verus mirror and drift guard workflow, including the
   re-use policy for the `policy` module (owned by this crate; not to be
   imported by other lints; promote to a shared crate only through a new
   decision).
4. Add a `missing_docs_in_test_functions` entry to `docs/users-guide.md`
   §"Available Lints", stating that it is not yet part of the suite, the
   in-scope forms, the documentation rule, the pre-expansion limitations, and
   the two-attribute suppression workaround.
5. Add the crate to `docs/repository-layout.md` §`crates/`, the ADR and the
   new developers' guide section to `docs/contents.md`, and annotate RFC 0003
   §11 open question 2 with the answer from D-5 and a link to ADR 0014.
6. Mark roadmap item 2.2.11 `[x]` in `docs/roadmap.md`.
7. Run `make fmt`, `make markdownlint`, `make nixie`, `make spelling`,
   `make check-fmt`, `make lint`, and `make test`, each through `tee`.

Acceptance evidence `EV-M4`: all gates pass; `git diff --stat` shows the
documentation files above; the roadmap shows `[x] 2.2.11`.

Conformance check: every decision is recorded in ADR 0014 or the design
document; no upstream artefact is left contradicting the implementation.

Recovery: documentation-only; revert and retry.

Remaining gaps: 2.2.12 to 2.2.16.

Compatibility decision: none.

## Concrete steps

Run every command from the repository root:
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/82a0f684-e8e8-4e9a-9a3f-77ed71436818`.

Gate commands, run sequentially for each milestone (never in parallel for the
same worktree):

```sh
BRANCH=$(git branch --show-current)
make fmt 2>&1 | tee "/tmp/fmt-whitaker-${BRANCH}.out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-whitaker-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-whitaker-${BRANCH}.out"
```

For documentation changes also run:

```sh
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-whitaker-${BRANCH}.out"
make spelling 2>&1 | tee "/tmp/spelling-whitaker-${BRANCH}.out"
```

For `EP-M2` and later:

```sh
make verus-missing-docs 2>&1 | tee "/tmp/verus-whitaker-${BRANCH}.out"
make verus 2>&1 | tee "/tmp/verus-all-whitaker-${BRANCH}.out"
```

Expected success tails: `make test` ends with nextest's
`Summary […] N tests run: N passed`; Verus prints
`verification results:: N verified, 0 errors`.

Commit after each milestone, once all its gates pass, with an imperative
subject line and a body explaining what and why.

## Validation and acceptance

The item is accepted when:

- compiling `fail_undocumented_free_function.rs` with `--test` through the
  UI harness produces exactly one `missing_docs_in_test_functions` diagnostic
  whose primary span is the function identifier, and compiling the same source
  without `--test` produces none;
- every fixture listed under `EP-M3` matches its blessed `.stderr`;
- `make verus-missing-docs` reports zero errors and the drift guard passes;
- `make check-fmt`, `make lint`, and `make test` pass;
- the roadmap marks 2.2.11 done and the documentation listed in `EP-M4`
  exists.

Quality criteria:

- Tests: unit, BDD, snapshot, property, and UI suites in the new crate pass
  under `make test`.
- Verification: `L-SOUND`, `L-COMPLETE`, `L-PRECEDENCE`, `L-MONOTONE`, and
  `L-STACK` discharged by Verus; `T-DOCS`, `T-SPAN`, and `T-HOOKS` discharged
  by tests.
- Lint and type checks: `make lint` passes with no new suppressions beyond
  the documented `mod declaration` `expect(missing_docs)` precedent.
- Security: no new trust boundary; the lint reads only the session, the
  source map, and the existing Dylint configuration.

## Idempotence and recovery

All steps are additive and can be rerun. UI blessing can be repeated after
reading diffs. If a gate fails, read the cited log under `/tmp` rather than
rerunning it, fix, and rerun only that gate before rerunning the full sequence.
To roll back, revert the milestone commit; no data or external state is changed
by this work.

## Artefacts and notes

Record here, as the work proceeds:

- `EV-M0`: the spike transcript.
- `EV-M1-unit`: the red and green test transcripts and the insta snapshot
  path.
- `EV-M2`: the Verus summary line and the seeded-fault rejections.
- `EV-M3-ui`: the red and green UI transcripts and two blessed `.stderr`
  excerpts.

## Interfaces and dependencies

Crate: `crates/missing_docs_in_test_functions`, package name
`missing_docs_in_test_functions`. Dependencies are all existing workspace
dependencies listed in `EP-M0`.

Public items in `missing_docs_in_test_functions::policy` at the end of `EP-M1`:

```rust
pub enum HarnessStatus { TestHarness, NotTestHarness }
pub enum Provenance { Source, Expansion }
pub enum PathSelection { Selected, NotSelected }
pub enum Documentation { Documented, Undocumented }
pub enum FunctionContext {
    ModuleFunction,
    InherentMethod,
    TraitDefaultMethod,
    TraitImplMethod,
    BodylessTraitMethod,
    Nested,
}
pub struct CandidateFacts {
    pub harness: HarnessStatus,
    pub provenance: Provenance,
    pub context: FunctionContext,
    pub path: PathSelection,
    pub documentation: Documentation,
}
pub enum SkipReason {
    NotTestHarness,
    NotSourceAuthored,
    OutOfScopeContext,
    PathNotSelected,
    Documented,
}
pub enum Verdict { Diagnose, Skip(SkipReason) }
pub fn select(facts: CandidateFacts) -> Verdict;

pub enum ContainerKind { Module, InherentImpl, TraitImpl, Trait, Other }
pub enum FunctionShape { Free, Associated { has_body: bool } }
pub struct ContainerStack { /* Vec<ContainerKind> */ }
impl ContainerStack {
    pub fn enter(&mut self, kind: ContainerKind);
    pub fn exit(&mut self) -> Result<(), UnbalancedExit>;
    pub fn classify(&self, shape: FunctionShape) -> FunctionContext;
}

pub enum DocAttributeShape {
    DocComment,
    DocNameValue,
    DocList,
    CfgAttr,
    Other,
}
pub fn documentation_status(
    shapes: impl IntoIterator<Item = DocAttributeShape>,
) -> Documentation;

pub struct SpanFacts { pub is_dummy: bool, pub is_from_expansion: bool, pub is_within_item: bool }
pub fn choose_diagnostic_span<S>(ident: Option<(S, SpanFacts)>, item: S) -> S;
```

All enums derive `Clone`, `Copy`, `Debug`, `PartialEq`, and `Eq`.
`ContainerStack::exit` returns `Err(UnbalancedExit)`, a unit error struct
derived with the workspace `thiserror` dependency (`thiserror = "2"`, added to
the crate as a non-optional dependency so the policy module can use it),
because an unbalanced exit is impossible under `A-VISITOR`; the driver logs it
at `debug` with the workspace `log` crate, matching
`string_continuation_style`, and continues rather than panicking.

Driver items (feature `dylint-driver`):
`pub static MISSING_DOCS_IN_TEST_FUNCTIONS: &Lint` and
`pub struct MissingDocsInTestFunctions` implementing
`rustc_lint::EarlyLintPass`.

Skills to load when implementing: `rust-router`, then `rust-unit-testing`,
`proptest`, `verus`, and `rust-types-and-apis` as each milestone needs them;
`execplans` to maintain this document; `codegraph-mcp` for navigation;
`en-gb-oxendict-style` for documentation.

Documentation to read first: `AGENTS.md`;
`docs/rfcs/rfc-0003-missing-docs-in-test-functions.md`;
`docs/whitaker-dylint-suite-design.md`; `docs/developers-guide.md` §"Writing an
early pass" and §"Proof workflows"; `docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`; and
`docs/whitaker-clone-detector-design.md` for the existing proof-sidecar
conventions.

## Revision note

- 2026-11-11: initial draft written from Wyvern reconnaissance and direct
  inspection of the pinned rustc sources. Awaiting community-of-experts review
  and user approval.
