# Architectural decision record (ADR) 0011: offline test execution with prepared metadata

## Status

Accepted (2026-11-06): Prepare every dependency and the pinned Dylint driver
once, online, before the test lane starts; run every test-owned nested Cargo
call offline; and answer the coverage lane's workspace metadata query from the
prepared file while routing its Nextest call to the standalone binary.

## Date

2026-11-06.

## Context and problem statement

Several tests and test harnesses start their own `cargo` processes: the
packaging test, the lint exclusion and no-default-features builds, the Dylint
UI harness, and the trybuild suites. None of them passed `--offline`,
`--locked`, or `--frozen`, so each could resolve dependencies or update the
crates.io index in the middle of a test run.

Every dependency-resolving Cargo command takes an exclusive lock on
`$CARGO_HOME/.package-cache`, and every agent, worktree, and repository on a
host shares one `$CARGO_HOME`. A nested call could therefore wait behind
another repository's Cargo for as long as that process held the lock; a wait of
about 19 minutes was observed locally on 24 October 2026. It could also take
the lock itself for a network round trip and stall everyone else.

The coverage lane adds a second problem. `cargo-llvm-cov` invokes Cargo with
`--cargo-metadata` and a `--manifest-path`, and Nextest rejects that flag
alongside `--cargo-metadata`. The lane needs a Cargo-shaped shim that answers
the metadata query and forwards the test run, without changing what either tool
observes.

## Decision drivers

- Move every download into one window, before any test is waiting.
- Make a missing dependency fail immediately and legibly, not hang.
- Keep the commit gate deterministic: no test may rewrite `Cargo.lock`.
- Leave the production installer's online path untouched.
- Preserve Cargo's own error reporting for a manifest that is missing or
  unreadable.
- Keep the coverage lane's metadata and Nextest invocation semantically
  identical to the real tools.

## Options considered

### Leave nested calls online

The status quo. Rejected: it is the defect. A nested call can hold or wait on
the shared lock for an unbounded time, and the failure looks like flakiness.

### Give each test a private `CARGO_HOME`

A private home removes the contention, but alternating homes invalidates build
fingerprints and forces rebuilds. Measured on the development host at 220 s and
123 s. Rejected: the cost falls on every run, not only contended ones.

### Fetch once, then run offline

Chosen. One online preparation step downloads the workspace lockfile and warms
the pinned Dylint driver; every test-owned nested call then runs with
`--frozen` or `CARGO_NET_OFFLINE=true`. A crate the preparation missed fails at
once with Cargo's offline error.

### A Nextest setup script instead of a Make prerequisite

A setup script would cover the Nextest lane only. `make coverage` reaches the
tests through `cargo-llvm-cov`, and the preparation also has to produce the
metadata files and the driver example that the coverage lane consumes. Rejected
in favour of a Makefile prerequisite that both lanes inherit.

## Decision outcome / proposed direction

### Preparation

`make fetch` is a prerequisite of `make test`, and `make coverage` inherits it
through its `make test` call. It runs, with `CARGO_NET_OFFLINE` cleared:

1. `cargo fetch --locked` for the workspace, which downloads every package in
   `Cargo.lock`, including crates only reachable through optional features.
2. The pinned Dylint driver warm-up through `dylint_testing::prepare_driver`,
   invoked by the `warm_driver` example.
3. Full workspace metadata for Nextest and `--no-deps` metadata for
   `cargo-llvm-cov`, written under the warm-up target directory.
4. A build of the `cargo_test_driver` example that the coverage lane uses as
   its Cargo shim.

A future committed standalone fixture manifest is fetched with its own
`cargo fetch --locked --manifest-path <m>`; the generated Dylint fixture has no
dependencies and needs none.

### Execution

- `CARGO_LOCKED` defaults to `--locked`, so the commit-gate targets cannot
  rewrite the workspace lockfile.
- Harness-owned nested Cargo commands pass `--frozen`.
- trybuild-generated projects and the lockfile-free Dylint fixture inherit
  `CARGO_NET_OFFLINE=true`, which lets Cargo write a temporary lockfile without
  network access.
- `installer/src/builder.rs` is unchanged. The installer runs on an end user's
  machine with an empty cache and must keep its network access.
- `ci.yml` and `coverage-main.yml` run `make fetch` before the test step, so a
  runner with an empty registry follows the same path as a developer.

### Prepared-metadata and Nextest routing boundary

`dylint_testing::cargo_test_driver` is the coverage lane's Cargo shim. It
serves the prepared `--no-deps` metadata only when all of the following hold:

- the request is `cargo metadata` with `--no-deps`, optionally behind a rustup
  toolchain selector;
- the working directory is the prepared workspace root;
- the prepared workspace's own root `Cargo.toml` can be opened;
- every `--manifest-path` selector resolves to that root manifest and can be
  opened.

Every other request reaches the real Cargo unchanged. A full metadata query is
forwarded because the prepared file carries no dependency graph and would
answer it with a silently incomplete document.

`cargo nextest run` is routed to the standalone `cargo-nextest` binary, and a
`--manifest-path` naming the prepared workspace is dropped, because Nextest
rejects it alongside `--cargo-metadata`. A selector naming any other workspace
is forwarded, since the prepared metadata describes the wrong workspace. The
drop is also conditioned on the working directory being the prepared root: from
anywhere else, dropping the selector would move the run to a different
workspace. Argument scanning stops at a bare `--`, because everything after it
belongs to the program Cargo runs rather than to Cargo.

### Manifest-error policy

A selector that cannot be resolved or opened is never treated as the prepared
manifest. The driver does not answer from metadata describing a file that is
gone; the real Cargo reports the problem, and Nextest routing keeps the
selector it would otherwise drop. The classifier reports the underlying
`io::Error` with its own `ErrorKind`, so a missing file stays distinguishable
from a permission problem, and failures that happen before a child exists name
the operation that failed.

## Goals and non-goals

- Goals:
  - One online preparation window per test run.
  - Deterministic, immediate failure for an unfetched dependency.
  - A coverage lane that behaves like the real Cargo and Nextest.
- Non-goals:
  - Removing the package-cache lock from offline Cargo calls; see the known
    limitation below.
  - Changing the installer's online build path.
  - Caching a driver built from different inputs.

## Known risks and limitations

- **Contention is reduced, not removed.** Measurement during issue #518 showed
  that `--offline`, `--frozen`, and `CARGO_NET_OFFLINE=true` all still take the
  exclusive `$CARGO_HOME/.package-cache` lock. No Cargo flag can satisfy the
  contention criterion, so coordinating nested calls so they never wait on the
  lock is tracked in issue #543. This decision delivers the offline preparation
  and fetch-once work.
- **Warm timing is unmeasured.** A like-for-like warm comparison is not
  obtainable on the shared development host, where compile admission dominates
  wall time. The host-exclusive comparison also belongs to issue #543.
- **A cold driver still needs the network.** The warm-up is the only step
  allowed to build the driver; a test process that bypasses `make fetch` on a
  fresh host fails rather than downloading.
- **The prepared metadata is a snapshot.** It describes the workspace at
  preparation time. A test that changes the manifest mid-run is forwarded to
  the real Cargo only if its selector no longer matches; the driver does not
  detect content changes.

## Consequences

- A missing crate fails fast with Cargo's offline error instead of waiting on
  the lock.
- The commit gate cannot rewrite `Cargo.lock`.
- The coverage lane needs no change to `cargo-llvm-cov` or Nextest.
- The driver's routing is a library module with unit, property, and
  process-boundary tests, so the shim is testable without the coverage lane.
- The preparation step is the single place to add a new fixture manifest or a
  new prepared artefact.
