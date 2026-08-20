# Architectural decision record (ADR) 004: pin-only mutation-testing contract

## Status

Accepted (2026-07-22): Keep Whitaker's mutation-testing adoption pin-only and
contract-test its declared configuration rather than claiming parity with the
continuous integration (CI) test baseline.

## Date

2026-07-22.

## Context and problem statement

Whitaker delegates mutation testing to the shared
`leynos/shared-actions/.github/workflows/mutation-cargo.yml` workflow. The
caller supplies workspace paths, exclusion globs, and `--all-features`, but the
shared workflow cannot reproduce the per-invocation `RUSTFLAGS` used by the
repository's `test` and `typecheck` Makefile targets.

Those flags are required by the `whitaker`, `function_attrs_follow_docs`,
`module_max_lines`, and `no_expect_outside_tests` crates when the
`dylint-driver` feature enables `feature(rustc_private)`. Applying the flags
workspace-wide is not viable because, as documented in `.cargo/config.toml`,
they break `cargo install` for `whitaker-installer`.

The decision is whether the mutation workflow should claim a workspace baseline
equivalent to `make test`, or expose the closest safe mutation scope and test
only that declared configuration.

## Decision drivers

- Keep the mutation workflow usable without breaking installer builds.
- Describe the mutation scope honestly rather than imply CI parity.
- Detect accidental caller drift while permitting automated commit-pin updates.
- Keep mutation testing informational and independent of pull-request gates.

## Options considered

| Option                                                  | Shared-workflow feasibility                                                                         | Mutation coverage                                                               | Operational risk                                                                   | CI-baseline claim / verification                                                                 | Rationale                                                                               |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| A: require full CI-baseline parity                      | Unsafe and unimplementable: the current shared workflow cannot inject the required flags per crate. | Aims for full workspace coverage equivalent to `make test`.                     | Applying the flags workspace-wide breaks `cargo install` for `whitaker-installer`. | Would claim parity, but cannot verify it through the shared workflow.                            | Missing per-crate flags make this option unsafe and unimplementable.                    |
| B: exclude every crate that needs dynamic-linking flags | Runs reliably with the shared workflow by removing the affected crates.                             | Omits important production code, making the reported scope less useful.         | Lower run risk, but accepts the risk of untested affected crates.                  | Makes no full-baseline claim; verification covers only the reduced scope.                        | Reliable execution deliberately omits important affected crates.                        |
| C: adopt a pin-only declared-configuration contract     | Closest safe approximation supported by the shared workflow.                                        | Retains useful informational coverage without claiming full workspace coverage. | Keeps mutation testing informational and independent of pull-request gates.        | Verifies the caller's declared security and configuration shape, not a full workspace assertion. | A declared-shape contract is the closest safe approximation, not full workspace parity. |

_Table 1: Comparison of mutation-testing options._

## Decision outcome / proposed direction

Adopt Option C.

In the context of shared mutation testing, facing incompatible per-crate
compiler-flag requirements, Whitaker chooses a pin-only declared-configuration
contract over either claimed CI parity or excluding every affected crate, to
retain useful informational mutation coverage while accepting that the run
cannot reproduce `make test` crate-for-crate.

The caller remains pinned to a full commit SHA. Its contract test validates the
pin shape, permissions, concurrency, triggers, and declared inputs. It does not
hard-code the pin value or assert successful full-workspace mutation testing.

## Known risks and limitations

- Mutation results do not certify the same crate scope as `make test`.
- Crates needing the dynamic-linking flags may fail or provide incomplete
  mutation results under the shared workflow.
- The decision must be revisited if the shared workflow gains a safe per-crate
  mechanism for supplying the required flags.
