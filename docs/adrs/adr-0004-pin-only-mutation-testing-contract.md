# Architectural decision record (ADR) 0004: pin-only mutation-testing contract

## Status

Accepted (2026-07-22): Keep Whitaker's mutation-testing adoption pin-only and
contract-test its declared configuration rather than claiming parity with the
continuous integration (CI) test baseline.

Amended (2026-11-11): Select each package's default features and guard the
baseline on every pull request; see [Amendments](#amendments).

Amended (2026-11-28): Mutate every workspace member with `--workspace` and
require change detection to cover each member; see [Amendments](#amendments).

## Date

2026-07-22. Amended 2026-11-11 and 2026-11-28.

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

*Table 1: Comparison of mutation-testing options.*

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

## Amendments

2026-11-11: default features and a pull-request baseline guard. The decision
stands; what changes is the caller's feature selection and the pull-request
check that holds it.

### Context

Run 33961579896 (2026-09-05) failed every one of its six shards before testing
a mutant. The shared workflow reported
`mutation_cargo_outcome=baseline tests failing`, and each target failed to
compile with 19 errors of the form:

```text
error: cannot satisfy dependencies so `alloc` only shows up once
  = help: `feature(rustc_private)` is needed to link to the compiler's
    `rustc_driver` library
```

The failing targets were the lint crate's integration-test binaries
(`ui_harness`, `lint_template`, and `config_loading`; reported as `whitaker` at
the time, and now `whitaker_lint_core` after the 3.5.1 extraction). The caller
passed `extra-args: --all-features`, which enabled `dylint-driver` across the
workspace. Cargo then unified that feature into the integration-test binaries,
and a test binary in that configuration cannot link the compiler's crates.

Nothing in the estate had noticed. The main test lane excludes the affected
package from its all-feature pass and runs its tests feature-free, and no other
CI step links a test binary in the driver configuration. The mutants job was
also failing in its post step until `leynos/shared-actions#460`, and scheduled
runs were finding zero mutants, so no run reached the baseline.

### Decision

Remove `extra-args: --all-features` from the caller. cargo-mutants runs once
per package directory and now tests each package with its default features: the
lint crates' default set excludes `dylint-driver`, and the root `whitaker`
package declares no features at all.

Add `make mutation-baseline`, which runs the default-feature baselines of the
root `whitaker` package and of `whitaker_lint_core`, whose `ui_harness`,
`lint_template`, and `config_loading` binaries the original run failed to link.
Each command carries no feature flags and only the lane's standard
`-D warnings`, with none of the Dylint dynamic-linking flags. Run the target in
the `coverage-check` pull-request lane. The root package declares no features,
so its default set is its whole set; what no other lane builds is the plain
shape. The main test lane and `make coverage` inject
`-C prefer-dynamic -Z force-unstable-if-unmarked`, `test-root-lib` builds only
the root library, and the core package's ordinary pass uses Nextest with
`--no-default-features`, so this is the check that notices a baseline which
stops linking. Contract tests pin the commands, their placement after checkout
and Rust setup, the absence of skip or environment overrides, the caller's
feature selection, and the root package's default features.

The trunk writer compiles the same shapes. `coverage-warmup` runs
`make mutation-baseline-build`, the compile-only twin, so the guard adds no
cold build to a pull request and the twin runs no test. The job's other steps
keep their existing behaviour, including the doctest run and the two nested
build guards `make coverage-build` delegates to.

### Options rejected

- **Add a `features` input to `mutation-cargo.yml`.** The shared workflow
  already accepts shell-lexed `extra-args`; the defect is what `--all-features`
  means, not how the argument travels. A `features` input is a reasonable
  upstream convenience and remains a possible follow-up, but it would not have
  changed this failure while `--all-features` was the value passed.
- **Inject `-C prefer-dynamic -Z force-unstable-if-unmarked` for the affected
  targets.** The shared workflow has no per-crate RUSTFLAGS mechanism, and a
  workspace-wide flag breaks `cargo install` for `whitaker-installer`. This is
  the limitation the original decision already recorded.
- **Exclude `whitaker_lint_core` and the lint crates from mutation.** That
  would make the baseline pass by removing production code from scope, which is
  Option B of the original decision and was already rejected.

### Consequences

- Mutation no longer covers code gated on `dylint-driver`. `make publish-check`
  still builds every lint crate in that configuration, so the driver build is
  exercised, but it is not mutated.
- A caller that reintroduces `--all-features`, or any spelling that reaches
  `dylint-driver`, fails the workflow contract tests rather than a nightly run.
- The guard adds a root-package and a lint-core test execution to every pull
  request. They are distinct from the all-feature run: the root package
  declares no features, the all-feature run builds its test binaries with
  prefer-dynamic flags, `test-root-lib` builds only the root library, and the
  core package's ordinary pass uses Nextest with `--no-default-features`.

### Amendment (2026-11-28): workspace mutation scope

#### Workspace-scope context

[Issue 412](https://github.com/leynos/whitaker/issues/412) reported a scheduled
run that found zero mutants, recorded `all mutants caught`, and stayed green.
`cargo-mutants` mutates the current package unless given `--workspace`. This
workspace has a root package beside its members and sets no `default-members`,
so the caller's root-package selection could never enumerate a changed file
under `common/`, `crates/`, `installer/` or `suite/`, even though `paths`
listed those directories for change detection. `cli/` was also absent from
`paths`, so a cli-only change scheduled no run at all.

#### Workspace-scope decision

Pass `extra-args: --workspace --test-tool nextest`, retaining the
default-feature selection of the 2026-11-11 amendment, and install nextest in
each mutants job through `setup-commands`. Nextest's process-per-test isolation
matches every other gate; the installer tracing test depends on it, and
`cargo test` shares global callsite interest between tests. Add `cli/` to
`paths`, and exclude the vendored `dylint_testing` fork from mutation scope:
`--workspace` reaches it, but its surviving mutants describe upstream's suite
rather than this repository's.

Also exclude source the default-feature baseline never compiles: the lint
crates' `dylint-driver`-gated driver, config, diagnostics, context, and helper
modules, `suite/src/driver.rs`, the Kani harnesses, and the Windows-only
installer test module. `cargo-mutants` enumerates module files without
evaluating their `cfg`, so an unexcluded gated module would report survivors
for code the tests never built. Kani-only items that share a file with
production code are filtered by mutant name, so the compiled code in that file
stays mutable.

Ask Cargo for the effective workspace members and every declared target in the
workflow contract tests, and require each source to appear in `paths` or be
matched by `exclude-globs`. A negative control covers a custom target outside
`src/`, and another reports a source that neither setting covers.

The baseline guard from the 2026-11-11 amendment still covers the two shapes
the original all-feature run failed to link, the root package and
`whitaker_lint_core`. Other members are mutated with their own default
features; the pull request does not claim a full `--test-workspace` baseline.

#### Workspace-scope consequences

- Mutation enumerates changed files in every workspace member, so the scoped
  scheduled lane and the manual dispatch mutate the tree that `paths` describes.
- A workspace source added without a `paths` prefix or a matching exclusion
  fails the workflow contract tests.
- The vendored fork and source compiled only outside the default features
  remain out of mutation scope. `docs/developers-guide.md` records the
  operational detail in "Mutation-testing workflow contract tests".
- The local zero-mutant cause is fixed. The pinned shared workflow already
  fails a readable, empty, unsharded inventory by default, so an excluded-only
  change set fails rather than reporting success. Detection does not yet see
  the caller's exclusions, so that run is a failure rather than a distinct
  skip; making detection exclusion-aware is an upstream change in
  `leynos/shared-actions`, not here.
