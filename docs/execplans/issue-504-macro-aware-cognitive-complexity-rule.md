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
origin classifier, its adapter, the Verus proof, and ADR 0011; the standalone
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
[ADR 0011](../adrs/adr-0011-node-origin-provenance-classifier.md) records the
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
`crates/whitaker_lint_core/src/hir/node_origin/mod.rs`, exported behind `dylint-driver`, with
`node_origin(span: Span) -> NodeOrigin` and `is_generated(span: Span) -> bool`.
An internal context-taking helper may support tests without duplicating policy.
Root returns Authored immediately.

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
cargo test -p whitaker --features dylint-driver --lib node_origin
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
- [ ] M2: standalone lint, parity, localization, and logging matrix.
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
        harnesses. The milestone CodeRabbit CLI review follows the commit.
  - [ ] M2c: macro-free differential parity against pinned Clippy.
  - [ ] M2d: logging feature-graph matrix and negative control.
- [ ] M3: experimental suite/installer registration and documentation.
- [ ] M4: separately gated adoption by both existing lints.

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
  other lint crate that links the compiler is excluded for the same reason.
- 2026-11-09: Have `common/build.rs` declare `cargo:rerun-if-changed=locales`
  so the embedded Fluent bundle is rebuilt when a resource file is added.
  Nothing in Cargo's dependency tracking notices a new file inside a directory
  the macro scanned earlier, and the resulting stale bundle resolves missing
  keys through `en-GB` without reporting an error. The gate failure that
  exposed this was environmental, but the trap is inherent to `static_loader!`
  and would otherwise recur for every later locale addition.

## Outcomes & retrospective

The Wyvern reconnaissance and six-lens expert review are complete. Review
corrections distinguish Cargo package/import names, keep policy dependencies
ungated, enable library tests explicitly, make proof mutations independent, and
isolate matrix library staging. The initial documentation gates passed and
draft PR #574 was published; the toolchain clarification was gated and
published in follow-up commits.

M1 is implemented, gated, and committed as `93cc4f2c`. The shared facility
consists of the compiler-free policy in `common/src/node_origin/mod.rs`, the
adapter in `crates/whitaker_lint_core/src/hir/node_origin/mod.rs`, shared hygiene test support, the Verus
sidecar `verus/node_origin.rs`, the `make verus-node-origin` target, ADR 0013,
and the reuse and proof-boundary text in the developer's guide. Focused
results: 16 policy tests pass, 16 adapter tests pass, and the proof reports 8
verified, 0 errors. The real-lowering probe covered source, macro-argument,
macro-body, mixed-`when!`, desugaring, and nested-macro cases. Four
injected-rule mutations failed as required. The scrutineer recorded a clean
`make lint`, `make test` (2324 passed plus 19 `dylint_testing` tests), Markdown
lint, Mermaid validation, the Verus proof, and `make check-fmt`. The milestone
CodeRabbit CLI review raised two trivial documentation findings and no code
findings.

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
`make verus-node-origin` (8 verified, 0 errors), and the four UI harnesses. No
lint runtime acceptance exists yet; M2c, M2d, M3, and M4 remain. Update this
section after each approved milestone and retain both remaining gaps and the
evidence that discharged earlier obligations.

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
