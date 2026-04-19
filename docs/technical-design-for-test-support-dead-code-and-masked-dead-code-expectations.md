# Technical design for `test_support_dead_code` and `masked_dead_code_expectations`

## Design summary

Whitaker should treat this problem as a workspace-cleanup and refactoring
problem, not as a style scold. Cargo compiles each integration-test target
under `tests/` as its own executable crate, so `dead_code` remains target-local
even when projects follow the Book’s shared-support pattern with
`tests/common/mod.rs`. The Reference also makes `#[expect(...)]` meaningful
only when that exact `expect` suppresses the lint; if some other lint-level
attribute changes handling, the expectation goes stale and
`unfulfilled_lint_expectations` should fire instead. In other words, the core
pathology is real, upstream behaviour explains it, and a single-target lint
cannot answer the whole question by itself.
[^1]

I recommend a split design. `test_support_dead_code` should exist as a normal
Whitaker lint crate in the Dylint suite, but only as a fast detector and
metadata emitter. The fully actionable result should come from a Whitaker
workspace analysis phase that runs under `whitaker check` and produces per-item
inventories, target classifications, and usage sites. That split matches
Dylint’s execution model, where lints run as dynamic libraries implementing
`LateLintPass`, and it also fits Whitaker’s documented direction toward a
first-class `whitaker` CLI that owns richer workspace-aware analysis.
[^2][^3]

Whitaker’s current workspace layout already accommodates this cleanly. The root
workspace contains `common`, `crates/*`, `installer`, and `suite`, and the
aggregated suite registry already enumerates individual lint crates. The
existing design document also assumes separate lint crates plus shared
infrastructure. Adding two new lint crates and one shared workspace-analysis
engine therefore fits the repository’s architecture rather than fighting it.
[^3]

## Repository evidence and the actual pathology

The axinite example matters because it shows how the original “shared bag of
helpers” story can stop being true over time. The repository now has a
consolidated test harness at `tests/e2e_traces.rs`, which declares
`mod support;` once at the harness root and then pulls submodules in through
`#[path = "..."]` attributes. Axinite’s own consolidation plan says the suite
moved from 40 test binaries down to 9 and that only harnesses that actually
need shared support declare `mod support;`. In the current layout, connector
search for `support::metrics` only surfaced `tests/e2e_traces/metrics.rs` and
`tests/support/test_rig/rig.rs`, which means `tests/support/metrics.rs` now
looks like support for the `e2e_traces` harness rather than support for the
whole integration-test universe. That makes the file-level
`#![allow(dead_code)]` in `tests/support/metrics.rs` especially suspect: it no
longer has a strong cross-target justification. [^4]

Axinite also shows why the report must inventory real items rather than emit
only counts. `tests/support/metrics.rs` defines `TraceMetrics`,
`ToolInvocation`, `TurnMetrics`, `ScenarioResult`, `RunResult`, `MetricDelta`,
and helper methods such as `total_tool_calls`, `failed_tool_calls`,
`total_tool_time_ms`, `from_scenarios`, and `compare_runs`. Search results and
direct file reads show that `TraceMetrics`, `ToolInvocation`, `RunResult`,
`ScenarioResult`, `compare_runs`, and `from_scenarios` have downstream
consumers in the E2E metrics test and the test rig, while `TurnMetrics`,
`MetricDelta`, and `total_tool_time_ms` only surfaced in the defining file.
That makes `metrics.rs` an excellent worked example for a lint whose output
should say “these items look globally dead” and “these items are single-target
live”, rather than merely “5 dead, 4 alive”. The compiler run must remain the
final oracle, but repository evidence already points in that direction.
[^4]

Axinite also contains a second, subtler smell that the design should account
for: manual keepalive shims. `tests/support/mod.rs` defines `touch!`,
`touch_const!`, anonymous `const _:` type assertions, and whole families of
`touch_*` functions whose only job is to reference support symbols so the
compiler treats them as used. Those references are not genuine business or test
uses. A cleanup tool that treats them as ordinary evidence of liveness will
misclassify papered-over dead code as organically live. The design should
therefore classify “synthetic keepalive uses” separately from ordinary uses.
[^5]

The mdtablefix example is the inverse pathology. `tests/common/mod.rs` begins
with `#![allow(unfulfilled_lint_expectations)]` and then annotates individual
helpers with `#[expect(dead_code, reason = "...")]`. `tests/prelude/mod.rs`
imports `../common/mod.rs` via `#[path = "../common/mod.rs"] mod common;` and
reexports `common::*`. Multiple test targets then pull `mod prelude;` in:
search results surfaced `code_emphasis.rs`, `footnotes.rs`, `fences.rs`,
`lists.rs`, `markdownlint.rs`, `parallel.rs`, `wrap_renumber.rs`, `cli.rs`,
`breaks.rs`, and grouped harness roots such as `wrap/mod.rs` and
`table/mod.rs`. That means `#[path]` is not the root cause, but it absolutely
matters for module-graph traversal because it is the edge that leads Whitaker
from `prelude` into `common`. [^6]

Repository evidence already shows that several mdtablefix helpers are plainly
live. `run_cli_with_args` appears in `parallel.rs` and `code_emphasis.rs`;
`run_cli_with_stdin` appears in `wrap/cli.rs` and `code_emphasis.rs`;
`assert_wrapped_list_item` appears in `wrap/lists.rs` and `wrap/footnotes.rs`;
`assert_wrapped_blockquote` appears in `wrap/blockquotes.rs`; and
`broken_table` appears in `parallel.rs`, `cli.rs`, and `table/reflow.rs`. Under
the Reference semantics, any `#[expect(dead_code)]` on those helpers is stale
in targets that actually use them, and the file-level
`#![allow(unfulfilled_lint_expectations)]` removes the only warning that would
tell the developer so. This is exactly what `masked_dead_code_expectations`
should expose. [^6][^7][^1]

## Proposed rule contracts

`test_support_dead_code` should fire on non-top-level modules reachable from
integration-test targets when Whitaker finds any of the following in scope:
`#![allow(dead_code)]`, item-level `#[allow(dead_code)]`,
`#[expect(dead_code)]`, or other dead-code suppression that prevents ordinary
liveness reporting. The rule should restrict itself to test-support code: files
under `tests/support/`, `tests/common/`, `tests/prelude/`, or any module
reachable from a `tests/*.rs` integration-test target through ordinary `mod`
edges or `#[path]` edges. In plain Dylint mode it should emit a lightweight
warning that says, in effect, “this suppression needs workspace analysis”. In
full Whitaker mode it should emit an inventory-based report that classifies
every suppressed item as globally dead, organic single-target live, organic
multi-target live, or live only through synthetic keepalive shims.
[^1][^2]

`masked_dead_code_expectations` should stay narrower than a general “masked
expectations” lint, at least in v1. It should fire only when
`#[expect(dead_code)]` sits inside a lexical scope dominated by
`allow(unfulfilled_lint_expectations)`. That focuses the rule on the exact
family of “I tried to be precise, then turned the alarm off” patterns that the
exploration identified. In plain Dylint mode it should flag the mask
immediately. In full Whitaker mode it should attach a stale-expectation
inventory showing which `expect(dead_code)` annotations are stale in which
targets and where the corresponding organic uses sit.
[^1][^6]

Both rules should share one analysis engine and produce one merged report when
they hit the same file. In mdtablefix, for example,
`masked_dead_code_expectations` should not merely say “you masked stale
expectations”; it should also reuse the `test_support_dead_code` machinery to
say which helpers are globally dead, which are genuinely shared, and which
expectations are stale because the helper is live in at least one importer
target. That keeps the user-facing story unified and actionable.
[^6]

## Multi-pass execution path

The workspace analysis should run in four deliberate phases.

The first phase should discover candidate files and importer targets. Whitaker
should call `cargo metadata --format-version 1 --no-deps` to enumerate
workspace members and targets, then filter to target kinds that Cargo reports
as integration tests. Starting from each `tests/*.rs` harness, Whitaker should
walk the local module graph, following both ordinary `mod foo;` edges and
explicit `#[path = "..."] mod foo;` edges. This is the point where `#[path]`
matters: not as the trigger, but as the mechanism required to discover support
modules such as `mdtablefix/tests/common/mod.rs` behind `tests/prelude/mod.rs`.
[^1][^6]

The second phase should build a temporary overlay workspace and apply minimal
source edits. Whitaker should never rewrite the user’s working tree in place.
Instead it should copy the relevant package trees to a temporary directory and
compute text edits for each candidate file. The dead-code replay edit should
remove only `dead_code` suppressions from `allow` and `expect` attributes,
preserve unrelated lints, and inject a probe item such as
`fn whitaker_dead_code_probe_7f3c2e() {}` immediately after inner attributes.
The probe must not begin with an underscore, because the compiler explicitly
documents underscore-prefixed names as a way to silence `dead_code`.
[^1]

The third phase should run an instrumented per-target compilation. For each
importer target, Whitaker should invoke
`cargo check -p <package> --test <target> --message-format=json`, threading
through the same feature flags, target triple, and manifest path that the
surrounding Whitaker invocation already resolved. The JSON stream gives
Whitaker structured compiler diagnostics, and a small collector lint loaded
during the same run can record def-use edges for the candidate support files.
`cargo check` already supports selecting specific integration-test targets, and
Cargo’s JSON message stream is designed for external tools.
[^1]

That instrumented run should collect two distinct classes of evidence. First,
rustc’s own `dead_code` diagnostics remain the ground truth for whether an item
is dead in a given target. Second, the collector lint should record usage edges
so Whitaker can produce the inventory the user actually wants. The collector
should map each support-file item to a canonical identifier such as
`fn compare_runs`, `struct TurnMetrics`, or
`impl TraceMetrics::total_tool_time_ms`, then record every resolved path or
method-call use site that points back to that item. When the collector cannot
localize an organic use but rustc still treats the item as live, Whitaker
should report that as “alive; usage not localized”, rather than pretending
certainty. That keeps rustc as the liveness oracle while still giving a useful
inventory. [^2][^1]

The fourth phase should replay masked expectations only where needed. For files
that contain `#[expect(dead_code)]` under
`allow(unfulfilled_lint_expectations)`, Whitaker should run a second overlay
build in which it restores the original `expect(dead_code)` annotations but
removes the mask. This pass should gather `unfulfilled_lint_expectations`
diagnostics so Whitaker can say exactly which expectations are stale in which
targets. That second pass is what turns “masked expectations exist” into “these
eight expectations are stale in these four importer targets”.
[^1]

The classification logic should stay simple:

```text
importer_targets(file)        = targets that emitted probe dead_code
dead_targets(item)            = importer_targets(file) with a rustc dead_code warning on item
alive_targets(item)           = importer_targets(file) - dead_targets(item)

organic_use_targets(item)     = targets with at least one non-synthetic recorded use
synthetic_use_targets(item)   = targets whose only uses are keepalive shims
stale_expectation_targets(item) = targets that emitted unfulfilled_lint_expectations
```

From that, Whitaker can derive four useful states. If `alive_targets(item)` is
empty, the item is globally dead. If `alive_targets(item)` is non-empty but
`organic_use_targets(item)` is empty, the item is only being kept alive by
synthetic assertions or keepalive shims. If `organic_use_targets(item)`
contains one target, the item is not really shared support. If it contains two
or more targets, the item is genuinely shared. The expectation replay then
overlays “stale expectation” information on top of those states. This is the
execution path that makes the lint genuinely multi-pass rather than just
multi-target. The usefulness of the synthetic/organic split follows directly
from axinite’s existing `touch!`, `touch_const!`, and anonymous `const _:`
keepalive code. [^5]

## Diagnostic model and report format

The default human-readable diagnostic should inventory items, not counts. A
file-level warning should lead with the suppression site, then group items
under headings such as “globally dead”, “single-target live”, “shared across
targets”, and “alive only via keepalive shims”. For each item Whitaker should
print the importer targets, the use kind, and concrete use sites when the
collector found them. That gives the developer the next edit to make, not just
a bad feeling.

A representative format looks like this:

```text
warning[test_support_dead_code]: dead_code suppression hides actionable cleanup
  --> tests/support/metrics.rs:1:1
   |
 1 | #![allow(dead_code)]
   | ^^^^^^^^^^^^^^^^^^^^
   |
   = importer target:
     - axinite::test/e2e_traces

   = globally dead in all importers:
     - struct TurnMetrics
     - struct MetricDelta
     - impl TraceMetrics::total_tool_time_ms

   = live in exactly one importer:
     - struct TraceMetrics
       uses:
         - tests/e2e_traces/metrics.rs
         - tests/support/test_rig/rig.rs
     - struct ToolInvocation
       uses:
         - tests/support/test_rig/rig.rs
     - struct RunResult
       uses:
         - tests/e2e_traces/metrics.rs
     - struct ScenarioResult
       uses:
         - tests/e2e_traces/metrics.rs
     - fn compare_runs
       uses:
         - tests/e2e_traces/metrics.rs

   = help: delete globally dead items, move single-target helpers closer to
            the owning test target, and keep only truly shared helpers in
            common support
```

That specific shape matches the current axinite evidence:
`tests/support/metrics.rs` begins with `#![allow(dead_code)]`; only the
`e2e_traces` path currently surfaced as an importer; and repository evidence
already differentiates likely dead items such as `TurnMetrics`, `MetricDelta`,
and `total_tool_time_ms` from items that the E2E metrics tests or test rig
actively consume. The compiler run should still determine the final
classification, but the report should look like this, not like a histogram.
[^4]

`masked_dead_code_expectations` should use the same idea. It should not stop at
“scope contains `allow(unfulfilled_lint_expectations)`”. It should show exactly
which expectations are stale and why:

```text
warning[masked_dead_code_expectations]: dead_code expectations are masked by allow(unfulfilled_lint_expectations)
  --> tests/common/mod.rs:2:1
   |
 2 | #![allow(unfulfilled_lint_expectations)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = masked stale expectations:
     - fn run_cli_with_args
       stale in:
         - mdtablefix::test/code_emphasis
         - mdtablefix::test/parallel
       uses:
         - tests/code_emphasis.rs
         - tests/parallel.rs

     - fn run_cli_with_stdin
       stale in:
         - mdtablefix::test/code_emphasis
         - mdtablefix::test/wrap
       uses:
         - tests/code_emphasis.rs
         - tests/wrap/cli.rs

     - fn assert_wrapped_list_item
       stale in:
         - mdtablefix::test/wrap
       uses:
         - tests/wrap/lists.rs
         - tests/wrap/footnotes.rs

     - fn assert_wrapped_blockquote
       stale in:
         - mdtablefix::test/wrap
       uses:
         - tests/wrap/blockquotes.rs

   = help: remove the mask, then keep expect(dead_code) only on items that are actually dead in a given importer target
```

That shape reflects the repository’s current state far better than a simple
“masked expectation exists” warning. The Reference semantics justify the
stale-expectation language, and the mdtablefix search results already show
those helpers as live across real importer targets.
[^1][^6][^7]

Whitaker should also expose the same inventory as machine-readable JSON. The
CLI design already treats JSON output as a first-class interface for
operational summaries, and this analysis belongs in the same family. A JSON
form makes later autofix or reporting work possible without locking Whitaker
into automatic deletion or code movement in v1. [^3]

## Worked examples

In axinite, the most important design conclusion is that the tool should not
hard-code the “shared across many test crates” story. The current repository
layout consolidated the old explosion of integration-test binaries into grouped
harnesses, and the current `support::metrics` evidence points to one effective
importer harness, `e2e_traces`, not to the whole suite. That means Whitaker
should phrase the output conservatively: not “this shared helper is used by
many test targets”, but “under the current workspace configuration, this file
imports into one target, so the blanket suppression now masks dead or overly
broad support code”. The same report should explicitly separate genuine test
uses in `tests/e2e_traces/metrics.rs` and `tests/support/test_rig/rig.rs` from
synthetic keepalive shims in `tests/support/mod.rs`.
[^4][^5]

In mdtablefix, the worked example should show the opposite conclusion.
`tests/common/mod.rs` really is shared support, because `tests/prelude/mod.rs`
reexports it and many top-level test targets import `prelude`. The right output
therefore is not “move everything local”; it is “keep the genuinely shared
helpers shared, delete anything globally dead, and stop pretending that every
helper is expected dead everywhere”. The collector evidence already shows that
several helpers are genuinely shared. That makes mdtablefix the canonical
justification for a rule that inventories per-item target reach, rather than
just yelling about the presence of `#[expect(dead_code)]`.
[^6][^7]

## Constraints, failure modes, and rollout

The scope should stay tight in v1. Whitaker should analyse integration-test
support only, and only for the active package, feature set, and target triple.
Cargo documentation explicitly frames target selection and feature selection as
part of how external tools should integrate with builds, so Whitaker should
report what is dead under the build the user actually asked for, not under an
imagined cross-product of every feature combination.
[^1]

Whitaker should not auto-move code between files in v1. Deletion is tempting
for items proved dead in all importer targets, but even there rustc’s own
`dead_code` documentation warns about cases where apparently unused fields or
items still matter through drop side effects or other type-level behaviour. The
safe v1 contract is “report precisely, suggest concretely, and maybe offer a
later opt-in fix for removing the suppression attribute itself”.
[^1]

The design should also explicitly treat `#[expect(dead_code)]` as a dubious
migration target rather than the canonical remedy. The Reference semantics
already make it awkward in this domain, since expectations are per-target and
only fulfilled when that exact `expect` suppresses the lint. There is also at
least one current rustc issue where `#[expect(dead_code)]` appears in an
incremental-compilation ICE involving `shallow_lint_levels_on`. Whitaker
therefore should not frame “replace `allow(dead_code)` with `expect(dead_code)`
everywhere” as the recommended end state for shared test support.
[^1]

The rollout path should be incremental. First, add the two lint crates and the
shared analyser crate or module, and wire the lightweight detector into the
suite. Second, integrate full workspace analysis into `whitaker check`, where
Whitaker already plans richer workspace-aware behaviour. Third, add JSON output
and a small corpus of UI and behaviour tests drawn from the axinite and
mdtablefix patterns. That path keeps Dylint compatibility, fits Whitaker’s
existing architecture, and delivers the part the exploration asked for: a tool
that helps people unbork the codebase by telling them which support items are
actually dead, which are local, and which are truly shared.
[^3]

## References

[^1]: Cargo and Rust documentation covering integration-test target structure,
      `dead_code`, `#[expect(...)]`, `unfulfilled_lint_expectations`,
      underscore-name suppression, and external-tool integration.
      <https://doc.rust-lang.org/cargo/reference/cargo-targets.html?highlight=test&utm_source=chatgpt.com>
      <https://doc.rust-lang.org/cargo/reference/external-tools.html?utm_source=chatgpt.com>
      <https://doc.rust-lang.org/reference/attributes/diagnostics.html?utm_source=chatgpt.com>
      <https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/builtin/static.DEAD_CODE.html?utm_source=chatgpt.com>
[^2]: Dylint documentation describing dynamic-library lint execution and the
      `LateLintPass` model.
      <https://github.com/trailofbits/dylint?utm_source=chatgpt.com>
[^3]: Whitaker workspace references used for the proposed split between
      lint-time detection and richer `whitaker check` analysis: the CLI design,
      workspace manifest, suite registry, and Dylint suite design.
      <https://github.com/leynos/whitaker/blob/HEAD/docs/whitaker-cli-design.md>
      <https://github.com/leynos/whitaker/blob/HEAD/Cargo.toml>
      <https://github.com/leynos/whitaker/blob/HEAD/suite/src/lints.rs>
      <https://github.com/leynos/whitaker/blob/HEAD/docs/whitaker-dylint-suite-design.md>
[^4]: Axinite repository evidence for the consolidated `e2e_traces` harness and
      the support metrics module plus its organic use sites.
      <https://github.com/leynos/axinite/blob/HEAD/tests/e2e_traces.rs>
      <https://github.com/leynos/axinite/blob/HEAD/docs/execplans/consolidate-test-binaries.md>
      <https://github.com/leynos/axinite/blob/HEAD/tests/support/metrics.rs>
      <https://github.com/leynos/axinite/blob/a824bb0bcab672e353e607ba6c785f5a83f6f2ce/tests/support/metrics.rs>
      <https://github.com/leynos/axinite/blob/a824bb0bcab672e353e607ba6c785f5a83f6f2ce/tests/e2e_traces/metrics.rs>
      <https://github.com/leynos/axinite/blob/HEAD/tests/e2e_traces/metrics.rs>
      <https://github.com/leynos/axinite/blob/a824bb0bcab672e353e607ba6c785f5a83f6f2ce/tests/support/test_rig/rig.rs>
      <https://github.com/leynos/axinite/blob/HEAD/tests/support/test_rig/rig.rs>
[^5]: Axinite keepalive shim examples in `tests/support/mod.rs`.
      <https://github.com/leynos/axinite/blob/HEAD/tests/support/mod.rs>
[^6]: mdtablefix module-graph evidence showing how `tests/prelude/mod.rs` pulls
      `tests/common/mod.rs` into many importer targets via ordinary `mod` and
      `#[path]` edges.
      <https://github.com/leynos/mdtablefix/blob/HEAD/tests/common/mod.rs>
      <https://github.com/leynos/mdtablefix/blob/HEAD/tests/prelude/mod.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap/mod.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/footnotes.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/fences.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap_renumber.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/table/mod.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/markdownlint.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/lists.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/parallel.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/cli.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/breaks.rs>
[^7]: mdtablefix helper use sites demonstrating genuinely shared helpers and
      stale `dead_code` expectations.
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/code_emphasis.rs>
      <https://github.com/leynos/mdtablefix/blob/HEAD/tests/code_emphasis.rs>
      <https://github.com/leynos/mdtablefix/blob/HEAD/tests/parallel.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/table/reflow.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap/lists.rs>
      <https://github.com/leynos/mdtablefix/blob/HEAD/tests/wrap/lists.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap/footnotes.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap/blockquotes.rs>
      <https://github.com/leynos/mdtablefix/blob/8c50fc81b2ae0220672b7afc62792911e9930c43/tests/wrap/cli.rs>
