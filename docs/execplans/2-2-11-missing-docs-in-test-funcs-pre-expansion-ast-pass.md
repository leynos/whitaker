# Deliver the `missing_docs_in_test_functions` pre-expansion pass (2.2.11)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

This plan must be approved before implementation begins. Do not start milestone
`EP-M0` until the user has explicitly approved this document, including the
decisions marked "needs approval" in `Decision log` (D-2 and D-8).

## Purpose / big picture

Roadmap item 2.2.11 delivers the first half of RFC 0003: a Whitaker lint named
`missing_docs_in_test_functions` that runs as a Dylint pre-expansion pass and
reports source-authored functions that lack documentation, but only when rustc
is compiling a test harness (`rustc --test`, which is what `cargo test` and
`cargo check --tests` do for unit and integration test targets).

A pre-expansion pass is a lint pass that rustc runs over the abstract syntax
tree (AST) immediately after parsing and before any macro is expanded. That
timing matters for test code because procedural attribute macros such as
`#[rstest]` or `rstest-bdd`'s `#[given(...)]` rewrite the function they
decorate into wrappers and generated machinery. Before expansion, the lint sees
exactly the function the developer wrote, with its own attributes and doc
comments, and nothing that a macro generated.

After this change, a developer who builds the new lint library and runs it over
a test target observes:

- ``warning: test function `counter_starts_at_zero` has no documentation comment``
  with the caret under the function's name, for an undocumented `fn` in a
  test-harness compilation, including one decorated with a procedural attribute
  macro such as `#[given("...")]`;
- no diagnostic for the same function when it carries a `///` comment, a
  `/** ... */` comment, an inner `//!` comment in its body, or a
  `#[doc = "..."]` attribute;
- no diagnostic for anything when the same source is compiled without
  `--test`;
- no diagnostic for functions that macros generate, for functions nested
  inside function bodies, or for methods in trait implementations.

This item delivers the pass and its candidate-selection predicate only. The
configurable include/exclude path policy is roadmap item 2.2.12, registration
in the Whitaker suite is 2.2.13, translation of the messages into Welsh and
Scottish Gaelic is 2.2.14, the SARIF-contract UI matrix is 2.2.15, and the
property-test suite for path configuration is 2.2.16. Until 2.2.13 lands, the
lint is buildable and testable inside this repository but is not shipped to
users through the suite, the installer, or the rolling release. The section
`Hand-off to later roadmap items` lists what each later item inherits.

## Constraints

- The lint must be implemented as a Dylint pre-expansion pass, registered by
  `dylint_linting::impl_pre_expansion_lint!`. A late (HIR) pass, a
  post-expansion early pass, a Clippy pass, a rustdoc JSON consumer, or a text
  scanner is forbidden (RFC 0003 §7, technical requirements).
- The predicate must return "no diagnostic" unless every gate in RFC 0003 §6
  "Candidate selection" holds: test harness, source-authored function item,
  selected source path, and absence of documentation.
- Documentation comments in every sugared form (`///`, `/** */`, and the
  inner forms `//!` and `/*! */` inside a function body) and the `#[doc = ...]`
  attribute must count as documentation.
- The diagnostic must be attached to the function's identifier whenever the
  identifier span is reliable, falling back to the item span otherwise.
- Do not register the lint in `suite/`, the installer's lint lists
  (`installer/src/resolution.rs`, `installer/src/bin/package_lints.rs`), the
  `Makefile` `LINT_CRATES` list, `.github/workflows/rolling-release.yml`, or
  `EXPERIMENTAL_LINT_CRATES`. Those belong to 2.2.13.
- Do not add `.ftl` locale resources. Adding an `en-GB` resource without `cy`
  and `gd` counterparts fails `common/tests/i18n_quality`, and the translations
  belong to 2.2.14. Messages must nevertheless flow through the shared
  `whitaker_common::i18n` resolution helpers with an English fallback, so that
  2.2.14 only adds resources.
- Do not implement `dylint.toml` include/exclude configuration, glob matching,
  workspace-relative path normalization, or a shared path-glob helper. Those
  belong to 2.2.12, and RFC 0003 §6 requires a shared helper's ownership to be
  documented before it exists.
- The policy module must not depend on any `rustc_*` crate, so its unit,
  behavioural, snapshot, and property tests run on plain values.
- No source file may exceed 400 lines (`AGENTS.md`). Every module starts with
  a `//!` comment; every function carries a `///` comment.
- Use only dependencies already declared in the root `Cargo.toml`
  `[workspace.dependencies]`. In particular, `tokio` is not a workspace
  dependency, so no fixture may use `#[tokio::test]`.
- Do not unset or override `RUSTC_WRAPPER`, and do not bypass the shared
  compile-admission queue.
- Documentation follows `docs/documentation-style-guide.md`: en-GB Oxford
  spelling, 80-column wrapping for prose, dashes for bullets.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 60 files or more than
  4,500 net lines of code and other documentation, stop and escalate. This
  ExecPlan file is excluded from both counts, as are blessed `.stderr` goldens
  and the insta snapshot, which are generated rather than authored.
- Interface: if delivering the item requires changing a public signature in
  `whitaker-common`, `whitaker_lint_core`, or `suite`, stop and escalate. The
  new lint crate owns its helpers.
- Dependencies: if any new external dependency, or a workspace dependency
  version change, is required, stop and escalate.
- Toolchain: if the `EP-M0` spike shows that `impl_pre_expansion_lint!` from
  the locked `dylint_linting` 6.3.0 does not register a working pass on
  `nightly-2026-05-28`, stop and escalate rather than upgrading Dylint or the
  toolchain.
- Semantics: if the spike shows that pre-expansion hooks do not see functions
  in out-of-line module files (`mod helpers;`), or that item-level
  `#[allow(missing_docs_in_test_functions)]` is not honoured, stop and
  escalate, because both break RFC 0003 requirements that later items build on.
- Expectations: if the spike shows that
  `#[expect(missing_docs_in_test_functions)]` on a function carrying a
  procedural attribute macro both suppresses the warning and raises
  `unfulfilled_lint_expectations`, record it, keep going with `allow`-only
  guidance, and raise it with the user at the next milestone boundary, because
  it affects how the estate suppresses the lint.
- Iterations: if a gate still fails after three fix attempts for the same root
  cause, stop and escalate.
- Time: if a milestone takes more than one working day of wall time, excluding
  time spent queued for compile admission, stop and escalate.
- Ambiguity: if a newly discovered source form, not listed in D-5, materially
  changes which items are candidates, stop and present options.

## Risks

- Risk: rustc documents `register_pre_expansion_pass` as "softly deprecated
  … might be removed in the future" (`rustc_lint/src/context.rs:175-180` in the
  pinned sources), and Dylint 6.3.0 already switches to a renamed registration
  method for compilers from 2026-06-22. A future toolchain bump could remove
  the interface the RFC mandates. Severity: high (for 2.2.13 onwards, when the
  lint joins the suite). Likelihood: medium over a year. Mitigation: ADR 0014
  records the dependency and an exit plan: move to a post-expansion early pass
  that treats a function as authored when its identifier span has the root
  syntax context (attribute macros that re-emit the authored item usually
  preserve the identifier's span), accepting lower precision for macros that
  rename or synthesize functions; the developers' guide adds this crate to the
  toolchain-bump checklist; the hand-off asks 2.2.13 to keep the registration
  behind a suite feature so the suite can drop it without failing to build.

- Risk: the lint is silently inert for typical invocations. `opts.test` is set
  only by `--test`, so `cargo dylint` over a plain `cargo check` (no test
  targets) never fires it, and `harness = false` targets and doctests are never
  checked. Severity: medium. Likelihood: high. Mitigation: the users' guide
  states that the lint needs `--all-targets` or `--tests`; the driver emits a
  `log::debug!` event for every function it classifies, with the verdict and
  skip reason, so an inert run is observable.

- Risk: `#[expect(missing_docs_in_test_functions)]` misfires before
  expansion. Expectations are recorded against attribute identifiers, and
  procedural-macro output is re-parsed with fresh identifiers
  (`rustc_lint/src/levels.rs:240-264`, `rustc_lint/src/expect.rs:28-51`,
  `rustc_ast/src/attr/mod.rs:54`), so an `expect` on an `#[rstest]` or
  `#[given]` function may suppress the warning and still raise
  `unfulfilled_lint_expectations`. `#[cfg_attr(..., expect(...))]` and the ADR
  0002 macro are invisible before expansion. Severity: high for estate
  adoption. Likelihood: medium (inferred from source; not yet run). Mitigation:
  the `EP-M0` spike measures it; the users' guide prescribes the two-attribute
  `allow` workaround only; the result feeds the 2.2.15 suppression matrix.

- Risk: `rstest` expansion through the Dylint compiletest driver is known to
  hang on Windows
  (`crates/no_unwrap_or_else_panic/examples/pass_unwrap_in_rstest_harness.rs:31`).
  Severity: medium. Likelihood: low. Mitigation: reuse that fixture's
  `cfg(windows)` fallback pattern if the hang reappears.

- Risk: doctests on the public policy module fail with `E0658` under
  `make test`, which runs doctests with `--all-features` and therefore links a
  crate that enables `rustc_private`. Severity: low. Likelihood: high.
  Mitigation: this is a planned `EP-M1` task, following the
  `bumpy_road_function` precedent of hidden
  `# #![cfg_attr(feature = "dylint-driver", feature(rustc_private))]` lines
  (`crates/bumpy_road_function/src/analysis.rs:71-76`).

- Risk: driver code runs inside nested driver processes during UI tests, so
  coverage tooling may not count it against the 80% patch target in
  `codecov.yml`. Severity: low. Likelihood: medium. Mitigation: keep the driver
  thin and the policy thick; report the coverage figure in
  `Outcomes & retrospective` rather than adding tests merely to satisfy it.

- Risk: compile admission on the shared host makes gates slow. Severity: low.
  Likelihood: high. Mitigation: allow generous timeouts, check
  `build-limits-status` before blaming the candidate, and never start a second
  build to "help".

## Progress

- [x] (2026-11-11) Rename the branch to
  `2-2-11-missing-docs-in-test-funcs-pre-expansion-ast-pass`.
- [x] (2026-11-11) Reconnaissance by a Wyvern team: lint crate template, AST
  and pre-expansion APIs on the pinned toolchain, verification infrastructure,
  and test-harness fixture precedents.
- [x] (2026-11-11) Draft this ExecPlan (commit `bf53e8fa`).
- [x] (2026-11-11) Community-of-experts design review (four reviewers:
  structure and contracts, compiler failure modes, verification strategy,
  alternatives and repository fit) and revision of this ExecPlan.
- [ ] User approval of this ExecPlan and of decisions D-2 and D-8.
- [ ] `EP-M0` pre-expansion spike and crate skeleton.
- [ ] `EP-M1` pure policy kernel with unit, behavioural, snapshot, and
  property tests.
- [ ] `EP-M2` pre-expansion driver, diagnostics, and UI fixtures.
- [ ] `EP-M3` documentation, ADR, and roadmap completion.

## Surprises & discoveries

- Observation: the repository has no pre-expansion lint yet; the only early
  lint, `string_continuation_style`, is a post-expansion early pass. Evidence:
  no use of `declare_pre_expansion_lint!` or `impl_pre_expansion_lint!` outside
  `docs/`. Impact: this crate sets the pattern for pre-expansion lints, so its
  conventions are written down in the developers' guide and in an ADR.

- Observation: rustc runs a pre-expansion pass once for the crate root and
  once more for every out-of-line module file as expansion loads it
  (`rustc_interface/src/passes.rs:87-109`,
  `rustc_expand/src/expand.rs:1383-1410`), and Dylint's registration closure
  builds a fresh pass value for each run
  (`dylint_linting-6.3.0/src/lib.rs:438`). Module-file runs walk only the
  module's attributes and items and never call `check_crate` or
  `check_crate_post` (`rustc_lint/src/early.rs:299-310`). Impact: no state or
  configuration may be initialized in `check_crate`; the localizer is cached
  process-wide; each module file starts with an empty container stack, so a
  `#[path] mod m;` declared inside a function body classifies its functions as
  module-level (recorded in D-5).

- Observation: each pre-expansion run builds its lint-level stack from the
  command line plus the attributes it walks
  (`rustc_lint/src/levels.rs:400-418`). A module file's run sees only the outer
  attributes on its `mod` line and the file's inner attributes. A crate-root
  `#![allow(...)]`, or an allow on an enclosing inline module, therefore does
  not reach items in an out-of-line module file, whereas `-A`/`-D`,
  `--cap-lints`, and Cargo `[lints]` apply to every run, and item-level
  `#[allow]` (including `#![allow]` inside a function body) is honoured
  (`rustc_lint/src/early.rs:79-85`). Evidence: source reading by the compiler
  reviewer; `EP-M0` confirms empirically. Impact: documented in the users' and
  developers' guides; covered by a UI fixture.

- Observation: before expansion, node identifiers are dummies, `cfg` has not
  been evaluated, and macro invocations are `MacCall` nodes. Functions written
  inside macro invocations (`proptest! { ... }`, `macro_rules!` bodies,
  `include!` files) are token trees, never items, and are never linted. A
  `#[cfg]`-disabled inline function is linted, but a `#[cfg]`-disabled `mod x;`
  file is never loaded and therefore never linted. Impact: recorded as known
  limitations in D-5 and D-7 and covered by fixtures.

- Observation: `EarlyLintPass` offers paired hooks (`check_item` and
  `check_item_post`, `check_impl_item` and `check_impl_item_post`,
  `check_trait_item` and `check_trait_item_post`), and the visitor calls each
  pair unconditionally inside one closure around the walk of that node
  (`rustc_lint/src/early.rs:79-85, 208-228`). `check_item` also fires for items
  in statement position, `const` blocks, and enum discriminants, and for
  `ItemKind::MacCall` and `MacroDef`. Foreign items get no hooks
  (`rustc_lint/src/early.rs:87-91`). Impact: a stack that pushes exactly one
  frame in every pre-hook and pops exactly one in every post-hook is balanced
  by construction.

- Observation: before expansion, a source-authored function's identifier span
  always has the root syntax context, and `from_expansion()` is never true for
  items the pass sees. Impact: the provenance gate and the span fallback are
  defence in depth; no UI fixture can reach them, so their non-vacuity rests on
  the pure-kernel tests and on axiom `A-PREEXP`.

- Observation: `Test::example` copies a single `.rs` file into a temporary
  directory (`vendor/dylint_testing/src/example.rs:65-79`), so example fixtures
  cannot use `mod helpers;` or `include_str!` with sibling files. Impact:
  fixtures that need sibling files live in a `ui/` directory run by
  `Test::src_base`, following
  `crates/no_expect_outside_tests/src/lib_ui_tests.rs:104-122`.

- Observation: ADR number 0013 is already used on the unmerged branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`. Evidence:
  scanning every fetched `refs/remotes/origin/*` tree for `adr-001[3-9]`.
  Impact: this plan allocates ADR 0014.

## Decision log

- Decision D-1: put the rustc-free policy in a public `policy` module inside
  the new lint crate (`crates/missing_docs_in_test_functions/src/policy/`), not
  in a new support crate and not in `whitaker-common`. Rationale: the kernel is
  small and specific to this lint. A sweep found nothing reusable: `common/` and
  `crates/whitaker_lint_core` hold HIR-bound documentation and span helpers
  and rstest-specific detection;
  `crates/string_continuation_style/src/exclusion.rs` and
  `whitaker_common::SimplePath` match module paths, not file paths. A separate
  rustc-free crate (the `whitaker_string_literals` precedent) would avoid the
  doctest `rustc_private` lines, but adds a crate for roughly 300 lines of
  policy; the hidden-line precedent is cheaper. If 2.2.12 needs a shared
  path-glob helper, that item records its ownership. Date/Author: 2026-11-11,
  planning agent, revised after panel review.

- Decision D-2 (needs approval): split the path gate into a permanent source
  recovery step and an interim selection step. Source recovery maps the item
  span to `SourceFile::Real(path)` when the source map yields a real local
  path, and `SourceFile::Virtual` otherwise; virtual sources are never
  selected. Until 2.2.12 arrives, a crate-private
  `PathPolicy::InterimUnrestricted` selects every real file. Rationale: RFC
  0003 §6 makes the path gate clause 3 of the predicate and matches "after the
  compiler source map has recovered a real file path", so recovery is
  permanent. The interim selector is the panel's main concern: it reproduces,
  until 2.2.12, the rejected RFC §10 alternative ("require docs for every
  `--test` function") and violates the RFC §9 expectation that a library source
  function compiled under a harness passes. The plan accepts that for one
  roadmap step because 2.2.13 (the first shipping item) requires 2.2.12, and it
  makes the debt visible: a UI fixture `fail_library_source_interim.rs` records
  the interim behaviour and is inverted by 2.2.12, the policy variant is named
  `InterimUnrestricted`, and the hand-off requires 2.2.12 to make an absent or
  empty `include` inert, as RFC §8's opt-in contract demands (answering RFC §11
  open question 3). Alternatives: (a) inert until configured, which honours the
  RFC today but needs `dylint.toml` parsing of `include` now, pulling part of
  2.2.12 into this item; (b) a built-in default such as "paths containing a
  `tests/` component", which invents a path grammar that 2.2.12 must own. The
  panel noted that `.dylint.toml` fixture sidecars already exist, so (a) costs
  a config loader rather than a test-only seam. Date/Author: 2026-11-11,
  planning agent, revised after panel review.

- Decision D-3: represent each gate as a two-variant enum rather than a `bool`
  (`HarnessStatus`, `Provenance`, `BodyPresence`, `PathSelection`,
  `Documentation`) and the function's location as a `FunctionContext` enum.
  `PathSelection` stays binary permanently; 2.2.12 adds a separate `PathPolicy`
  that produces it, with its own reason type if it needs one. Rationale:
  `AGENTS.md` asks for newtypes over primitive soup; named variants make the
  truth-table snapshot readable and stop arguments being transposed; keeping
  `PathSelection` binary stops 2.2.12 churning the snapshot and the exhaustive
  tests. Date/Author: 2026-11-11, planning agent.

- Decision D-4: the predicate returns a `Verdict` that is either `Diagnose` or
  `Skip(SkipReason)`, where the reason names the first failing gate in the
  fixed order harness, provenance, context, body, path, documentation.
  Rationale: the reason powers `log::debug!` events and makes gate precedence a
  testable property. The order puts the cheapest session-wide gate first and
  the attribute scan last. The context and body gates refine RFC clause 2 ("a
  source-authored function item"). Date/Author: 2026-11-11, planning agent.

- Decision D-5: in-scope function forms are free functions whose every
  enclosing item is a module, inherent associated functions (in an `impl Type`
  block whose every enclosing item is a module), and trait methods with a
  default body. `async`, `const`, `unsafe`, and `extern "ABI"` functions with
  bodies are all `ItemKind::Fn` and are in scope. Out of scope: methods in
  trait implementations (`impl Trait for Type`), because their documentation
  conventionally lives on the trait; trait methods without a body (RFC 0003
  §6); functions nested inside a function, constant, static, enum discriminant,
  or other non-module item; foreign functions (which receive no hooks);
  closures; and anything generated by a macro. Known limitations, documented
  rather than solved: functions written inside macro invocations (`proptest!`,
  `macro_rules!` bodies, `include!` files) are never seen, and a
  `#[path] mod m;` inside a function body classifies its functions as
  module-level because each module file's run starts with an empty stack.
  Rationale: RFC 0003 §6 asks the UI matrix to state the included forms and §11
  leaves associated functions open. "Every enclosing item is a module" is
  simple to state, track, and test, and matches rustc's own `missing_docs`,
  which ignores items inside bodies. Date/Author: 2026-11-11, planning agent,
  revised after panel review.

- Decision D-6: documentation is present when any attribute on the function
  (outer, or inner within the body) is a sugared doc comment, a `doc` attribute
  with an `=` value of any expression form, or a `cfg_attr` whose attribute
  list contains such a `doc = ...` entry. `#[doc(hidden)]` and other list-form
  `doc` attributes do not count. Empty or whitespace-only documentation counts
  as present. Rationale: the success criterion requires `///` and
  `#[doc = "..."]` to count. Classifying by shape rather than by
  `AttributeExt::doc_str()` keeps `#[doc = include_str!("...")]` working,
  because its value is an unexpanded macro call (`AttrArgs::Eq { expr }`,
  `rustc_ast/src/ast.rs:2058-2069`). Counting `cfg_attr(..., doc = ...)` errs
  towards silence, which is the safe direction for a predicate whose contract
  is "MUST only diagnose undocumented functions". Counting empty documentation
  follows rustc's `missing_docs`; documentation quality is out of scope.
  Date/Author: 2026-11-11, planning agent, revised after panel review.

- Decision D-7 (known limitation, not a policy choice): functions under a
  `#[cfg(...)]` that later removes them are still candidates when written
  inline, because `cfg` is unevaluated before expansion, whereas functions in a
  `#[cfg]`-disabled out-of-line module file are never seen. The fixture
  `cfg_gated_functions.rs` records both behaviours as a limitation record, not
  a requirement; a later item may revisit it. Rationale: evaluating `cfg` would
  reimplement rustc configuration; skipping every `#[cfg]`-gated function would
  also skip `#[cfg(test)]` helpers, which are exactly the functions the policy
  targets. Date/Author: 2026-11-11, planning agent, revised after panel review.

- Decision D-8 (needs approval): discharge the selection contract by
  exhaustive enumeration of the production code, discharge the container-stack
  invariant with a property test against an independent tree oracle, and do not
  add a Verus proof in 2.2.11. Record `L-MONOTONE` as a contract that 2.2.12
  must prove in Verus once the path gate quantifies over unbounded include and
  exclude pattern lists. Rationale: three of four reviewers judged that Verus
  proofs here would restate their specifications. The selection domain is
  finite (160 records); a model of a six-way conjunction checked by an SMT case
  split over a copy of the code adds a drift surface and no assurance beyond
  running the real code over all 160 records, which is itself an exhaustive
  proof. The stack proof as first sketched defined `open(p)` by the same
  append-and-drop rule as the implementation, so "frames equal `open(p)`" held
  by definition, and the real risk (rustc calling hooks in pairs) is an axiom
  no proof touches. In 2.2.12 the anti-widening lemma ("adding an exclude
  pattern never creates a diagnostic; an empty include list diagnoses nothing")
  ranges over unbounded lists with matching as an uninterpreted predicate,
  which is the substantive setting `AGENTS.md` and ADR 0003 have in mind.
  Deviation note: `AGENTS.md` says to "use an exhaustive proof with verus for
  introduced lemmas or contractual business logic". This plan meets the
  "exhaustive proof" part by exhaustion over the real code and does not use
  Verus; that is why this decision needs explicit approval. Alternative
  offered: add `EP-V`, a Verus proof of `L-STACK` with a declarative
  specification of the open containers (an `Enter` at position `i` is open
  after prefix `p` exactly when every suffix of `p[i+1..m]` has non-negative
  balance), a witness lemma showing that the well-nestedness precondition is
  satisfiable, and a drift check implemented as a Rust unit test that reads the
  Verus file with `include_str!` and compares the normalized `classify` logic.
  Cost: one proof file, both group lists in `scripts/run-verus.sh`, a
  `verus-missing-docs` Makefile target, the `proofs.yml` path trigger, and the
  developers' guide trust-boundary section. Date/Author: 2026-11-11, planning
  agent, revised after panel review.

- Decision D-9: diagnostics resolve through
  `whitaker_common::i18n::safe_resolve_message_set` with `noop_reporter` and an
  English fallback message set, with no `.ftl` resources yet. The localizer is
  loaded once per process through a `std::sync::OnceLock`, not in
  `check_crate`. Rationale: this satisfies the RFC's localization conventions
  structurally and leaves the translated resources to 2.2.14. The `OnceLock`
  avoids reloading `SharedConfig` for every out-of-line module run and works in
  runs that never call `check_crate`. Date/Author: 2026-11-11, planning agent,
  revised after panel review.

- Decision D-10: the lint's default level is `Warn`, and the help text tells
  the reader to add a `///` comment above the function and its attributes.
  Rationale: every other Whitaker lint declares `Warn`. Placing the comment
  above the attributes matches RFC 0003 §6's example and does not contradict
  `function_attrs_follow_docs`, which rejects doc comments placed after
  attributes. Date/Author: 2026-11-11, planning agent, revised after panel
  review.

- Decision D-11: driver debug events use the workspace `log` crate rather than
  `tracing`. Rationale: `AGENTS.md` prefers `tracing`, but every existing lint
  driver (for example `crates/string_continuation_style/src/driver/facts.rs:10`
  and `crates/no_unwrap_or_else_panic/src/driver.rs:5`) and the shared i18n
  helpers use `log::debug!`. Consistency inside the Dylint driver is worth more
  than a one-crate divergence. No metrics are emitted, because a lint library
  must not install recorders and the item has no usage requirement.
  Date/Author: 2026-11-11, planning agent.

- Decision D-12: keep the balanced-hook container stack rather than the
  strongest alternative the panel examined (parent hooks record container and
  trait-impl member spans; child hooks test span containment). Rationale: the
  alternative removes post hooks and the stack, but adds a trusted assumption
  that spans nest within a file and a containment lookup per function. The
  stack's balance follows directly from rustc's visitor (`A-VISITOR`). A private
  `Visitor` run from `check_crate` was rejected because module-file runs never
  call `check_crate`, and emitting from a parent hook was rejected because lint
  levels are scoped to the hook of the item being linted, which would ignore a
  method-level `#[allow]`. Date/Author: 2026-11-11, planning agent, after panel
  review.

### Design review outcome (2026-11-11)

The panel's verdict was "proceed with conditions". The conditions, all now
reflected above, were: re-examine D-2 against RFC §§4, 8, 9, and 10 (now
explicit and flagged for approval); drop or reshape Verus obligations that
restate their specifications (D-8); measure `#[expect]` behaviour and stop
relying on `check_crate` for state (Risks, D-9); restructure fixtures around
`Test::src_base` because `Test::example` copies one file; remove the
non-workspace `tokio` dependency; fix the help text; separate body presence
from location in `FunctionContext`; replace the generic span-choice API with a
driver-private helper; return the popped frame from `ContainerStack::exit`
instead of a `thiserror` error; and raise the file tolerance. The pre-mortem
scenarios were an "expect storm" that turns CI red across the estate, months of
silently inert runs because test targets were not built, and a toolchain bump
that removes the pre-expansion interface; each has a mitigation in `Risks`.

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
New crates under `crates/` join the workspace automatically (`Cargo.toml`
`members = [..., "crates/*", ...]`) and are built, linted, and tested by
`make lint` and `make test`, which use
`--workspace --all-targets --all-features`.

The specification is `docs/rfcs/rfc-0003-missing-docs-in-test-functions.md`.
Sections 6 and 7 define the pass boundary, the four-clause candidate predicate,
the documentation forms, and the diagnostic placement. The roadmap entry is
`docs/roadmap.md` item 2.2.11.

Useful precedents and where to look:

- `crates/string_continuation_style/` is the only early lint. Its `Cargo.toml`
  shows the feature layout. `src/driver/mod.rs:34-57` shows the
  `mod declaration { #![expect(missing_docs, ...)] ... }` wrapper around the
  lint macro. `src/diagnostics.rs` shows localized emission through
  `cx.emit_span_lint(..., DiagDecorator(...))`. `src/lib.rs:19-22` shows the
  `#[cfg(all(test, feature = "dylint-driver"))]` test-module gate.
- `crates/no_expect_outside_tests/src/lib_ui_tests.rs` shows UI tests compiled
  with `--test`: `Test::src_base` over a staged fixture directory with
  `test.rustc_flags(...)` (lines 104-122), and `Test::example` for fixtures
  that need dev-dependencies.
- `crates/function_attrs_follow_docs/src/tests/` shows `rstest-bdd`
  scenarios placed in the unit-test target, which avoids linking an
  integration-test crate against `rustc_private`.
- `crates/bumpy_road_function/src/analysis.rs:71-76` shows the hidden
  doctest lines needed in a crate that enables `rustc_private`.
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
  outermost first, used to decide whether a function is module-level, an
  inherent or trait-default method, a trait-implementation method, or nested
  elsewhere.
- Reliable identifier span: the function identifier's span when it is not the
  dummy span.

## Conformance basis

Upstream artefacts and revisions:

- RFC 0003 `docs/rfcs/rfc-0003-missing-docs-in-test-functions.md` at commit
  `84eed837` (status: Proposed), §§6-7, with §§4, 8, 9, 10, and 11 for
  boundary, test, and open-question context.
- `docs/roadmap.md` at commit `d9eb8a09`, item 2.2.11 (this plan) and items
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
  `policy::select` -> `L-SOUND` -> `EP-M2` directory `ui-no-harness/` ->
  `EV-M2-ui`.
- `RFC3-FR-SOURCE` (only source-authored functions; generated items never
  diagnosed) -> `EP-M1` provenance gate -> `EP-M2` fixtures
  `macro_generated_functions.rs` and example `proc_macro_functions.rs` ->
  `EV-M2-ui`.
- `RFC3-FR-DOCS` (`///` and `#[doc = "..."]` accepted) -> `EP-M1`
  `policy::documentation` -> `T-DOCS` -> `EP-M2` fixture `documented_forms.rs`
  -> `EV-M1-unit`, `EV-M2-ui`.
- `RFC3-FR-SPAN` (identifier span when reliable) -> `EP-M2` driver helper ->
  `T-SPAN` -> fixture `undocumented_functions.rs` caret placement -> `EV-M2-ui`.
- `RFC3-FR-PROCMACRO` (procedural-attribute functions diagnosed once at the
  authored location) -> example `proc_macro_functions.rs` -> `EV-M2-ui`.
- `RFC3-TR-PREEXP` (pre-expansion pass) -> `EP-M0` skeleton -> `EV-M0`.
- `RFC3-TR-L10N` (localization conventions) -> `EP-M2` `diagnostics.rs`
  fallback resolution -> `EV-M2-ui` (English text); completed by 2.2.14.
- `RFC3-DES-SCOPE` (associated-function shapes stated explicitly; refinement
  of RFC clause 2) -> D-5 -> `EP-M1` `policy::context` -> `L-STACK` -> fixtures
  `associated_functions.rs` and `nested_functions.rs` -> `EV-M2-ui`.
- `RFC3-FR-PATH` (configured path selection) -> partially advanced: real-file
  recovery (D-2); include/exclude deferred to 2.2.12.
- `RFC3-FR-EXCLUDED` (MUST not report excluded paths) -> deferred to 2.2.12.
- `RFC3-TP-LIBSOURCE` (RFC §9: library source compiled under a harness
  passes) -> known interim violation under D-2, recorded by fixture
  `fail_library_source_interim.rs` and inverted by 2.2.12.
- `RFC3-TR-SUPPRESS` (two-attribute workaround) -> fixture
  `item_level_allow.rs` -> `EV-M2-ui`; full matrix in 2.2.15.

## Verification plan

### Abstract model

The policy is specified independently of rustc as a function over facts.

`Facts` is a record of six fields:

- `harness`: `TestHarness` or `NotTestHarness`;
- `provenance`: `Source` or `Expansion`;
- `context`: `Module`, `InherentImpl`, `Trait`, `TraitImpl`, or `Nested`;
- `body`: `Present` or `Absent`;
- `path`: `Selected` or `NotSelected`;
- `documentation`: `Documented` or `Undocumented`.

There are 2 x 2 x 5 x 2 x 2 x 2 = 160 fact records. The specification
`should_diagnose(f)` is true exactly when `f.harness = TestHarness`,
`f.provenance = Source`, `f.context` is `Module`, `InherentImpl`, or `Trait`,
`f.body = Present`, `f.path = Selected`, and `f.documentation = Undocumented`.

Gate order is harness, provenance, context, body, path, documentation. For a
record that fails at least one gate, `first_failing_gate(f)` is the earliest
failing gate in that order.

The suppression order `f ⊑ g` ("g is at least as suppressed as f") holds when g
is obtained from f by changing any subset of fields towards suppression:
harness to `NotTestHarness`, provenance to `Expansion`, context to `TraitImpl`
or `Nested`, body to `Absent`, path to `NotSelected`, or documentation to
`Documented`.

The container model is an item tree. Each node has a kind (`Module`,
`InherentImpl`, `TraitImpl`, `Trait`, or `Other`) and children. The reference
oracle classifies a function node by recursion over its ancestors: a free
function is `Module` context when every ancestor is a module, otherwise
`Nested`; an associated function takes the kind of its parent (`InherentImpl`,
`Trait`, or `TraitImpl`) when every earlier ancestor is a module, otherwise
`Nested`. The implementation under test is `ContainerStack`, driven by the
event sequence that rustc's visitor produces for the same tree (`Enter(kind)`
before a node's children, `Exit` after them).

### Invariants and lemmas

- `L-SOUND`: for every `f`, `select(f) = Diagnose` implies
  `should_diagnose(f)`.
- `L-COMPLETE`: for every `f`, `should_diagnose(f)` implies
  `select(f) = Diagnose`.
- `L-PRECEDENCE`: for every `f` with `¬should_diagnose(f)`,
  `select(f) = Skip(r)` with `r = first_failing_gate(f)`.
- `L-MONOTONE` (anti-widening): for every `f ⊑ g`, `select(g) = Diagnose`
  implies `select(f) = Diagnose`. No change towards suppression can create a
  diagnostic. 2.2.12 restates it for pattern lists and proves it in Verus (D-8).
- `L-STACK`: for every item tree and every function node in it, the
  classification `ContainerStack::classify` returns when the function's hook
  runs equals the oracle's classification; after the whole event sequence the
  stack is empty; `exit` on an empty stack returns `None` without panicking.
- `T-DOCS`: for every attribute shape in D-6 (outer `///`, outer `/** */`,
  inner `//!`, inner `/*! */`, `#[doc = "lit"]`, `#[doc = include_str!(..)]`,
  `#[cfg_attr(p, doc = "x")]`, `#[doc(hidden)]`, `#[doc(alias = "x")]`, and a
  non-doc attribute such as `#[given("...")]`), the classification matches D-6,
  and a function is `Documented` exactly when at least one attribute classifies
  as documentation.
- `T-SPAN`: the diagnostic's primary span is the identifier span unless that
  span is the dummy span, in which case it is the item span.
- `T-HOOKS` (integration): on the pinned toolchain, the pass observes free
  functions in the crate root and in out-of-line module files, inherent and
  trait associated items, and honours item-level
  `#[allow(missing_docs_in_test_functions)]`.

### Axioms (trusted, not verified here)

- `A-VISITOR`: rustc's early-lint visitor calls `check_item` before and
  `check_item_post` after walking every item, and likewise for impl and trait
  associated items (`rustc_lint/src/early.rs:79-85, 208-228`), with no early
  exit between the pair.
- `A-OPTS-TEST`: `Session::opts.test` is true exactly for `--test`
  compilations (`rustc_session/src/options.rs:328`).
- `A-PROVENANCE`: `Span::from_expansion()` is true exactly when the span's
  syntax context is not the root context
  (`rustc_span/src/span_encoding.rs:308`).
- `A-PREEXP`: before expansion, macro invocations are `MacCall` nodes, not
  items, so generated items are not visited
  (`rustc_interface/src/passes.rs:87-109`).
- `A-DYLINT`: `impl_pre_expansion_lint!` in `dylint_linting` 6.3.0 registers
  the given pass value with `register_pre_expansion_pass` on compilers before
  2026-06-22 (`dylint_linting-6.3.0/src/lib.rs:328-335, 431-439`); `EP-M0`
  checks this empirically.

### Methods, artefacts, and evidence

`L-SOUND`, `L-COMPLETE`, `L-PRECEDENCE`, `L-MONOTONE`:

- Method: exhaustive enumeration of the production `select` over all 160
  records, compared with an independently written `should_diagnose` and
  `first_failing_gate` in the test module; for `L-MONOTONE`, every pair
  `(f, g)` with `f ⊑ g` (at most 160 x 160 checks) is enumerated.
- Rationale: the domain is finite, so enumeration over the real code is a
  complete proof for every admissible input, with no model to drift (D-8).
- Artefacts:
  `crates/missing_docs_in_test_functions/src/policy/selection_tests.rs`; an
  insta snapshot of the full truth table (enum names only, one row per record)
  under `src/policy/snapshots/`.
- Evidence: `make test` passes; the snapshot is reviewed once and then stable.
- Non-vacuity: the enumerator must yield 160 distinct records (asserted); at
  least one record diagnoses and at least one record fails each gate first
  (asserted). Seeded faults that must be rejected, each tried once and recorded
  in `Artefacts and notes`: deleting the documentation gate, swapping the path
  and context checks, treating `TraitImpl` as in scope, and, to show the oracle
  can fail, dropping the context clause from `should_diagnose`. The insta
  snapshot also catches a change made to both the code and the oracle together.

`L-STACK`:

- Method: a `proptest` property running the production `ContainerStack` over
  generated item trees linearized in `A-VISITOR` order and compared with the
  recursive oracle; `rstest` cases for empty-stack `exit` and for each
  `FunctionContext` variant.
- Rationale: nesting depth is unbounded, so enumeration is impossible; the
  oracle recurses over a tree, which is a formulation independent of the stack.
  A Verus alternative is offered in D-8.
- Domain: trees up to depth 6 and breadth 4, 256 cases, with
  `proptest-regressions/` committed.
- Artefacts: `src/policy/context_tests.rs`, `src/policy/context_props.rs`.
- Non-vacuity: a deterministic `#[test]` draws a fixed number of trees from
  the same strategy with a fixed-seed `TestRunner` and asserts that every
  `FunctionContext` variant is reached, that `Other` containers appear, and
  that depth 3 or more is reached. Seeded faults that must be rejected:
  classifying by the innermost frame alone, and treating `TraitImpl` as
  `InherentImpl`. A conditional push is impossible by construction, because the
  driver maps every item to a frame through the total function
  `container_kind`; the UI fixture `nested_functions.rs` is the layer that
  catches a driver that skips a push, and the seeded fault "push only for
  module, impl, and trait items" must make it fail.

`T-DOCS`:

- Method: `rstest` table in the pure module over `DocAttributeShape`, plus the
  UI fixture `documented_forms.rs`, which passes real rustc attributes through
  the driver's attribute mapper.
- Rationale: the risk lies in rustc's attribute representation before
  expansion, which only a real compilation exercises.
- Non-vacuity: the `#[doc = include_str!(..)]` row fails under a naive
  `doc_str()` mapper (seeded fault); `doc_hidden_only.rs` fails if list-form
  `doc` attributes are counted.

`T-SPAN` and `T-HOOKS`:

- Method: UI tests compiled with `--test`, plus the `EP-M0` spike transcript.
- Evidence: `make test` passes with the blessed `.stderr` files.
- Non-vacuity: the out-of-line module fixture's expected output contains a
  diagnostic located in the module file; the `ui-no-harness/` run shows that
  the same sources otherwise diagnose; the caret in
  `undocumented_functions.stderr` is under the identifier, and the seeded fault
  "always use the item span" must change it. The span fallback itself is
  unreachable from source before expansion (see `Surprises & discoveries`) and
  is covered only by review of a two-line helper.

### Where formal verification is and is not used

No Verus or Kani artefact is added in 2.2.11 (D-8, needs approval). Diagnostic
text, fallback messages, Fluent resolution, the AST-to-facts adapter, and lint
registration are integration plumbing covered by UI and unit tests. The first
substantive Verus obligation in this feature is the 2.2.12 anti-widening lemma
over unbounded pattern lists.

### Relationship between proofs and tests

Exhaustive enumeration proves the selection contract for the production code.
The property test ties the production stack to an independent tree oracle. The
UI tests prove that rustc supplies the facts the policy assumes and that the
driver maps them faithfully. Each layer fails for a different class of defect,
and none of them replaces the others.

## Plan of work

### Stage A: understand and propose (this document)

No code changes. The draft has been reviewed by a community-of-experts panel
and awaits the user's approval.

### Stage B: spike and red tests

`EP-M0` builds the crate skeleton and measures the pre-expansion mechanism.
`EP-M1` and `EP-M2` begin with failing tests.

### Stage C: implementation

The policy and the driver are developed milestone by milestone, each closing
its obligations before the next starts.

### Stage D: documentation and wider validation

`EP-M3` updates the design document, records ADR 0014, documents conventions,
and marks the roadmap item done.

## Milestones and plateaus

### `EP-M0`: pre-expansion spike and crate skeleton (prototyping)

Outcome: a new crate `crates/missing_docs_in_test_functions/` that builds as a
standalone Dylint library and registers a pre-expansion pass with
`impl_pre_expansion_lint!`. During the spike only, the pass emits a placeholder
diagnostic on every function it sees.

Work:

1. Create `crates/missing_docs_in_test_functions/Cargo.toml` modelled on
   `crates/string_continuation_style/Cargo.toml`:
   `crate-type = ["cdylib", "rlib"]`, `publish = false`; features
   `dylint-driver` (enabling optional `dylint_linting`, `log`, `rustc_ast`,
   `rustc_lint`, `rustc_session`, `rustc_span`, `whitaker-common`,
   `whitaker_lint_core`) and `constituent`; dev-dependencies `dylint_testing`,
   `googletest`, `insta`, `pretty_assertions`, `proptest`, `rstest`,
   `rstest-bdd`, `rstest-bdd-macros`, `serial_test`, `camino`,
   `whitaker-common`, `whitaker_lint_core`, and `whitaker_test_macros`, all
   inherited from the workspace; `[lints] workspace = true`.
2. Create `src/lib.rs` with
   `#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]`,
   `pub mod policy;`, a feature-gated `mod driver;` that re-exports
   `MISSING_DOCS_IN_TEST_FUNCTIONS` and `MissingDocsInTestFunctions`, and a
   `#[cfg(all(test, feature = "dylint-driver"))] mod tests;`.
3. Create `src/driver/mod.rs` declaring the lint inside
   `mod declaration { #![expect(missing_docs, reason = "...")] ... }` with
   `dylint_linting::impl_pre_expansion_lint!`.
4. Regenerate `Cargo.lock` with `cargo metadata --format-version 1 > /dev/null`
   (no `--locked`) and commit it, because the Makefile passes `--locked` to
   every gate.
5. Add spike fixtures under a temporary `ui-spike/` directory, run by
   `Test::src_base` with `rustc_flags(["--test"])`, and one temporary
   `examples/spike_proc_macro.rs`, covering: free functions; an out-of-line
   module through `#[path = "auxiliary/helpers.rs.inc"] mod helpers;`;
   `#[cfg(not(test))]` items and a `#[cfg(any())] mod gone;`;
   `#[doc = include_str!("auxiliary/doc.md")]`; `#[given(...)]` and `#[rstest]`
   functions; the two-attribute item-level allow; a crate-level
   `#![allow(missing_docs_in_test_functions)]` with an out-of-line module; and
   `#[expect(missing_docs_in_test_functions)]` on a plain function, on a
   `#[given]` function, and in `#[cfg_attr(test, expect(...))]` form. Check
   whether compiletest treats files under `auxiliary/` or with a `.rs.inc`
   extension as test inputs, and adjust the naming if it does.
6. Record the output as `EV-M0` in `Artefacts and notes`.

Acceptance evidence `EV-M0`: the transcript shows placeholder diagnostics in
both the root file and the out-of-line module file; none on the item-allowed
function; whether the crate-level allow reaches the module file; that
`#[given]` and `#[rstest]` functions are seen once each with the attribute
intact; the attribute kind of `#[doc = include_str!(..)]`; and the observed
`#[expect]` behaviour for each form.

Conformance check:

- `RFC3-TR-PREEXP` is satisfied by the registered pre-expansion pass.
- If the "Toolchain" or "Semantics" tolerance fires, stop.
- The "Expectations" tolerance outcome is recorded.
- No public interface outside the new crate; no new dependency.
- Trace links unchanged.

Recovery: the spike is additive. Delete `ui-spike/`, the spike example, and the
placeholder emission before the milestone commit, keeping the skeleton. The
milestone commit builds, and `make check-fmt`, `make lint`, and `make test`
pass.

Remaining gaps: no policy, no diagnostics.

Compatibility decision: none.

### `EP-M1`: pure policy kernel

Outcome: `crates/missing_docs_in_test_functions/src/policy/` contains
rustc-free types and functions, fully tested and not yet used by the driver.
The module is public so doctests can demonstrate it; the crate is pre-1.0 and
`publish = false`, so the module carries no compatibility promise.

Files (each under 400 lines):

- `policy/mod.rs`: module docs and re-exports.
- `policy/facts.rs`: the fact enums, `CandidateFacts`, `Verdict`, and
  `SkipReason`.
- `policy/selection.rs`: `select`.
- `policy/context.rs`: `ContainerKind`, `FunctionShape`, and
  `ContainerStack`.
- `policy/documentation.rs`: `DocAttributeShape` and `documentation_status`.
- `policy/selection_tests.rs`, `policy/context_tests.rs`,
  `policy/context_props.rs`, and `policy/documentation_tests.rs`, wired with
  `#[cfg(test)] #[path = ...]` as the precedents do.
- `src/tests/behaviour.rs` and `tests/features/candidate_selection.feature`.

Red-green-refactor:

1. Red: write the tests and the feature file first against stubs that return
   `Verdict::Skip(SkipReason::NotTestHarness)`, `Documentation::Undocumented`,
   and `FunctionContext::Nested`. Run the focused command below and expect
   failures for the asserted reasons, not compile errors. Expect these to pass
   while red, because the stubs happen to agree with them: the "not a test
   harness" and "nested function" scenarios and their table rows, and every
   `documentation_tests` row whose expected status is `Undocumented`. Record
   the actual red list in `Artefacts and notes`.
2. Green: implement each function minimally; rerun and expect every test to
   pass.
3. Refactor: tidy names and documentation, add doctest examples with the
   hidden `rustc_private` lines, rerun, then run the full gates.

Focused command (the Makefile's flags matter; a bare `cargo nextest run` without
`--all-features` selects nothing in feature-gated modules):

```sh
BRANCH=$(git branch --show-current)
RUSTFLAGS="-D warnings" cargo nextest run --locked --all-features \
  -p missing_docs_in_test_functions 2>&1 \
  | tee "/tmp/test-focused-whitaker-${BRANCH}.out"
```

BDD feature (`tests/features/candidate_selection.feature`). It holds three
readable contract examples; the exhaustive table, not the feature, is the
coverage.

```gherkin
Feature: Candidate selection for missing_docs_in_test_functions

  Scenario: An undocumented module function in a test harness is diagnosed
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is diagnose

  Scenario: A doc attribute with an include_str value suppresses the diagnostic
    Given a module function in a selected source file
    And the compilation is a test harness
    And the function has a doc attribute with an include_str value
    When the candidate predicate is evaluated
    Then the verdict is skip because the function is documented

  Scenario: Methods in trait implementations are out of scope
    Given an associated function inside a trait implementation
    And the compilation is a test harness
    And the function has no documentation
    When the candidate predicate is evaluated
    Then the verdict is skip because the function context is out of scope
```

Steps follow the repository conventions: a `world` fixture of `RefCell` fields
annotated with `#[allow_fixture_expansion_lints]`, `given` and `when` steps
returning `Result<(), String>` without assertions, and assertions only in
`then` steps using `googletest` matchers
(`assert_that!(verdict, eq(Verdict::Diagnose))`).

Unit tests use `rstest` tables, `googletest` matchers, and
`pretty_assertions::assert_eq` for structured comparisons. The truth table over
all 160 records is rendered as fixed-width text and snapshotted with
`insta::assert_snapshot!`.

Acceptance evidence `EV-M1-unit`: the red and green transcripts, the seeded
fault rejections, and the full gates passing.

Conformance check: D-3 to D-6 implemented as written;
`rg -n 'rustc_' crates/missing_docs_in_test_functions/src/policy` returns
nothing; no new dependency; trace links current.

Recovery: the module is additive and unused by the driver; revert the milestone
commit to retry.

Remaining gaps: driver and UI tests.

Compatibility decision: none.

### `EP-M2`: pre-expansion driver, diagnostics, and UI fixtures

Outcome: the pass converts AST items into `CandidateFacts`, consults the
policy, and emits a diagnostic at the chosen span. UI tests compiled with and
without `--test` cover the behaviour matrix.

Driver files (under `src/driver/`, each under 400 lines):

- `mod.rs`: lint declaration and the `EarlyLintPass` implementation with
  `check_item`, `check_item_post`, `check_impl_item`, `check_impl_item_post`,
  `check_trait_item`, and `check_trait_item_post`. Every pre-hook first
  classifies the item when it is a function, then pushes exactly one frame
  computed by `container_kind`; every post-hook pops exactly one frame and logs
  at `debug` if the popped kind differs from the recomputed kind.
- `container.rs`: the total function
  `container_kind(&ItemKind) -> ContainerKind` and its associated-item
  counterpart.
- `facts.rs`: builds `CandidateFacts` from `&EarlyContext` and the AST item:
  harness from `cx.sess().opts.test`, provenance from
  `item.span.from_expansion()`, body presence from `Fn::body`, source recovery
  from `cx.sess().source_map().span_to_filename(item.span)` and the interim
  `PathPolicy` (D-2), and documentation through `policy::documentation_status`.
- `attributes.rs`: maps `rustc_ast::Attribute` to `DocAttributeShape` by
  inspecting `AttrKind::DocComment`, a `Normal` attribute whose path is `doc`
  with `AttrArgs::Eq`, and a `cfg_attr` whose list contains such an entry (D-6).
- `diagnostics.rs`: a process-wide `OnceLock<Localizer>` loaded from
  `whitaker_lint_core::SharedConfig::load()` and `get_localizer_for_lint`,
  message resolution with an English fallback (D-9), the private
  `diagnostic_span` helper (identifier span unless dummy), and emission with
  `cx.emit_span_lint` and `DiagDecorator`. Primary message:
  ``test function `{name}` has no documentation comment``. Help:
  ``add a
  `///` comment above the function and its attributes that explains what the
  test exercises``.
- Observability: module-file runs never call `check_crate_post`, so there is
  no end-of-run hook for a summary. Instead, each classified function emits a
  `log::debug!` event carrying the function name, the source file, and the
  verdict (including the skip reason), which makes an inert run observable with
  `RUST_LOG=missing_docs_in_test_functions=debug`.

UI tests live in `src/tests/ui.rs`, so the nextest serial-lane filter
`test(tests::ui::)` in `.config/nextest.toml` applies. They run three batches:

1. `ui/`, through `Test::src_base` with
   `rustc_flags(["--test", "-D", "missing_docs_in_test_functions"])`;
2. `ui-no-harness/`, through `Test::src_base` without `--test`, holding the
   same undocumented sources so that the absence of diagnostics proves the
   harness gate;
3. `examples/proc_macro_functions.rs`, through `Test::example` with
   `--test`, because it needs the `rstest` and `rstest-bdd-macros`
   dev-dependencies.

Fixtures in `ui/` (each with a blessed `.stderr`; support files under
`ui/auxiliary/` named as `EP-M0` established):

- `undocumented_functions.rs`: plain, `async`, `const`, and `unsafe`
  undocumented functions, each diagnosed with the caret on the identifier; a
  documented `fn main` is not.
- `documented_forms.rs`: `///`, `/** */`, inner `//!`, `#[doc = "..."]`,
  `#[doc = include_str!(...)]`, and `#[cfg_attr(test, doc = "...")]`, none
  diagnosed.
- `doc_hidden_only.rs`: `#[doc(hidden)]` alone is diagnosed.
- `associated_functions.rs`: an undocumented inherent method and an
  undocumented trait default method are diagnosed; an undocumented
  trait-implementation method and an undocumented bodiless trait method are not.
- `nested_functions.rs`: functions inside a function body, a `const`
  initializer, an enum discriminant, and an inherent `impl` inside a function
  body are not diagnosed.
- `macro_generated_functions.rs`: a `macro_rules!` macro that expands to an
  undocumented function, and its invocation, produce nothing.
- `out_of_line_module.rs` with `auxiliary/out_of_line_helpers.rs.inc`: the
  diagnostic is reported in the module file, and a crate-root
  `#![allow(missing_docs_in_test_functions)]` does not suppress it if `EV-M0`
  confirms the scoping observation.
- `item_level_allow.rs`: the two-attribute workaround on one function and an
  undocumented control function that is still diagnosed.
- `cfg_gated_functions.rs`: limitation record for D-7.
- `fail_library_source_interim.rs`: an undocumented private helper outside
  any test module is diagnosed under the interim D-2 policy; its header comment
  states that 2.2.12 inverts this expectation.

Example `examples/proc_macro_functions.rs`: an undocumented `#[given("...")]`
step and an undocumented `#[rstest]` test, each diagnosed exactly once at the
identifier, and an undocumented function inside `proptest! { ... }`-style macro
input (using a local `macro_rules!` stand-in if `proptest` macros add too much
compile time) that is not diagnosed, documenting the D-5 limitation. Fixture
files carry narrowly scoped `#[expect(dead_code, reason = "...")]` where
needed, as the Makefile comment on `examples/` describes.

Red-green-refactor:

1. Red: add the fixtures and `.stderr` files written by hand from the expected
   behaviour, run the focused command with the driver emitting nothing, and
   expect a failure on every fixture that expects a diagnostic.
2. Green: wire the driver; rerun and expect all to pass. Re-bless only after
   reading each diff and confirming it matches the handwritten expectation.
3. Apply the seeded faults listed in `Verification plan` for `T-SPAN`,
   `T-DOCS`, and the conditional push; confirm each is rejected; revert.
4. Refactor; run the full gates.

Acceptance evidence `EV-M2-ui`: the red and green transcripts, the seeded fault
rejections, and the blessed `.stderr` for `undocumented_functions.rs` and
`out_of_line_module.rs` quoted in `Artefacts and notes`.

Conformance check: every traced requirement except `RFC3-FR-PATH`,
`RFC3-FR-EXCLUDED`, and `RFC3-TP-LIBSOURCE` (deferred or interim by design) is
observable; the driver contains no policy decisions beyond mapping facts; no
suite or installer registration; trace links current.

Recovery: the driver is confined to the new crate; revert the milestone commit
to retry.

Remaining gaps: documentation.

Compatibility decision: none.

### `EP-M3`: documentation, ADR, and roadmap

Outcome: documentation describes the lint and its conventions; the roadmap
marks 2.2.11 done.

Work:

1. Write
   `docs/adrs/adr-0014-pre-expansion-pass-for-missing-docs-in-test-functions.md`
   recording the pre-expansion pass shape and its soft-deprecation risk with
   an exit plan, the container-stack design and the rejected alternatives
   (D-12), the in-scope function forms (D-5), the documentation rule (D-6), the
   interim path gate (D-2), and the verification split (D-8), following the
   documentation style guide's ADR format. Re-scan fetched remote branches for
   ADR 0014 immediately before committing, and renumber if it has been taken.
2. Add a `missing_docs_in_test_functions` section to
   `docs/whitaker-dylint-suite-design.md` that summarizes the design, records
   that 2.2.13 needs a `PassKind::PreExpansion` descriptor and a feature-gated
   `register_pre_expansion_pass` call, and links ADR 0014.
3. Add "Writing a pre-expansion pass" to `docs/developers-guide.md` beside
   "Writing an early pass": per-module pass instantiation, no `check_crate` in
   module runs, dummy `NodeId`s, unevaluated `cfg`, lint-level scope in
   out-of-line modules, `#[expect]` behaviour as measured in `EV-M0`, the
   container stack and its balanced-hook rule, `--test` UI harnesses with
   `Test::src_base`, the toolchain-bump checklist entry, and the re-use policy
   for the `policy` module (owned by this crate; not to be imported by other
   lints; promotion to a shared crate needs a new decision).
4. Add a `missing_docs_in_test_functions` entry to `docs/users-guide.md`
   §"Available Lints" that states it is not yet part of the suite and does not
   describe the interim path behaviour as a feature. It lists the in-scope
   forms, the documentation rule, the need for `--all-targets` or `--tests`,
   the pre-expansion limitations (macro bodies, `cfg`, lint-level scope in
   module files), and the rule "suppress with the two-attribute `allow`
   workaround, not `expect` or the ADR 0002 macro".
5. Add the crate to `docs/repository-layout.md` §`crates/`, the ADR and the
   new developers' guide section to `docs/contents.md`, and annotate RFC 0003
   §11 open questions 2 and 3 with the answers from D-5 and D-2 and a link to
   ADR 0014.
6. Mark roadmap item 2.2.11 `[x]` in `docs/roadmap.md`.
7. Run `make fmt`, `make markdownlint`, `make nixie`, `make spelling`,
   `make check-fmt`, `make lint`, and `make test`.

Acceptance evidence `EV-M3`: all gates pass; `git diff --stat` shows the
documentation files above; the roadmap shows `[x] 2.2.11`.

Conformance check: every decision is recorded in ADR 0014 or the design
document; no upstream artefact is left contradicting the implementation.

Recovery: documentation-only; revert and retry.

Remaining gaps: 2.2.12 to 2.2.16.

Compatibility decision: none.

## Hand-off to later roadmap items

- 2.2.12 replaces `PathPolicy::InterimUnrestricted` with configured include
  and exclude patterns over workspace-relative paths, makes an absent or empty
  `include` inert (RFC §8), inverts `fail_library_source_interim.rs`, and
  proves the anti-widening lemma over pattern lists in Verus (D-8).
- 2.2.13 adds a `PassKind::PreExpansion` descriptor and registers the pass
  behind a suite feature, so a toolchain that removes pre-expansion passes can
  drop it without breaking the suite build; it should add a newer-nightly
  canary build of this crate.
- 2.2.14 adds `en-GB`, `cy`, and `gd` `.ftl` resources for the message keys
  this item resolves with fallbacks, and the localized UI directories.
- 2.2.15 moves the `EP-M2` fixtures onto the SARIF diagnostic-input contract
  rather than adding parallel fixtures, and extends `item_level_allow.rs` into
  the full suppression matrix, including the measured `#[expect]` behaviour.
- 2.2.16 adds property tests for path configuration. The candidate-selection
  predicate is already proved by exhaustion; 2.2.16 may reuse the 160-record
  enumerator rather than sampling it.

## Concrete steps

Run every command from the repository root:
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/82a0f684-e8e8-4e9a-9a3f-77ed71436818`.

Gate commands, run sequentially for each milestone (never in parallel for the
same worktree), preferably delegated to the `scrutineer` agent:

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

Expected success tail for `make test`: nextest's
`Summary […] N tests run: N passed`, followed by the doctest summary with no
failures.

Commit after each milestone, once all its gates pass, with an imperative
subject line and a body explaining what and why.

## Validation and acceptance

The item is accepted when:

- compiling `ui/undocumented_functions.rs` with `--test` through the UI
  harness produces one `missing_docs_in_test_functions` diagnostic per
  undocumented function, each with its primary span on the identifier, and
  compiling the same source in `ui-no-harness/` produces none;
- every fixture listed under `EP-M2` matches its blessed `.stderr`;
- the exhaustive selection tests, the stack property test, and their
  non-vacuity checks pass, and each recorded seeded fault was rejected;
- `make check-fmt`, `make lint`, and `make test` pass;
- the roadmap marks 2.2.11 done and the documentation listed in `EP-M3`
  exists.

Quality criteria:

- Tests: unit, behavioural, snapshot, property, doctest, and UI suites in the
  new crate pass under `make test`.
- Verification: `L-SOUND`, `L-COMPLETE`, `L-PRECEDENCE`, and `L-MONOTONE`
  discharged by exhaustion; `L-STACK` by property test against the oracle;
  `T-DOCS`, `T-SPAN`, and `T-HOOKS` by UI tests. If the user selects the D-8
  alternative, `make verus-missing-docs` and `make verus` must also report zero
  errors.
- Lint and type checks: `make lint` passes with no new suppressions beyond the
  documented `mod declaration` `expect(missing_docs)` precedent and scoped
  fixture `expect(dead_code)` attributes.
- Security: no new trust boundary; the lint reads only the session, the source
  map, and the existing Dylint configuration.

## Idempotence and recovery

All steps are additive and can be rerun. UI blessing can be repeated after
reading diffs. If a gate fails, read the cited log under `/tmp` rather than
rerunning it, fix, and rerun only that gate before rerunning the full sequence.
To roll back, revert the milestone commit; no data or external state is changed
by this work.

## Artefacts and notes

Record here, as the work proceeds:

- `EV-M0`: the spike transcript, including the `#[expect]` observations.
- `EV-M1-unit`: the red list, the green transcript, the snapshot path, and the
  seeded-fault rejections.
- `EV-M2-ui`: the red and green UI transcripts, the seeded-fault rejections,
  and two blessed `.stderr` excerpts.
- `EV-M3`: the final gate summaries.

## Interfaces and dependencies

Crate: `crates/missing_docs_in_test_functions`, package name
`missing_docs_in_test_functions`, `publish = false`. Dependencies are the
existing workspace dependencies listed in `EP-M0`; no `thiserror`.

Public items in `missing_docs_in_test_functions::policy` at the end of `EP-M1`
(all enums derive `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, and `Hash`):

```rust
pub enum HarnessStatus { TestHarness, NotTestHarness }
pub enum Provenance { Source, Expansion }
pub enum FunctionContext { Module, InherentImpl, Trait, TraitImpl, Nested }
pub enum BodyPresence { Present, Absent }
pub enum PathSelection { Selected, NotSelected }
pub enum Documentation { Documented, Undocumented }

pub struct CandidateFacts {
    pub harness: HarnessStatus,
    pub provenance: Provenance,
    pub context: FunctionContext,
    pub body: BodyPresence,
    pub path: PathSelection,
    pub documentation: Documentation,
}

pub enum SkipReason {
    NotTestHarness,
    NotSourceAuthored,
    OutOfScopeContext,
    Bodyless,
    PathNotSelected,
    Documented,
}
pub enum Verdict { Diagnose, Skip(SkipReason) }

#[must_use]
pub fn select(facts: CandidateFacts) -> Verdict;

pub enum ContainerKind { Module, InherentImpl, TraitImpl, Trait, Other }
pub enum FunctionShape { Free, Associated }

#[derive(Default)]
pub struct ContainerStack { /* frames: Vec<ContainerKind> */ }
impl ContainerStack {
    pub fn enter(&mut self, kind: ContainerKind);
    #[must_use]
    pub fn exit(&mut self) -> Option<ContainerKind>;
    #[must_use]
    pub fn classify(&self, shape: FunctionShape) -> FunctionContext;
}

pub enum DocAttributeShape {
    DocComment,
    DocNameValue,
    CfgAttrDoc,
    DocList,
    Other,
}
#[must_use]
pub fn documentation_status(
    shapes: impl IntoIterator<Item = DocAttributeShape>,
) -> Documentation;
```

Crate-private driver items (feature `dylint-driver`):
`SourceFile { Real(Utf8PathBuf), Virtual }`, `PathPolicy::InterimUnrestricted`
with `fn select(&self, source: &SourceFile) -> PathSelection`,
`container_kind`, and `diagnostic_span`. Public driver items:
`MISSING_DOCS_IN_TEST_FUNCTIONS` and `MissingDocsInTestFunctions`, which
implements `rustc_lint::EarlyLintPass`.

Skills to load when implementing: `rust-router`, then `rust-unit-testing`,
`proptest`, and `rust-types-and-apis` as each milestone needs them (`verus`
only if the D-8 alternative is chosen); `execplans` to maintain this document;
`codegraph-mcp` for navigation; `en-gb-oxendict-style` for documentation.

Documentation to read first: `AGENTS.md`;
`docs/rfcs/rfc-0003-missing-docs-in-test-functions.md`;
`docs/whitaker-dylint-suite-design.md`; `docs/developers-guide.md` §"Writing an
early pass" and §"Proof workflows"; `docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`; and
`docs/whitaker-clone-detector-design.md` for the existing proof-sidecar
conventions referenced by the D-8 alternative.

## Revision note

- 2026-11-11: initial draft written from Wyvern reconnaissance and direct
  inspection of the pinned rustc sources (commit `bf53e8fa`).
- 2026-11-11: revised after the community-of-experts review. Changes: D-2
  rewritten as recovery plus a named interim selector, with the RFC conflict
  stated and flagged for approval; D-8 changed from Verus proofs to exhaustion
  and a tree-oracle property test, with a Verus `L-STACK` alternative offered
  and the `AGENTS.md` deviation flagged for approval; the Verus milestone
  removed, so milestones renumbered to `EP-M0` to `EP-M3`; new risks for
  rustc's soft deprecation of pre-expansion passes, inert runs, and `#[expect]`
  misfires; module-run state moved out of `check_crate`; fixtures restructured
  around `Test::src_base` with a `ui-no-harness/` batch; `tokio` removed;
  `FunctionContext` split from `BodyPresence`; span choice made driver-private;
  `ContainerStack::exit` returns the popped frame; `cfg_attr(..., doc = ...)`
  counted as documentation; help text placed above attributes; tolerance raised
  to 60 files and 4,500 lines; a hand-off section added. Remaining work is
  unchanged in intent: implement `EP-M0` to `EP-M3` after approval.
