# Debugging plan: slow Bumpy Road API contract

- **Generated:** 2026-10-24T20:01:38Z
- **Issue:** [#514](https://github.com/leynos/whitaker/issues/514)
- **Prerequisite:** [PR #519](https://github.com/leynos/whitaker/pull/519)
- **Status:** Waiting for #519 to merge and a maintainer ruling on Option 3.

## Problem statement

`bumpy_road_function::api_contract` takes 229 seconds in Coverage (main) run
[40088274299](https://github.com/leynos/whitaker/actions/runs/40088274299).
That is over a third of its 616-second test phase and about six times the
38-second `whitaker-common::public_api_contract` test. The Bumpy Road contract
runs in `serial-dylint-ui`, which it holds while nested compilation runs.

## Findings

The supplied measurements and #519 report identify a cold nested build as the
dominant cost:

- The Bumpy Road test costs about 148 seconds with a cold trybuild target and
  about 15 seconds when warm.
- The cold nextest phase reported with #519 fell from 382 to 300 seconds after
  trybuild contracts moved into their own serial group.
- A cold trybuild build remains above the 40-second target. #519 removes
  contention with the Dylint UI group but not the nested build.

The trybuild 1.0.122 source confirms the mechanism. `Runner::prepare` creates
one generated project under `target/tests/trybuild/<crate>` for the selected
cases. Its manifest combines the tested package's regular and development
dependencies and forwards enabled features. The nested Cargo command removes
inherited `RUSTFLAGS` and sets a separate trybuild target directory. A suite
with a `pass` case uses `cargo build`; one without a `pass` case uses
`cargo check`. Each fixture is built separately, and a passing fixture is also
run. Both Bumpy Road cases share the generated project and target.

Sources:

- [`src/run.rs`](https://github.com/dtolnay/trybuild/blob/1.0.122/src/run.rs)
- [`cargo.rs`](https://github.com/dtolnay/trybuild/blob/1.0.122/src/cargo.rs)

### Local baseline

No targeted run has been recorded yet. The initial inspection found no
`target/tests/trybuild` directory. `sccache --show-stats` reported zero compile
requests, hits, and misses; capture it again immediately before the test. The
build-limits supervisor was healthy but fully occupied (4/4 slots) with a long
compiler queue. This was not an idle host, so its timing would not meet the
requested baseline condition.

Run the selected contract twice on an idle host and preserve the shared Cargo
and sccache caches. Use the Makefile test `RUSTFLAGS` and `--all-features`.
Record wall time, trybuild target size, and sccache hits and misses before and
after. The targeted command is:

```sh
RUSTFLAGS='-C prefer-dynamic -Z force-unstable-if-unmarked -D warnings' \
  cargo nextest run --all-features -p bumpy_road_function \
  -E 'test(normalize_settings_is_public_and_the_legacy_name_stays_removed)'
```

## Decision gate

The current issue discussion recommends moving the negative check into a
doctest but defers implementation until a ruling. No maintainer ruling has
approved that lane change. The GitHub API reported #519 open, based on `main`,
when this note was written.

Present these alternatives after #519 merges:

1. Keep the trybuild test. #519 removes its blocking of the Dylint UI group;
   the cold cost remains.
2. Give trybuild a shared persistent target. Warm local runs take about 15
   seconds, while Coverage CI remains cold because it recreates its target.
3. Remove the trybuild harness. Keep the existing integration test as the
   positive public-path check and move the removed-name check into a
   `compile_fail,E0432` doctest. This removes the nested build, activates three
   positive doctests, and builds the library with all features in
   `make test-doc`. The negative check then runs only in lanes that execute
   `make test-doc`.

Do not change code until #519 merges and a maintainer approves Option 3. Record
the ruling and its issue or pull request comment here. If Option 1 is selected,
stop with this note. If Option 2 is selected, stop and open a follow-up plan.

## Remaining evidence

After approval, update this note with the local replacement measurements,
mutation results, and PR and main coverage timings required by the issue plan.
The current baseline does not include an idle-host local run or a
before-and-after CI comparison.
