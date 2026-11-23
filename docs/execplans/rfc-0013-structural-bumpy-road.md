# Implement RFC 0013: structural Bumpy Road detection

This execution plan (ExecPlan) is a living document. The sections Constraints,
Tolerances, Risks, Progress, Surprises & Discoveries, Decision Log, and
Outcomes & Retrospective must be kept up to date as work proceeds.

Status: DRAFT (awaiting the lead's go)

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

`bumpy_road_function` detects sustained peaks in a weighted per-line signal.
CodeScene's Bumpy Road diagnostic, which ADR 0011 names as the reference,
counts separate regions of nested conditional logic. The two disagree: a
function with two `loop { ... if cond { break } ... }` blocks scores
`bumps = 2` in CodeScene and is silent in Whitaker (issue
[#640](https://github.com/leynos/whitaker/issues/640)).

After this work a user may write `strategy = "structural"` under
`[bumpy_road_function]` in `dylint.toml`. The lint then reports a function
once, with two labelled spans, when it contains two or more separate structural
regions, whatever the weighted signal says. The default stays `weighted`. A
recorded differential corpus shows, per sample, what CodeScene said, what each
strategy says, and every mismatch. Changing the default is a later release
decision for the maintainer, not part of this work.

Observable success: in structural mode the two-loop fixture and its `?`-free
control each report once with two labelled regions; each one-region reduction
is silent; the weighted UI corpus and boundary tests pass unchanged.

## Conformance basis

No Terms of Reference or technical-design revision exists for this lint. The
governing artefacts are:

- Issue #640 (specification and acceptance list).
- RFC 0013, `docs/rfcs/rfc-0013-bumpy-road-semantic-alignment.md`: delivery
  order (steps 1 to 4) and acceptance matrix (Table 2).
- ADR 0011, `docs/adrs/adr-0011-bumpy-road-reference-semantics.md`: CodeScene
  is the reference; "subject to review" is resolved by this work.
- ADR 0016, "`?` is a branch" (branch `qmark-is-a-branch`, owned by row
  jm5-qmark-branch; not on `main` at planning time).

Identifiers: `EP-M1` corpus, `EP-M2` classifier, `EP-M3` configuration and
wiring, `EP-M4` differential run and documentation. Each maps to RFC 0013 steps
1 to 4 and to the acceptance bullets of #640.

## Constraints

- Delivery order follows RFC 0013 steps 1 to 4.
- The structural representation lives in `crates/bumpy_road_function` only
  (RFC: "only this lint's driver and its tests may construct/consume the new
  representation"). No new shared crate, trait or I/O port.
- The default strategy stays `weighted`. The existing weighted UI corpus,
  `tests/numeric_boundaries.rs` and weighted behaviour are unchanged.
- CodeScene is never called from committed tests or evaluations. Reference
  observations are refreshed by hand with the version recorded.
- No source-hash or exact-text contract tests: a reformat must not break the
  corpus tests. The corpus files are the source bytes.
- No process-wide environment mutation in tests.
- Rebase on the `?` work (ADR 0016) once it merges. Do not edit that work.
- Each file at most 400 lines; every module has a `//!` comment; every
  function has a `///` summary; assertions carry messages; new fixtures
  declared with item-scoped `expect` as `AGENTS.md` requires.
- Gate for every commit: `make check-fmt`, `make lint`, `make test`,
  `make markdownlint` (and `make nixie` if diagrams change), run one at a time.
- Whitaker builds on its pinned nightly.

## Tolerances (exception triggers)

- Scope: more than 3 PRs, or any change outside `crates/bumpy_road_function`,
  its locale files, `docs/` and `dylint.toml` fixtures, stops for the lead.
- Interface: a new public Rust item, a new crate dependency, or a change to
  `whitaker-common` stops for the lead.
- Corpus agreement: if the classifier needs more than three rule revisions to
  match CodeScene on the RFC's seven named samples, stop and report; do not
  tune toward fixtures.
- Config: if rejecting weighted-only keys requires changing behaviour of the
  weighted strategy for existing configurations, stop.
- Reference drift: if CodeScene 1.0.36 and 1.0.58 disagree on any corpus row,
  record both and ask the lead which governs that row.
- Size: any single PR over about 600 changed lines (excluding fixtures and
  snapshots) stops for a split.

## Risks

- Risk: CodeScene's counting rules are unpublished; the RFC rules are
  hypotheses. Mitigation: the corpus is the oracle; every mismatch is a
  documented revision or unresolved case (step 4).
- Risk: HIR lowers `if let`, `while let`, `for`, `?` and `let ... else` into
  `match`/`loop` scaffolding. Mitigation: collect through the same
  `is_generated` / `DesugaringKind` / `LoopSource` / `MatchSource` tests the
  `SegmentBuilder` already uses, and add one unit fixture per lowering.
- Risk: the `?` ruling is pending (below). Mitigation: structural mode treats
  `?` as a branch with no authored body (never a region); the corpus has the
  control and `through_question` rows to prove it, so either ruling can be
  measured.
- Risk: the config loader today logs and defaults on parse errors. A blanket
  switch to hard errors would break existing weighted configurations that carry
  stale keys. Mitigation: see Configuration design; hard errors only once the
  user has opted into `strategy`.
- Risk: UI test fixtures are analysed by the repository's CodeScene gate.
  Mitigation: each fixture carries a function-scoped `@codescene(disable:...)`
  directive with a dated rationale, as the existing fixtures do.

## Corpus (EP-M1)

Location: `crates/bumpy_road_function/corpus/`, one `.rs` file per case plus
`corpus/reference.toml`. Cases are plain Rust files that compile as UI fixtures
(a `fn main` calls each function), so the same bytes feed the UI harness and
the CodeScene review. The harness copies them to a temp directory exactly as
`prepare_fixture` does today.

`reference.toml` records, per case: file name, the functions under test, the
CodeScene version and build per observation, rule configuration (`none`:
`cs check-rules` reports no matching rule), the CodeScene result (`clean` or
`bumpy` with `bumps = n`, plus the score), and the expected structural region
count with the expected Whitaker outcome for each strategy. A loader test
parses the manifest and asserts every `.rs` file has an entry and every entry
has a file (so a case cannot be silently dropped). Nothing hashes file contents.

Cases (named by the issue; the first five come from the scratchpad samples
`qm/` and the RFC fixture, copied byte for byte):

| Case                                           | Source                                  | CodeScene (1.0.36)                  | Regions           |
| ---------------------------------------------- | --------------------------------------- | ----------------------------------- | ----------------- |
| `relative_to_manifest`                         | `ui/pass_two_shallow_clusters.rs`       | bumpy, `bumps = 2` (1.0.52 per RFC) | 2                 |
| reductions: no match / no inner `if` / neither | three temp variants in RFC              | clean (per RFC)                     | 1, 1, 0           |
| `two_loops_with_try`                           | `fail_question_mark_read_loop.rs`       | bumpy, `bumps = 2`, 9.84            | 2                 |
| `two_loops_no_try` (control)                   | `control_no_question_marks.rs`          | bumpy, `bumps = 2`, 9.84            | 2                 |
| `one_loop_unsplit`                             | `streams_pre.rs` (pre-split cuprum)     | clean, 10.0                         | 1                 |
| `question_mark_read_loop_split`                | `pass_question_mark_read_loop_split.rs` | clean, 10.0                         | 0 or 1 (measured) |
| `flat_try_chain_12`                            | `pass_flat_try_chain.rs`                | clean, 10.0                         | 0                 |
| `through_question` / `through_match`           | `fail_try_versus_match.rs`              | clean / bumpy `bumps = 2`           | 0 / 2             |

Syntax matrix (new, one small function pair per construct, each as a "nested
pair" that should be bumpy and a "flat" twin that should be clean, so the
oracle verdict is explicit): `if` in `if`; `if let`; `while`; `while let`;
`for`; match-arm conditional; guard (`if` guard on an arm, flat); let-chain
(`if let ... && let ...`, edition 2024); `let ... else` (flat, and with nested
work in the `else` block); `else if` chain (one family); closure with nested
regions inside a function (must not attribute to the enclosing function;
`include_closures` both ways); and a macro-argument case. The reference column
for each is filled by the lead running `cs review` on
`/data/leynos/Projects/whitaker.scratch/experiments/jm5-rfc0013/cs-queue/` (no
CodeScene token on this row). Rows where the RFC leaves the answer open
(multiple nested arms in one match; let-chain forms) are exactly the RFC's open
questions, and get measured before any rule is written for them.

### Which CodeScene version governs

Recommendation: 1.0.58 governs. It is the current release and the one a fresh
install gets; the RFC's own reproduction used 1.0.52, so neither local version
is the original. Run the whole corpus on both 1.0.36 (local) and 1.0.58 and
record both columns in `reference.toml`. Where they agree (expected for every
row so far), nothing more is said. Where they differ, 1.0.58 governs and the
1.0.36 observation is kept as a note. The corpus header states: "refreshed by
hand; never invoked from tests". Asking the lead to run both is one extra `cs`
invocation per file.

## Structural representation (EP-M2)

New module `crates/bumpy_road_function/src/structure.rs` (compiler-free,
compiled without the `dylint-driver` feature, like `analysis.rs`, so unit and
property tests run without `rustc_private`).

```rust
pub(crate) enum ConstructKind { If, ElseIf, Loop, Match, ... }
pub(crate) struct Construct { kind, parent: Option<ConstructId>, order: u32, span_id: SpanId }
pub(crate) fn classify_regions(constructs: &[Construct]) -> Vec<Region>
```

`ConstructId` and `SpanId` are newtypes over `u32`; concrete `rustc_span` spans
stay in the compiler adapter (`driver/structure_collector.rs`, a `Visitor` that
mirrors `SegmentBuilder`'s authored-versus-generated tests and returns the flat
`Vec<Construct>` plus a `SpanId -> Span` table). `Region` carries the outermost
construct id, depth reached, and the line size, from which the diagnostic picks
its two labelled regions. Rules, as RFC 0013:

1. A region exists where a conditional or loop is nested inside another
   authored conditional or loop. The maximal connected nested chain under one
   outer construct is one region.
2. Separate outer constructs in statement order are separate regions.
   Plain blocks add no depth.
3. An `else if` chain is one family. Flat guards and flat match arms do not
   create a region.
4. `if let`, `while let`, `for` are recognized as authored kinds; generated
   match and loop scaffolding is not counted.
5. `let ... else` alone is a guard (no region); nested work in its blocks is
   classified separately.
6. `?` is a branch with no authored body, so never forms a region (pending
   ruling).
7. Macro-expansion policy unchanged. Regions inside a closure body count
   towards the enclosing function (revised 2026-11-16 to match CodeScene);
   `include_closures` additionally analyses closures as their own targets.

Rule revisions arising from the corpus are Decision-log entries.

Diagnostic: a function with two or more regions reports once, with labels on
the two largest regions (reusing the existing diagnostic builder and locale
keys; new Fluent keys only if the structural wording must differ, in all three
locale files). The diagnostic names its strategy.

Tests (pure, `rstest`): two disjoint nested regions report; one connected deep
chain does not; flat siblings and flat guards do not; ordered region identities
exact. `proptest` metamorphic tests: reflow, blank lines, comments preserve
classification (applied to the pure layer by varying the line numbers that
`Construct` carries); extracting a region removes the finding.

## Configuration design (EP-M3)

`Strategy` enum (`Weighted`, `Structural`),
`#[serde(rename_all = "lowercase")]`, default `Weighted`. Config layout:

```toml
[bumpy_road_function]
strategy = "structural"     # "weighted" (default) | "structural"
include_closures = true     # common to both
# threshold, window, min_bump_lines, weights: weighted only
```

Loader change. Today `load_configuration` deserializes into a `Config` full of
defaults, so an explicit `threshold` is indistinguishable from its default, and
any error is debug-logged and defaulted. Change:

1. Read the raw table with `dylint_linting::config_toml(LINT_NAME)`.
2. Parse `strategy` (absent means `weighted`; unknown name is an error).
3. For `weighted`, deserialize exactly as today (so every existing weighted
   configuration, including today's log-and-default behaviour on a bad weighted
   key, is unchanged).
4. For `structural`, reject any of `threshold`, `window`, `min_bump_lines`,
   `weights` present in the table, and deserialize `include_closures`.
5. Errors become a `ConfigError` returned from a pure
   `resolve(raw: Option<toml::Value>) -> Result<ResolvedConfig, ConfigError>`
   (testable without rustc); `check_crate` reports it with
   `cx.tcx.sess.dcx().err(...)` naming the offending key and the fix ("remove
   `threshold` or set `strategy = \"weighted\"`"). No silent fallback once the
   user has written `strategy`. Interaction: a bad `strategy` value is an error
   even though the rest of the loader stays lenient for weighted.
6. `analyse_body` dispatches on `Strategy`; weighted code path untouched.

Config tests (`rstest`): absent strategy preserves weighted defaults; both
explicit strategies dispatch; invalid name and each incompatible key produce
the exact actionable message. Behavioural (`rstest-bdd`): a user selects each
strategy in `dylint.toml` and receives the matching diagnostic and labels
(extends `tests/features/bumpy_road.feature`).

## PR split

1. **PR A, corpus (EP-M1).** Corpus files, `reference.toml`, manifest-loader
   test, users'-guide/RFC note that the corpus exists. No lint behaviour
   change. The structural expectations are present but unused, and the weighted
   outcomes are asserted through the existing UI harness (so the weighted
   column is verified; the structural column is the target).
2. **PR B, classifier (EP-M2).** `structure.rs`, collector, pure tests, plus
   a hidden test-only entry point that runs the structural classifier over the
   corpus UI fixtures, still unreachable from `dylint.toml`.
3. **PR C, config and wiring (EP-M3).** `strategy`, loader/`resolve`, error
   reporting, dispatch, structural diagnostics wording, UI tests with
   `dylint.toml` selecting structural, BDD scenarios, users' and developers'
   guides, RFC status, roadmap line, ADR 0011 "subject to review" resolved.
4. **Step 4 (EP-M4):** differential run and the agreement table go in a
   documentation PR D (or the tail of PR C if small): rows, both strategies,
   CodeScene, mismatches documented as revised rule or unresolved case.

Each PR: full gate, one CodeRabbit question, merge with `--match-head-commit`
only after the clearing answer.

## The `?` question for the maintainer

ADR 0016 rules that `?` is a branch (option C: charged as the written
`if`/early-return it abbreviates) for the weighted strategy. CodeScene does not
make `?` a nesting level: a `?` inside a loop forms no region (the control case,
`bumps = 2` with every `?` removed, is identical to the sample with them).
Recommendation: keep `?` a branch in both strategies, but give it no authored
body, so in structural mode it can never be the inner half of a region. That
reading is consistent with ADR 0016 and with every CodeScene row measured so far
(`through_question` clean, `flat_try_chain_12` clean, two-loop pair
identical). Record it as an addendum to ADR 0016 (or a superseding ADR) before
PR C relies on it. The corpus rows `through_question`, `through_match` and the
two-loop pair are the evidence. The question for the user: approve the
addendum, or want `?` to count structurally (in which case the corpus will show
the disagreement with CodeScene).

## Progress

- [x] 2026-11-16: Read issue #640, RFC 0013, ADR 0011, scratchpad samples and
  CodeScene results; created worktree
  `/data/leynos/Projects/whitaker.worktrees/rfc-0013-structural-bumpy-road`.
- [x] 2026-11-16 21:25 UTC: Lead's go for PR A; PR B after A merges; PR C
  waits for the user's `?` ruling. SHA-256 recorded but not asserted.
- [x] 2026-11-16: CodeScene 1.0.36 observations received (1.0.58 not
  installed; updating `cs` would rewrite the user's binary). 1.0.36 governs
  until the user decides otherwise.
- [ ] EP-M1 corpus PR (files, manifest, manifest test and weighted UI test
  written; gate evidence below; `make lint-whitaker` blocked, see Surprises).
- [ ] EP-M2 classifier PR.
- [ ] EP-M3 config and wiring PR.
- [ ] EP-M4 differential run and report.

## Surprises & Discoveries

- The primary checkout (`/data/leynos/Projects/whitaker`) is stale and lacks
  `driver/` modules and the RFC; work is based on `origin/HEAD` (40a85aa5).
- The ADR 0016 branch is unpushed, so `try_desugar.rs` may change under this
  work; PR B is the first to touch the collector and must rebase on it.
- Weighted today flags a flat `else if` family (`flat_else_if`), a false
  positive against CodeScene, and misses every plain nested pair except guard
  and `else if` shapes. Both are in the corpus manifest.
- CodeScene charges regions inside closure bodies to the enclosing function,
  contradicting RFC 0013 rule 7. Lead ruled 2026-11-16: align with CodeScene;
  rule 7 revised in the RFC and plan, corpus expects 2 regions.
- `ifs in two arms of one match` is one region (CodeScene is clean), and a
  `let ... else` with an `if` in its else block is not a region.
- `make lint-whitaker` is blocked by shared state: the shared bootstrap file
  `~/.local/share/whitaker-bootstrap/bootstrap.json` was repointed at 21:24 UTC
  to another row's scratch suite (the unmerged `?`-is-a-branch build), which
  flags `installer/src/scanner.rs`, code this work does not touch. Clippy,
  rustdoc, `make test`, `make check-fmt` and `make markdownlint` pass.

## Decision Log

- 2026-11-16: Propose CodeScene 1.0.58 as governing, with 1.0.36 recorded.
  Superseded the same day: only 1.0.36 is installed, so it governs until the
  user approves `cs update`.
- 2026-11-16: Propose representation in `bumpy_road_function/src/structure.rs`
  only; no `whitaker-common` change.
- 2026-11-16: Propose that only a user-written `strategy` turns config errors
  into hard errors; the weighted path keeps today's lenient loader.
- 2026-11-16: No source hashes in the corpus (a reformat must not break it).

## Outcomes & Retrospective

To be completed.
