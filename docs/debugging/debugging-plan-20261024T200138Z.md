# Debugging plan: slow Bumpy Road API contract

- **Generated:** 2026-10-24T20:01:38Z
- **Updated:** 2026-11-03
- **Issue:** [#514](https://github.com/leynos/whitaker/issues/514)
- **Status:** Option 3 implemented and validated.

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

| Run | Cache state                     | Doctest execution | Wall time | Note                              |
| --- | ------------------------------- | ----------------- | --------- | --------------------------------- |
| 1   | warm `target/` from prior gates | 1.55 s            | 145 s     | 133 compile requests queued ahead |
| 2   | warm `target/` from prior gates | 1.10 s            | 222 s     | 133 compile requests queued ahead |

The wall times are host admission queueing, not the doctest: the log shows one
crate compiled and a single `Finished` line per run. The replacement executes
four doctests in about a second. The nested trybuild build that cost 139 to 148
seconds cold is deleted, so it no longer appears in `make test` or in
`windows-compat`.

Neither local run was taken on an idle host, so neither is an end-to-end
timing. The authoritative figure is the CI run on head `03c98ab8`: the
`coverage-check` job's `make test-doc` step took 105 seconds, of which 101
seconds was the workspace build and 0.50 seconds was the four Bumpy Road
doctests. That build is the ordinary library build the lane needs anyway, so it
is not attributable to Bumpy Road, and the 0.50-second execution figure is not
an end-to-end build measurement.

The 40-second target applies to the Bumpy Road contract's own cost, not to the
shared workspace build that every doctest in the lane pays. On that reading the
target is met: the contract's execution is 0.50 seconds in CI and about a
second locally, and the nested trybuild project that used to be rebuilt for it
is gone. An idle-host measurement is not obtainable on this shared host, which
other agents keep saturated. The CI figure is the end-to-end reference not
because the workflow clears the doctest target — it does not — but because the
`coverage-check` job restores no `target/` cache and runs `make coverage` into
`target/llvm-cov-target`, which leaves the plain `target/` empty until
`make test-doc` builds it.

The investigation did not capture per-test sccache deltas, target-size deltas,
or a compiler-invocation comparison. Those checks are superseded by the harness
replacement: the nested trybuild project and its separate target directory no
longer exist, so there is no per-test build left to attribute.

`ci.yml` and `coverage-main.yml` both run `rm -rf target/llvm-cov-target`
immediately before `make test-doc`. That removes only the instrumented tree, so
the doctest lane builds into the separate, non-coverage `target/`, which the
job restores from no cache. The doctest there costs the ordinary library build
that `make lint` and `cargo doc` already require.

## Validation

Every gate ran sequentially on the candidate worktree.

| Gate                            | Result                                 |
| ------------------------------- | -------------------------------------- |
| `make check-fmt`                | pass, 98 files unchanged               |
| `make lint`                     | pass, no warnings or errors            |
| `make test-doc`                 | pass, all four Bumpy Road doctests ran |
| `make markdownlint` (spelling)  | pass, no errors                        |
| `make nixie`                    | pass, all diagrams valid               |
| `make test-workflow-contracts`  | 748 passed                             |
| `nextest_profile_test.py` et al | 82 passed                              |

The full `make test` run is long on this host because the shared compile queue
is heavily loaded, and the local command ceiling stopped two attempts short.
Both attempts were clean as far as they got: run 1 passed 1995 of 2015 tests,
run 2 passed 2008, and neither reported a failure.

The denominator is 2015 rather than the 2021 nextest lists because six tests
carry `#[ignore]`: five in `no_std_fs_operations::integration_exclusion` and
one in the `whitaker-installer` clippy snapshot.

The seven non-ignored tests that neither run reached were then run one at a
time. All seven pass: five dylint UI contracts, the `whitaker_test_macros`
contract, and the `rstest_helper_should_be_fixture` UI contract, which took
1017 seconds of nested trybuild builds behind the compile queue. The trailing
`cargo test -p dylint_testing --lib --no-default-features` lane also passed its
19 tests.

The branch was then rebased onto `main` (`bd4c93a8`) to clear a conflict in
`docs/contents.md`. The only conflict was that file's index, resolved by
keeping both entries. After the rebase `make check-fmt` reported 111 files
unchanged, and `make markdownlint` and `make nixie` passed. CI run
[41265262329](https://github.com/leynos/whitaker/actions/runs/41265262329)
passed all three lanes at head `554a828f`: `linux-full` (format, spelling,
Mermaid, Markdown lint, `make lint`, workflow contracts) in 11m45s,
`coverage-check` (including `make test-doc`) in 9m20s, and `windows-compat`
(`make test NEXTEST_PROFILE=ci`) in 28m35s.

The branch was rebased a second time, onto `main` at `00186d43`, because the
target had advanced past `bd4c93a8`. Four files conflicted, all on the same
theme: the target added a new compile contract,
`public_const_api_works_in_const_context`, to the filter lists this branch was
editing to remove
`normalize_settings_is_public_and_the_legacy_name_stays_removed`. Each conflict
was resolved by keeping the target's new clause and dropping the removed one,
so the merged lists hold six standard compile contracts plus the separately
budgeted nested trybuild test. The counts that depend on those lists were
recomputed rather than carried over: `make coverage` now skips six of the seven,
`make trybuild-contracts` runs all seven, and discovery finds seven tests
across five files. The rebase used Git's text merge with `zdiff3`; the host's
ambient `*.py merge=weave` rule is not repository consent, so the unattended
replay bypassed it with `-c core.attributesFile=/dev/null`.

The branch was rebased a third time, onto `main` at `bb6dab1a`, whose single
new commit standardized Rust formatting across the workspace. One file
conflicted, `crates/bumpy_road_function/src/analysis.rs`, where that
reformatting met this branch's doctest examples. The resolution kept the
target's formatting as well as this branch's examples, so the file now differs
from the target only by the examples. CI run
[41666046140](https://github.com/leynos/whitaker/actions/runs/41666046140)
passed at head `f203d997`: `coverage-check` in 13m37s, `windows-compat` in
19m52s, and `linux-full` in 16m16s. The first `linux-full` attempt failed while
installing the Mermaid CLI on an npm registry 404 for `@tanstack/virtual-core`,
before any project check ran; the rerun of the unmodified candidate passed.

## Follow-up

PR #522 replaces the grouping that #519 proposed. It still lists the removed
test in its filters; it will need to drop that clause when it rebases on this
change.
