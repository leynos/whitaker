# Debugging plan: slow Bumpy Road API contract

- **Generated:** 2026-10-24T20:01:38Z
- **Updated:** 2026-10-31
- **Issue:** [#514](https://github.com/leynos/whitaker/issues/514)
- **Status:** Option 3 implemented; validation in progress.

## Problem statement

`bumpy_road_function::api_contract` took 229 seconds in Coverage (main) run
[40088274299](https://github.com/leynos/whitaker/actions/runs/40088274299).
That was over a third of its 616-second test phase and about six times the
38-second `whitaker-common::public_api_contract` test. The Bumpy Road contract
ran in `serial-dylint-ui`, which it held while nested compilation ran.

## Findings

The supplied measurements identify a cold nested build as the dominant cost:

- The Bumpy Road test cost about 139 to 148 seconds with a cold trybuild target
  and about 15 seconds when warm.
- The cold nextest phase reported with PR
  [#519](https://github.com/leynos/whitaker/pull/519) fell from 382 to 300
  seconds after trybuild contracts moved into their own serial group. #519 was
  closed without merging; PR
  [#522](https://github.com/leynos/whitaker/pull/522) is its open successor for
  the group split.
- The trybuild 1.0.122 source confirms the mechanism. `Runner::prepare` creates
  one generated project under `target/tests/trybuild/<crate>`. Its manifest
  combines the tested package's regular and development dependencies and
  forwards enabled features. The nested Cargo command removes inherited
  `RUSTFLAGS` and sets a separate trybuild target directory. A suite with a
  `pass` case uses `cargo build`; one without a `pass` case uses `cargo check`.
  Each fixture is built separately, and a passing fixture is also run. Both
  Bumpy Road cases shared the generated project and target, but the dependency
  graph was rebuilt because the `pass` case required `cargo build`.

Sources:

- [`src/run.rs`](https://github.com/dtolnay/trybuild/blob/1.0.122/src/run.rs)
- [`cargo.rs`](https://github.com/dtolnay/trybuild/blob/1.0.122/src/cargo.rs)

### The coverage cost was removed by a different change

While this investigation was in progress, PR
[#520](https://github.com/leynos/whitaker/pull/520) merged. `make coverage` now
passes nextest a filterset that skips the trybuild compile contracts, which run
no workspace code the coverage report attributes. `coverage-check` fell from 17
minutes to 10 minutes 12 seconds. PR
[#521](https://github.com/leynos/whitaker/pull/521) then removed the
`linux-full` step that ran the contracts uninstrumented, because it paid a cold
1,693-second workspace test build.

The 229-second Coverage (main) figure is therefore already gone. What remained
was the local `make test` cost (139 to 148 seconds cold) and the same cost in
the `windows-compat` lane, whose `make test NEXTEST_PROFILE=ci` still runs
every compile contract.

### Decision

The issue discussion recommended moving the removed-name check into a
`compile_fail` doctest, then keeping the positive check as an ordinary use in
an integration test. That is Option 3 in the options below:

1. Keep the trybuild test. #519 removes its blocking of the Dylint UI group; the
   cold cost remains.
2. Give trybuild a shared persistent target. Warm local runs take about 15
   seconds, while Coverage CI remains cold because it recreates its target.
3. Remove the trybuild harness. Keep the existing integration test as the
   positive public-path check and move the removed-name check into a
   `compile_fail,E0432` doctest.

The maintainer handed this branch the Option 3 implementation plan on
2026-10-31, after #520 had already removed the coverage cost. No separate issue
comment records a second ruling, so the plan and this note are the record.

## Implementation

- `crates/bumpy_road_function/src/analysis.rs`: the three positive examples
  carry hidden `rustc_private` setup lines. `normalize_settings` also carries a
  `compile_fail,E0432` example importing and calling the removed legacy name.
- `crates/bumpy_road_function/tests/api_contract.rs` and `tests/api_ui/`
  deleted. `tests/analysis_behaviour.rs` remains the positive public-path check.
- `crates/bumpy_road_function/Cargo.toml`: the `trybuild` dev-dependency is
  removed; `Cargo.lock` loses only that edge.
- `Makefile`: `bumpy_road_function` leaves `DOCTEST_EXCLUDES`, and the
  trybuild-contract counts and filters lose its clause.
- `.config/nextest.toml`: the compile-contract override in both profiles loses
  the removed test's clause. Groups, timeouts and retries are unchanged.
- `tests/workflow_contracts/nextest_profile_test.py`: `COMPILE_CONTRACT_FILTER`
  and the derived counts lose the clause.
- `docs/developers-guide.md`: the trybuild inventory, timeout rationale and
  `test-doc` lane are updated.

## Mutation evidence

### The negative doctest detects the restored legacy name

With `pub use normalize_settings as normalise_settings;` temporarily restored,
`RUSTDOCFLAGS='-D warnings' make test-doc DOCTEST_CARGO_FLAGS='-p
bumpy_road_function --all-features'`
failed exactly as required:

```text
test crates/bumpy_road_function/src/analysis.rs - analysis::normalize_settings (line 100) - compile fail ... FAILED
Test compiled successfully, but it's marked `compile_fail`.
```

The diagnostic is the control that matters: the example compiled, so the
failure is the expectation rather than a setup error such as `E0658` or a link
failure.

### The example really compiles when the legacy name exists

With the alias still present and the fence changed from `compile_fail,E0432` to
`rust`, the same command passed all four doctests. This proves the example
exercises the legacy name rather than failing for an unrelated reason. Both the
fence and the alias were then reverted.

### The positive integration test detects the removed public name

Renaming `normalize_settings` to `normalise_settings` in `analysis.rs` and
`driver/mod.rs` made `analysis_behaviour` fail to compile under the Makefile
flags:

```text
error[E0432]: unresolved import `bumpy_road_function::analysis::normalize_settings`
  |
13 |     DEFAULT_THRESHOLD, Settings, Weights, detect_bumps, normalize_settings, top_two_bumps,
  |                                                         ^^^^^^^^^^^^^^^^^^ no `normalize_settings` in `analysis`
```

The rename and every other mutation were reverted before the gates ran.

## Measurements

Before, from the supplied runs:

- Coverage (main) run 40088274299: 229 seconds for the contract, 616 seconds for
  the test phase.
- Cold local run: 139 to 148 seconds; warm: about 15 seconds.
- PR #519 draft: nextest phase 382 seconds cold, 300 seconds with the group
  split.

After, locally on the candidate worktree:

| Run | Doctest execution | Wall time | Note                              |
| --- | ----------------- | --------- | --------------------------------- |
| 1   | 1.55 s            | 145 s     | 133 compile requests queued ahead |
| 2   | 1.10 s            | 222 s     | 133 compile requests queued ahead |

The wall times are host admission queueing, not the doctest: the log shows one
crate compiled and a single `Finished` line per run. The replacement executes
four doctests in about a second, against the 40-second target. The nested
trybuild build that cost 139 to 148 seconds cold is deleted, so it no longer
appears in `make test` or in `windows-compat`.

`ci.yml` and `coverage-main.yml` both run `rm -rf target/llvm-cov-target`
immediately before `make test-doc`, so the doctest lane starts with the
non-coverage target rather than the instrumented one. The doctest there costs
the ordinary library build that `make lint` and `cargo doc` already require.

## Follow-up

PR #522 replaces the grouping that #519 proposed. It still lists the removed
test in its filters; it will need to drop that clause when it rebases on this
change.
