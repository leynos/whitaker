# Plan: Experimental macro-aware cognitive complexity (#504)

## Status and purpose

Status: IN PROGRESS

Approved for implementation on 2026-11-08.

This plan delivers an experimental `macro_aware_cognitive_complexity` lint. A
function containing four logging calls should score 1 regardless of whether
Cargo enables `tracing/log`. Source branches inside macro arguments remain
chargeable. Source desugarings remain visible, while generated branching does
not increase the score. This is a structural proxy, not a validated measurement
of human understanding.

Implementation began after approval on 2026-11-08. M1 delivered the shared
origin classifier, its adapter, the Verus proof, and ADR 0013; the standalone
lint and its delivery follow. The pull request must not close issue #504 until
the feature and its acceptance gates are complete; `Closes #504` is added only
then.

## Conformance basis

The repository baseline is `9fc35c08643351ede3b8e15e7b6773abc9aff033`,
inspected on 2026-11-08. The controlling requirements are the user's four-task
Coding Plan for [issue #504](https://github.com/leynos/whitaker/issues/504),
including its proof and testing instructions. The branch is
`issue-504-macro-aware-cognitive-complexity-rule`.

[RFC 0010](../rfcs/rfc-0010-macro-aware-cognitive-complexity.md), currently
Proposed, and [roadmap](../roadmap.md) phases 15 and 16 supply prior design,
not authority to expand this task.
[ADR 0013](../adrs/adr-0013-node-origin-provenance-classifier.md) records the
accepted provenance-classifier decision implemented by M1. `docs/design.md`,
requested as a reference, does not exist at this baseline; use the indexed
[suite design](../whitaker-dylint-suite-design.md) instead.
[Documentation contents](../contents.md) and
[repository layout](../repository-layout.md) identify the other owners.

Requirements used below are:

- R1: one compiler-free origin policy and one rustc adapter, with desugarings
  transparent and macro/AST-pass marks generated.
- R2: Clippy-compatible arithmetic on authored nodes, independent guard
  classification, unchanged body boundaries, and unconditional expression
  descent within each selected body.
- R3: experimental-only registration, default limit 25, strict greater-than
  comparison, configuration fallback, and three localized diagnostics.
- R4: real-compiler regression evidence, macro-free parity, isolated logging
  feature graphs, and a substantive proof of the shared pure policy.
- R5: separately reviewable adoption by both existing lints, retaining their
  weighting and closure policies, with user-facing migration documentation.

There are deliberate scope differences from RFC 0010. This plan follows the
supplied Coding Plan: three mark kinds, no special `GeneratedDesugaring`, no
`model` setting, no excluded-unit diagnostic note, no per-item limit override,
and no traversal into generated nested closure bodies. Generated functions and
closures are skipped. Authored closures are scored separately. RFC §7.5's
transparent generated closures and recovered-anchor generated functions are not
implemented. Consequently an authored argument moved into a generated closure
can remain unscored; document that limitation and pin it with a test.
Identity-macro invariance applies only when expansion preserves body boundaries
and token contexts. Do not claim the RFC's stronger invariance universally.

The roadmap currently postpones stable-lint adoption until phase 16. The user
explicitly requests that adoption as Task 4 here. After approval, reconcile the
RFC and roadmap to describe this narrowly scoped provenance correction, without
claiming that phase 16's harmonized weighting or phase 15's Sonar model
shipped. The initial outer-mark-only issue sketch is superseded by the user's
detailed Task 1, which already requires peeling desugaring marks before
Macro/AstPass.

## Constraints

Track the same dated nightly as Clippy. The initial inspection used
`nightly-2026-05-28`, rustc `57d06900fd7d9ee06d3a7f323bb77f17ab3cfaf8`; that is
a historical evidence baseline, not a requirement to retain that toolchain.
Issue #504's August measurements are also historical evidence, not acceptance
results for a later compiler.

[Issue #561](https://github.com/leynos/whitaker/issues/561) governs safe
nightly rollout so existing installer callers retain a working generation.
[Issue #579](https://github.com/leynos/whitaker/issues/579) proposes scheduled
monitoring of the gap from Clippy's own dated nightly: warning at 45 days,
failure above 60 days. That monitoring threshold is not the alignment goal or
proof that an upgrade is safe. Both issues were open when read for this
revision; their proposals must not be represented as implemented safeguards.

At implementation start, read Clippy's `main` toolchain at a recorded commit
and Whitaker's current toolchain and embedded Clippy snapshot. Record the
nightly, rustc commit, Clippy revision, and Dylint version used by this work.
Use the repository's safely adopted Clippy-aligned generation; if a gap
remains, record it and coordinate with #561 rather than silently treating May
as the permanent target or moving the pin from this lint branch. Upgrade
rollout and drift-monitor implementation remain owned by #561 and #579.

Keep each acceptance run on one coherent compiler generation. After an adopted
pin change, recheck the HIR APIs, lowering, origin chains, and Clippy
arithmetic, then rerun V2–V5 and affected milestone gates before reusing their
conclusions. Refresh the evidence links and version tuple. Do not import full
Clippy utilities or freeze an obsolete toolchain to avoid an adapter problem.

Keep domain policy free of rustc types and I/O. The compiler adapter depends
inward on that policy; no additional trait hierarchy or compatibility facade is
needed. The policy returns origin, never a traversal decision or diagnostic
span. Classify original contexts; never use `from_expansion()`,
`source_callsite()`, `walk_span_chain`, or `recover_user_editable_hir_span` as
classification shortcuts.

Always descend into expression children, including generated wrappers, within
the selected body. Preserve default nested-body/item boundaries. Guard origin
is independent of its enclosing match. Generated expressions do not update
return-adjacency state. Preserve existing Bumpy Road weights and its authored
closure handling; issue #357 is out of scope.

Publish only as experimental, with no default-suite activation. Never add
`assume`, `admit`, external/trusted proof bodies, or assertions that merely
assume the conclusion. Keep all code files below 400 lines, document modules
and functions, and use caret dependency requirements. Use shared Cargo cache
and compiler admission; no builds under `/tmp`, no wrapper bypasses, and no
service recovery by agents. `/tmp` is for logs and scratch text only.

## Tolerances (exception triggers)

Complete each approved milestone before moving on. Stop and record a proposed
deviation if original-context traversal cannot recover generated desugaring
provenance, if the pinned HIR cannot support the specified body boundaries, or
if macro-free score parity fails for a reason outside the documented selection
differences. Do not silently follow call sites or redesign the metric.

New runtime dependencies, lint settings, stable default activation, changed
weights, or work outside the named classifier/lint/integration/documentation
surfaces require explicit approval. Authorized test dependencies include
`googletest`, `pretty_assertions`, `rstest`, `rstest-bdd`, `proptest`, and
`insta`; first reuse workspace versions. This plan imposes no artificial total
line/file budget; ExecPlan files are excluded from any later change-size
budget. The per-code-file 400-line constraint still applies.

An infrastructure failure is not a source failure or a passing gate. Preserve
logs and candidate identity, report unhealthy compiler admission, and continue
independent documentation work. Never relaunch a queued gate. Escalate an
unresolvable requirement conflict with the evidence and options recorded here.

## Context and proposed interfaces

`common/` is package `whitaker-common` (Rust import `whitaker_common`);
`src/hir/` belongs to root package `whitaker`. Existing `src/hir/mod.rs` span
recovery is diagnostic-only. Its synthetic hygiene tests are in
`src/hir/tests.rs`. A repository sweep found no shared classifier with the
required Macro/Desugaring/AstPass policy.

Add `common/src/node_origin/mod.rs`, exported by `common/src/lib.rs`, with
`NodeOrigin::{Authored, Generated}`, a three-variant mark enum, and
`classify(marks)` accepting outermost-to-innermost marks. Skip desugarings; the
first Macro or AstPass returns Generated; exhaustion returns Authored. Add
`crates/whitaker_lint_core/src/hir/node_origin/mod.rs`, exported behind
`dylint-driver`, with `node_origin(span: Span) -> NodeOrigin` and
`is_generated(span: Span) -> bool`. An internal context-taking helper may
support tests without duplicating policy. Root returns Authored immediately.

At the historical May source revision, `SyntaxContext::remove_mark(&mut self)`
returns an `ExpnId` and changes the context to its parent. `marks()` collects
marks and reverses them, so do not assume it already yields the required
direction. `outer_expn_data()` returns `ExpnData`. Prefer incremental mark
removal without allocating a vector per HIR expression. Pinned
`Span::mark_with_reason` applies a desugaring mark to its input span, and
lowering forwards that span unchanged. This supports the original context scan.
Transparent-mark hygiene normalization remains a caveat: M1 must establish that
real HIR retains the required parent origin. Synthetic nested marks alone
cannot answer that question.

Permitted consumers are provenance-sensitive lint drivers. They may gate
charges, predicates, or segments, but must not derive traversal from origin.
Span recovery remains a separate diagnostic facility. Record that reuse rule in
the developer's guide and an ADR before adoption.

Use `crates/conditional_max_n_branches/Cargo.toml` and `src/lib.rs` as the new
lint scaffold: empty default features, `constituent` wiring, a feature-off
stub, pure `policy`, and a `dylint-driver`-gated private driver. The actual
conditional driver path is now `src/driver/mod.rs`, not the older issue's
`src/driver.rs`. Its private registration module uses `impl_late_lint!` at
`Warn`. It loads configuration and localization in `check_crate`. Do not copy
its optional dependency declarations blindly: any common origin types or serde
configuration used by the ungated policy must be available without
`dylint-driver`; only compiler dependencies belong behind that feature.

The new configuration has only `max_complexity`, defaults to 25, and derives
`serde(default, deny_unknown_fields)`. Missing configuration uses defaults;
malformed, negative, wrong-type, and unknown-field input also uses defaults
with a structured debug event. Do not install a global tracing subscriber.

## Behavioural contracts and proof sketch

Define the specification before implementing the policy. A mark sequence is a
finite sequence over D (Desugaring), M (Macro), and A (AstPass). Its origin is
Generated exactly when some element is M or A. Otherwise it is Authored. The
empty sequence is a valid authored witness. `[D, M]` and `[D, A]` are valid
generated witnesses. All finite lengths and orderings are admissible; no
premise assumes the classification result.

Candidate semantic lemmas are: inserting or removing D anywhere preserves
origin; inserting M or A makes origin Generated; concatenation is Generated
exactly when either component is Generated; and the executable early-exit scan
refines this existential specification and terminates. A natural loop invariant
says the consumed prefix contained only D while scanning continues, and the
remaining suffix shrinks. These are proof obligations, not a demand for this
exact eventual lemma decomposition.

Use Verus for this small unbounded shared contract. The proof sidecar
`verus/node_origin.rs` must connect its executable scan to the independent
sequence specification. Document the representation relation to the production
mark enum and iterator, with an explicit reviewable mapping of each transition.
If a shared executable kernel can be verified directly without contaminating
the public domain API, prefer it. Otherwise label the sidecar as a model proof
and retain an explicit production-refinement review and differential tests;
never claim that a model proof verifies rustc hygiene or arbitrary Rust code.
No unproved theorem source is required during this planning change.

For scoring, define a preorder stream of expression events from one selected
body, excluding nested bodies/items. Each event has its own origin and kind;
match guard charges are separate origin-labelled facts, evaluated once. Filter
expression events to Authored before tracking adjacency. The raw count B is 1
plus authored if events, authored multi-arm match events, and authored guards
(even if their match is generated or single-arm). R counts authored return
events whose previous authored expression is not a return. Other authored
expressions reset that adjacency state; generated ones do not.

Let A be R for a body whose result type is the diagnostic-item `Result`, and
integer division R/2 otherwise. Score is B-A if B is at least A, else B. Report
exactly when score exceeds the configured limit. Baseline 1 is not a minimum
after adjustment: zero is possible. A source `?` contributes lowered
branch/return events but its net score can remain 1 after Result adjustment. Do
not add an extra loop or question-mark charge beyond pinned Clippy HIR
arithmetic. Use a wide integer type and checked representability assumptions;
property tests cover adjustment boundaries and arithmetic overflow behaviour.

No Verus proof is proposed for rustc lowering, diagnostic formatting, Fluent,
Cargo, or suite registration. Conventional compiler tests and end-to-end checks
exercise those boundaries. Score arithmetic and ordered return handling use an
independent event-list oracle, parameterized tests, and property tests; there
is no claim of an exhaustive score proof.

## Verification plan

V1 discharges R1's abstract policy: `rstest` includes empty, each singleton,
`[D,M]`, `[M,D]`, `[D,D]`, `[M,M]`, `[D,A]`, `[A,D]`, `[D,D,M]`, and `[M,D,M]`.
Exhaustively enumerate lengths 0 through 4 over three marks (121 sequences).
Generate additional lengths 0 through 256 with `proptest`, without rejection
filters, checking against the existential oracle. This is bounded test
coverage, not an unbounded proof. Verus supplies the latter. Mutating Macro to
Authored or inspecting only the outermost D must fail both appropriate tests.
Apply the corresponding mutation separately to the Verus executable scan and
require verification failure; a production mutation cannot invalidate a
disconnected model automatically. Record a reviewed production/model transition
correspondence receipt beside the proof results. Inspect proof dependencies and
show non-empty witnesses for each class; no contradictory preconditions are
allowed.

V2 discharges the rustc-to-policy boundary: parameterize the existing synthetic
constructor by `ExpnKind` and parent context. Cover root; bang, attribute, and
derive macro marks; AST-pass marks; for/question-mark desugaring over root;
desugaring over macro and AST-pass; nested macros; two desugarings over macro;
and macro over desugaring. Verify sequence direction explicitly. Real-compiler
fixtures separately cover the same source constructs in source, macro
arguments, and macro bodies, including mixed-provenance `when!` wrappers.
Record actual context chains and expected classification on the pin.

V3 discharges R2 scoring and traversal with independent expectations. Pure
cases cover baseline, generated-only events, authored/generated if and match,
zero/one/multiple match arms, guards of each origin under each match origin,
consecutive returns, authored non-return separation, generated interleaving,
Result/non-Result adjustment, and B below/equal/above A. Check limits 0, exact
score, and score minus/plus one, plus malformed config. `googletest` assertions
express semantic predicates; `pretty_assertions` compares event lists and
expected diagnostic structures. A generated event insertion must leave authored
projection and score unchanged; inserting an authored non-return between
returns deliberately need not. Negative controls charge generated nodes, prune
generated children, or let generated nodes reset return adjacency.

V4 is compiler UI and differential evidence. Add driver-gated `tests/ui.rs` with
`fn ui()` and threshold 0. Keep fixtures using `std` and local `macro_rules!`
separate from the tracing matrix. Hand-derive scores, then compare macro-free
functions with pinned Clippy; do not copy Whitaker output as the oracle. Cases
include:

- `matches!` alone: 1; source if inside `assert!` and `println!`: 2;
  source if wrapping `matches!`: 2; nested generated branches: 1.
- Authored guard inside generated `matches!`: 2; generated guards ignored;
  ordinary single-arm and multi-arm guarded matches pin independent charging.
- Authored closure passed to a macro: caller stays at its own score and closure
  body with one if scores 2; generated closure/function skipping is explicit.
- Source for, while, question mark, async function, async block, and await;
  each meaningful equivalent in a macro body and macro argument. Assert raw
  charges as well as net score for return-producing desugarings.
- Mixed source if/match/guard and generated branches; Result and non-Result
  returns; generated returns; identity macros preserving body boundaries;
  nested items; methods; `#[test]` exclusion; mixed keyword/body provenance.

Include positive and silent fixtures at a nonzero limit as well as threshold 0
diagnostics. Add en-GB, cy, and gd localization assertions using existing
Fluent resources under `common/locales/`. Use `insta` for stable rendered
multilocale messages where it complements, rather than duplicates, UI stderr.
No unread snapshot updates: score, span, warning count, and help are reviewed.
Macro-free parity compares scores on every body Clippy selects; document any
additional source-desugared body Whitaker selects instead of forcing silence.

V5 discharges R3/R4 externally. `tests/feature_matrix.rs` generates two
standalone projects, each with `[workspace]`, identical source and locked
tracing version, explicit identical default features, and only `log` differing.
Reuse the nested-process discipline in
`crates/no_std_fs_operations/tests/test_support/mod.rs` and
`crates/no_std_fs_operations/tests/no_default_features_build.rs`. Build and
stage the lint library once. Run separate `cargo dylint` subprocesses with
distinct `CARGO_TARGET_DIR` paths outside `/tmp`, shared default Cargo cache,
and scoped child environments. Do not mutate the parent's environment. Preserve
compiler wrappers.

Assert with `cargo tree -e features` that log is absent/present respectively;
assert four logging calls score 1 at threshold 0 in both. Compare normalized
lint diagnostics byte-for-byte after replacing only known temporary path
prefixes and removing Cargo progress output. Never normalize scores, spans,
message text, or counts. Keep raw logs. Seed a local negative-control
classifier that charges generated nodes: the fixture must detect a score
increase on the pin, without assuming August's exact 29. Restore code and rerun
the good case. Mark this test `#[ignore]` because registry access is required,
but explicitly run it for milestone acceptance. Add a dedicated Nextest serial
group with max threads 1 and a 120-minute termination timeout. Its library
build and staging directory must also be private to the matrix run, not shared
with ordinary UI tests; separate consumer targets alone are insufficient.
Retain the shared Cargo cache. List the selected tests first and require a
nonzero count; do not claim the ordinary test gate ran ignored tests.

V6 covers R5 with executable per-lint UI harnesses and before/after evidence.
Conditional fixtures require silence for generated `assert!(a && b && c)`, a
diagnostic for an authored three-predicate if in a macro argument, and a
diagnostic for a source three-predicate while. Independently authored match
guards remain checked even inside generated matches. Bumpy Road fixtures must
first demonstrate the current source-for/macro-argument loss, then show
restored segments and diagnostics under controlled thresholds. Generated
branches must add neither predicates nor depth; descendants must be visited
exactly once. Keep existing while-desugaring handling and authored closure
semantics.

The external assumptions are rustc hygiene preserving metavariable contexts,
HIR visitor order/body boundaries, Dylint loading the selected library, and
Cargo isolating independent workspaces. Verus does not prove these assumptions;
V2, V4, and V5 exercise them. Procedural macros can manufacture
authored-looking spans, so provenance remains a hygiene approximation. Document
that limit.

## Milestones

### M1 — Establish and prove origin classification (R1, R4)

After approval, first establish the compiler-generation receipt described in
Constraints and resolve any rollout dependency with #561. Then add the smallest
failing policy/synthetic/real-compiler probes, record the red result, and
inspect lowering on that generation. Record the chain walk decision in a new
ADR using the next available independent ADR number; do not reserve a number
now. If contexts lose the parent origin, stop at this decision gate before
implementing a misleading classifier. The required contract must not be
weakened to make a synthetic test green.

Implement the common policy and adapter, then make focused tests pass. Add
Verus specification, scan/refinement proof, and the production bridge review.
Use the existing runner's explicit-file support:

```sh
./scripts/run-verus.sh verus/node_origin.rs
cargo test -p whitaker-common node_origin
cargo test -p whitaker_lint_core --features dylint-driver --lib node_origin
```

Add `make verus-node-origin` wrapping that explicit-file invocation and include
it in the documented proof inventory; do not migrate unrelated prover tooling.
Acceptance requires zero Verus verification errors, V1/V2 evidence, and the
ordinary gates below. Tests of root `whitaker` are explicit because the normal
Make test lane excludes it. Commit the coherent shared facility, ADR, and reuse
rules. Remaining gap: no new lint yet. Recovery is a revert of this atomic
commit, not a toolchain change.

### M2 — Deliver the standalone experimental lint (R2, R4)

Create `crates/macro_aware_cognitive_complexity/` and add it to explicit
workspace members if needed. Keep its policy ungated and driver private/gated.
Follow red-green-refactor for pure scoring, config, localized messages, and UI
cases. Unlike the reference scaffold, explicitly enable library tests so the
ungated policy suite runs under `--lib`. Require nonzero focused test counts. A
default-nested-filter `intravisit::Visitor` visits the selected body and calls
`walk_expr` for every expression. Charge authored matches independently of arm
guards; charge each guard by its own span. No generated expression changes
previous-authored-expression state.

Use `check_fn` for functions, methods, and closures, skip generated function or
closure spans and `#[test]`, and extract async bodies as pinned Clippy does.
Verify actual HIR before copying helpers. Determine Result from the selected
body's result ADT and diagnostic item through `tcx`, not its textual type name.
Emit at function ident or closure `fn_decl_span` with score, limit, and help
explaining excluded macro-generated branching. Use common i18n emission and
Fluent keys in en-GB, cy, and gd; initialize localization in `check_crate`.

Add the V4 UI harness and a driver-gated differential integration target
`tests/parity.rs` comparing the pinned installed Clippy component. Add V5's
ignored feature matrix and Nextest group. If Cargo all-target tests would run
compiler-only examples incorrectly, follow existing UI fixture selection; do
not weaken global warnings. Update doctest selection deliberately: use
compiler-free policy examples, and keep rustc-dependent snippets accurate.

Focused commands, in addition to the ordinary gates, are:

```sh
cargo test -p macro_aware_cognitive_complexity --no-default-features --lib
cargo test -p macro_aware_cognitive_complexity --features dylint-driver --test ui
cargo test -p macro_aware_cognitive_complexity --features dylint-driver --test parity
cargo nextest list -p macro_aware_cognitive_complexity --features dylint-driver --test feature_matrix
cargo nextest run -p macro_aware_cognitive_complexity --features dylint-driver --test feature_matrix --run-ignored only
```

V3, V4, and V5 must pass, including the restored negative-control run. Commit
coherent standalone functionality and its tests. Remaining gap: suite and
installer do not yet expose it. Revert this milestone independently if needed.

### M3 — Wire experimental delivery and document the contract (R3)

Add the optional dependency and `experimental-macro-aware-cognitive-complexity`
feature in `suite/Cargo.toml`, following `rstest_helper_should_be_fixture`,
including constituent wiring. Add the gated descriptor/declaration in
`suite/src/lints.rs` and the driver import/combined-pass entry in
`suite/src/driver.rs`. Add the crate to `EXPERIMENTAL_LINT_CRATES` in
`installer/src/resolution.rs` and its resolution contracts. Confirm all
explicit lint packaging inventories include the crate only under the
appropriate experimental path.

Add registry tests for default absence and enabled presence exactly once,
installer selection tests, and an end-to-end suite invocation that emits the
new warning only with the feature enabled. The new behavioural feature file
`installer/tests/features/macro_aware_complexity.feature` states:

```gherkin
Feature: Opt in to macro-aware complexity
  Scenario: Stable installation omits the experimental lint
    Given an installation request without experimental lints
    When the installer resolves lint crates
    Then macro_aware_cognitive_complexity is absent

  Scenario: Experimental installation includes the lint
    Given an installation request enabling experimental lints
    When the installer resolves lint crates
    Then macro_aware_cognitive_complexity is selected exactly once
```

Bind those scenarios in the new integration target
`installer/tests/macro_aware_complexity.rs` with existing `rstest-bdd`
patterns; assert resolution independently of the suite compiler smoke test. Use
injected configuration and fallible fixtures, not global environment mutation.

Update the user's guide with limit 25, strict `>`, a `dylint.toml` example,
experimental opt-in, closure selection, macro hygiene limitations, source
lowering, and return adjustment. Update suite design with Clippy revision,
per-node origin, guards/returns, AstPass rationale, and separation from the
Sonar-style brain-type model. Update developer's guide with reuse and proof
boundaries. Link the ADR and reconcile RFC/roadmap scope as described above;
use the `mapsplice` skill for structural roadmap edits. Do not add duplicate
phase-15 entries or mark deferred work complete.

Add named suite integration targets `tests/macro_aware_registry.rs` and
`tests/macro_aware_smoke.rs`, with assertions conditional on the experimental
feature. The smoke target stages the built suite and runs a fixed consumer
through `cargo dylint`; its feature-off case must load a real driver, not a
stub. Run these commands sequentially and require nonzero test counts:

```sh
cargo build -p whitaker_suite --features dylint-driver
cargo test -p whitaker_suite --features dylint-driver --test macro_aware_registry --test macro_aware_smoke
cargo build -p whitaker_suite --features experimental-macro-aware-cognitive-complexity
cargo test -p whitaker_suite --features experimental-macro-aware-cognitive-complexity --test macro_aware_registry
cargo test -p whitaker_suite --features experimental-macro-aware-cognitive-complexity --test macro_aware_smoke
cargo test -p whitaker-installer --test macro_aware_complexity
```

Acceptance requires V3 config/localization, V5 suite/installer coverage, full
gates, and documentation review. Commit the integration. The lint remains
experimental even after all tests pass; promotion is separate.

### M4 — Correct existing lint provenance separately (R5)

First migrate `conditional_max_n_branches/src/driver/mod.rs`: gate condition
inspection for generated If and While loop nodes, independently gate guard
expressions, and leave rustc traversal unchanged. Its current UI harness is
`src/lib_ui_tests.rs`; add a driver-gated integration `tests/ui.rs` with
`fn ui()` if necessary for executable coverage, moving rather than duplicating
the same harness. Establish red fixtures before edits, pass V6, update its
guide/design section and changelog/migration note, run all gates, and commit.

Then migrate `bumpy_road_function/src/driver/mod.rs` item/body guards and
`driver/segment_builder.rs`. Replace generated-expression pruning with an
explicit walk of children without recording generated segments, predicates, or
depth contributions. Preserve authored traversal's custom depth management and
avoid double visits. Replace `push_segment`'s expansion predicate with the
shared classifier. Keep `DesugaringKind::WhileLoop` special handling and
closure rules. Run red-green source-for, macro-argument, and generated-only
fixtures, plus existing bump tests. Update documentation and separate
migration/changelog entry, gate and commit this second adoption independently.

Acceptance requires V6 and unchanged weighting expectations, alongside all
ordinary gates after each adoption. Review the changed code after each
functional commit for the repository's refactoring heuristics; any required
refactor is a separately gated commit. Recovery can revert either adoption
without disabling the experimental lint. No #357 weighting work is included.

## Concrete gate and delivery procedure

All commands run from the repository root. Record `git rev-parse HEAD` and the
working diff alongside each gate receipt. Run formatting, format checks, lint,
and tests sequentially for the candidate. Delegate full commit gates to a
`scrutineer`, the only subagent running them. Capture each command with
`set -o pipefail` and `tee /tmp/issue-504-<milestone>-<gate>.out`; a nonzero
command must remain nonzero through the pipeline.

For every implementation milestone and atomic adoption commit, run:

```sh
make fmt
make check-fmt
make lint
make test
make markdownlint
make nixie
```

Run the focused commands and relevant proof check as additional acceptance, not
replacements. `make test` currently uses Nextest and excludes several packages;
inspect `TEST_EXCLUDES` and `.config/nextest.toml`, and explicitly run omitted
relevant root/driver/ignored targets. `make markdownlint` includes spelling;
generated dictionary changes must follow AGENTS.md's separate-base policy.
Check formatter changes before staging; do not commit unrelated edits.

For this documentation-only proposal, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`, plus link/diff checks. No runtime
feature or proof pass is claimed. Publish only the gated plan and its contents
entry.

Commit using a reviewed message file. Push via standard SSH Git as `leynos`,
using the clean environment prescribed by AGENTS.md. Verify SSH identity first.
After an explicit-URL push, fetch
`+refs/heads/issue-504-macro-aware-cognitive-complexity-rule:refs/remotes/origin/issue-504-macro-aware-cognitive-complexity-rule`,
set the branch's named upstream, and compare `ls-remote` with local HEAD.
Create a draft PR titled
`Plan: Propose a macro-aware cognitive complexity rule (#504)`. Its summary
links this document using a branch-wise GitHub URL. End the body with
`## References` and the Lody session link:
<https://lody.ai/leynos/sessions/cb8d046e-4118-488d-943b-301559dd39ce>. Read
back the PR's title, draft state, head, and body. Await plan approval.

## Risks

The main risk is information loss in desugaring hygiene; the M1 decision gate
must precede claims that the classifier handles real generated loops. Default
nested-body boundaries intentionally leave the generated-closure blind spot.
Procedural macro re-spanning also limits origin fidelity. Document both rather
than adding ad hoc macro allowlists.

Compiler generations can change between planning and implementation. Freeze the
parity corpus, dependency locks, and compiler-generation receipt for each
acceptance run, not the repository nightly indefinitely. Never relabel May
source inspection or August measurements as evidence for an upgraded compiler.
The test matrix must use truly separate feature graphs, and ignored tests need
explicit execution. Comparing entire Cargo stderr would add path noise; compare
only deliberately normalized diagnostics.

A pure proof can drift from runtime code. Maintain the representation mapping,
transition comparison, and mutation evidence whenever the policy changes.
Compiler API changes can invalidate the adapter without touching the proof;
real-compiler fixtures remain mandatory. Sequential gates and shared admission
may be slow; queued work is not hung and must not be duplicated.

## Progress

- [x] 2026-11-08: Read requested skills, repository guidance, RFC, and tests.
- [x] 2026-11-08: Rename local branch and Lody session to the requested names.
- [x] 2026-11-08: Gather source evidence with two Wyvern agents and context
      packs.
- [x] 2026-11-08: Retrieve pinned Clippy/hygiene source through Firecrawl.
- [x] 2026-11-08: Six expert lenses reviewed the draft; executable commands,
  policy-test discovery, proof mutation correspondence, and matrix staging
  isolation were corrected.
- [x] 2026-11-08: Initial documentation gates passed; commit `c0d2c4a7`
  published with verified upstream in draft PR #574.
- [x] 2026-11-08: Read #561 and #579; replace the fixed-May constraint with
  Clippy alignment, safe-rollout dependency, and per-generation revalidation.
- [x] 2026-11-08: Gate and publish the toolchain clarification.
- [x] 2026-11-08: Explicit user approval received; implementation begins.
- [x] M1: classifier, adapter, evidence, and Verus proof.
  - [x] Compiler-generation receipt established: `nightly-2026-05-28`,
        rustc `57d06900fd7d9ee06d3a7f323bb77f17ab3cfaf8`, Clippy snapshot
        `e4608a25c2e20116fbc1e97a51a73f094a1db7a1`, selected revision
        `9fca3bc9fc2bc83c60bde26d18ed68f11564b228`, Dylint 6.3.0. Upstream
        Clippy `main` at `146674fd0b6b414a2ae3e0a0b05a1b4885567b08`
        (2026-11-08) pins `nightly-2026-10-30`, a gap of about 155 days left
        to #561/#579.
  - [x] Red policy, synthetic adapter, and real-compiler probes recorded.
  - [x] Real rustc lowering inspected with a temporary Dylint probe; contexts
        retain parent origin, so the M1 decision gate did not trigger.
  - [x] Chain-walk decision recorded in
        `docs/adrs/adr-0013-node-origin-provenance-classifier.md` and indexed
        in `docs/contents.md`.
  - [x] `whitaker_common::node_origin` policy added with exhaustive and
        property tests.
  - [x] `src/hir/node_origin/mod.rs` adapter added with synthetic `ExpnKind`
        cases
        and a direction-pinning test.
  - [x] `verus/node_origin.rs` added; `make verus-node-origin` wraps it and
        `make verus` includes it. Result: 8 verified, 0 errors.
  - [x] Four mutation receipts recorded (policy and model, Macro-transparent
        and outermost-desugaring-stop).
  - [x] `docs/developers-guide.md` records reuse rules and proof boundaries.
  - [x] Focused commands and the ordinary gates passed via the scrutineer;
        commit `b229d604` records the shared facility.
  - [x] Milestone CodeRabbit CLI review completed; its two trivial
        documentation findings are corrected in a follow-up documentation
        commit.
  - [x] Rebased onto `origin/main` (`d31030b5`), re-ran every gate on the
        rebased candidate, and force-pushed `78695935` over the draft head.
        The remote branch, its tracking ref, and local `HEAD` all name
        `78695935eb5cd734fd5a228a452c4b94046ee6b1`.
- [x] M2: standalone lint, parity, localization, and logging matrix.
  - [x] 2026-11-12: The logging matrix now also compiles an attributed
        `#[tracing::instrument] async fn` (roadmap 15.4.2). The attribute
        macro re-emits the function, but its authored identifier survives, so
        the function remains a scoring unit and both graphs report it at the
        authoring baseline of one. An earlier draft of the comment and the
        roadmap claimed both tools skip it; that was contradicted by the run,
        which reports two diagnostics, and is corrected here and in
        `docs/roadmap.md`. The focused rerun passed in 31.8 s (1 passed, 59
        skipped) at commit `176726b5`.
  - [x] Compiler-free scoring kernel and crate scaffold added: authored-only
        branch, guard, and return charges with generated events discarded
        before any state change, `Result`-dependent return adjustment, and a
        strict `exceeds` limit predicate. Coverage is 33 tests (rstest cases,
        an independent oracle, proptest properties, and a googletest semantic
        suite). Clippy and formatting are clean, all six gates passed via the
        scrutineer, and commit `1632e7b0` records the policy.
  - [x] M2b: rustc driver, configuration, localization, and UI fixtures.
        `check_fn` scores functions, methods, and closures. It skips
        `span.from_expansion()`, generated anchors, `#[test]` items, and the
        compiler-synthesized coroutine wrapper of every `async fn` and `async`
        block, then unwraps an `async fn` body exactly as pinned Clippy does.
        Traversal is a default-nested-filter `intravisit::Visitor`, which
        reproduces Clippy's `for_each_expr_without_closures` boundary; each
        match arm guard is charged by its own span. Configuration reads
        `max_complexity` (default 25) and messages ship in en-GB, cy, and gd.
        The threshold-zero oracle table and all 43 blessed diagnostics agree
        (26 pre-existing plus 17 added for the V4 cases), and the localization
        suite pins the rendered en-GB, cy, gd, and `zz` (unknown-locale
        fallback) text. Focused results: 33 no-default-feature lib tests, 46
        driver lib tests, 4 UI harness tests, and 5 localization tests. The
        crate joins `DOCTEST_EXCLUDES`: under `--all-features` its library
        links `rustc_private`, which a doctest crate cannot load, and all three
        policy examples fail to compile there; the same three pass under the
        default feature set. `common/build.rs` adds the locale watcher described
        in Surprises. Full gates passed on the candidate: `make check-fmt`,
        `make lint`, `make test` (2394 passed, 6 skipped, plus 74
        `-p whitaker --lib` and 48 `nextest_ui_filter`), `make test-doc`,
        `make markdownlint` (117 files, 0 errors), `make nixie`,
        `make test-workflow-contracts` (966 passed),
        `make verus-node-origin` (8 verified, 0 errors), and the four UI
        harnesses. The milestone CodeRabbit CLI review (v0.9.3,
        `--committed --fresh --base-commit d1b0f460`) completed over all 45
        files and raised seven findings: four crate-wide `#![allow(dead_code)]`
        suppressions in UI fixtures, two `-ise` spellings, and the policy
        doctests running in no gate after the `DOCTEST_EXCLUDES` change. All
        seven were valid and are repaired: the fixtures now expose their items
        through a `pub mod fixture` with no suppression and were re-blessed
        from captured compiler output, the comments use Oxford `-ize`, and
        `make test-doc` gained a default-feature doctest pass (3 passed) for
        the crate.
  - [x] 2026-11-09: Rebased the nine-commit M1/M2 series onto `origin/main`
        (`2b7a0189`, from base `d31030b5`) with Git's built-in merge and
        `zdiff3`. The user-wide `*.rs merge=weave` attribute was bypassed
        because the repository does not opt in through tracked attributes.
        All nine commits replayed with no conflicts and `range-diff` reports
        them identical. The lockfile keeps main's dependency bumps and adds
        only this crate; it validates under `cargo metadata --locked`. The
        new crate's version moved from `0.2.11` to the workspace's `0.2.12`
        because main had bumped every sibling after the crate was created.
  - [x] 2026-11-09: Full post-rebase gate run at `a9c6d3ec` passed via the
        scrutineer: `make check-fmt` (117 files unchanged), `make lint`,
        `make test` (2398 passed, 6 skipped), `make test-doc`,
        `make typecheck`, `make markdownlint` (117 files, 0 errors),
        `make nixie`, `make test-workflow-contracts` (966 passed),
        `make spelling`, and `make verus-node-origin` (8 verified, 0 errors).
        Force-pushed with lease over the verified remote head `8cfaf27b`;
        the remote branch, its tracking ref, and local `HEAD` all named
        `a9c6d3ec` at publication.
  - [x] 2026-11-09: Re-ran the milestone CodeRabbit CLI review on the rebased
        candidate (`coderabbit review --agent --committed --fresh
        --base-commit 413e1eca`, CLI 0.9.3) over
        `724e18b4..0891d03b`. It reviewed all 47 changed files and reported
        zero findings.
  - [x] M2c: macro-free differential parity against pinned Clippy.
        `tests/parity.rs` builds the lint library, then runs pinned Clippy and
        the packaged Dylint lint over one generated macro-free fixture. Both
        tools report the same 19 `(anchor line, score)` pairs and exactly one
        diagnostic, on the `async_if` line. The fixture spans authored
        branching, match arms and guards, return adjacency, the `Result`
        adjustment, `for`/`while`/`?` desugaring, closures, and `async`.
        First cold run 380.67 s; warm run 37.00 s. Because a single test
        performs three nested Cargo builds, `.config/nextest.toml` gains a
        30-minute override in both profiles that joins
        `serial-nested-cargo-builds` and disables retries; the 300 s base bound
        cannot hold a cold fixture. `nextest_profile_test.py` reports 28
        passed, and focused Clippy is clean after replacing two needless
        raw-string hash delimiters.
  - [x] 2026-11-09: Full M2c gate run at `89ad47ec` passed via the
        scrutineer: `make check-fmt` (117 files unchanged), `make lint`,
        `make test` (2399 passed, 6 skipped; the parity test passed in
        455.7 s; `dylint_testing` 19, root lib 74, nextest filter 48),
        `make test-doc` (399 passed, 17 ignored), `make typecheck`,
        `make markdownlint` (117 files, 0 errors), `make nixie`,
        `make test-workflow-contracts` (966 passed), `make spelling`, and
        `make verus-node-origin` (8 verified, 0 errors). The first attempt
        failed `make spelling` on two `hand-written` spellings in the new
        test; both were corrected as `handwritten` and the commit amended,
        after which all ten targets passed. The single `make test` process
        was stopped by the runner's 90-minute harness cap after its main
        nextest suite and `dylint_testing` lib passed; its two remaining
        sub-targets (`test-root-lib`, `test-nextest-filter`) were then run
        directly and passed, so every component is accounted for.
  - [x] 2026-11-09: The M2c milestone CodeRabbit CLI review raised five
        findings over `724e18b4..4efb55cf`; all five were verified against
        source and repaired. `tests/parity.rs` shrank from 619 to 200 lines:
        the reusable plumbing moved to `tests/support/mod.rs` (387 lines) and
        the fixture manifest, `clippy.toml`, `dylint.toml`, and 37-line source
        moved to `tests/data/` as `include_str!` data. `pinned_toolchain` now
        returns `anyhow::Result` and fails when no `RUSTUP_TOOLCHAIN` is
        visible, instead of staging a library under an `unknown-toolchain`
        sentinel. Both nextest override comments now state the actual
        guarantee: the nested-build group serializes nested Cargo builds and
        is independent of `serial-dylint-ui`, so the test can still run beside
        a UI harness and share compiler admission. `lint_scores` delegates its
        primary-span lookup to `primary_span_line`, preserving the skip and
        error contexts. `tests/nextest_ui_filter/build_allowances.rs` gains a
        `PARITY_CHECK` identity with default and ci cases pinning 1800 s,
        `serial-nested-cargo-builds`, and `retries = 0`. Focused evidence: the
        parity test passes with the full 19-entry table and the single
        `async_if` diagnostic (449.1 s), both focused Clippy runs are clean,
        the nextest filter suite passes 50/50, and `nextest_profile_test.py`
        reports 28 passed.
  - [x] 2026-11-10: Full gate run at `f93b982b` passed via the scrutineer:
        `make check-fmt`, `make lint`, `make test`, `make test-doc` (399
        doctests), `make typecheck`, `make markdownlint` (117 files, 0
        errors), `make nixie` (119 files), `make test-workflow-contracts` (966
        passed), `make spelling`, and `make verus-node-origin` (8 verified, 0
        errors). The sub-agent harness capped the foreground command at 600 s
        and a background command at 30 minutes, so both `make test`
        invocations were stopped mid-suite (2392 and 2393 of 2399). The
        scrutineer then ran the six residual tests (`79`, `19`, and `1`
        passed) plus `dylint_testing` (19), `test-root-lib` (74), and
        `test-nextest-filter` (50), so all 2399 listed tests are accounted for
        with zero failures; the parity test passed in both runs (394 s and
        591 s). The single `make test` exit status is therefore unobserved, and
        the composed evidence is recorded as such rather than as one clean
        exit. The milestone CodeRabbit CLI review (0.9.3, `--committed
        --fresh --base-commit 4efb55cf`) reviewed all nine changed files and
        reported zero findings; the stale prior-pass list still shows the five
        original findings, each verified resolved. Its one unadopted
        sub-suggestion, extracting the diagnostic-code predicate from
        `lint_scores`, was explicitly left inline by the repair packet and
        keeps the function single-purpose.
  - [x] M2d: logging feature-graph matrix and negative control.
  - [x] 2026-11-10: `tests/feature_matrix.rs` plus
        `tests/support/feature_matrix/` implement V5. The test generates
        `logging_without_log` and `logging_with_log` from one source, pins
        `tracing` (and `log`) to the workspace lockfile versions, proves the
        graphs differ with `cargo tree -e features`, and requires byte-identical
        normalized diagnostics scoring the authored baseline of one in both.
        An acceptance run passed in 2429.5 s with both vectors reading
        ``src/lib.rs:1:1:8:26: `four_logging_calls` has a cognitive complexity
        of 1 (limit 0).``; the plain tree lists only `tracing`'s `attributes`
        feature while the `log` tree adds `log v0.4.35` and `tracing`'s `log`
        feature. The negative control was measured, not assumed: making the
        policy charge generated neutral events
        (`!event.origin.is_generated() || event.observed != Observed::Neutral`)
        produced a score of 29 on this pin, and the policy file was restored
        byte-for-byte (`git diff` empty). `nextest_profile_test.py` passes 30
        tests, and `nextest show-config test-groups --run-ignored only` assigns
        the test to `serial-feature-matrix`.
  - [x] 2026-11-10: Supervision review of the M2d worktree raised two repairs
        before gating. `project_manifest` no longer continues long literals
        with backslash-newlines; the manifest is one `concat!` template, as
        `AGENTS.md` requires. The run receipts moved from fixed `/tmp` names to
        `target/tests/feature_matrix/receipts/<mode>/`, so two worktrees cannot
        overwrite each other's evidence and no absolute temporary path is
        written by the test; the duplicated stream-writing block became one
        `write_streams` helper fed by a `RunStreams` pair, which also keeps
        `persist_logs` inside the argument limit. `format!` disables implicit
        named capture when its template is a macro invocation, so the optional
        dependency is passed positionally. Witnessed by the full gate set
        recorded below.
  - [x] 2026-11-12: The M2d milestone CodeRabbit CLI review raised two
        findings, both verified against source and repaired. The graph
        assertion proved only that the `log` package was present, which a
        direct `log` dependency would also satisfy even with `tracing`'s `log`
        feature dropped; `tree_lists_feature_edge` now also requires the
        rendered `tracing feature "log"` node, and
        `assert_log_feature_graph` checks both properties in both directions.
        `ensure_tree_succeeded` now rejects a failed `cargo tree` run before
        its empty stdout can be read as evidence that `log` is absent. Three
        unit tests in `runs::tests` cover the reviewer's negative case (a
        package node without the feature edge), a feature node nested under a
        tree prefix, and the failed-run rejection that quotes the captured
        stderr; they pass in 0.00 s and focused Clippy is clean.
  - [x] 2026-11-12: Full gate run at `c660b840` passed via the scrutineer on a
        clean tree: `make check-fmt` (117 files unchanged), `make lint`
        (interrogate 100%, rustdoc, Clippy, and the Whitaker suite; 270 s),
        `make test` (2730 s; nextest 2399 passed, 7 skipped — the extra skip is
        the ignored feature-matrix test — plus `dylint_testing` 19, root lib
        74, and nextest filter 50), `make test-doc` (399 doctests, 864 s),
        `make typecheck`, `make markdownlint` (117 files, 0 errors),
        `make nixie`, `make test-workflow-contracts` (969 passed),
        `make spelling`, and `make verus-node-origin` (8 verified, 0 errors).
        The acceptance command
        (`cargo test -p macro_aware_cognitive_complexity --features
        dylint-driver --test feature_matrix -- --ignored`) passed in 94 s with
        byte-identical vectors in both projects, the plain tree free of `log`,
        and the `log` tree listing `log v0.4.35` and `tracing feature "log"`.
        The two review repairs above changed this candidate afterwards, so all
        ten targets and the milestone CLI review are re-run at the repaired
        head before publication rather than being inferred from this run.
  - [x] 2026-11-12: Restacked the whole branch from `2b7a0189` onto
        `374b6f00` (seven intervening `main` commits) with `--reapply-cherry-picks
        --keep-empty --empty=stop`. Five commits needed manual resolutions: the
        plan added `docs/contents.md` beside `main`'s reworded planning entry;
        the classifier commit had to follow `main`'s move of the root library
        into `crates/whitaker_lint_core` and lost a numbering race, so ADR 0011
        became **ADR 0013**, with every path and citation updated; the M1
        completion entry re-anchored on that paragraph; the driver commit
        merged `main`'s rewrite of the nextest override discussion; and the
        review-findings commit kept both `main`'s feature-free
        `whitaker_lint_core` doctest step and the branch's macro-aware doctest
        step. The lint crate now depends on `whitaker_lint_core` rather than
        the root `whitaker` package, which `main` turned into a binary facade.
        Commit hashes named in earlier entries are pre-restack and no longer
        resolve; the current series starts at `a44cc107` and its M1 classifier
        commit is `93cc4f2c`. The tree checks clean with
        `cargo check --workspace --all-targets --all-features`, and every gate
        is re-run at the restacked head before publication.
  - [x] 2026-11-12: Whole-branch CodeRabbit CLI review at the restacked head
        raised nineteen findings (one major, ten minor, eight trivial). All
        were verified against source before action. Seventeen were valid and
        repaired: imperative Rustdoc summaries across the classifier, adapter,
        span helpers, policy tests and UI harness; two `# Errors` sections that
        claimed spawning was the only failure path when `pinned_toolchain()`
        can fail first; the ignored adapter examples imported `node_origin` and
        `is_generated` from `whitaker` rather than `whitaker_lint_core`; the
        stale ADR number and link, and the adapter test command, in this plan.
        Three findings were behavioural. `cargo tree -e features` now passes
        `--charset=utf8`, so the ASCII `|--` prefixes cannot defeat node
        stripping. The parity child pins `DYLINT_LOCALE=en-GB` so a
        Welsh/Gaelic developer environment cannot hide the score marker. Match
        arms are walked through `visit_arm`, which records each guard between
        the preceding arms and its own body; recording guards up front moved
        them ahead of the scrutinee and could make returns adjacent that the
        source separates. The new `ui/guard_return_order.rs` fixture pins that
        ordering: with the limit at one it reports a score of two, where the
        old ordering reports three. Two findings were spurious and are
        dismissed with evidence: the four localization baselines are tracked
        (`tests/snapshots/localization__{en_gb,cy,gd,zz}.snap`, all passing),
        and `#[test]` is not conditional on test-mode compilation, so
        `ui/threshold_zero.rs` does compile `test_shaped` and its absent
        diagnostic does prove the skip rule.
- [x] M3: experimental suite/installer registration and documentation.
  - [x] 2026-11-12: Registered the lint behind
        `experimental-macro-aware-cognitive-complexity` in the suite and the
        installer (commit `5b02ba5a`). Suite feature-off and feature-on
        builds load a real driver, the registry target asserts presence
        exactly once under the feature, and the smoke target stages the built
        suite and runs a fixed consumer through `cargo dylint`. The installer
        resolution selects the crate only under the experimental path.
  - [x] 2026-11-12: Documented the lint in the users' guide, the dylint suite
        design, the developers' guide, the README, and the repository layout;
        added RFC 0010 §16 recording every material implementation difference;
        reconciled the phase-15 roadmap entries without ticking deferred work
        (commit `cb929408`). The spelling gate then exposed a committed
        `artifact` binding in the suite smoke test, fixed in `dfe29cd2`
        against the existing exact-line exception for the external
        `Message::CompilerArtifact` name. `make check-fmt`,
        `make markdownlint` (123 files, 0 errors), `make nixie`, and
        `make spelling` pass at this head.
- [ ] M4: separately gated adoption by both existing lints.
  - [x] 2026-11-12: `conditional_max_n_branches` now gates each inspected
        condition on its own node through the shared classifier (commit
        `d5a3aa63`). Generated `if` conditions and `while` loops are skipped,
        each match guard is gated independently so an authored guard on a
        generated `match` is still charged, and the `while`-lowering `If`
        early return remains. The UI harness moved from a `#[cfg(test)]`
        module under `src/` to `tests/ui.rs`, so nextest's default target
        selection reaches it; explicit `--lib` reachability alone was not
        coverage. Four fixtures pin the matrix: an authored `if` through an
        identity macro, a source `while` with three branches, an authored
        guard inside `matches!`, and generated-only `assert!`/local-macro
        conditionals. Users-guide and suite-design paragraphs document the
        rule. Repair commit `d35ea17c` splits the feature-matrix score parser
        from its assertions (`panic_in_result_fn`) and replaces `expect` in
        the suite smoke harness with `let ... else` and interpolated panics.
  - [x] 2026-11-12: Ten-gate evidence at `d5a3aa63` (logs
        `/tmp/gate-{1..10}-*-issue-504.out`): `make check-fmt`, `make lint`
        (rustdoc, Clippy, interrogate, skill manifests, Whitaker suite),
        `NEXTEST_PROFILE=ci make test` (2552 passed, 7 skipped),
        `make test-doc`, `make typecheck`, `make markdownlint` (123 files),
        `make nixie`, `make test-workflow-contracts` (1003 passed) and
        `make spelling` all pass; `make verus-node-origin` reports 8 verified,
        0 errors. The branch was published with a lease-bound force push
        because the remote still held the pre-restack series.
  - [x] 2026-11-12: `bumpy_road_function` now takes provenance from the
        shared classifier and descends generated structure for authored
        descendants. `analyse_if_not_expanded` became `analyse_if_authored`,
        `analyse_body` and `push_segment` gate on `is_generated`, and
        `visit_expr` walks a generated node's children without recording a
        segment. A generated `match` is handled explicitly so an authored
        guard passed as a macro argument is still charged; a plain walk would
        visit it as an ordinary expression and lose the predicate. Three
        fixtures pin the matrix: two separated clusters inside a source `for`
        body, a `matches!` guard as the first cluster, and a generated-only
        function that must stay silent. Red baseline
        (`/tmp/issue-504-m4b-red.out`) showed all three silent; the green run
        (`/tmp/issue-504-m4b-ui-final.out`) shows nine fixtures passing, with
        `analysis_behaviour` (14) and `numeric_boundaries` (7) also green. No
        pre-existing `.stderr` changed other than the line anchors the
        inserted CodeScene directives shifted.
  - [x] 2026-11-12: Repaired the CodeScene advisory gate. The return-credit
        assertions were five near-identical functions, so they became one
        named `rstest` table. Function-level findings carry inline directives
        with reasons: the trait-mandated `check_fn` signature, and the fixture
        conditions that exist to be lint inputs. `Primitive Obsession` is a
        module-level signal that an inline directive cannot reach, so
        `.codescene/code-health-rules.json` disables it for exactly the two
        fixture files whose primitive parameters are the subject, with the
        reason recorded beside each rule set.
  - [x] 2026-11-12: Provisioned the host tool the parity test spawns. Hosted
        pull-request CI failed `macro_aware_cognitive_complexity::parity` with
        `error: no such command: dylint` in `coverage-check` and
        `windows-compat`: `tests/parity.rs` runs `cargo dylint`, but only
        `linux-full` installed it, and that lane runs no tests at all.
        `coverage-check` and `coverage-main`'s `coverage-upload` now run
        `make dylint-tools` and append the install root to `GITHUB_PATH`
        before `make coverage`. The download is the same digest-verified
        upstream archive `linux-full` caches, and `make fetch` already warms
        the driver it reuses offline. Upstream publishes no Windows archive
        for the 6.3.0 pin, so the test carries a reasoned
        `#[cfg_attr(windows, ignore = "...")]`: it stays compiled and counted
        as skipped there, and the Linux lanes keep the parity obligation.

## Surprises & discoveries

The existing RFC and roadmap are broader than this request. In particular, RFC
generated-closure traversal conflicts with the requested Clippy boundaries, and
the roadmap defers existing-lint adoption. This plan records the differences
rather than treating proposed documentation as implemented policy.

Pinned source inspection confirms Clippy adds guards outside the multi-arm
condition and conditionally subtracts returns. A source question mark therefore
needs raw-event assertions as well as a final-score assertion. `docs/design.md`
is absent, and the conditional driver has moved into `driver/mod.rs`.

Real lowering confirmed the chain direction and the mark shapes. `remove_mark`
pops the outermost mark, while `SyntaxContext::marks()` reports
innermost-first, so passing `marks()` straight to an outermost-first policy
would invert a future order-sensitive rule. Macro arguments carry no mark at
all, including arguments to a macro whose definition body is marked. A
desugared macro-body span can carry the same macro `ExpnId` twice, and a nested
macro chain can retain only the inner macro mark; origin must be a presence
test, never a count or depth test. `Root` never appears as a chain element.

The workspace's `self_named_module_files` Clippy policy requires
`mod.rs`-rooted directories, so the planned `common/src/node_origin.rs` plus
sibling test directory became `common/src/node_origin/mod.rs`; the HIR adapter
followed the same layout. RFC 0010 and the roadmap still show the proposed
paths and are reconciled in M3.

Verus rejected `by (compute)` on a datatype existential with
`vir/src/poly.rs:454:66: no entry found for key`, so the class witnesses are
written as explicit ground assertions instead. The Clippy snapshot lags upstream
`main` by about 155 days at this revision; that gap is recorded, not closed.

The crate's four UI harnesses did not all match the serialized Dylint lane. The
broad override's integration-binary term is `(binary(ui) & test(=ui))`, which
selects only the harness literally named `ui`; `ui_cy`, `ui_gd`, and
`ui_unknown_locale_falls_back_to_english` fell to the 300 s base and were
killed mid-build under `make test`. The in-process `#[serial]` attribute cannot
help: nextest runs each test in its own process, so each one takes a fresh
`serial_test` mutex and nothing is serialized. A package-scoped override,
`package(macro_aware_cognitive_complexity) & binary(ui)`, now covers every
harness in the crate, mirrors the broad lane's 30-minute allowance and Windows
retry policy, and is pinned by both the workflow contract and
`tests/nextest_ui_filter.rs`.

Pinned Clippy charges compiler desugarings. An early score table gave `awaited`
1, `try_option` 1, and `guarded_match` 2. Re-deriving from
`clippy_lints/src/cognitive_complexity.rs` at snapshot
`e4608a25c2e20116fbc1e97a51a73f094a1db7a1` shows `.await` lowers to a two-arm
poll match that the structural visitor charges (2), `?` on `Option` lowers to a
two-arm match plus a return and only a `Result` body credits returns in full
(2), and a guard adds one outside the multi-arm condition (3). The UI fixture
and the parity packet were corrected to the source, not the other way round;
`ui/threshold_zero.rs` records the `.await` deviation beside the function.

Pinned Clippy avoids a second diagnostic for `async fn` only by accident: its
closure anchor scans the wrapper's source span for the first `|` and returns
when none is found. Whitaker instead skips the wrapper explicitly by closure
kind. The diagnostic sets agree; the underline differs, because Whitaker
anchors at `fn_decl_span` while Clippy anchors at the first `|`. The parity
target therefore compares scores per anchor line, never columns.

`[lib] test = false` does not make an in-crate test harness unreachable. An
explicit `cargo test --lib` still builds the library test target and listed the
eleven UI tests; what the setting removes is default and nextest workspace
selection, which skip the target entirely. Moving the
`conditional_max_n_branches` harness to `tests/ui.rs` is therefore a coverage
fix rather than the removal of dead code. The `src/tests/behaviour.rs` module
stays in the same position and is still outside the default nextest selection;
that pre-existing gap is reported here, not repaired.

The vendored `dylint_testing` never sets compiletest's `bless` flag, so
`BLESS=1` is inert. Blessing copies each fixture's "Actual stderr saved to"
file by hand, and a single test must run per process while iterating: a
mismatch poisons compiletest's shared `MUTEX` and cascades into the remaining
UI tests. compiletest also defaults to edition 2015, which rejects `async`, so
the threshold fixture carries `// edition:2024`.

Adding the `cy` and `gd` resources broke the locale-parity test only in the
`--all-features` build, because `whitaker_common` had been compiled for that
feature set before the files existed. `static_loader!` reads `locales/` while
the macro expands, and Cargo watched only the files included by the previous
expansion, so the new files were invisible: the crate was reused, the bundles
lacked `macro_aware_cognitive_complexity`, and `fluent-templates`' `try_lookup`
resolved the text through the `en-GB` fallback with no error. Two guards now
close the trap: `common/build.rs` declares `cargo:rerun-if-changed=locales`, so
adding a resource rebuilds the bundle, and `locales_render_distinct_text` still
fails if a secondary locale ever renders English. The stale artefact was
cleared by touching `common/src/i18n/mod.rs`; no source defect was involved.

The spelling gate could only see the Gaelic resources after they were committed.
`typos` reads tracked files, so the `Tha` lines in the new `gd` resource,
snapshot, and blessed stderr passed `make markdownlint` while the files were
untracked and failed it once the milestone commit made them tracked. Five
line-anchored patterns in `typos.local.toml` now exempt exactly the emitted
Gaelic lines, following the existing per-diagnostic entries for the other lints'
`gd` text; `typos.toml` is regenerated from that file.

## Decision log

- 2026-11-08: The user clarified that Whitaker should track Clippy's nightly.
  Supersede the initial fixed-May constraint. Keep historical evidence
  labelled, coordinate safe adoption through #561, and distinguish #579's drift
  alarm from release safety and the alignment target. Revalidate
  compiler-dependent evidence after each adopted pin change.

- 2026-11-08: Treat the user's detailed four-task plan as controlling scope;
  retain RFC 0010 as prior design and record each material difference.
- 2026-11-08: Keep a pure shared classifier and thin compiler adapter, with no
  infrastructure types in domain code and no new trait facade.
- 2026-11-08: Select Verus only for the unbounded three-mark classification
  contract. Use properties, independent arithmetic oracles, and real compiler
  tests for the rest. Preserve the proof/runtime boundary explicitly.
- 2026-11-08: Preserve default body boundaries and skip generated bodies;
  document generated-closure loss rather than implementing RFC extensions.
- 2026-11-08: Accept ADR 0013's chain walk. Read marks outermost first with
  `SyntaxContext::remove_mark`, treat every desugaring as transparent, and let
  the first macro or AST-pass mark prove generation. Keep `ExpnKind` matching
  exhaustive and keep the policy rustc-free in `whitaker-common`.
- 2026-11-08: Model the policy with an existential sequence specification
  rather than by restating the loop, and record a production-to-model
  transition table plus mutation receipts as the correspondence evidence.
- 2026-11-09: At the `check_fn` boundary, skip when
  `span.from_expansion()` (rustc's predicate, matching pinned Clippy) as well
  as when `is_generated(anchor)`. `classify` treats desugaring marks as
  transparent so authored source constructs stay scored, but rustc synthesizes
  a coroutine wrapper closure for every `async fn` and `async` block and calls
  `check_fn` for it. Classifying that wrapper's anchor as authored would emit a
  second diagnostic for one source. Consequences: an `async fn` body is scored
  once through the explicit unwrap, and an `async` block's body is unscored
  exactly as under pinned Clippy. This boundary rule does not change
  expression-level provenance. Record it in the user-facing blind spots in M3.
  Evidence: pinned Clippy `cognitive_complexity.rs` at rustc
  `57d06900fd7d9ee06d3a7f323bb77f17ab3cfaf8` returns early on
  `body_span.from_expansion()`; `rustc_lint/src/late.rs` at the same pin uses
  `NestedFilter::All`, so `check_fn` runs for synthetic closure bodies.
- 2026-11-09: Give `macro_aware_cognitive_complexity` a package-scoped nextest
  override instead of widening the broad Dylint clause's
  `(binary(ui) & test(=ui))` term. The exact-name clause is load-bearing for
  other crates whose `ui` binary also declares unrelated tests, and
  `tests/nextest_ui_filter.rs` pins the clause text; a package-scoped clause
  covers this crate's four harnesses without reaching any other package. Place
  it after the broad clause so each settings resolution stays with the entry
  that already matches, and give both entries the same 30-minute allowance and
  retries so the split cannot produce two budgets.
- 2026-11-09: Split `tests/parity.rs` into `tests/support/mod.rs` and
  `tests/data/` even though the 400-line module budget exempts integration-test
  crate roots. The findings are right on two independent grounds: `AGENTS.md`
  requires large test data to live in external files, and the M2d feature
  matrix reuses the same nested-build plumbing. Keeping the pinned fixture
  source as `include_str!` data also keeps its byte-exact line anchors visible
  to a reviewer as a fixture rather than as an inline string literal.
  `pinned_toolchain` becomes fallible rather than returning a sentinel: a
  sentinel would stage the Dylint library under a name Dylint never searches
  for, turning a toolchain-resolution fault into a misleading "lint not found"
  result.
- 2026-11-09: V4 asks for raw charges as well as net scores on
  return-producing desugarings. The diagnostic exposes only the net score and
  no event list crosses the driver boundary. The raw-charge assertion is
  therefore discharged by the policy suite (V3), which feeds the exact
  authored/generated sequences a lowering produces, and by fixture pairs whose
  scores pin both charges where the arithmetic leaves them observable: for
  example `authored_argument_try` scores 1, reachable only if the two-arm poll
  match is charged and its single return credited, whereas a lone halved return
  in a non-`Result` body such as `authored_argument_await` is invisible in the
  net score. Adding an environment-variable or public seam to dump events would
  put test-only surface in the production driver, which the plan's tolerances
  do not authorize; the gap and its compensating evidence are recorded here
  rather than closed silently.
- 2026-11-09: Normalize an unknown UI or user locale to en-GB and render the
  English text, rather than consulting `FALLBACK_LOCALE` for alternative
  wording. The message fields substitute identically in every locale, and the
  `zz` UI run and snapshot pin byte-identical output to en-GB.
- 2026-11-09: Inspect `hir_attrs` directly for the `#[test]` exclusion because
  `TyCtxt::has_attr` is unavailable at the pinned toolchain.
  `rustc_attr_data_structures` would have to be linked for the alternative,
  which the crate does not need.
- 2026-11-09: Exclude `macro_aware_cognitive_complexity` from the
  `--all-features` doctest lane and keep its three policy examples
  compiler-free. A doctest is compiled as its own crate and cannot load a
  library built with `rustc_private`; verified in both directions
  (`--all-features`: 0 passed, 3 failed; default features: 3 passed). Every
  other lint crate that links the compiler is excluded for the same reason. The
  exclusion alone left the examples unexecuted, so `make test-doc` now follows
  the workspace lane with a default-feature `cargo test --doc` for this crate;
  the three examples pass there.
- 2026-11-09: Expose each UI fixture's items through a `pub mod fixture`
  instead of silencing dead-code warnings crate-wide. The repository keeps
  suppressions for the last resort, and `bumpy_road_function` already shows
  that publicly reachable fixture items produce no `dead_code` diagnostic under
  the UI harness. Re-blessing the four affected `.stderr` files from the
  compiler's captured "Actual stderr saved to" output keeps the positions
  honest; `nested` items that cannot be `pub` are called instead of suppressed.
  No `#[allow]` remains in the crate's fixtures.
- 2026-11-09: Exempt the Gaelic `Tha` lines with full-line patterns in
  `typos.local.toml` rather than an accepted word. `Tha` is the Gaelic copula,
  but a bare accepted word would silence the same token in unrelated prose; the
  five patterns cover only the resource comment, the message, the snapshot
  line, and the two blessed diagnostics. `typos.toml` stays generated.
- 2026-11-09: Have `common/build.rs` declare `cargo:rerun-if-changed=locales`
  so the embedded Fluent bundle is rebuilt when a resource file is added.
  Nothing in Cargo's dependency tracking notices a new file inside a directory
  the macro scanned earlier, and the resulting stale bundle resolves missing
  keys through `en-GB` without reporting an error. The gate failure that
  exposed this was environmental, but the trap is inherent to `static_loader!`
  and would otherwise recur for every later locale addition.
- 2026-11-09: Rebase with Git's built-in merge under `zdiff3` and
  `core.attributesFile=/dev/null`. The host installs a user-wide
  `*.rs merge=weave` attribute, but the repository does not opt in through
  tracked `.gitattributes`, so ambient selection is not repository consent for
  an unattended nine-commit replay. The replay was conflict-free; only
  `Cargo.lock`, `Makefile`, and `docs/developers-guide.md` were touched by both
  sides, and the `Makefile` and guide hunks do not overlap. Keep main's
  dependency bumps in the lockfile and regenerate it with Cargo rather than
  hand-merging, and align the new crate to `0.2.12` so every lint crate stays
  on the release train main just moved to.
- 2026-11-09: Give the parity test its own nextest allowance instead of
  folding it into the package's UI harness filter. The test is not a UI
  harness: it builds the library once, then invokes `cargo clippy` and
  `cargo dylint` in a generated fixture, so it belongs to the nested-build
  serial lane. Selecting it by test name rather than by the package binary
  leaves the UI lane's retry policy unchanged, and `retries = 0` avoids
  repeating three builds after a genuine failure. The allowance is restated in
  both profiles because the profile contract compares them per profile.
- 2026-11-10: Discharge V5 with two generated standalone projects rather than
  two feature builds of one fixture. A single project cannot select `tracing`'s
  `log` feature conditionally while proving the resolved graphs differ, and the
  workspace's own lockfile would leak unrelated dependency features into the
  measurement. Each project pins the exact `tracing` and `log` versions read
  from the workspace `Cargo.lock` at run time, so a dependency bump cannot
  silently change what the matrix measures, and `cargo tree -e features`
  supplies the graph evidence instead of a manifest assertion. The normalized
  diagnostics keep the span, score, text, and counts byte-comparable and
  replace only the generated project root; normalizing anything else would let
  a real difference pass as equality.
- 2026-11-10: Keep the feature matrix out of the default suite with
  `#[ignore]` and give it its own `serial-feature-matrix` test group instead of
  reusing `serial-nested-cargo-builds`. The test resolves two registry
  dependency graphs before it builds anything, which the parity test does not,
  and two long attempts would double the cost after a genuine failure; the
  group keeps it off the nested-build lanes while `serial_test::serial` stops a
  direct `cargo test` from racing the UI harnesses. Because the test is
  ignored, milestone acceptance runs it explicitly with `--run-ignored only`,
  which is also required for `nextest list` and `show-config` to report its
  override at all.
- 2026-11-10: Retain the matrix's captured streams under
  `target/tests/feature_matrix/receipts/<mode>/` rather than in `/tmp`. A
  hard-coded `/tmp` path is not portable and a fixed receipt name lets two
  worktrees running the same test overwrite each other's evidence; the
  workspace-relative path is per-checkout, survives the test, and stays inside
  the existing ignored `target` tree. The fixture copy remains beside the
  generated project for the duration of the run.

- 2026-11-12: Migrate `bumpy_road_function` by descending generated nodes
  rather than pruning them. A generated wrapper records no segment of its own,
  so the only difference from the previous behaviour is that authored
  descendants inside a macro argument or a lowered loop body become visible. A
  generated `match` needs an explicit child walk: `walk_expr` treats an arm
  guard as an ordinary expression, and `push_segment` then discards it as
  generated, so an authored `matches!` guard would still be invisible. Walking
  the scrutinee, charging each guard through `push_predicate_segment`, and
  visiting each arm body matches the authored `visit_match_arm` path exactly
  once per node. The default nested filter still keeps closure bodies out,
  which preserves the `include_closures` boundary.
- 2026-11-12: Answer the CodeScene advisory gate by rule scope. Function-level
  findings take an inline directive naming the reason, following the existing
  `depth.rs` and UI-fixture precedent. `Primitive Obsession` is reported per
  module and an inline directive cannot suppress it, so a local
  `.codescene/code-health-rules.json` disables only that rule for only the two
  fixture files that exist to feed the lints, with `usage` and
  `matching_content_path_doc` recording why. Widening the glob to every `ui/` or
  `tests/data/` file would hide future findings in fixtures that do not need
  the exception.
- 2026-11-12: Keep the return-credit assertions as one named `rstest` table.
  Five functions asserted the same `Case` shape with different literals, which
  is exactly the duplication `AGENTS.md` asks to replace with parameterised
  cases. `#[case::...]` names keep each clause identifiable in test output, so
  the table loses no diagnostic value while the shared assertion exists once.
- 2026-11-12: Run the parity test in every lane that runs the suite instead of
  ignoring it. `cargo dylint` was the one suite prerequisite no test lane
  installed; marking the test `#[ignore]`, as the feature matrix is, would have
  hidden a core RFC 0010 claim from every pull request. The Linux suite lanes
  install the pinned host tools and export the root on `PATH`, which is the
  only way Cargo discovers a subcommand. Windows cannot install them: upstream
  publishes no Windows archive for the 6.3.0 pin, and this repository does not
  rebuild a published tool. A reasoned `ignore` keeps the test compiled and
  visible in the skip count there, with the Linux lanes enforcing parity.

## Outcomes & retrospective

The Wyvern reconnaissance and six-lens expert review are complete. Review
corrections distinguish Cargo package/import names, keep policy dependencies
ungated, enable library tests explicitly, make proof mutations independent, and
isolate matrix library staging. The initial documentation gates passed and
draft PR #574 was published; the toolchain clarification was gated and
published in follow-up commits.

M1 is implemented, gated, and committed as `93cc4f2c`. The shared facility
consists of the compiler-free policy in `common/src/node_origin/mod.rs`, the
adapter in `crates/whitaker_lint_core/src/hir/node_origin/mod.rs`, shared
hygiene test support, the Verus sidecar `verus/node_origin.rs`, the
`make verus-node-origin` target, ADR 0013, and the reuse and proof-boundary
text in the developer's guide. Focused results: 16 policy tests pass, 16
adapter tests pass, and the proof reports 8 verified, 0 errors. The
real-lowering probe covered source, macro-argument, macro-body, mixed-`when!`,
desugaring, and nested-macro cases. Four injected-rule mutations failed as
required. The scrutineer recorded a clean `make lint`, `make test` (2324 passed
plus 19 `dylint_testing` tests), Markdown lint, Mermaid validation, the Verus
proof, and `make check-fmt`. The milestone CodeRabbit CLI review raised two
trivial documentation findings and no code findings.

M2b is implemented and gated. The crate now owns a rustc driver behind the
`dylint-driver` feature, configuration, en-GB/cy/gd diagnostics, a 43-case
threshold-zero fixture, four localized UI harnesses, the package-scoped nextest
allowance, and the `common/build.rs` locale watcher. The hand-derived oracle
table and every blessed diagnostic agree, and the locale-parity test
distinguishes the three translations rather than accepting silent English
fallback. Focused results: 33 no-default-feature policy tests, 46
`--all-features` library tests, and 4 UI harnesses. The scrutineer recorded a
clean `make check-fmt`, `make lint`, `make test` (2394 passed, 6 skipped, plus
74 `-p whitaker --lib` and 48 `nextest_ui_filter`), `make test-doc`,
`make markdownlint` (117 files, 0 errors after the `artifact`/`artefact`
correction), `make nixie`, `make test-workflow-contracts` (966 passed),
`make verus-node-origin` (8 verified, 0 errors), and the four UI harnesses. The
milestone CodeRabbit CLI review raised seven valid findings: four crate-wide
dead-code suppressions, two `-ise` spellings, and policy doctests left
unexecuted by the `DOCTEST_EXCLUDES` change. The fixtures now use a
`pub mod fixture` and were re-blessed from captured compiler output, the
comments use Oxford `-ize`, and `make test-doc` runs the three policy examples
under default features; the four UI harnesses and the doctest pass were
re-verified after the repair. No lint runtime acceptance exists yet; M2c, M2d,
M3, and M4 remain. Update this section after each approved milestone and retain
both remaining gaps and the evidence that discharged earlier obligations.

## References and implementation reading

Implementation reading:

- [Suite design](../whitaker-dylint-suite-design.md).
- [Developer's guide](../developers-guide.md).
- [User's guide](../users-guide.md).
- [Documentation style guide](../documentation-style-guide.md).
- [Rstest fixtures](../rust-testing-with-rstest-fixtures.md).
- [Dependency injection](../reliable-testing-in-rust-via-dependency-injection.md).
- [Doctests](../rust-doctest-dry-guide.md).
- [Complexity/refactoring](../complexity-antipatterns-and-refactoring-strategies.md).
- [Rstest-bdd](../rstest-bdd-users-guide.md).

Relevant skills are `execplans`, `codegraph-mcp`, `rust-router`,
`hexagonal-architecture`, `rust-verification`, `verus`, `firecrawl-mcp`,
`logisphere-experts`, `pr-creation`, and `commit-message`. Use `leta` and
CodeGraph for semantic navigation; exchange code references through
`context_pack`. Planning evidence packs are `pk_4m6ifjij` and `pk_vpm46u3g`;
this document is self-contained and does not require those temporary packs.

Pinned primary sources retrieved on 2026-11-08 are
[Clippy arithmetic](https://github.com/rust-lang/rust/blob/57d06900f/src/tools/clippy/clippy_lints/src/cognitive_complexity.rs)
and
[rustc hygiene](https://github.com/rust-lang/rust/blob/57d06900f/compiler/rustc_span/src/hygiene.rs).
The
[Clippy rationale issue](https://github.com/rust-lang/rust-clippy/issues/3793)
is background, not evidence that this proxy measures understanding.
