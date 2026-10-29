# Architectural decision record (ADR) 009: verified Clippy snapshot for the Dylint driver bootstrap

## Status

Accepted (2026-10-29): Adopt Dylint 6.3.0, publish the Clippy history its
driver build reads as a commit-named, checksummed Git bundle, build the driver
on first use in an identity-keyed directory, and route every installed run
through `whitaker-installer run`.

## Date

2026-10-29.

## Context and problem statement

Running a Dylint lint library needs a driver: a small binary that links the
nightly compiler's `rustc_driver`. `cargo dylint` builds it on first use. The
driver's build script reads Clippy source at the revision matching the pinned
nightly, and Dylint selects that revision by walking Clippy's Git history.

Dylint 6.0.1 cloned public Clippy and checked out `master`. When Clippy renamed
its default branch to `main`, every cold driver build failed in every consumer,
including all Python lint lanes in Femtologging (issue #526,
[trailofbits/dylint#2146][dylint-2146]). Whitaker had delegated three things it
needed to control to the public network: the driver version, the Clippy
history, and the identity of the cached driver.

## Decision drivers

- A cold driver build must not depend on mutable public Clippy history.
- The pinned nightly (`nightly-2026-05-28`) must not change to suit a tool.
- A missing or invalid input must fail clearly rather than fall back.
- Concurrent cold runs must not race, and a driver built from other inputs
  must never be reused.
- Installations must survive relocation of the data directory.
- The existing prebuilt-tool and lint-library policies stay as they are.

## Options considered

### Dylint baseline

- **Upstream 6.3.0.** Fixes the branch name (`ac3323f`) and adds
  `DYLINT_CLIPPY_REPO` (`82da1fa`).
- **An immutable backport** of those two commits onto `v6.0.1`, if 6.3.0 had
  proved incompatible with the pinned nightly.

A cold probe on 2026-10-28 settled it. With an empty driver directory, a
`main`-only Clippy repository, public Clippy denied, and the real
`no_std_fs_operations` library, 6.3.0 built its driver, emitted the expected
diagnostic, reused the driver warm, and behaved the same under
`RUSTC_WRAPPER=sccache`. The backport was not needed.

### Clippy history

- **A source archive of one commit.** Insufficient: the selection walks
  history.
- **A shallow bundle.** Fragile: it encodes a guess about how far the walk
  reaches.
- **The full first-parent history of `main` as a Git bundle.** About 81 MB,
  verifiable with Git's own tools, and materialized as an ordinary local
  repository.

### Driver delivery

- **Prebuilt driver per platform and toolchain.** Dylint links the driver
  with an absolute `rpath` to the rustup toolchain, so a prebuilt driver is
  bound to the builder's paths and is not relocatable.
- **Build on first use.** Needs only the verified tools, the verified
  snapshot, and the pinned toolchain, all of which the installer already
  provisions.

## Decision outcome / proposed direction

### Baseline

Pin `cargo-dylint`, `dylint-link`, `dylint_linting`, and `dylint_testing` at
6.3.0. Rebase the vendored `dylint_testing` patch, which lets example targets
be found behind compiler wrappers, onto 6.3.0 and keep its regression tests.

### Snapshot format and identity

`installer/clippy-snapshot.toml`, compiled into the installer, pins the Clippy
`main` commit, the nightly, the revision Dylint selects for it, and the Dylint
release that selection belongs to. The release publishes
`whitaker-clippy-snapshot-<commit>.bundle` and a `.sha256` sidecar. The bundle
carries exactly `refs/heads/main` (at the pinned commit) and `HEAD`, with the
complete history behind them.

Git bundles are not byte-reproducible, so no bundle digest can be committed
ahead of publication. The trust anchor is instead the embedded commit object
ID. A repository whose only ref is `main` at that ID and which passes
`git fsck --full --strict` holds exactly the pinned history, because object IDs
hash both content and ancestry. The sidecar protects the transfer.

### Verification

One module, `whitaker_installer::clippy_snapshot`, serves the producer, the
producer's tests, and the installer. Installation runs these steps in order:

1. Verify the sidecar digest.
2. Run `git bundle verify` and check the bundle's heads.
3. Lock the snapshot root.
4. Clone into a temporary sibling.
5. Remove `origin`.
6. Verify the clone fully.
7. Write the generation metadata atomically.
8. Rename the clone to `<root>/<commit>/`.

The full verification requires all of the following:

- a bare repository;
- a symbolic `HEAD` to `main`;
- `main` at the pinned commit, and no other refs (so no `master`);
- non-shallow history;
- no alternates and no remotes;
- a clean `git fsck`;
- Dylint's selection, reproduced from the first-parent toolchain history,
  equal to the recorded revision, with that revision naming the pinned nightly.

An existing generation is verified and reused, or rejected; it is never
modified. Installation and verification permit only the `file` transport, so
they cannot reach a remote by accident. Only `whitaker-clippy-snapshot build`
may clone public Clippy, and only the producer runs it.

### No fallback

A missing, corrupt, or insufficient snapshot is an error in every source-policy
mode. Every such error says that Whitaker never falls back to cloning public
Clippy.

### Driver construction and identity

Build the driver on first use. `whitaker-installer run` exports
`DYLINT_DRIVER_PATH=<bootstrap>/drivers/dylint-<version>/clippy-<commit>`, and
Dylint adds the toolchain and host directory. A Dylint upgrade or a snapshot
re-pin therefore selects a new slot.

`run` holds a per-toolchain lock across the child process:

- If the driver exists and its ready marker is written, `run` takes the lock
  shared.
- Otherwise it takes the lock exclusively, re-checks, fully verifies the
  snapshot, removes any unmarked driver so Dylint builds from the snapshot just
  checked, runs, and writes the marker once the driver exists.
- A driver without a marker counts as cold.
- Dylint 6.3.0 re-probes a cached driver with `-V` and rebuilds it if the probe
  fails, so a warm run can still read the snapshot. Full verification takes
  seconds on the real snapshot, so warm runs do not pay it up front. Instead
  `run` compares the driver file's identity before and after the child, since
  Dylint persists a rebuild by renaming a new file into place. When a warm run
  rebuilt the driver, `run` verifies the snapshot before reporting success. A
  snapshot that fails fails the run and clears the ready marker, and the next
  cold run removes that driver before rebuilding.

### Installed layout and dispatch

The bootstrap lives beside the managed clone, never inside it, because the
clone directory is itself a Git working tree:
`<data root>/whitaker-bootstrap/{bootstrap.json,clippy/,drivers/}`.
`bootstrap.json` stores paths relative to the data root where possible, so a
relocated data root keeps working.

The `whitaker` and `whitaker-ls` wrappers contain no paths. They find
`whitaker-installer` beside themselves or on `PATH` and call
`whitaker-installer run`. That command overwrites `DYLINT_LIBRARY_PATH`,
`DYLINT_CLIPPY_REPO`, and `DYLINT_DRIVER_PATH`, preserves every other variable
(including `RUSTC_WRAPPER`), and exits with the child's status.

### Unchanged policies

The prebuilt-tool and lint-library policies (`deps/install.rs`, `prebuilt.rs`,
`cli/source_policy.rs`) are unchanged. The snapshot has no source fallback in
either mode.

## Consequences

- Consumers no longer read public Clippy at all. A Clippy history rewrite or
  rename cannot break a cold driver build.
- Each installation downloads about 81 MB once. Full verification costs about
  11 seconds, paid at installation and on the cold driver build; warm runs
  repeat only cheap ref checks.
- Snapshot assets are never replaced or deleted from the rolling release,
  because every installer version downloads the generation it pins.
- Re-pinning Clippy or moving the nightly means updating the manifest,
  recomputing the selected revision, and publishing a new bundle (see the
  developers' guide).
- Shell users who set the variables themselves (`--skip-wrapper`) do not get
  the driver lock and must not start concurrent cold runs.

[dylint-2146]: https://github.com/trailofbits/dylint/issues/2146
