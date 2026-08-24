# Debugging Plan: M0 core integration-test linkage

**Generated**: 2026-08-24
**Issue ID**: EP-M0 gate failure
**Severity**: medium
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem Statement

After extracting `whitaker_lint_core`, `make test NEXTEST_PROFILE=ci` fails
while linking its `lint_template` integration-test target. The milestone
requires the six formerly excluded test targets to run, so the correction must
preserve that coverage without suppressing a lint or weakening the gate.

## Context Summary

| Aspect | Details |
| --- | --- |
| First observed | 2026-08-24, first M0 full gate run |
| Reproduction rate | deterministic under workspace `--all-features` |
| Affected components | core test target and Dylint driver feature |
| Recent changes | root driver library moved to `crates/whitaker_lint_core` |

### Error Artefacts

```plaintext
cannot satisfy dependencies so std/core/alloc only shows up once
feature(rustc_private) is needed to link to rustc_driver
```

## Hypotheses

### H1: Feature unification enables the compiler driver in the test executable

**Claim**: Workspace `--all-features` enables `whitaker_lint_core`'s
`dylint-driver` feature through each lint crate, causing its integration-test
executable to link `rustc_driver` with ordinary dependencies.

**Plausibility**: High — the failing target is the core integration test and
the linker explicitly names `rustc_driver`.

**Prediction**: The focused test passes without the feature and reproduces the
link failure when it is enabled directly.

#### H1 Falsification Plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Run `cargo test -p whitaker_lint_core --test lint_template --no-default-features --no-run`. | A link failure disproves H1. |
| 2 | Run the same command with `--features dylint-driver`. | Success disproves H1. |

**Tooling**: Cargo only; do not edit tracked files or run a repository gate.

**Confidence on falsification**: High. The two commands vary only the feature
that owns the compiler-private runtime.

## Recommended Execution Order

1. **H1** — it is the smallest decisive experiment and fully explains the
   captured linker diagnostics.

## Termination Criteria

- **Root cause identified**: H1 survives both focused tests.
- **Escalation trigger**: Either result contradicts H1; revise the debugging
  plan before editing implementation code.

## Notes for Executing Agent

Use the shared Cargo cache. Capture only concise outputs and report a verdict
of falsified, not-falsified, or inconclusive.
