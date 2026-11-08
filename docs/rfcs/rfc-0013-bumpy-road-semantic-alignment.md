# RFC 0013: Bumpy Road semantic alignment

## Preamble

- RFC number: 0013.
- Status: Proposed.
- Created: 2026-11-08.
- Issue: [Whitaker #357](https://github.com/leynos/whitaker/issues/357).
- Governing decision:
  [ADR 0011](../adrs/adr-0011-bumpy-road-reference-semantics.md).

## Summary

Semantic alignment with CodeScene's Bumpy Road diagnostic is a Whitaker project
goal. CodeScene is the reference implementation for observable classification,
not merely inspiration for the lint's name. The current `bumpy_road_function`
implementation detects sustained peaks in a weighted line signal. That is a
different rule from recognizing separate regions of nested conditional logic.

This RFC proposes a structural detector alongside the existing weighted
strategy, with a differential Rust corpus governing its eventual adoption as
the default. This investigation adds a regression fixture and documents the
gap; it does not change production detection or introduce a configuration key.

## Reference evidence

CodeScene describes the smell in terms of multiple regions of nested logic.
Depth, number of regions and region size affect severity.[^1] Its published
Code Health guide describes the encapsulation problem, but does not specify a
complete Rust algorithm or numeric Bumpy Road thresholds.[^2] Those sources
cannot establish how every Rust construct is classified. Reference behaviour
must therefore be measured with a pinned tool version and configuration.

The report originated in `TrackedFeaturePath::relative_to_manifest` in
[rstest-bdd PR #657](https://github.com/leynos/rstest-bdd/pull/657). The
[historical source at `482b0f0d`](https://github.com/leynos/rstest-bdd/blob/482b0f0dda4f754456005b778446910f730e642c/crates/rstest-bdd-macros/src/codegen/tracking/mod.rs)
contains prefix validation inside a `match`, an `if` inside a `while let`, and
a later `let ... else`. The PR head inspected during this investigation,
`891ac15eb2c69e3aa269af72571f0b353d3de7a6`, has already extracted the root
check. Reproductions must use the historical shape, not the mutable PR head. The
[historical `dylint.toml`](https://github.com/leynos/rstest-bdd/blob/482b0f0dda4f754456005b778446910f730e642c/dylint.toml)
has no `[bumpy_road_function]` override.

### Reproduction

The executable fixture is
[`pass_two_shallow_clusters.rs`](../../crates/bumpy_road_function/ui/pass_two_shallow_clusters.rs).
It retains the historical control flow, replaces the application error and
wrapper with `Result<String, ()>`, and compares component text directly. It
exercises syntax rather than filesystem behaviour; Windows prefix arms remain
present in the Rust syntax even on Linux. Its adjacent
[configuration](../../crates/bumpy_road_function/ui/pass_two_shallow_clusters.dylint.toml)
contains no lint table, so the per-fixture override tests actual defaults
rather than inheriting the shared UI configuration.

Run the following commands from the repository root. The Make target also runs
the existing positive diagnostic fixtures, so an absent lint library cannot
masquerade as a successful negative case.

```sh
make test TEST_CARGO_FLAGS='-p bumpy_road_function --all-targets --all-features'
cs version
cs review --output-format json crates/bumpy_road_function/ui/pass_two_shallow_clusters.rs
```

The reference run used CodeScene CLI 1.0.52, build
`efcaeed1f35ec099062c9e30bc98f092aed16b6f` (2026-10-09), on 2026-11-08. It
reported `Bumpy Road Ahead` for `relative_to_manifest`, with `bumps = 2`. The
JSON diagnostic also states that a bump requires two levels of conditionals. No
repository `.codescene/` rule override was present; `cs check-rules` reported
no matching rule for the fixture. The relevant fields from the reference JSON
response are:

```json
{
  "category": "Bumpy Road Ahead",
  "functions": [{
    "title": "relative_to_manifest",
    "details": "bumps = 2",
    "start-line": 14,
    "end-line": 48
  }]
}
```

This is a measured result for the equivalent fixture, independently of the
original PR report. It is not a claim that all CodeScene releases or project
rule configurations produce identical results.

Three manual oracle controls used temporary copies of the same fixture: remove
the entire prefix-validation match, remove only the common-prefix loop's inner
`if`, or remove both. CodeScene reported no Bumpy Road diagnostic for any of
those variants. This supports treating the two nested regions as the cause of
the original finding; it does not infer a hidden bump count from an absent
diagnostic. These probes are not committed tests or evaluations.

Whitaker's focused run passed all 32 tests with none skipped, including the UI
harness containing this fixture and the existing positive controls. The fixture
produced no `bumpy_road_function` diagnostic. The expectation records the
current gap and must move to a positive structural-mode fixture when that
strategy is implemented. It must remain negative under the weighted
compatibility strategy.

## Current state and gap analysis

The production pipeline has four stages:

1. [`SegmentBuilder`](../../crates/bumpy_road_function/src/driver/segment_builder.rs)
   converts compiler high-level intermediate representation (HIR) spans into
   weighted line segments.
2. [`complexity_signal`](../../common/src/complexity_signal.rs) adds overlapping
   segments, then applies a centred moving average.
3. [`detect_bumps`](../../crates/bumpy_road_function/src/analysis.rs) retains
   contiguous runs meeting the threshold and minimum length.
4. [`analyse_body`](../../crates/bumpy_road_function/src/driver/mod.rs) emits a
   diagnostic only for two or more retained intervals. Diagnostic ranking uses
   area above threshold, then length, then source order.

| Dimension        | Current Whitaker contract                                 | Reference or alignment gap                                          |
| ---------------- | --------------------------------------------------------- | ------------------------------------------------------------------- |
| Detection        | At least two separated retained line intervals            | CodeScene reports two structural regions in the fixture             |
| Threshold        | Smoothed score at least `2.5`, inclusively                | No corresponding numeric equivalence established                    |
| Minimum span     | `2` contiguous qualifying lines per bump                  | Short nested regions can be meaningful to the reference             |
| Smoothing        | Centred window `3`; contracts at function edges           | Formatting and short valleys can suppress or merge Whitaker bumps   |
| Weights          | Depth `1.0`, predicate branch `0.5`, match-arm flow `0.5` | These are Whitaker parameters, not CodeScene parameters             |
| `let` conditions | Direct HIR `ExprKind::Let` adds no predicate segment      | Structural nesting must recognize authored `if let` and `while let` |
| `let ... else`   | No dedicated scored `if` region                           | Reference treatment requires its own corpus cases                   |
| Separation       | At least one below-threshold sample between retained runs | Sibling logical regions need not be separated by blank source lines |

*Table 1: Verified weighted semantics and the limits of reference knowledge.*

A simple `if` predicate contributes `0.5`; its body contributes `1.0`. Those
are local contributions, not necessarily the total line score. Enclosing blocks
contribute too. The historical prefix predicate has two branches joined by
`||`, contributing `1.0`, and sits in a match arm with depth and flow weights.
Treating every line in that region as capped at `1.5` would therefore be
incorrect.

The common-prefix loop's inner `if` contributes only one predicate branch. Its
`while let` condition adds no predicate score; this exclusion does not mean
that the loop's body has no depth contribution. For a conventional multiline
body, the inner `if` can reach `2.5` including the enclosing loop, but its
adjacent body lines reach only `2.0`. A three-line average of `[2.5, 2.0, 2.0]`
is approximately `2.167`, below the default threshold. Even an unsmoothed
single qualifying line would fail the two-line minimum. A surviving
prefix-region bump alone cannot trigger a function diagnostic.

The later `let Some(text) = ... else` is a local declaration, not an HIR `If`.
There is no dedicated predicate/depth pair for its pattern test. Ordinary
traversal can still inspect expressions inside it. Likewise, the exclusion for
a direct `ExprKind::Let` does not establish how every compound let-chain is
scored; that needs explicit tests.

Smoothing has no fixed percentage reduction. An isolated raw peak of `3.0`
surrounded by zeroes becomes `1.0` under window `3`; a constant plateau is
unchanged away from its edges. A user cannot infer reference compatibility from
a universal 15–20% adjustment.

## Proposed design

### Structural regions

Introduce an internal, compiler-independent structural representation owned by
`bumpy_road_function`. Each authored control-flow node carries its kind, parent
identity, source order and diagnostic span identifier. The compiler adapter
supplies those facts; the pure detector returns region identities. Concrete
compiler spans stay in the adapter. No new shared service, I/O port or dynamic
trait is needed.

Use the existing `SegmentBuilder`, `analysis::Settings`, interval detector and
shared signal helpers for the weighted path. A repository survey found these
existing boundaries; structural analysis should not duplicate their
normalization or move compiler dependencies into `common`.

The initial structural hypothesis is:

- Count a region when a conditional or loop is nested within another authored
  control-flow construct. The fixture's match-arm/`if` and loop/`if` pairs must
  become two regions.
- Group connected nested constructs under the same outer construct into one
  maximal region; a deeper chain is one bump, not one bump per level.
- Treat separate outer constructs in statement order as separate regions,
  even without blank lines. Plain lexical braces do not add control depth.
- Treat an `else if` chain as one conditional family. Flat guard clauses and
  a match with flat arms do not create nested regions by themselves.
- Recognize authored `if let`, `while let` and `for` constructs before
  compiler desugaring can hide their source roles. Do not count generated
  match/loop scaffolding as extra nesting.
- Keep `let ... else` alone as a guard rather than a nested region in the
  initial hypothesis. Measure nested work inside its branches separately.
- Preserve the current function/method scopes, macro-expansion policy and
  opt-in closure boundary. Do not attribute a closure's internal regions to its
  enclosing function.

These are proposed rules to test against CodeScene, not a recovered
specification of its internals. Any differential failure blocks a parity claim
and requires either a revised rule or a documented unresolved case. Severity
should use region count, depth and size after classification; the weighted
threshold must not veto an otherwise qualifying structural region.

### Configuration and migration

Propose `strategy = "weighted" | "structural"` under `[bumpy_road_function]`,
initially defaulting to `weighted` for compatibility. The names are provisional
and **not accepted configuration today**.

The weighted strategy retains `threshold`, `window`, `min_bump_lines` and
`weights` unchanged. Structural mode must reject explicit weighted-only
settings with an actionable configuration diagnostic rather than silently
ignoring them or silently falling back to defaults. The current loader's
log-and-default handling is insufficient for that migration and must be changed
deliberately, with tests. `include_closures` remains common to both.

After corpus agreement and review, make structural detection the default in a
documented release. Keep explicit weighted mode for existing users, describe
new warnings in the migration guide, and make the diagnostic identify its
strategy. Do not advertise a CodeScene-compatible mode before that evidence
exists. No runtime dependency on CodeScene is proposed.

## Implementation approach and acceptance tests

Deliver the proposed implementation in the following order:

1. Record a differential corpus with source bytes, CodeScene version, matching
   rule configuration and expected region count. Include this fixture and its
   one-region reductions. Keep the reference results in the repository so
   ordinary tests do not require a CodeScene account or network connection.
   CodeScene must not be invoked by committed tests or evaluations; refresh
   reference observations manually during investigations and corpus reviews.
2. Add structural collection at the compiler boundary and a pure region
   classifier. Only this lint's driver and its tests may construct/consume the
   new representation initially. Reuse across lints requires a separate design
   review; avoid a speculative generic control-flow framework.
3. Add strategy parsing, strategy-specific validation and diagnostic wiring.
   Retain the existing weighted regression corpus and numeric boundary tests.
4. Run the corpus against both strategies and CodeScene. Review every mismatch,
   then document the release/default decision and its compatibility impact.

| Test layer              | Required cases and oracle                                                                                                                                                     |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pure unit tests         | Two disjoint nested regions report; one connected deep region does not; flat siblings and flat guards do not; ordered region identities are exact                             |
| HIR/UI tests            | This fixture reports once with two labelled regions in structural mode; remains silent with weighted defaults; deleting either nested region removes the structural finding   |
| Rust syntax matrix      | `if`, `if let`, `while`, `while let`, `for`, match-arm conditionals, guards, let-chains, `let ... else`, `else if`, closures and macros; each has recorded reference outcomes |
| Weighted boundary tests | Inclusive `2.5` threshold, one versus two contiguous lines, windows `1` and `3`, endpoint contraction, valley merging, predicate exclusion and nested segment accumulation    |
| Configuration tests     | Absent strategy preserves weighted defaults; both explicit strategies dispatch correctly; invalid names and incompatible keys produce actionable errors                       |
| Behavioural tests       | A user selects each strategy through `dylint.toml` and receives the corresponding function-level diagnostic and labels                                                        |
| Metamorphic tests       | Reflow, blank lines and comments preserve structural classification; extracting one region removes the two-region warning                                                     |
| Differential tests      | Recorded reference classifications and region counts agree; manual pinned CodeScene runs validate corpus updates outside committed tests or evaluations                       |

*Table 2: Implementation acceptance matrix, not completed implementation work.*

Use parameterized `rstest` cases for finite syntax/configuration matrices,
`rstest-bdd` for user-visible strategy selection, and bounded `proptest`
transformations for formatting invariance. Verify span labels with UI
snapshots. Run the existing positive UI controls alongside negative cases. The
reference labels, rather than the proposed detector itself, are the independent
classification oracle.

## Alternatives considered

Lowering the weighted threshold, reducing the window or allowing one-line bumps
can increase sensitivity. It cannot guarantee structural alignment: it also
admits simple isolated predicates and remains sensitive to line layout. A zero
threshold can even merge the whole signal into one interval. Use such settings
only to tune the current heuristic, not as a parity preset.

Immediately replacing the default would surprise users with new warnings
without establishing reference agreement. Keeping the current algorithm
indefinitely under an implied equivalence would preserve the reported gap. The
staged strategy change permits explicit compatibility and measured alignment.

## Open questions

- Which CodeScene version and rule configuration should govern the release
  corpus after this initial CLI 1.0.52 reproduction?
- How does the reference group multiple nested conditionals in separate arms
  of one match, and which let-chain or `let ... else` forms create regions?
- What diagnostic/configuration migration best fits the suite-wide loader
  while preventing silent fallback from an explicitly selected strategy?

## Recommendation

Adopt the structural approach subject to the differential corpus. Preserve the
weighted strategy as a documented compatibility option. The implementation must
detect the two-region fixture, but that single success is necessary rather than
sufficient evidence of CodeScene alignment.

[^1]: [CodeScene: Bumpy Road complexity](https://codescene.com/blog/bumpy-road-code-complexity-in-context/),
    consulted 2026-11-08.

[^2]: [CodeScene 5.3.18: Code Health](https://docs.enterprise.codescene.io/versions/5.3.18/guides/technical/code-health.html),
    consulted 2026-11-08; this publication version is separate from the CLI
    version used for the reproduction.
