# Debugging plan: wrapped rustc breaks example UI tests

## Problem statement

The Linux coverage job runs the `rstest_helper_should_be_fixture` example UI
harness with `RUSTC_WRAPPER=sccache`. Three example tests fail because
`dylint_testing` reports that it found no bare `rustc` invocation in Cargo's
JSON output.

## Hypotheses

### H1: the wrapper is not cleared on Linux (high confidence)

`run_with_runner` calls a Windows-only environment guard. On non-Windows
targets that guard is a no-op, so Cargo invokes `sccache rustc` and
`dylint_testing` cannot recognize the invocation it expects.

- Prediction: an ordinary targeted UI test fails with the reported error when
  `RUSTC_WRAPPER=sccache` is set and passes when it is unset.
- Falsification: run the same targeted case in both environments. If the
  outcome does not depend on the wrapper, reject this hypothesis.
- Investigator: alchemist agent.

### H2: LLVM coverage output is independently incompatible (medium confidence)

Coverage instrumentation or its target directory may alter Cargo output in a
way that prevents `dylint_testing` from finding the compiler command.

- Prediction: the test passes under ordinary nextest with the wrapper set but
  fails under `cargo llvm-cov nextest`.
- Falsification: reproduce the exact failure outside coverage with the wrapper
  set.

### H3: parallel package-cache contention truncates output (low confidence)

Concurrent example harnesses may interfere while Dylint builds its driver.

- Prediction: the failure disappears when the affected case runs alone.
- Falsification: reproduce the failure in a single targeted test process.

## Intended fix boundary

If H1 survives, make the existing runner environment guard clear and restore
`RUSTC_WRAPPER` on every platform while retaining the Windows-only
`VCPKG_ROOT` adjustment. Add a Linux-capable regression test around
`run_with_runner`; do not disable sccache for the wider CI step.

## Validation

- Run the targeted test with the inherited wrapper.
- Run `make check-fmt`, `make lint`, `make typecheck`, and `make test`.
- Run `make coverage` to exercise the original CI path.

## Outcome

H1 survived falsification. The exact UI case failed outside coverage with
`RUSTC_WRAPPER=sccache` and passed when the variable was unset. This rejects H2
and H3 as necessary causes. The fix widens the runner-scoped wrapper guard to
all platforms while retaining the Windows-only `VCPKG_ROOT` behaviour.

The targeted case then passed with `RUSTC_WRAPPER=sccache`. The repository
formatting, lint, typecheck, test, Markdown, and Mermaid gates passed. Finally,
`RUSTC_WRAPPER=sccache make coverage` passed all 1,475 executed tests and wrote
the LCOV report.
