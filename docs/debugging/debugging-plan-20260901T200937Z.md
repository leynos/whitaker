# Debugging Plan: Coverage UI example loses the `rstest` dependency

- **Generated:** 2026-09-01T20:09:37Z
- **Issue ID:** Whitaker PR #381, run 33552532676, job 100005211069
- **Severity:** High
- **Falsification sub-agent:** alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem Statement

The Namespace `coverage-check` job reaches the instrumented Nextest suite but
fails while `rstest_helper_should_be_fixture` UI cases compile examples. A
nested Cargo build passes an `--extern rstest=...` path to rustc, which then
reports `E0463: can't find crate for rstest`. The same committed suite passes
without coverage instrumentation, and the prior Namespace `linux-full` job
completed successfully. The coverage job should execute all 1,652 selected
tests without dependency artefacts disappearing or becoming unusable.

## Context Summary

| Aspect              | Details                                                             |
| ------------------- | ------------------------------------------------------------------- |
| First observed      | 2026-09-01, PR #381 run 33552532676                                 |
| Reproduction rate   | Two of two concurrent PR coverage jobs failed                       |
| Affected components | `cargo llvm-cov nextest`, Dylint UI example harness, Nextest groups |
| Recent changes      | `coverage-check` moved from UbiCloud to `namespace-profile-default` |

### Error Artefacts

```plaintext
error[E0463]: can't find crate for `rstest`
 --> .../examples/bootstrap_zero_diagnostic.rs:8:5
  |
8 | use rstest::rstest;
  |     ^^^^^^ can't find crate

FAIL rstest_helper_should_be_fixture::ui
     example_compiles_without_diagnostics::case_1
```

The failed job started about 12 seconds after it was queued on a Namespace
4-vCPU/16-GB runner. Its log shows concurrent Dylint UI cases and a nested
Cargo process waiting for the shared build-directory lock. The Nextest
configuration serializes several Dylint UI test names but does not explicitly
match
`rstest_helper_should_be_fixture::ui::example_compiles_without_diagnostics`.

### Information Gaps

The current log does not reveal whether the referenced `librstest` file was
absent, partially replaced, or incompatible when rustc opened it. The Code
Scene cache action also reports write failures, but no cache read error or
compiler failure before the nested Cargo invocation.

______________________________________________________________________

## Hypotheses

### H1: The failing UI cases bypass the serial Dylint group

**Claim**: The Nextest override omits the
`example_compiles_without_diagnostics` and
`example_harness_collects_call_site_evidence` names, so their nested Cargo
builds race other Dylint UI cases in the shared coverage target directory.

**Plausibility**: High — the log shows concurrent UI cases, build-directory
lock contention, and a dependency artefact failure; the configuration already
documents this exact shared-target risk for other UI names.

**Prediction**: If this hypothesis holds, Nextest metadata assigns the failing
cases no `serial-dylint-ui` group, and the focused coverage run succeeds after
the filter includes them.

#### H1 Falsification Plan

| Step | Action                                                                      | Expected Negative Result                           |
| ---- | --------------------------------------------------------------------------- | -------------------------------------------------- |
| 1    | Inspect `cargo nextest list` metadata for the two exact test names          | Either case already belongs to `serial-dylint-ui`  |
| 2    | Run the focused coverage cases after adding only the missing filter clauses | `E0463` still occurs while the cases run serially  |
| 3    | Repeat the focused serial coverage run several times                        | Any serial repetition reproduces the missing crate |

**Tooling**: `cargo nextest list`, `cargo llvm-cov nextest`, temporary Nextest
configuration or a minimal filter patch, and the pinned repository toolchain.

**Confidence on falsification**: High. A reproducible failure with confirmed
single-threaded nested UI execution rules out inter-test target contention.

#### H1 execution checkpoint

- The focused metadata command selected
  `example_compiles_without_diagnostics::case_1` and
  `example_harness_collects_call_site_evidence` from
  `rstest_helper_should_be_fixture` under the `ci` profile. lists the exact
  failing UI example cases under `rstest_helper_should_be_fixture::ui`.
- `.config/nextest.toml` matches the existing `driver::ui`, `tests::ui`,
  `ui::ui`, and named `no_unwrap_or_else_panic` clauses, plus
  `(binary(ui) & test(=ui))`, which does not mention either failing example.
- The isolated coverage command used a fresh
  `/data/tmp/whitaker-cov-h1.*` target directory and selected
  `example_compiles_without_diagnostics::case_1` under the `ci` profile. passed
  in isolation, so coverage instrumentation alone does not reproduce the
  `E0463` failure.
- The same focused pair run with `-j 2` and with `-j 1` both passed:
  - The concurrent run used a fresh
    `/data/tmp/whitaker-cov-h1-concurrent.*` target directory and `-j 2`.
    -> `2 tests run: 2 passed (2 slow), 39 skipped`
  - The serial run used a fresh `/data/tmp/whitaker-cov-h1-serial.*` target
    directory and `-j 1`.
    -> `2 tests run: 2 passed, 39 skipped`
- H1 does not reproduce on the exact failing pair under either concurrent or
  serial execution, so the missing `serial-dylint-ui` mapping is not the active
  cause of the recorded `E0463`.

______________________________________________________________________

### H2: Coverage instrumentation alone makes the nested dependency unusable

**Claim**: `cargo llvm-cov nextest` produces an `rstest` artefact that the
Dylint example's nested Cargo invocation cannot load, independently of test
concurrency.

**Plausibility**: Medium — the error occurs only in the coverage job, but the
log's build-lock contention provides a more specific explanation.

**Prediction**: If this hypothesis holds, one isolated failing case still
reports `E0463` in a fresh coverage target directory.

#### H2 Falsification Plan

| Step | Action                                                                      | Expected Negative Result                |
| ---- | --------------------------------------------------------------------------- | --------------------------------------- |
| 1    | Run one failing case under `cargo llvm-cov nextest` with no competing tests | The isolated instrumented case succeeds |
| 2    | Compare with the same exact case under ordinary `cargo nextest`             | Both isolated variants succeed          |

**Tooling**: a fresh task-specific target directory, the exact Nextest test
expression, and `cargo llvm-cov` 0.6.24.

**Confidence on falsification**: High. A clean isolated coverage pass shows
instrumentation is insufficient to trigger the defect.

#### H2 execution checkpoint

- The isolated coverage run used a fresh `/data/tmp/whitaker-h2-cov.*` target
  directory, `cargo llvm-cov nextest --no-report -j 1`, and the failing case.
  passed with `1 test run: 1 passed (1 slow), 40 skipped`.
- The ordinary control used a fresh `/data/tmp/whitaker-h2-nextest.*` target
  directory, `cargo nextest run -j 1`, and the same test filter. passed with
  `1 test run: 1 passed, 40 skipped`.
- H2 is falsified: the exact failing case does not depend on
  `cargo llvm-cov nextest` instrumentation when run alone in a fresh target
  directory.

______________________________________________________________________

### H3: The GitHub Actions sccache backend corrupts the dependency artefact

**Claim**: The 515 sccache write failures leave the referenced `rstest` rlib in
an unusable state for the nested Cargo build.

**Plausibility**: Low — sccache reports zero read errors and zero compilation
failures, while the missing crate appears during an unwrapped nested Cargo
build.

**Prediction**: If this hypothesis holds, disabling `RUSTC_WRAPPER` makes the
isolated or concurrent coverage case reliable without changing Nextest grouping.

#### H3 Falsification Plan

| Step | Action                                                            | Expected Negative Result                                 |
| ---- | ----------------------------------------------------------------- | -------------------------------------------------------- |
| 1    | Run the concurrent focused coverage cases with sccache disabled   | The same `E0463` failure recurs                          |
| 2    | Inspect whether the nested Cargo command inherits `RUSTC_WRAPPER` | It does not use sccache for the failing rustc invocation |

**Tooling**: the focused coverage expression, `RUSTC_WRAPPER=`, and captured
verbose Cargo output.

**Confidence on falsification**: Medium. A recurrence without sccache removes
the cache backend from the causal path, although it would not explain every
possible stale filesystem artefact.

#### H3 execution checkpoint

- The focused test used a fresh `/data/tmp/whitaker-h3-norustcwrapper.*`
  target directory with `cargo llvm-cov --no-rustc-wrapper nextest` and the
  failing-case filter. passed with `1 test run: 1 passed, 40 skipped`.
- Combined with the earlier isolated coverage and ordinary nextest controls,
  disabling the wrapper does not change the outcome for the exact testcase.
- H3 is falsified: the wrapper and sccache backend are not the cause of the
  recorded `E0463` on the isolated failing case.

#### H4 candidate hypothesis

The recorded `E0463` requires another concurrent Dylint UI path from the full
coverage job, not just the exact `rstest_helper_should_be_fixture` testcase.
The next falsifiable step is to rerun the minimal CI-shaped subset that was
failing in the original job log and add the smallest additional Dylint UI case
from the same target directory until the failure reappears or stays gone.

#### H4 execution checkpoint

- The concurrent run used a fresh `/data/tmp/whitaker-h4-concurrent.*` target
  directory, both lint packages, the two named filters, and `-j 2`. passed with
  `2 tests run: 2 passed (1 slow), 64 skipped`.
- The serial control used a fresh `/data/tmp/whitaker-h4-serial.*` target
  directory, the same packages and filters, and `-j 1`. passed with
  `2 tests run: 2 passed, 64 skipped`.
- H4 is falsified: the cross-crate sibling Dylint UI pair does not reproduce
  the recorded `E0463` in either concurrent or serial mode.

#### H5 candidate hypothesis

The remaining full-suite-specific factor is a third Dylint UI binary or another
workspace test binary that was active in the original coverage job but is not
part of the bounded two-case subset. The next falsifiable step is to add one
more nested-Cargo UI path from a different crate while keeping the same fresh
coverage target directory and compare concurrent versus serial execution again.

#### H5 execution checkpoint

- `make coverage` used a fresh `/data/tmp/whitaker-h5-coverage.*` target
  directory via `CARGO_TARGET_DIR`. completed successfully with
  `1652 tests run: 1652 passed (10 slow), 5 skipped`.
- The checked-in CI-shaped coverage command did not reproduce the recorded
  `E0463` in a fresh target directory, so the remaining hypothesis is
  runner/resource-specific rather than a stable local testcase interaction.

#### H5 resolution

Both Namespace `coverage-check` jobs failed in the nested-Cargo
`rstest_helper_should_be_fixture::ui` cases with `E0463`, while the fresh local
full-coverage run passed all 1,652 selected tests. The recorded CI failure and
the nextest metadata show that the existing `serial-dylint-ui` filter omitted
`example_compiles_without_diagnostics` and
`example_harness_collects_call_site_evidence`; the directly equivalent
`trybuild_fixtures_compile_without_diagnostics` case was omitted too.

Extend that existing, narrow test group for these three named harnesses. This
does not change lint production behaviour, serialize the whole suite, or add
retries. It prevents concurrent nested Cargo/compiler work against shared
target resources on the constrained Namespace runners. The checked-in
`tests/nextest_ui_filter.rs` contract must keep the three clauses present.

#### H6: Unmapped sibling example harnesses race the mapped case

**Claim**: The fresh Namespace run's
`no_unwrap_or_else_panic::ui::example_compiles_under_test_harness::case_1`
already belongs to `serial-dylint-ui`, but two sibling example harnesses in the
same crate do not. They can therefore run concurrently with the mapped case and
overwrite the coverage-instrumented target artefacts that nested Cargo uses.

**Prediction**: Nextest's resolved default-profile group contains the observed
failing case but omits
`hand_written_test_companion_does_not_exempt_parent_function` and
`aliased_test_crate_non_companion_does_not_exempt_parent_function`. Both
omitted tests call the same `run_example_under_test_harness` helper.

**Falsification**: If either omitted test is already in `serial-dylint-ui`, or
if it does not call that helper, this scheduling extension would not be
supported.

#### H6 execution checkpoint

- `cargo nextest show-config test-groups` with the Makefile's dynamic-linking
  `RUSTFLAGS` proved the failing `example_compiles_under_test_harness` cases
  are already in `serial-dylint-ui`.
- Source inspection found five, and only five, non-Windows callers of
  `run_example_under_test_harness` in `no_unwrap_or_else_panic`. The existing
  group covered three; it omitted the handwritten-companion and aliased-test
  negative cases.
- The analogous `no_expect_outside_tests` harnesses are excluded by the
  checked-in `TEST_EXCLUDES`, so they are not active in `make coverage` and do
  not belong in this narrowly scoped scheduling group.
- Extend the filter and its checked-in contract by exactly the two omitted
  `no_unwrap_or_else_panic` clauses. This completes the active shared-target
  set without serializing unrelated tests or changing production lint code.
- A fresh `cargo llvm-cov nextest --no-report` run selected the completed set
  and passed. The resolved group contained all seven concrete executions: three
  parameterized example cases and four single negative cases.

#### H7: Sequential Dylint example builds mutate a non-coverage target

**Claim**: `cargo llvm-cov` passes its instrumented target directory to Nextest
as `--target-dir`, but does not export it as `CARGO_TARGET_DIR` to the test
process. `dylint_testing::Test::example` therefore rebuilds each example in the
workspace's ordinary `target` directory while inheriting coverage `RUSTFLAGS`.
Sequential Dylint test processes share and mutate that directory, leaving Cargo
with a fresh fingerprint for `rstest` but a missing usable rlib.

**Prediction**: The nested Cargo command reports paths below `target/debug`, not
`target/llvm-cov-target`; a unique `CARGO_TARGET_DIR` held for one runner
closure gives that nested build self-consistent dependencies and prevents the
`E0463` failure.

**Falsification**: If the nested command already uses the LLVM coverage target,
or if a uniquely assigned target still reports `E0463`, target sharing is not
the active cause.

#### H7 execution checkpoint

- Namespace run `33562381054` ran `cargo nextest` with
  `--target-dir .../target/llvm-cov-target`, while the failed nested Cargo
  command emitted both the example and `rstest` paths under `.../target/debug`.
- The preceding aliased-companion case passed in the same serial test group;
  the first `example_compiles_under_test_harness` case then failed three times.
  This falsifies an inter-test concurrency-only explanation.
- `dylint_testing` 6.0.1 removes and rebuilds example artefacts in the target
  directory returned by Cargo metadata. Its source explicitly records a
  temporary target directory as the unresolved safer alternative.
- Test a scoped `CARGO_TARGET_DIR` guard in Whitaker's shared UI test helper.
  It must cover preparation and the runner closure, restore the ambient
  environment, and leave lint production code unchanged.

#### H7 decision

Use the coverage target already created by `cargo-llvm-cov`, rather than a new
target per example. This decision was falsified by Namespace run `33564463057`:
the normal coverage driver did not export `CARGO_LLVM_COV_TARGET_DIR` to test
processes, so the UI helper could not derive that target.

- The helper mapping is removed rather than retaining a source of driver
  assumptions or its resulting `option_option` and `shadow_reuse` lints.

#### H8: The coverage driver needs an explicit shared Cargo target

**Claim**: `cargo-llvm-cov` 0.6.24 passes its coverage directory to Nextest
only with `--target-dir`. Its `show-env` subcommand prints
`CARGO_LLVM_COV_TARGET_DIR`, but the normal Nextest invocation does not export
that variable to the test process. The H7 helper therefore cannot derive the
outer target on CI and leaves nested Dylint Cargo builds in `target/debug`.

**Prediction**: If the `coverage` recipe explicitly sets both
`CARGO_LLVM_COV_TARGET_DIR` and `CARGO_TARGET_DIR` to the same exact coverage
target before invoking `cargo llvm-cov`, the outer Nextest command and every
nested Cargo build use that directory. A verbose selected UI run will report
the nested example `--out-dir` below that target, rather than `target/debug`.

**Falsification**: If the local verbose nested Cargo command still emits an
ordinary `target/debug` path with both variables set, Cargo does not inherit
the explicit target boundary and this explanation is incomplete.

#### H8 execution checkpoint

- cargo-llvm-cov `v0.6.24` source confirms the claim: `show-env` prints
  `CARGO_LLVM_COV_TARGET_DIR`, whereas `run_nextest` calls `set_env` and passes
  its target only through `--target-dir`.
- The coverage recipe now exports both variables as
  `$(CURDIR)/target/llvm-cov-target`, so 0.6.24 consumes the former as its
  exact outer target and nested Dylint Cargo inherits the latter.
- A fresh verbose selected coverage run used
  `/data/tmp/whitaker-h8-coverage.dXwowg` as Nextest's `--target-dir`; all three
  `example_compiles_under_test_harness` cases passed without `E0463`.

#### H8 decision

Keep coverage target selection at the Makefile command boundary. The shared UI
helper remains responsible only for Dylint's known `RUSTC_WRAPPER` and Windows
`VCPKG_ROOT` adjustments; it must not infer cargo-llvm-cov implementation
details. The Makefile contract requires the two outer/nested Cargo variables to
remain equal.

______________________________________________________________________

## Recommended Execution Order

1. Extend `serial-dylint-ui` with the five active named nested-Cargo clauses.
2. Prove them with `tests/nextest_ui_filter.rs` and run the focused UI suite.
3. Run the repository formatting, typecheck, lint, and coverage gates.

## Termination Criteria

- **Scheduling remediation accepted**: the five named nested-Cargo cases
  match `serial-dylint-ui`, the focused regression is reliable, and the
  complete coverage target passes.
- **Escalation trigger**: a Namespace coverage job still reports `E0463` after
  the five active tests are visibly in this group, or the failure signature
  changes.

## Notes for Executing Agent

Work only in the `adopt-namespace-runners` worktree. Do not change production
lint behaviour or globally serialize the suite. Extend the existing
`serial-dylint-ui` filter and retain its no-blanket-retry policy. Record the
focused commands and complete `make coverage` result in this plan before
returning the branch for integration.
