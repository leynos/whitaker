# Make the Dylint driver bootstrap reproducible

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

Issue: [leynos/whitaker#526](https://github.com/leynos/whitaker/issues/526)

Upstream defect:
[trailofbits/dylint#2146](https://github.com/trailofbits/dylint/issues/2146)

## Purpose / big picture

A cold Dylint driver build in a consuming repository fails once Rust Clippy's
default branch is renamed from `master` to `main`. Dylint 6.0.1 resolves the
revision it needs for the pinned nightly by cloning
`https://github.com/rust-lang/rust-clippy` and checking out `master`. Clippy
has since deleted `master`, so the clone either fails outright or checks out
nothing, and the driver build dies before any lint runs.

The producer must therefore own three things it currently delegates to the
public network: the Dylint driver version, the Clippy history Dylint's
selection algorithm reads, and the driver cache identity. After this change,
the Clippy history is a committed, checksummed Git bundle published with each
release, the driver is built on first use inside an identity-keyed directory,
and public Clippy access is never needed or used.

## Constraints

- Keep `rust-toolchain.toml` at `nightly-2026-05-28`; the separate vendor-test
  lane and `-D warnings` enforcement stay as they are.
- Preserve the vendored `dylint_testing` compiler-wrapper parsing patch and its
  regression tests.
- Never fall back to public Clippy when a managed snapshot is missing or
  invalid, in either source-policy mode.
- Leave the prebuilt-tool and lint-library policies (`deps/install.rs`,
  `prebuilt.rs`, `cli/source_policy.rs`) unchanged.
- Do not weaken lint scopes or warning enforcement.

## Tolerances

- If Dylint 6.3.0 had proved incompatible with the pinned nightly, the
  fallback was a two-commit immutable backport onto `v6.0.1`; the probe made
  that unnecessary.
- Hosted-runner and post-publication checks cannot run before a release
  exists. They are written and contract-tested here, and their first real run
  is recorded as follow-up rather than claimed.

## Risks

- Git bundles are not byte-reproducible, so the bundle digest cannot be
  embedded in the installer ahead of publication. Mitigation: the trust anchor
  is the embedded commit object ID, checked together with `git fsck --full`,
  and the sidecar digest protects transport integrity.
- Several agents share `~/.dylint_drivers`. A driver built elsewhere can be
  reused silently. Mitigation: every Whitaker path sets an identity-keyed
  `DYLINT_DRIVER_PATH`.

## Progress

- [x] (2026-10-28) Cold and warm 6.3.0 probe against the unchanged nightly and
  a `main`-only snapshot, with public Clippy denied.
- [x] (2026-10-29) Bumped every Dylint pin to 6.3.0 and rebased the vendored
  `dylint_testing` patch onto upstream 6.3.0; `diff -ru` against the pristine
  crate shows only the wrapper-parsing patch and one scoped Clippy allow.
- [x] (2026-10-29) Fixed the UI-harness library clobber introduced by 6.3.0
  and the rebased call-site regression in the wrapper patch (see Surprises).
- [x] (2026-10-29) Clippy snapshot module, manifest and
  `whitaker-clippy-snapshot` binary. The real bundle (81 MB, SHA-256
  `8b7e4716285185e9c799e48fe8f05096b73d31f0467806f0b8290476f408810c`) builds
  from a local mirror, self-verifies, installs in about 21 s, and both Dylint's
  own `Revs::channel_iter` and Whitaker's reproduction select `9fca3bc9` from
  it.
- [x] (2026-10-29) Producer provisioning in the Makefile and CI, replacing the
  mirror. `make dylint-smoke` built the driver cold (68 s) and reused it warm
  (21 s, byte-identical driver `fa0b26c0…`) with public Clippy unreachable from
  this host (`git ls-remote` canary exit 128). ADR 009 and the developers'
  guide section were written alongside.
- [x] (2026-10-29) Installed bootstrap: `run` subcommand, locks, thin
  wrappers. An end-to-end run on this host installed the prebuilt suite from
  the rolling release and the real snapshot via `--clippy-snapshot-bundle` into
  isolated XDG directories, then `verify-installed-bootstrap.sh` passed: the
  cold run compiled `dylint_driver` and failed with `no_std_fs_operations`, the
  warm run reused it, of two simultaneous cold runs one built while the other
  waited and reused it, a relocated data root still linted, and a deleted
  snapshot failed with the no-fallback message.
- [ ] Release publication and post-publication checks.
- [ ] Documentation, ADR, and the draft pull request.

## Surprises & discoveries

- 2026-10-29: Dylint 6.3.0's `dylint_internal::link::dylint_libs` now copies
  `lib<crate>.so` over `lib<crate>@<toolchain>.so`. `dylint_testing` first
  rebuilds the crate with default features, and every Whitaker lint crate has
  `default = []`, so the copy replaced the harness-built `dylint-driver`
  library with a 6.7 KB stub. `bumpy_road_function::ui` failed with
  ``could not find `dylint_version` ``. The harness now sets
  `DYLINT_LINK_ENABLE_COPY_LIBRARY=1`, Dylint's own signal that the
  toolchain-qualified copy already exists, rather than patching upstream code.
- 2026-10-29: The earlier rebase of the wrapper-parsing patch onto 6.3.0
  passed the `Running` regex's inner capture to `rustc_invocation`, which
  applies the same regex itself, so every example UI test failed with
  ``Found no `rustc` invocations``. The helper's unit tests passed because they
  feed it whole lines. The call site now passes the whole line, as the 6.0.1
  patch did, and a regression case records the helper's contract.
- 2026-10-29: Several agents share `~/.dylint_drivers`, so an unkeyed driver
  directory is not an isolation boundary; gates now export an identity-keyed
  `DYLINT_DRIVER_PATH`.
- 2026-10-29: The probe's recorded snapshot commit (`7b5c5db7`) differs from
  the final probe repository's `HEAD` (`e4608a25`). The pin uses `e4608a25`,
  the history actually probed, and the selection was recomputed for it.
- 2026-10-29: `git bundle verify` needs a repository even for a bundle without
  prerequisites, and `-c protocol.allow=never -c protocol.file.allow=always`
  blocks network transports while still permitting bundle and local clones.
- 2026-10-29: The ignored `integration_exclusion` snapshot test had silently
  broken on `main`: the test moved into an `assertions` submodule, so insta
  looked for a new snapshot path. The stored diagnostics were byte-identical to
  a fresh run, so the snapshot was moved rather than re-blessed.
- 2026-10-29: `tests/workflows` has 20 failures on unmodified `HEAD` in this
  environment (a host `rustup` shadows stubs, the `gh` stub is bypassed, and
  some harnesses lack scripts). The candidate was compared against a `HEAD`
  worktree baseline: no new failures, after teaching two Makefile harnesses to
  provide the snapshot generation.
- 2026-10-29: ADR 008 is already taken on the unmerged
  `issue-523-adopt-ortho-config-v0-10-0-for-the-whitaker-cli` branch, so this
  work's ADR is 009.
- 2026-10-29: Two end-to-end installer tests broke once installation began
  requiring the snapshot, and one of them wrote to the developer's real data
  directory. Rather than skip the step for tests, debug binaries accept a
  fixture manifest (`WHITAKER_INSTALLER_TEST_CLIPPY_SNAPSHOT_MANIFEST`), the
  shared `SnapshotFixture` supplies a small real bundle, and the affected test
  isolates `XDG_DATA_HOME`.
- 2026-10-29: Clippy's early commits use a plain-text `rust-toolchain`
  file that Dylint's TOML parser cannot read. Dylint's walk never reaches them,
  but an eager reader does, so the reproduction stops at the first unreadable
  toolchain file exactly as Dylint's walk would fail there.
- 2026-10-29: Two builds of the bundle from the same mirror produced the same
  digest. That is not guaranteed across Git versions or source packs, so the
  installer still anchors trust in the commit object ID.
- 2026-10-29: `git fsck --full` of the full Clippy history takes about 11 s,
  so it belongs on installation and cold driver construction, not on every warm
  run.

## Decision log

- Adopt upstream Dylint 6.3.0 (probe evidence below).
- Pin Clippy `main` at `e4608a25c2e20116fbc1e97a51a73f094a1db7a1`. For
  `nightly-2026-05-28`, Dylint's first-parent channel walk selects
  `9fca3bc9fc2bc83c60bde26d18ed68f11564b228` ("Rustup (#17109)"), whose first
  parent is on `nightly-2026-05-13`.
- Publish the full `main` history as a Git bundle; the walk needs the selected
  revision and its parent, and a shallow cut is a failure mode to detect, not
  an optimization.
- Build the driver on first use. Dylint links it with an absolute rpath to the
  rustup toolchain, so it is host-specific and a prebuilt copy would not be
  relocatable.
- Place the bootstrap beside the managed clone, not inside it:
  `whitaker_data_dir()` is itself the Git clone.

## Baseline decision: Dylint 6.3.0

Dylint 6.3.0 fixes the branch rename (`ac3323f`) and adds `DYLINT_CLIPPY_REPO`,
an absolute path to a Dylint-exclusive Clippy clone (`82da1fa`). The repository
therefore evaluated 6.3.0 against the unchanged nightly pinned in
`rust-toolchain.toml` (`nightly-2026-05-28`) with the real
`no_std_fs_operations` lint library before accepting it.

### Cold compatibility probe, 2026-10-28

Recorded observations, not intentions. The probe used a bare Clippy snapshot
containing `refs/heads/main` and no `refs/heads/master`, with public Clippy
access denied.

| Input                  | Value                                                                 |
| ---------------------- | --------------------------------------------------------------------- |
| Nightly                | `nightly-2026-05-28` (unchanged)                                      |
| Tools                  | `cargo-dylint` 6.3.0, `dylint-link` 6.3.0, prebuilt, SHA-256 verified |
| Clippy snapshot commit | `7b5c5db7df79f18932d7e7781f4d54ee9dbd6d4b`                            |
| Snapshot refs          | `refs/heads/main` only; `master` absent                               |
| Driver directory       | empty before the cold run                                             |
| Lint library           | `no_std_fs_operations`, built for the pinned nightly                  |
| Public Clippy access   | denied via `url.<deny>.insteadOf` rewrite                             |

The denial was confirmed before the probe:

```plaintext
$ git ls-remote https://github.com/rust-lang/rust-clippy refs/heads/main
fatal: remote helper 'https' aborted session
$ echo $?
128
```

Cold run, empty `DYLINT_DRIVER_PATH`:

```bash
cd /tmp/probe-fixture
export PATH=/tmp/dylint-probe/tools630/bin:$PATH
export RUSTUP_TOOLCHAIN=nightly-2026-05-28
export DYLINT_DRIVER_PATH=/tmp/probe-drivers
export DYLINT_CLIPPY_REPO=/tmp/dylint-probe/clippy-snapshot-final.git
export DYLINT_LIBRARY_PATH=/tmp/probe-libs
cargo dylint --all -- --message-format json
```

Observed: the driver compiled from `dylint_driver v6.3.0` against the local
snapshot, the fixture compiled, and cargo exited `1` because the deliberate
`std::fs` violation produced the expected diagnostic:

```plaintext
error: std::fs operation `std::fs::read_to_string` bypasses the
       capability-based filesystem policy.
  --> src/lib.rs:2:13
   = note: `#[deny(no_std_fs_operations)]` implied by `#[deny(warnings)]`
```

Warm run, same directory: `EXIT=1`, one `no_std_fs_operations` diagnostic, no
`Compiling dylint_driver` line, and the driver's SHA-256 was unchanged.

`RUSTC_WRAPPER=sccache` cold run with a second empty driver directory:
`EXIT=1`, one `no_std_fs_operations` diagnostic, and a freshly built driver.

### Decision

Adopt upstream 6.3.0. The two-commit immutable backport (`v6.0.1` plus
`ac3323f` and `82da1fa`) is unnecessary, so the repository keeps an upstream
release rather than maintaining a fork. The nightly is unchanged.

### Snapshot identity is HEAD-only in 6.3.0

Dylint 6.3.0 reads exactly one environment variable for Clippy provenance,
`DYLINT_CLIPPY_REPO` (`dylint_internal-6.3.0/src/clippy_utils/repository.rs`).
It opens that repository, clones its object database into a temporary
directory, and then walks backward from the clone's `HEAD` looking for the
newest commit whose `rust-toolchain` channel is at or before the pinned nightly
(`clippy_utils/revs_no_preinstall.rs::iter`). There is no `DYLINT_CLIPPY_REV` or
`DYLINT_CLIPPY_BRANCH` in `dylint_internal-6.3.0`. Upstream documents
`rev`/`tag` only for `[workspace.metadata.dylint]` libraries, never for the
driver.

Two consequences follow, and both were confirmed by experiment:

- A snapshot whose `HEAD` is unresolvable cannot be detected by setting a
  revision variable; the variable is simply ignored. Reproducibility must come
  from the snapshot's own `HEAD`, so the producer has to publish a snapshot
  whose `refs/heads/*` point at the chosen commit.
- Because selection walks history rather than reading one commit, the snapshot
  must retain the ancestor chain back to the nightly's Clippy. A single-commit
  shallow bundle is therefore not a sufficient artefact.

Negative controls, run 2026-10-28 against the same nightly and library:

| Control                                         | Observed                                                                                                |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `DYLINT_CLIPPY_REPO` pointing at a missing path | driver build fails with `DYLINT_CLIPPY_REPO must point to a Git repository`                             |
| Driver cache root absent                        | harness fails with `Condition failed: dylint_drivers.is_dir()`, so the root must exist before any build |
| `DYLINT_CLIPPY_REV` set to an all-zero OID      | ignored; the build succeeds and the diagnostic fires                                                    |

The third row is the load-bearing one: it proves that commit pinning is not an
upstream capability and must be supplied by the producer's snapshot contents
and by the identity key used for the driver cache directory.

### Verification after the pin bump

`cargo test -p dylint_testing` passes (10 tests) with the wrapper-parsing patch
rebased onto the 6.3.0 sources, and `Cargo.lock` resolves `dylint`,
`dylint_internal`, `dylint_linting`, and `dylint_testing` at 6.3.0.

Running a real UI test without `DYLINT_CLIPPY_REPO` reproduces the original
defect on 6.3.0, which shows the variable is load-bearing rather than advisory:

```plaintext
$ cargo test -p no_unwrap_or_else_panic --features dylint-driver --lib -- ui::ui
Cloning into '/tmp/.tmpX0rLyc'...
fatal: remote helper 'https' aborted session
Error: command failed: "git" "clone" "https://github.com/rust-lang/rust-clippy" "/tmp/.tmpX0rLyc"
```

`dylint_internal` 6.3.0 reads `DYLINT_CLIPPY_REPO` in
`src/clippy_utils/repository.rs` and rejects a relative path, so snapshot
provisioning must export an absolute, already-populated clone path before any
`cargo dylint` invocation.

## Outcomes & retrospective

To be completed when the work lands.
