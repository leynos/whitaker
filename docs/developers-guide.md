# Whitaker Developer's Guide

This guide is for contributors who want to develop new lints or work on
Whitaker itself. For using Whitaker lints in a project, see the
[User's Guide](users-guide.md).

## Prerequisites

- Rust nightly toolchain (version specified in `rust-toolchain.toml`)
- `jq` for extracting package metadata in release dry runs
- Python 3 for workflow tests and release checksum generation
- `cargo-dylint` and `dylint-link` installed:

  ```sh
  cargo install cargo-dylint dylint-link
  ```

  (`make publish-check` provisions these automatically at the pinned versions;
  see [Pre-publish validation](publishing.md#pre-publish-validation).)

CI also installs or provides job-specific tools such as `cargo-nextest`, `bun`,
`uv`, Mermaid CLI, and Nixie before running the targets that need them. Local
runs of those targets require the same tools on `PATH`.

### Installer compatibility

The workspace builds with the nightly toolchain pinned in
`rust-toolchain.toml`, but the published `whitaker-installer` supports Rust
1.85 and newer. Run the following check after changing the installer, its
dependencies, or packaging:

```sh
make installer-msrv-check
```

The target packages `whitaker-installer` into an isolated temporary target
directory, extracts the resulting `.crate`, then uses Rust 1.85 to install that
package with `--locked` into a temporary root and run `--version`. It removes
the temporary directory on exit. This checks the published package boundary; do
not replace it with a workspace-path installation.

### Linux release compatibility

Published x86_64 GNU/Linux installer, dependency, and lint artefacts target the
Ubuntu 22.04 glibc baseline and must not require a version newer than
`GLIBC_2.35`. The release workflows enforce that contract by inspecting ELF
version-needs metadata with `readelf`, rather than relying on a runner label.

Use the checker locally after changing a Linux release build or its
dependencies:

```sh
scripts/check_glibc_baseline.py --maximum-glibc GLIBC_2.35 <ELF>...
```

`readelf` must be available on `PATH`. The rolling-release workflow runs the
checker before upload; tagged releases download the packaged installer and
every dependency archive named by `installer/dependency-binaries.toml`, then
inspect their executable payloads before publishing. `make test-glibc-baseline`
runs the checker’s acceptance, rejection, and generated parser/invariant tests;
the Linux CI lane executes that target.

## Running Tests

Run the test suite from the workspace root:

```sh
make test
```

This executes unit, behaviour, and UI harness tests. The shared target enables
`rstest` fixtures and `rstest-bdd` scenarios.

### Integration tests for lint exclusion behaviour

The `no_std_fs_operations` crate includes end-to-end behavioural coverage for
the `excluded_crates` and `excluded_paths` configuration. These integration
tests invoke `cargo dylint` in a subprocess, so they exercise the full
lint-loading and configuration path, rather than only unit-level helpers.

Fixture projects are generated at runtime using
`create_fixture_project(crate_name, kind, is_excluded)`, which writes a
`Cargo.toml`, `dylint.toml`, and `src/lib.rs` into a `TempDir` and returns a
`FixtureProject` handle. The `FixtureProject` owns the `TempDir` so the
directory is cleaned up automatically when the handle is dropped. The
`FixtureKind` enum selects which suppression mechanism the fixture exercises:
`CrateExclusion` pairs a flat source module with `excluded_crates`, while
`PathExclusion` nests the `std::fs` usage inside a `guarded` module and
configures `excluded_paths = ["<crate_name>::guarded"]`. Passing
`is_excluded: true` lists the fixture under the relevant key; `false` writes an
empty list. Each fixture `Cargo.toml` contains an empty `[workspace]` table
(omitted here for brevity) so Cargo treats the fixture as its own workspace
root and does not resolve upwards to the enclosing Whitaker workspace.

The harness centres on `run_cargo_dylint`, which executes
`cargo dylint --all -- --message-format json` with `DYLINT_LIBRARY_PATH` set to
the built lint library and `DYLINT_RUSTFLAGS=-D warnings` set to deny warnings
during the run. `diagnostic_count` then parses the JSON message stream with
`cargo_metadata::Message` and counts only `CompilerMessage` entries whose
`code.code` is `no_std_fs_operations`, which keeps the assertions tied to the
lint's structured diagnostics instead of brittle text matching.

The shared helper
`run_fixture_exclusion_test(crate_name, is_excluded, expectation, kind)`
resolves the lint library path via a `OnceLock`-cached `build_lint_library`
call, creates the fixture project for the given `FixtureKind`, and delegates to
`assert_fixture_behaviour`. Two parametrized `#[rstest]` functions drive it and
are kept separate so the crate-wide and module-path mechanisms have their own
clearly named coverage: `exclusion_crates_behaviour_test` passes
`FixtureKind::CrateExclusion`, and `exclusion_paths_behaviour_test` passes
`FixtureKind::PathExclusion`.

The tests are annotated with `#[serial]` from `serial_test`, and the
repository-level nextest contract also requires them to match the
`serial-dylint-ui` test group in `.config/nextest.toml` when they are exercised
through `make test`. Both the attribute and the repo-level group are required
for correct serialized execution because nextest runs each test in a separate
process, so the in-process `#[serial]` mutex alone is not sufficient. They are
also marked `#[ignore]` by default because they depend on external tooling and
a buildable workspace. Before running them, install `cargo-dylint` and
`dylint-link`. The harness calls `build_lint_library()` before running
`cargo dylint`, so the workspace build is handled automatically. Run them with
one of the following commands:

```sh
cargo test -p no_std_fs_operations --test integration_exclusion -- --ignored
cargo nextest run -p no_std_fs_operations --test integration_exclusion --run-ignored ignored-only
```

Each parametrized case asserts the subprocess exit status and the
`no_std_fs_operations` diagnostic count, so the tests verify both the success
path for excluded targets (zero diagnostics, exit 0) and the failure path for
non-excluded targets (one or more diagnostics, non-zero exit). The
path-exclusion fixture places its only `std::fs` usage inside
`guarded::reader`, so a passing excluded case also confirms that the exclusion
is scoped to the module and reaches descendants, rather than reflecting an
accidental crate-wide suppression.

Keep the `cargo dylint` integration tests thin: they exist to confirm the
wiring (config loading, HIR path resolution, and diagnostic suppression) end to
end, not to re-check every matching edge case. The module-path matching itself
lives in the rustc-free `PathExclusions` type (`src/exclusion.rs`), which the
lint pass consults per candidate usage. Because it is independent of `rustc`,
it is covered cheaply and exhaustively by unit tests, a `proptest` property
test against a segment-wise-prefix oracle, and the `path_exclusion.feature`
behavioural scenarios — including the malformed-entry rejection that stops
`my_app::` from collapsing into a crate-wide exclusion. The configuration
schema and loading live in `src/config.rs` (`NoStdFsConfig`, the `ConfigReader`
seam), kept separate from the lint pass in `src/driver.rs` so each file stays
within the repository's size and single-responsibility limits.

### Fixture-based harness regressions

Some lint regressions need more than the plain `ui/` compiletest fixtures. For
those cases, crates such as `no_expect_outside_tests` keep a dedicated harness
runner in `src/lib_ui_tests.rs`.

The harness splits cases into two shapes:

- Example-based runs use `ExampleHarnessRun` plus
  `dylint_testing::ui::Test::example` when a single example target is enough
  and no extra fixture assets are needed.
- Fixture-based runs use `FixtureHarnessRun`, `prepare_fixture`, and
  `Test::src_base` when the case needs copied support files, a per-fixture
  `dylint.toml`, or additional `--extern` wiring.

We use `camino::Utf8Path` for fixture directory handling so temporary staged
paths remain explicitly UTF-8 and can be joined and passed through the harness
helpers without repeated lossy conversions.

When a fixture needs an external crate such as `tokio`, the harness resolves
the artefact from the dependency directory next to the current test binary.
`dependency_rlib` scans `target/.../deps` for `lib<crate>-*.rlib`, prefers the
most recently modified artefact from the current build, and falls back to a
stable path ordering when timestamps tie before emitting the `--extern` flag.

This split keeps ordinary UI fixtures simple while still letting regression
tests cover `rustc --test`, file-backed modules, per-case configuration, and
real proc-macro crates where needed.

### Test profiles

By default, `make test` excludes slow installer integration tests
(`behaviour_toolchain` and `behaviour_cli`) via a nextest default-filter
defined in `.config/nextest.toml`. These tests perform real `rustup` installs
and `cargo` builds, so they can take upwards of fifteen minutes. Note that the
exclusion relies on hardcoded binary names in `.config/nextest.toml`; renaming
or splitting these test binaries requires updating the filter to match (see
[#180][issue-180]).

To run the full suite including installer tests, pass the `ci` profile:

```sh
make test NEXTEST_PROFILE=ci
```

Continuous Integration (CI) always uses the `ci` profile, so installer tests
are never silently skipped in the pipeline.

### Coverage and nested Cargo builds

`make coverage` uses the same selected crate set and warning policy as
`make test`, but runs it through `cargo llvm-cov nextest`. The Makefile assigns
the absolute `COVERAGE_TARGET_DIR` (by default,
`$(CURDIR)/target/llvm-cov-target`) to both `CARGO_LLVM_COV_TARGET_DIR` and
`CARGO_TARGET_DIR`.

The first variable makes `cargo-llvm-cov` instrument its output in that
directory. The second is inherited by nested Cargo commands that Dylint UI
harnesses start while compiling examples. Both values must remain identical:
`cargo-llvm-cov` otherwise passes its target only as a Nextest argument, and a
nested Cargo process falls back to the ordinary `target/debug` tree. That can
mix instrumented and non-instrumented dependency artefacts.

When isolating a coverage run, override `COVERAGE_TARGET_DIR` rather than
either environment variable individually. Fixture workspaces that intentionally
use the same package identity must still select their own nested `--target-dir`
to avoid reusing one another's build-script output.

The CI workflow is split by purpose rather than running the same stack on every
operating system. `linux-full` is the authoritative gate for formatting,
Mermaid/Nixie/Markdown validation, `make lint`, and `make publish-check`.
`windows-compat` is a narrower compatibility lane that runs
`make test NEXTEST_PROFILE=ci`, `make install-smoke`, and
`make release-installer-dry-run` to prove the workspace still builds on
Windows, the installed binary can execute, and the Windows installer release
packaging path stays valid. The release dry-run target is a POSIX-shell target;
Windows CI runs it under Bash and requires the same command-line tools as local
POSIX environments.

The lanes share the workflow-level build contract: `BUILD_PROFILE=debug` narrows
`sccache` keys to debug builds only; `CARGO_INCREMENTAL=0` disables
incremental compilation, which is incompatible with `sccache`; and
`RUSTFLAGS=-D warnings` and `RUSTDOCFLAGS=-D warnings` deny compiler and doc
warnings across both lanes. `RUSTC_WRAPPER` is deliberately absent from that
contract. The shared Rust setup action installs `sccache` and exports the
wrapper as the absolute path of the binary it installed, and it stands aside
when a caller has already set the variable. A workflow-level
`RUSTC_WRAPPER=sccache` would therefore leave every lane wrapped by whichever
`sccache` happened to be on `PATH` rather than the one the action provisioned,
which is reported as `metric setup-rust.sccache.wrapper=caller-set`. Cache
storage is job-specific. The Linux lanes read one workflow-level
`SCCACHE_BACKEND` switch, while `windows-compat` sets
`SCCACHE_GHA_ENABLED=true` in its own job environment because it is
GitHub-hosted and shares none of the Ubicloud constants.

### CI build caching

Every Linux job runs on an ephemeral Ubicloud virtual machine whose disk is
destroyed when the job ends, so all durable state is an archive. Every lane,
Ubicloud and GitHub-hosted alike, uses `actions/cache/restore` and
`actions/cache/save` pinned to `55cc8345863c7cc4c66a329aec7e433d2d1c52a9`
(v6.1.0).

One action and one pin serve both providers because Ubicloud's transparent
cache intercepts that version. This was verified from the Ubicloud console's
cache listing on 2026-09-03: Axinite's Linux keys written by v6.1.0 appear in
its Ubicloud listing while its Windows keys appear in GitHub's, and Whitaker's
older v4.3.0 pin (`0057852b`) had left nothing in Ubicloud's listing at all.
The deprecated `ubicloud/cache` fork is deliberately not used: it reads
`UBICLOUD_CACHE_URL` and `UBICLOUD_RUNTIME_TOKEN` from the virtual machine's
environment, which a GitHub-hosted runner never supplies, so it could not serve
the Windows lane. Check the first `main` run in the listing again with:

```sh
ubi gh leynos/whitaker list-cache-entries
```

No job archives a Cargo `target` tree, and none may. `sccache` is the single
owner of compiler output for every build shape. The lint and test lanes compile
ordinary debug objects while the coverage lane compiles
`-C instrument-coverage` objects; both shapes coexist in one `sccache` store
keyed by their flags, and run 33744418209 recorded 0 non-cacheable compilations
for the instrumented build. A `target` archive would be a second owner, would
be invalidated far more often than the registry, and could hold only one shape
at a time. That is why every caller of the shared `setup-rust` action passes
`cache-provider: external`: its `github` provider archives
`target/${BUILD_PROFILE}` alongside the registry. `windows-compat` was
archiving exactly that until it moved to the external provider and gained its
own registry cache. The release and rolling-release workflows still call the
shared action with its default `github` provider because they are release
boundaries rather than developer-blocking lanes. They no longer archive a
`target` tree either: they pin the shared action at
`f6d4d5f549655c118f86f371b8d55c200d3efa50`, the first revision whose built-in
provider stopped archiving `target/<profile>`. Expect one cold Cargo cache on
those lanes after the repin because the key no longer carries the build profile.

The developer-blocking lanes have since moved ahead of them, to
`7cb894fe62c40951cccf33819548095e64a1291e`. That revision keeps the `target`
rule and adds two things the older pin lacks: it restores the caller's Actions
cache-service selection, and it starts the `sccache` server from a `run:` step
after those exports. The lanes that cut tags stay behind until a tag has run on
the newer wiring, since a green pull request is not evidence that a release
still builds. `tests/workflow_contracts/shared_action_pin_split_test.py` holds
each group to its own revision by value, so neither half can drift alone, and
fails once the two constants converge so the split is collapsed rather than
left describing nothing.

Every cached path has exactly one owner inside its job, and every key family
has exactly one job permitted to write it.

Table: Cache ownership for the Ubicloud Linux lanes.

| Key family                    | Cached paths                                                                    | Writer            | Restore-only lanes             |
| ----------------------------- | ------------------------------------------------------------------------------- | ----------------- | ------------------------------ |
| `cargo-registry-coverage-v1-` | `~/.cargo/registry`, `~/.cargo/git`                                             | `coverage-upload` | `coverage-check`               |
| `cargo-registry-lint-v1-`     | `~/.cargo/registry`, `~/.cargo/git`                                             | `linux-full`      | `linux-full`                   |
| `tools-coverage-v1-`          | `~/.rustup`, `~/.cargo/bin`, `~/.local/bin`, `~/.cache/uv`, `~/.local/share/uv` | `coverage-upload` | `coverage-check`               |
| `tools-lint-v1-`              | the same paths plus `~/.bun/install/cache` and `~/.cache/merman`                | `linux-full`      | `linux-full`                   |
| `dylint-tools-v1-`            | `~/.cache/whitaker-dylint-tools`                                                | `linux-full`      | `linux-full`                   |
| `clippy-mirror-v1-`           | `~/.cache/whitaker-mirrors`                                                     | `coverage-upload` | `linux-full`, `coverage-check` |
| `sccache-<lane>-v1-`          | `~/.cache/sccache`                                                              | the lane's writer | the lane's readers             |

Each key carries an explicit `v1` schema generation so the whole family can be
invalidated deliberately. Registry keys hash `rust-toolchain.toml` and
`Cargo.lock`; tool keys spell out each pinned tool version and hash
`rust-toolchain.toml`; the Dylint host-tools key names the `cargo-dylint` and
`dylint-link` versions the Makefile installs; and the Clippy mirror key names
the Dylint version whose build script performs the clone. Every key also carries
`runner.os`, `runner.arch`, and `runner.environment`, and the compiled-tool
keys add the Ubuntu release because a binary built on Ubuntu 24.04 must never
be restored onto 22.04.

Restores run on every event. Saves are guarded by
`github.ref == 'refs/heads/main'` and, except for the compiler cache, by a
missed restore. A pull request therefore reads the trusted generation without
publishing a competing write and without the `Unable to reserve cache` noise
that two racing lanes produce.

`coverage-main.yml` is the only Whitaker job that runs automatically on the
trunk, so it is the writer for the coverage-lane keys and for the shared Clippy
mirror. `ci.yml` has no push trigger, so `linux-full` publishes the lint-lane
keys when it is dispatched against `main`:

```sh
gh workflow run ci.yml --repo leynos/whitaker --ref main
```

Dispatch that run after changing a lint-lane tool pin, the pinned toolchain, or
a cache generation, and before comparing warm pull requests. Until it runs, the
lint lane restores the previous generation through its `restore-keys` prefix
and reports the miss in the job summary.

An archive-based cache needs no empty-directory guard. A restore whose key
misses creates nothing at all, so a script cannot mistake a materialized mount
point for a populated cache. That hazard belonged to the Namespace cache
volume, which appeared as an existing directory whether or not it held a
generation.

The Clippy source fetch needs a single cache owner. `dylint_driver` 6.0.1's
build script reads `clippy_utils/src/sym.rs` at the revision matching the
pinned dated nightly and obtains it by running a full
`git clone https://github.com/rust-lang/rust-clippy` into a temporary
directory. The build script performs that clone for any dated nightly on or
after 2025-05-06, and Whitaker pins `nightly-2026-05-28`, so every cold build of
`dylint_driver` repeats an unowned multi-hundred-megabyte network clone. That
clone is intermittent rather than merely occasionally broken: run 33692863541
hit `fatal: could not read Username for 'https://github.com'` on nextest tries
1 and 2 and passed only on try 3, while run 33704041072 failed all three tries
and failed the job. `scripts/provision-clippy-mirror.sh` gives that fetch one
owner. It maintains a bare mirror and rewrites the upstream URL to it with
`url.<mirror>.insteadOf`, so the build script's clone becomes a local,
hard-linking copy. The mirror sits below the cached directory rather than at
the cache path itself, so discarding a stale generation can never attempt to
remove the directory the cache action manages. A cold clone is authenticated
with the job token when one is present and retried with backoff; a refresh
failure on a warm mirror is only a warning because the revision the build
script needs is historical and is already present. All three Linux lanes now
restore the mirror and provision it before any Cargo invocation.

The script owns a deletion, so it proves what it is deleting. It resolves both
its argument and its trusted cache root (`CLIPPY_MIRROR_ROOT`, defaulting to
`~/.cache/whitaker-mirrors`) to physical paths and refuses anything that is not
the one `rust-clippy.git` inside that root. A suffix match such as
`/tmp/project/rust-clippy.git` no longer authorizes a removal.

It also classifies the restored path rather than reducing it to one boolean. A
generation is reused only when git reports it bare and its `remote.origin.url`
is the pinned upstream because a non-bare directory or a mirror of some other
repository would leave `dylint_driver` without the Clippy revision it needs. A
non-bare or wrongly pointed generation is rebuilt. An unreadable path, a
non-directory, or an unparsable verdict from git is an environment fault: the
script aborts and never repairs it by deleting the path.

The pinned Dylint host tools follow the same rule.
`scripts/install-dylint-tools.sh` once ran `cargo install --locked` for
`cargo-dylint` and `dylint-link` into a `mktemp -d` root, so both were
recompiled on every run and never cached. It now downloads the upstream
`trailofbits/dylint` v6.0.1 prebuilt Linux release archives, verifies each
against a SHA-256 digest pinned in the script, and installs the executables
into the caller's root. Only a version with a pinned digest can be installed;
any other version is a hard error rather than an unverified download or a
fallback to a source build. The Makefile passes `DYLINT_TOOLS_DIR`, defaulting
to `~/.cache/whitaker-dylint-tools`, instead of a temporary root, and prepends
its `bin` directory to `PATH` before invoking the script. `linux-full` owns
that directory as a cache, so a warm generation skips the download entirely.
The script keeps its `CARGO` and `TOOLCHAIN` arguments only so the caller's
contract is unchanged; neither participates in the download-and-verify install
path. `DYLINT_TOOLS_SHA256_CARGO_DYLINT` and `DYLINT_TOOLS_SHA256_DYLINT_LINK`
exist solely so the behavioural tests can verify a locally generated fixture
archive: they replace the expected digest and cannot disable verification.

Presence in that durable root is not proof of a version. `cargo-dylint` can be
probed with `cargo-dylint dylint --version`, but `dylint-link` is a linker shim
that forwards `--version` to `cc`, so it cannot be. Every install therefore
records the version it wrote in a `.<tool>.version` marker beside the binary,
and a cached `dylint-link` is reused only when that marker matches the
requested pin. Without it, a shim installed for an older `DYLINT_LINK_VERSION`
would survive a version bump indefinitely in the unversioned root and be paired
with a newer `cargo-dylint`. A system copy on `PATH` is accepted only when the
tools root holds no `dylint-link` at all; it predates the script and carries no
provenance, so the script says so on stderr rather than implying it verified
anything.

The shared compiler cache is intentionally scoped to debug builds:

- `BUILD_PROFILE=debug` keeps cache paths centred on the profile used by the
  normal test and typecheck jobs.
- `CARGO_INCREMENTAL=0` disables incremental build artefacts, which are
  poorly suited to shared CI cache reuse and can make cache contents larger
  without improving repeatability.
- `RUSTFLAGS=-D warnings` and `RUSTDOCFLAGS=-D warnings` preserve the
  warnings-as-errors contract even when builds are routed through `sccache`.

`sccache` is configured in exactly one place. The workflows declare
`SCCACHE_BACKEND`, and `scripts/select-sccache-backend.sh` translates that
single value into the backend's environment before any Cargo invocation.
`local` exports `SCCACHE_DIR` and `SCCACHE_CACHE_SIZE` and activates the
`~/.cache/sccache` archive steps; `gha` exports `SCCACHE_GHA_ENABLED` and
leaves those steps skipped. The two backends are never configured together:
`sccache` would then report a plausible hit rate while writing to a store
nobody owns.

The GitHub Actions backend needs `ACTIONS_RESULTS_URL` and
`ACTIONS_RUNTIME_TOKEN`, which GitHub exposes to actions rather than to `run`
steps. That is the shared Rust setup action's concern now, not this
repository's. It records the caller's cache-service selection before
`mozilla-actions/sccache-action` overwrites it, restores it afterwards, and
starts the server from a `run:` step positioned after those exports, so a
server started for the GHA backend comes up bound to the right endpoint. This
repository previously re-exported the two values from an
`actions/github-script` step immediately after checkout; that step is gone,
because two arms configuring one `sccache` is the failure it was written to
avoid.

The ordering that mattered still matters, expressed differently. `sccache`
binds its backend once, when the server starts, so the backend selector and the
compiler-cache directory restore both run before `Setup Rust`, which is what
starts the server. The contract tests enforce both positions and reject a lane
that installs or zeroes `sccache` itself. The GitHub-hosted Windows lane needs
no export, because there the variables are already visible to `run:` steps.

`local` is the deployed backend, chosen from measurement. The Actions cache
service is the store Ubicloud's transparent cache intercepts, and Cuprum's
Ubicloud cache listing on 2026-09-03 shows `sccache/...` keys written by its
pull-request run 33748907011, so the `gha` backend does reach Ubicloud's store
in that project. It does not work in this one.

Whitaker reproduced a total write failure twice, with identical counters. Runs
[33748602187][whitaker-run-33748602187] and
[33756048103][whitaker-run-33756048103] each reported `Cache location ghac` in
both Linux jobs, proving the credentials reached `sccache`, then failed every
store: 3,788 write errors against 3,788 attempts in `linux-full` and 2,245
against 2,245 in `coverage-check`, with 0 read errors both times. The second
run had already moved the credential export ahead of everything and swapped the
archive caches to `actions/cache` v6.1.0, so neither the ordering nor the cache
client explains it.

The same run's GitHub-hosted `windows-compat` job used the same backend and
wrote 1,529 hits with 0 write errors, which places the failure in the Ubicloud
cache proxy's write path rather than in `sccache` or in the credentials. One
difference is worth chasing before anyone re-enables `gha` here: the Windows
job reported a hashed cache name, `cb1f7e36...` because the shared setup action
sets `SCCACHE_GHA_VERSION`, while the Linux jobs reported the default
`sccache-v0.16.0`.

Switching back is one line. Treat write errors above roughly two percent of
requests, or an Ubicloud cache listing with no `sccache` entries for Whitaker,
as the signal to stay on `local`.

`local` has known trade-offs. Its archive grows with every new compilation unit
until `SCCACHE_CACHE_SIZE` trims it, so a warm run restores and re-saves the
whole directory even when only a few objects changed. That is why the key
carries the run identifier with a `restore-keys` prefix, and why the save is
restricted to the lane's single writer. The cap defaults to 4 GB rather than 2
GB because the store holds two build shapes, the ordinary debug objects and the
instrumented coverage objects; a one-shape cap would evict each shape in turn.
It also routes the compiler cache through the same `actions/cache` transport as
every other archive, so one working write path serves the whole design. Measure
the restore and save duration against the compile seconds avoided.

Each build lane starts from zeroed `sccache` counters and then runs
`scripts/record-sccache-effectiveness.sh`, which appends the human-readable
statistics to the job summary, retains the JSON statistics, and warns when
`sccache` reports zero compile requests. On the Linux lanes the zeroing is done
by the shared action's server start, which reports
`metric setup-rust.sccache.server=started` or `started-stats-not-zeroed` so a
failed zero is visible rather than inferred; `windows-compat` still zeroes
explicitly. A run with no compile requests paid the compiler cache's setup cost
while `RUSTC_WRAPPER` never reached a single `rustc` invocation, so treat zero
compile requests as a failed cache integration, not as a clean zero-miss
result. That failure mode is not hypothetical: `mozilla-actions/sccache-action`
exports only `SCCACHE_PATH` and does not set `RUSTC_WRAPPER`, so before a
wrapper was exported no Cargo invocation in `coverage-main.yml` was wrapped at
all.

`scripts/record-cache-observations.sh` renders every restore step's primary
key, the key it actually matched, and its `cache-hit` result into the job
summary, including the steps a lane does not use, so an operator can explain
any restore from the run evidence without re-reading the workflow. It runs under
`if: always()`.

A save needs disk headroom of its own. `actions/cache/save` stages a compressed
archive locally before uploading it, so a save costs roughly the size of the
archive on top of the tree it is archiving. Whitaker's first cache-writing run,
the merge of #393, died with `No space left on device` between two save steps on
`ubicloud-standard-2`. Every step before the saves had succeeded, so the
failure was capacity at save time rather than anything about the work.

Two changes keep that from recurring, and neither needs a larger runner. Every
job that saves now discards its scratch tree first, with no exemptions: the
coverage lanes drop `target/llvm-cov-target` once `cargo llvm-cov` has written
`lcov.info`, and `linux-full` and `windows-compat` drop `target` after their
last consumer. No job caches a target tree, so all three are pure scratch by
that point, and dropping the instrumented tree also gives the doctest build its
headroom. Every discard runs under `if: always()` so a later failure cannot
leave the disk full.

`windows-compat` stages a smaller archive onto a larger GitHub-hosted disk and
has never run out of space, but it is covered anyway. The rule follows from how
`actions/cache/save` stages an archive, which does not vary by platform, and a
contract with one exemption invites a second.

Free disk is then recorded rather than assumed.
`scripts/record-cache-observations.sh` prints `df -h` for the root volume and
for `RUNNER_TEMP`, where the cache action stages its archives, once in the main
observation block and again from its `headroom` form immediately before the
first save. The contract tests assert that every saving job discards its
scratch tree and records headroom before its first save step, so a new save
cannot be added without the headroom that makes it survivable.

The matched key, not `cache-hit`, is what classifies a restore. The cache
action reports `cache-hit: true` only for an exact primary-key match, so a
successful `restore-keys` restore and a complete miss both surface as a falsy
value. Every warm compiler-cache restore takes the prefix path because that key
ends with the current `github.run_id` and can never match exactly. The summary
therefore reports `exact hit`, `prefix restore from <key>`, or `miss`, and
prints the raw `cache-hit` value verbatim beside it, showing an absent value as
`unset` rather than coercing it to `false`. Restore and save byte counts and
durations are not step outputs; read the cache action's own `Cache Size` and
transfer lines from the job log, and confirm an entry exists on Ubicloud's side
with the cache-entries API rather than assuming a save succeeded.

Tool setup must not compile tools from source. `taiki-e/install-action` calls
pin a release whose catalogue contains each requested tool, disable fallbacks,
and use checksum-verified release artefacts. `mdtablefix` 0.5.0 is installed
from its official Linux x86_64 release asset after checking the SHA-256 pinned
in the workflow. The tools cache retains the installed executable under
`~/.cargo/bin`; a cold cache downloads it, while a warm cache verifies and
reuses it without invoking Cargo. The SHA-pinned shared `install-nixie` action
at `f6d4d5f549655c118f86f371b8d55c200d3efa50` (shared-actions PR #423, repinned
to `main`) owns Nixie 1.1.0 and Merman 0.7.0 setup. It verifies Merman's
official release archive and cached executable against pinned SHA-256 digests,
reconciles the uv-managed Nixie installation, and never falls back to a source
build.

The uv cache contract includes downloads under `~/.cache/uv`, installed tool
environments under `~/.local/share/uv`, and their executable shims under
`~/.local/bin`. Restoring only the environment store can make uv report a tool
as installed while leaving its command unavailable, so all three live in one
cache step with one owner. The shared Nixie installer therefore forces
installation only when its shim is absent, which repairs a partial cache
generation from the cached uv artefacts.

Table: Test profiles and typical usage.

| Profile   | What runs                                  | Typical use        |
| --------- | ------------------------------------------ | ------------------ |
| (default) | All tests **except** installer integration | Local development  |
| `ci`      | All tests                                  | CI and pre-release |

When working on `whitaker-installer` code, run the full suite locally before
pushing to catch installer regressions early.

### One execution of the test suite per pull request

A coverage job and a test-only job on the same platform bill twice for one
result. The coverage job is therefore the single execution of the Rust suite
per pull request on Linux, and no other Linux job executes tests at all.

Table: Jobs that execute the Rust test suite.

| Job               | Platform         | Command                          | Role                                      |
| ----------------- | ---------------- | -------------------------------- | ----------------------------------------- |
| `coverage-check`  | Ubicloud Linux   | `make coverage`, `make test-doc` | the single Linux pull-request execution   |
| `coverage-upload` | Ubicloud Linux   | `make coverage`, `make test-doc` | the trunk baseline, on `main` pushes only |
| `windows-compat`  | `windows-latest` | `make test NEXTEST_PROFILE=ci`   | a different platform                      |

`make coverage` delegates to the `make test` recipe and swaps only the driver to
`cargo llvm-cov nextest`, so the instrumented run keeps one package set, one
feature set, and one target set: `--workspace --all-targets --all-features`
minus the eleven CI-excluded crates. Both coverage lanes run identical flags,
so the pull-request gate and the trunk baseline are comparable.

`make test-doc` is the second half of that one executed set, not a second lane.
`cargo llvm-cov nextest` executes no doctests and `--all-targets` excludes
them, so without this step the workspace's doctest fences are compiled by
`cargo doc` in `make lint` and never run.

Two things about the doctest lane are easy to get wrong.

Its `RUSTFLAGS` deliberately omit the
`-C prefer-dynamic -Z force-unstable-if-unmarked` pair that the test lane needs
for its `cdylib` lint crates. A doctest is compiled as its own crate, and
`force-unstable-if-unmarked` then makes every doctest fail to load the very
library it is documenting, with `E0658`. With the plain `-D warnings` flags the
same doctests pass.

`DOCTEST_EXCLUDES` drops every crate that links `rustc_private`: the lint
crates, the `rustc_*` proxy shims, `clippy_utils`, the `whitaker` root, and the
suite. That is a structural limit rather than a policy choice. A doctest for
those crates has no `#![feature(rustc_private)]` of its own and cannot compile
at all, so the flags exclude what cannot run rather than what is not worth
running. The remaining four packages, `whitaker-common`, `whitaker-installer`,
`whitaker_sarif`, and `whitaker_clones_core`, hold the documented public API
and contribute 335 doctests in about 18 seconds warm.

`linux-full` executes no tests. It survives as a job because
`main-required-checks` requires that context by name, and it carries the
formatting, spelling, Markdown, Mermaid, lint, Dylint, workflow-contract,
GLIBC-baseline, MSRV, and packaging work. Its former uninstrumented run inside
`make publish-check` is gone, and `make publish-check` no longer runs the suite
at all.

That removal does cost one thing worth naming. The `publish-check` run used
production-like static linking, without `-C prefer-dynamic`, which no surviving
lane exercises. Linking is still covered at build time by the workspace and
per-lint release builds that `publish-check` continues to run; what is no
longer covered is executing the suite under that linkage.

`tests/workflow_contracts/lane_deduplication_contract_test.py` holds this
shape. It fails if a second Linux execution appears, if the surviving gate
narrows its package, target, or feature set, if either coverage lane loses its
doctest step, or if `publish-check` starts running the suite again.

### Runner placement policy

Linux developer-blocking jobs run on Ubicloud managed runners. Everything else
runs on GitHub-hosted runners.

Table: Runner placement for repository-owned jobs.

| Job                              | Workflow                             | Runner                            | Why                               |
| -------------------------------- | ------------------------------------ | --------------------------------- | --------------------------------- |
| `coverage-check`                 | `ci.yml`                             | `ubicloud-standard-2-ubuntu-2404` | Blocking Linux gate               |
| `linux-full`                     | `ci.yml`                             | `ubicloud-standard-2-ubuntu-2404` | Blocking Linux gate               |
| `coverage-upload`                | `coverage-main.yml`                  | `ubicloud-standard-2-ubuntu-2404` | Trunk Linux gate and cache writer |
| `windows-compat`                 | `ci.yml`                             | `windows-latest`                  | Ubicloud has no Windows image     |
| `mutation`                       | `mutation-testing.yml`               | Reusable workflow's own choice    | Nightly, not blocking             |
| `automerge`                      | `dependabot-automerge.yml`           | Reusable workflow's own choice    | API-bound                         |
| Release and rolling-release jobs | `release.yml`, `rolling-release.yml` | GitHub-hosted matrices            | Release boundaries                |

Ubicloud publishes Ubuntu images only, on x64 and arm64, so Windows and macOS
lanes have no Ubicloud counterpart and stay GitHub-hosted permanently. That is
not a temporary compromise: `windows-compat` queued for a median of two seconds
on `windows-latest` in the 2026-09-01 to 2026-09-03 sample, so GitHub-hosted
Windows is not the contention this migration targets. Public repositories pay
nothing for GitHub-hosted standard runners, which is also why the release and
administrative lanes stay there.

`ubicloud-standard-2-ubuntu-2404` is 2 vCPU and 8 GB. The label names the
Ubuntu release explicitly rather than relying on Ubicloud's default, so a
change to that default cannot silently move compiled artefacts between glibc
versions. `ubicloud-standard-4` is the ceiling, not the default. Escalate a job
to it only with evidence from at least three warm runs showing peak memory
above roughly 6 GB, or the larger shape at least halving the job's duration
(the per-minute rate doubles, so anything less increases billed minutes), or
the larger shape removing the job from the workflow's critical path. Whitaker's
earlier `linux-full` history on `ubicloud-standard-4-ubuntu-2404`, a median of
about 16 minutes, is not escalation evidence: those runs restored no Cargo
archive and installed tools from source on every run.

Every Ubicloud job declares `timeout-minutes`. Ubicloud runners register as
self-hosted just-in-time runners, so GitHub's five-day self-hosted limit
applies rather than the six-hour hosted limit, and a hung job would otherwise
bill for days.

Test and build concurrency is bounded by one named constant.
`LINUX_RUNNER_VCPUS` is declared once per workflow and a single step derives
both `CARGO_BUILD_JOBS` and `NEXTEST_TEST_THREADS` from it, so changing the
label cannot leave the suite oversubscribed. `windows-compat` keeps its own
value because `windows-latest` is a four-vCPU GitHub-hosted shape. No suite in
this repository uses `pytest-xdist`; if one adopts it, give it an explicit
worker count rather than `-n auto`.

`windows-compat` keeps the shared Rust setup action's own `sccache` setup,
which installs the binary and exports the Actions cache credentials. It needs
no separate export step, and it owns one registry archive of its own keyed by
`runner.os` and `runner.arch` so a Linux archive can never be restored onto
Windows.

`.github/actionlint.yaml` registers `ubicloud-standard-2-ubuntu-2404` as the
only self-hosted label in use. Keep that list equal to the labels the workflows
actually reference.

The `main` ruleset requires the `linux-full` and `windows-compat` status-check
contexts. GitHub derives a context from the job's name, so neither job may gain
an explicit `name` or a matrix that embeds the runner label; either change
would leave the ruleset waiting for a context the workflow no longer emits. The
migration deliberately changed no job name, so no ruleset edit was needed.
`tests/workflows/test_ubicloud_runner_placement.py` enforces this.

The pre-migration baseline was captured on 2026-09-01 with the following
read-only commands:

```sh
gh run view 33410178021 --repo leynos/whitaker \
  --json name,url,status,conclusion,createdAt,startedAt,updatedAt,jobs
gh run view 33369228466 --repo leynos/whitaker \
  --json name,url,status,conclusion,createdAt,startedAt,updatedAt,jobs
gh run view 33345742967 --repo leynos/whitaker \
  --json name,url,status,conclusion,createdAt,startedAt,updatedAt,jobs
gh run view 33340945546 --repo leynos/whitaker \
  --json name,url,status,conclusion,createdAt,startedAt,updatedAt,jobs
gh run view 33322310248 --repo leynos/whitaker \
  --json name,url,status,conclusion,createdAt,startedAt,updatedAt,jobs
```

All five GitHub `CI` runs completed successfully. Their workflow wall times
were 18m37s, 20m53s, 29m25s, 41m30s, and 59m29s respectively. Job execution
times were:

| Run                                     | `linux-full` | `windows-compat` | `coverage-check` |
| --------------------------------------- | ------------ | ---------------- | ---------------- |
| [33410178021][whitaker-run-33410178021] | 18m27s       | 15m54s           | 10m36s           |
| [33369228466][whitaker-run-33369228466] | 20m30s       | 16m57s           | 12m28s           |
| [33345742967][whitaker-run-33345742967] | 24m11s       | 25m18s           | 13m53s           |
| [33340945546][whitaker-run-33340945546] | 26m16s       | 26m44s           | 14m15s           |
| [33322310248][whitaker-run-33322310248] | 26m06s       | 25m47s           | 13m29s           |

The corresponding job queue waits were 9s/24s/25s, 23s/23s/23s, 20s/246s/21s,
8s/885s/21s, and 8s/2,021s/8s in the same column order. Median queue/execution
times were 9s/24m11s for `linux-full`, 246s/25m18s for `windows-compat`, and
21s/13m29s for `coverage-check`. After migration, compare queue time and
execution time separately, and read Ubicloud queue time as the roughly
20-second cost of creating a virtual machine per job rather than as contention.

### Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows, including
calls into `leynos/shared-actions`. Contract tests that assert a caller's exact
commit SHA create a lockstep dependency: every time Dependabot opens a bump PR,
the test fails until a human edits the pinned constant to match. That defeats
the purpose of automated dependency updates and turns a routine bump into a
manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller. They
must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.

### Failure-mock test helpers

`installer/src/toolchain/tests/failure_mocks.rs` provides reusable helpers for
installer error-path tests that should stay deterministic and offline. They let
tests exercise failure handling without talking to `rustup`, downloading a
toolchain, or relying on network state.

`TOOLCHAIN_INSTALL_FAILURE_MESSAGE` and `COMPONENT_INSTALL_FAILURE_MESSAGE` are
the verbatim stderr payloads emitted by the mocks. The assertions match those
messages with exact equality, so wording changes fail immediately instead of
being hidden by looser substring checks.

`FailureSetup` packages the `InstallFailure` variant under test together with
any `additional_components` needed for that scenario. Pass the resulting value
to `setup_failure_mocks` to configure the command-runner sequence, and then to
`assert_failure_error` to check the resulting `InstallerError` variant.

```rust
let setup = FailureSetup {
    failure: InstallFailure::ComponentAdd,
    additional_components: &["rustfmt"],
};
setup_failure_mocks(&mut runner, &mut seq, channel, setup);
let err = toolchain.ensure_installed_with(&runner, setup.additional_components)
    .expect_err("component-add scenario should fail");
assert_failure_error(err, channel, setup);
```

`setup_failure_mocks(runner, seq, channel, setup)` wires the
`MockCommandRunner` sequence for the failure described by `setup` on the given
channel. It covers the shared rustc-version probe and the branch-specific mock
responses for toolchain install failure, component-add failure, or post-install
unusable toolchain.

`assert_failure_error(err, channel, setup)` checks that `err` matches the
expected `InstallerError` variant for the same scenario. When the shape does
not match, it panics with a message like
`"<Variant> for channel {channel} while exercising {failure}"`, which makes
multi-toolchain or multi-failure test failures much easier to diagnose.

### Other useful commands

```sh
make lint       # Run Clippy
make check-fmt  # Verify formatting
make fmt        # Apply formatting
```

## Python script interpreter convention

Every Python script that a workflow or the `Makefile` runs by path carries the
same preamble, a shebang above a script-metadata block, and is committed with
its executable bit set:

```python
#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
```

Call sites invoke the script directly, never as `python <script>`. The reason
is release-specific. The x86_64 GNU/Linux legs of `release.yml` and
`rolling-release.yml` run on `ubuntu-22.04` to hold the `GLIBC_2.35` baseline,
and that image's `python` is 3.10. A script handed to that interpreter runs on
a version nobody chose, so the first use of a newer standard-library feature,
`tomllib` being the one that has already happened, fails at release time and
nowhere else. `setup-rust` installs `uv` on every runner, so the shebang
resolves on Linux, macOS, and Windows alike.

The executable bit matters as much as the shebang: without it a fresh checkout
fails with exit code 126, and files created by tooling default to mode 644.

`tests/workflows/test_ci_script_interpreter_contract.py` discovers the call
sites rather than listing them, so a new script or a new caller is covered
automatically. It fails if a call site reverts to an ambient interpreter, if a
directly invoked script loses its shebang or its `requires-python`, or if Git
records the script as non-executable.

## Markdown formatting checks

`make fmt` runs `mdformat-all`, which applies the repository's `mdtablefix`
options and then fixes Markdown lint findings. `make check-fmt` invokes
`scripts/check-markdown-format.sh` after the Rust formatter check. The Makefile
passes every Markdown source outside repository and tool caches to that script
in batches.

The checker owns only the non-mutating comparison boundary. It copies each
source to a temporary directory, asks `mdtablefix` and `markdownlint-cli2` to
apply the same fixing passes as `mdformat-all`, and compares the result with
the original source. It accepts either LF or CRLF when the content is otherwise
identical, and removes the temporary directory on exit. It never formats or
rewrites a working-tree file. `mdtablefix` remains the owner of table padding
and paragraph wrapping; `markdownlint-cli2` remains the owner of Markdown lint
rules and its fixing pass. The `linux-full` CI job installs both pinned tools
before running `make check-fmt`; it caches `mdtablefix` but verifies its
workflow-level version pin. This makes a cold runner and a stale cache produce
the same canonical output.

Keep the script scoped to `make check-fmt` and its focused process tests. Reuse
it when another repository-owned gate needs to verify this exact Markdown
canonical form, rather than reproducing the staging and line-ending logic in a
second wrapper. Keep its formatter flags in step with `mdformat-all`, and use
the `MDTABLEFIX` or `MDLINT` Makefile override when a locally installed
formatter is not on the default `PATH`.

Run the focused checker tests with:

```sh
make test-markdown-format
```

The target uses isolated `uv` dependencies and does not rewrite Markdown
sources or other tracked files.

## Mutation-testing workflow contract tests

Whitaker runs scheduled, informational mutation testing through a thin caller
workflow,
[`.github/workflows/mutation-testing.yml`](../.github/workflows/mutation-testing.yml),
which delegates to the shared reusable workflow
`leynos/shared-actions/.github/workflows/mutation-cargo.yml`. The heavy lifting
— running `cargo-mutants`, sharding, and summarizing survivors — lives in
`shared-actions`; this repository carries only declarative configuration. The
run is **informational only**: it never gates a pull request. Survivors are
reported through the job summary and downloadable artefacts so they can be
triaged into tests, not enforced as a blocking check.

The workflow runs in two modes. A **daily schedule** (04:50 UTC) fires a
change-scoped run that mutates only the source files touched within the
detection window, so quiet days are cheap no-ops. A **manual dispatch** (the
Actions "Run workflow" control) mutates the whole workspace; select a branch in
that control to exercise a feature branch.

The caller passes a small set of configuration inputs, each carrying intent:

- `paths` — `src/,common/,crates/,installer/,suite/`, the workspace member
  prefixes rooted at the repository root; there are no top-level `examples/` or
  `benches/` directories to add.
- `exclude-globs` — scaffolding whose surviving mutants would be noise rather
  than genuine test gaps: the `rustc_*` proxy crates and the `clippy_utils`
  stub (both re-export compiler internals), each lint's `ui/` and `examples/`
  dylint fixtures, and the shared test infrastructure in `src/testing` and
  `common/src/test_support`.
- `extra-args` — `--all-features`, matching the feature baseline the
  Makefile's `CARGO_FLAGS` uses for `make test`, so feature-gated code is not
  reported as untested.

Unlike the excludes applied by `make test`'s `TEST_EXCLUDES`, the mutation
caller does **not** exclude the `whitaker` root crate or the
`function_attrs_follow_docs`, `module_max_lines`, and `no_expect_outside_tests`
lint crates from mutation scope (only their `ui/`/`examples/` fixtures are
excluded). Those crates enable `feature(rustc_private)` under the
`dylint-driver` feature and need the dynamic-linking `RUSTFLAGS`
(`-C prefer-dynamic -Z force-unstable-if-unmarked`) that the `test` and
`typecheck` Makefile targets inject per invocation — see the note in
[`.cargo/config.toml`](../.cargo/config.toml), which deliberately keeps that
flag out of workspace-wide configuration because it would break `cargo install`
for `whitaker-installer`. The shared mutation workflow has no equivalent
per-crate RUSTFLAGS step, so it cannot reproduce `TEST_CARGO_FLAGS` faithfully
across the whole workspace. Consequently this adoption is **pin-only**: the
caller declares the best approximation of the CI scope it safely can, rather
than a `--test-workspace` run that mirrors `make test` crate-for-crate, and the
contract test asserts only that this declared configuration holds — not that a
full workspace mutation baseline passes.
[ADR 004](adr-004-pin-only-mutation-testing-contract.md) records this decision,
its alternatives, and the accepted limitations.

The `uses:` reference pins the shared workflow to a full 40-character commit
SHA rather than a branch or tag, so a force-push upstream cannot silently
change what runs here. The contract test asserts only that the pin is a full
commit SHA, not a particular value, so Dependabot bumps it automatically
without any accompanying test edit.

### Workflow contract tests

Because the caller is configuration rather than code, a contract test suite,
[`tests/workflow_contracts/mutation_testing_test.py`](../tests/workflow_contracts/mutation_testing_test.py),
pins the shape it must uphold, failing the pull request when the caller drifts
— repointing the pin at a branch, widening the token scope, or dropping a
configuration input — rather than letting the breakage surface only in a
scheduled run. Run it locally with:

```sh
make test-workflow-contracts
```

which wraps
`uv run --with 'pytest>=8' --with 'pyyaml>=6' pytest
tests/workflow_contracts -q`.
The suite validates:

- the `uses:` reference targets `mutation-cargo.yml` pinned to a full commit
  SHA;
- the `with:` block carries exactly the expected `paths`, `exclude-globs`,
  and `extra-args` configuration described above;
- job permissions are least-privilege (`contents: read`, `id-token: write`)
  and the workflow-level default token scope is empty;
- `concurrency` serializes runs per ref without cancelling one in progress;
  and
- the triggers keep the daily schedule and a plain `workflow_dispatch` with no
  legacy branch input.

## Proof workflows

Whitaker now ships repository-managed proof tooling for the formal verification
work introduced around decomposition advice and the clone-detector pipeline.
Run these commands from the workspace root.

### Clone-detector index structure

The clone-detector index code is grouped under
`crates/whitaker_clones_core/src/index/` by the candidate-generation feature it
serves. The module split keeps the public constructor contracts small enough to
test and verify directly:

- `fragment_id.rs` owns the `FragmentId` newtype. It is intentionally separate
  from the LSH and pair types because its lexical ordering is a contract that
  unit tests, BDD scenarios, and the Verus sidecar all rely on.
- `types.rs` owns `CandidatePair`, `LshConfig`, and the fixed MinHash
  signature types. `CandidatePair::new` consumes already validated `FragmentId`
  values, suppresses self-pairs, and canonicalizes distinct pairs by the
  ordering supplied by `FragmentId`.
- `lsh.rs` and `minhash.rs` own the indexing and sketching algorithms that use
  those small domain types, while `error.rs` keeps the index error contract out
  of the algorithm modules.
- `mod.rs` re-exports the public index surface so callers import
  `FragmentId`, `CandidatePair`, `LshConfig`, `LshIndex`, and `MinHasher`
  through `whitaker_clones_core` rather than depending on the internal module
  layout.

The `FragmentId` reorganization was done to remove a type-ownership tangle in
`types.rs`. Keeping the identifier newtype in its own module makes the trusted
ordering bridge in `verus/clone_detector_candidate_pair.rs` easy to audit:
Verus trusts the derived production ordering for `FragmentId`, while ordinary
tests pin the concrete string-backed behaviour.

The direct `CandidatePair::new` coverage lives in the clone-core crate:

- Unit tests in `crates/whitaker_clones_core/src/index/tests.rs` cover
  self-pair suppression, already ordered inputs, reversed inputs, and the
  lexical edge case `fragment-10 < fragment-2`.
- The BDD harness in
  `crates/whitaker_clones_core/tests/candidate_pair_behaviour.rs` exercises the
  same public constructor through `rstest-bdd` steps.
- The feature file is
  `crates/whitaker_clones_core/tests/features/candidate_pair.feature`.
- The test dependencies are declared in
  `crates/whitaker_clones_core/Cargo.toml` as `rstest`, `rstest-bdd`, and
  `rstest-bdd-macros`.

The direct `MinHasher::sketch` invariant coverage is split between ordinary
tests and Kani:

- Unit tests in `crates/whitaker_clones_core/src/index/tests.rs` cover
  deterministic sketches, duplicate-hash insensitivity, reordered set
  semantics, empty input, and representative full-width `u64` hash values.
- The BDD harness in
  `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs` includes a
  duplicate-retained-hash candidate-generation scenario.
- The Kani harnesses in `crates/whitaker_clones_core/src/index/kani.rs` call
  real `MinHasher::sketch` for the empty-input, deterministic-output, and
  duplicate-hash properties. They use a private `cfg(kani)` seed fixture and
  fixed-width signature builder so the proof focuses on sketch semantics rather
  than seed-stream array construction.

The direct `LshIndex` invariant coverage is also split between ordinary tests
and Kani:

- Unit tests in `crates/whitaker_clones_core/src/index/tests.rs` cover no
  self-pairs, canonical pair ordering, repeated-band deduplication, and
  insertion-order independence through the public `candidate_pairs()` API.
- The BDD harness in
  `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs` exercises LSH
  candidate generation as part of the token-pass behaviour surface.
- The Kani harnesses in `crates/whitaker_clones_core/src/index/kani.rs` verify
  bounded `LshIndex` states for the same invariants. Kani builds use a private
  fixed-size insertion log and compact band keys so the proof checks the LSH
  state transition and `CandidatePair::new` policy without modelling `BTreeMap`
  and `BTreeSet` allocator internals.

### Clone-detector AST structure

The clone-detector AST code is grouped under
`crates/whitaker_clones_core/src/ast/`. It is deliberately split into one
parser adapter and several parser-agnostic domain modules:

- `lowering.rs` is the only AST source file that may import `ra_ap_syntax`,
  `ra_ap_parser`, or `rowan`. It parses a Rust file, maps byte spans to the
  smallest covering syntax node, and lowers that node into the owned
  `NormalizedTree` representation.
- `tree.rs` owns the lowered domain types: `NormalizedTree`,
  `NormalizedNode`, `KindId`, `Depth`, `LeafClass`, and `ByteSpan`. `KindId` is
  an in-memory token and must not be persisted.
- `hash.rs` owns `AstHash` and `canonical_hash`.
- `features.rs`, `hash.rs`, and `cover.rs` operate only on the lowered domain
  types. They must not depend on parser crates or import the adapter module.
- `tests.rs` and the `tests/ast_*` behavioural suites cover feature math,
  parser lowering, snapshots, property tests, and the module-boundary guard.

The `tests/ast_boundary.rs` guard enforces this boundary. If it fails, fix the
module ownership problem rather than relaxing the guard: parser vocabulary
belongs in `lowering.rs`, and reusable AST algorithms belong in the lowered
domain.

AST feature vectors use an exact count substrate. `kind_counts` records exact
`(KindId, Depth) -> u32` counts, `kind_histogram` derives dyadic fixed-point
weights from those counts, `production_multiset` records deterministic
parent-child and parent-child-grandchild production counts, and
`canonical_hash` emits an `AstHash` seeded with `PARSER_SCHEMA_VERSION`.
Changing the parser pin, normalization rules, hash algorithm, or schema string
must produce a reviewable snapshot change.

`ra_ap_syntax` is exact-pinned in `Cargo.toml` because its parser vocabulary
and MSRV move with `0.0.x` snapshots. The dependency is behind the default
`parser` feature of `whitaker_clones_core`: normal builds compile the real
adapter, while Kani runs pass `--no-default-features` and compile the
parser-free adapter stub. Keep that split unless Kani's pinned toolchain can
compile the parser snapshot directly.

`crates/whitaker_clones_core/build_support.rs` owns pure build-time parser
dependency parsing. Only that crate's `build.rs` and its integration
verification test may import it; runtime code must not use it.

### Make targets

Use the Makefile targets for normal proof runs:

```sh
make verus                 # Run all Verus proof files
make verus-clone-detector  # Run clone-detector Verus proofs only
make kani                  # Run all Kani harness groups
make kani-clone-detector   # Run clone-detector Kani harnesses only
```

The Makefile prepends `~/.cargo/bin` and `~/.bun/bin` to `PATH` for all
make-target invocations
(`PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(PATH)`). This ensures that
`cargo`, `kani`, and Bun-based tooling installed in those locations are
resolved ahead of any system-level copies, without requiring developers to
modify their shell environment permanently.

`CARGO` and `MDLINT` are resolved at make-invocation time using `$(or ...)`
shell probes rather than fixed command names. `CARGO` is resolved at
make-invocation time: `command -v cargo` is tried first (it returns a
POSIX-safe path on all platforms, including Windows/Git Bash), falling back to
`~/.cargo/bin/cargo` when that file is executable. This ordering avoids Windows
drive-letter paths that confuse the POSIX shell used by make recipes. `MDLINT`
prefers the `markdownlint-cli2` binary found on `PATH` (typically a global npm
or Bun install), falling back to `~/.bun/bin/markdownlint-cli2`. Both variables
honour an existing environment value if set before invoking make, allowing
per-developer overrides without modifying the Makefile.

`make verus` currently runs both decomposition-advice proofs and the
clone-detector sidecars for `LshConfig::new`, `CandidatePair::new`, and AST
feature-count accumulation. `make kani` runs the decomposition adjacency
harnesses and the clone-detector harness group in one pass.

### Spelling gate

Run `make spelling` to enforce en-GB-oxendict spelling in tracked text. The
gate uses Typos 1.48.0 together with the repository's generated `typos.toml`,
and `make markdownlint` includes the spelling gate.

The tracked configuration is built from the shared estate dictionary and the
narrow `typos.local.toml` overlay. Run `make spelling-config-write` after an
intentional policy change, and run `make spelling-config` to verify that the
tracked output is current. The pinned builder refreshes the untracked local
cache only when the authoritative dictionary is newer, so an already populated
cache remains usable offline.

Do not edit `typos.toml` directly. Preserve public and serialized SARIF terms,
localization compatibility identifiers, compiler fixtures, workflow keys, and
formal diagnostic text through narrow local policy. The exact phrase gate
rejects the hyphenated variant in favour of `handwritten`, including in hidden
tracked source.

`make nixie` validates the repository's Mermaid diagrams. Continuous
Integration installs Nixie 1.1.0 and its Merman 0.7.0 dependency on the Linux
documentation leg through the SHA-pinned shared `install-nixie` action.

### Verus scope and trust boundary

The clone-detector Verus files are intentionally implementation-shaped models
or algebraic sidecars, not direct proofs of the compiled Rust bodies in
`crates/whitaker_clones_core`.

This distinction matters. In the current sidecar setup, Verus can describe the
contract of an external Rust function with mechanisms such as
`assume_specification`, `external_fn_specification`, or `external_body`, but
those routes add trusted assumptions rather than proving the production
implementation itself. The repository therefore keeps the Verus proof honest:
it mirrors the real branch order and `checked_mul` overflow behaviour, while
Kani calls the actual constructor and checks its runtime behaviour directly.

For the current clone-detector constructors, the split is:

- Verus proves the `LshConfig::new` constructor model rejects zero bands,
  rejects zero rows, accepts only exact products of `MINHASH_SIZE`, and rejects
  overflowing products via the same `checked_mul` semantics as the runtime code.
- Verus proves the `CandidatePair::new` constructor model suppresses equal
  inputs, preserves already ordered distinct inputs, and swaps reversed
  distinct inputs into canonical order. The sidecar now formulates that proof
  over a trusted `FragmentId` bridge lemma: `FragmentId::partial_cmp` is
  modelled as a strict total order via a ghost `nat` ranking, rather than being
  left as an implicit assumption.
- Kani executes the real constructor with one concrete acceptance harness, one
  bounded symbolic harness over `[0, 128]²`, and one overflow harness that
  forces the `checked_mul(None)` branch.
- Kani executes real `MinHasher::sketch` calls for empty input, deterministic
  output, and duplicate retained hashes. The non-empty harnesses keep their
  symbolic domains intentionally bounded: determinism proves equality at an
  arbitrary signature lane for a fixed retained hash, while duplicate-hash
  insensitivity proves a symbolic bounded retained hash at the first signature
  lane.
- Kani verifies bounded `LshIndex` states for no self-pairs, canonical pair
  ordering, repeated-band deduplication, and insertion-order independence. In
  `#[cfg(kani)]` builds, `LshIndex` records inserted fragments in a fixed
  four-slot proof log with compact two-band keys; normal builds keep the
  production `BTreeMap`/`BTreeSet` implementation and public API.
- Kani verifies AST span-cover selection and bounded AST feature invariants
  over synthetic `NormalizedTree` values. It calls the production
  `select_smallest_covering` helper for covering-node minimality and root
  fallback, and uses compact bounded tree fixtures for feature invariants so
  the proof does not compile or model `ra_ap_syntax`.
- Verus proves the AST feature-count accumulator algebra for supplied
  `(kind, depth)` contributions. The proof establishes adjacent two-item order
  independence for exact counts; ordinary tests and proptest remain responsible
  for proving that production traversal supplies the intended contribution
  multiset.
- Ordinary unit tests and `rstest-bdd` scenarios pin the concrete lexical
  `FragmentId` ordering contract that the `CandidatePair` proof's bridge still
  trusts rather than proving from `String` internals, and they pin the AST
  adapter's parser-facing behaviour.

### Tooling scripts

The proof targets are thin wrappers over repository scripts:

- `scripts/install-verus.sh` downloads the pinned Verus release into
  `${XDG_CACHE_HOME:-$HOME/.cache}/whitaker/verus`, makes the binaries
  executable, and installs the Rust toolchain that Verus requests.
- `scripts/run-verus.sh` selects proof groups and executes each `.rs` proof
  file in turn, including `verus/clone_detector_ast_features.rs` for the
  clone-detector group.
- `scripts/install-kani.sh` downloads the pinned pre-built Kani release into
  `${XDG_CACHE_HOME:-$HOME/.cache}/whitaker/kani`, installs the matching
  nightly Rust toolchain via `rustup`, and symlinks that toolchain into the
  Kani directory structure.
- `scripts/run-kani.sh` sets the Kani-specific environment, runs the
  decomposition/common harnesses through the existing workflow, and runs the
  clone-detector harnesses one harness per `cargo-kani` invocation so each
  proof appears explicitly in the output, including the overflow-specific
  harness for `LshConfig::new`, the bounded `MinHasher::sketch` harnesses, and
  the bounded `LshIndex` candidate-pair invariant harnesses. Clone-detector
  Kani invocations use `--no-default-features` so parser-independent proofs do
  not compile `ra_ap_syntax`.

The installer scripts are idempotent. The first proof run may take longer while
toolchains and verifier binaries are downloaded; later runs reuse the cached
installation.

### Examples

Run the narrow clone-detector proof workflow during iteration:

```sh
make verus-clone-detector
make kani-clone-detector
```

Run a Verus group directly through the wrapper:

```sh
./scripts/run-verus.sh clone-detector
./scripts/run-verus.sh all --time
```

Run all Kani groups, a specific decomposition harness, or the clone-detector
group directly:

```sh
./scripts/run-kani.sh
./scripts/run-kani.sh verify_build_adjacency_preserves_edges
./scripts/run-kani.sh clone-detector
```

## Toolchain and parser maintenance runbooks

Use these runbooks when the Rust nightly or `ra_ap_syntax` parser snapshot must
move. They are intentionally procedural because both changes affect Dylint,
parser APIs, snapshots, and proof tooling.

### Rust toolchain bump runbook

1. Change `rust-toolchain.toml` to the target pinned nightly channel.
2. Install or refresh the required components:
   `rustup component add rust-src rustfmt clippy rustc-dev llvm-tools-preview`.
3. Rebuild the whole workspace with the newly pinned channel before making
   feature changes. Fix `clippy_utils`, lint-crate, and `rustc_private` API
   drift in the production code rather than suppressing warnings.
4. Confirm `cargo-dylint` and `dylint-link` can drive the pinned nightly. If
   no compatible Dylint release exists, stop and record the blocker.
5. Run the UI tests and re-baseline `.stderr` fixtures only after reviewing the
   diagnostic drift. Treat changed spans, wording, and suggestions as evidence
   to review, not as an automatic blessing.
6. Update load-bearing toolchain references, including installer package
   scripts, release workflow strings, ADR-001 notes, and any design or roadmap
   text that names the old channel.
7. Run the normal gates in order: `make check-fmt`, `make lint`, `make test`,
   and `make markdownlint`. Run relevant proof targets if the bump touches
   clone-detector or decomposition proof surfaces.
8. Keep `CARGO_LOCKED` empty by default. Makefile Cargo recipes pass the value
   through unchanged so callers can opt into `--locked` explicitly when they
   need Cargo to enforce the lockfile for a build, lint, package, or test
   command.

### `ra_ap_syntax` re-pinning runbook

1. Choose a `ra_ap_syntax` snapshot contemporaneous with the pinned Rust
   nightly rather than backwards-bisecting to an older parser unless the plan
   explicitly requires that trade-off.
2. Exact-pin the parser dependency in the workspace `Cargo.toml` under
   `[workspace.dependencies]`. The accepted outcome is an entry such as
   `ra_ap_syntax = "=0.0.334"` with the `parser` feature wired through
   `whitaker_clones_core`. Loose pins, invalid specifiers, or a missing
   workspace dependency must fail the re-pin attempt. If more than three
   transitive crates need manual `cargo update --precise` pins, stop and record
   the mismatch.
3. Keep parser imports confined to `src/ast/lowering.rs`. If a parser API
   change tempts domain code to import `ra_ap_syntax`, update the lowered
   `NormalizedTree` boundary instead.
4. After a parser re-pin, rebuild `whitaker_clones_core` and confirm the build
   script re-derives the parser-version component of `PARSER_SCHEMA_VERSION`
   automatically; do not hand-edit that component. For a normalization change
   that alters the lowered AST shape, bump the AST schema revision (the
   `whitaker_ast=N` prefix in `hashing.rs`). Either way, a missing or non-exact
   workspace dependency pin must still fail before the hash module composes the
   value.
5. Refresh and review the AST snapshots, especially the parser schema
   snapshot and named-kind feature-vector snapshot, so syntax-kind drift is
   visible.
6. Run the parser-pin integration target
   `cargo test -p whitaker_clones_core --test build_script_integration` after
   changing the `[workspace.dependencies]` pin; it must confirm acceptance and
   the exported parser version for an exact pin, and rejection for
   loose/invalid and missing-dependency cases. Then run
   `cargo build -p whitaker_clones_core`, the AST-focused tests,
   `make check-fmt`, `make lint`, `make test`, `make verus-clone-detector`,
   `make kani-clone-detector`, and `make markdownlint`.
7. Preserve the default `parser` feature and the Kani `--no-default-features`
   proof path unless the Kani-pinned toolchain can compile the parser snapshot
   directly.

## Kani bounded model checking

Whitaker uses the [Kani model checker](https://model-checking.github.io/kani/)
to verify critical algorithms with bounded symbolic verification. Kani proofs
complement traditional testing by exhaustively checking properties over all
possible inputs within configured bounds.

### Writing Kani harnesses

Kani harnesses live colocated with the code they verify, typically in a
`#[cfg(kani)]` verification submodule. For example:

```rust
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn verify_property() {
        // Generate symbolic inputs
        let input: u32 = kani::any();

        // Add preconditions
        kani::assume(input > 0);
        kani::assume(input < 100);

        // Call function under test
        let result = function_to_verify(input);

        // Assert postconditions
        assert!(result.is_valid());
    }
}
```

Key principles:

- **Bounded symbolic inputs**: Use fixed-size arrays or bounded ranges to keep
  the state space tractable. Rust's standard `sort_by` and nested loops can
  cause CBMC (C Bounded Model Checker) state-space explosion at higher bounds.
- **Input contracts**: Use `kani::assume` to constrain symbolic inputs to match
  the preconditions that production code guarantees. Model the actual input
  contract, not arbitrary malformed inputs.
- **One property per harness**: Separate harnesses simplify root-cause analysis
  when a property fails. Focused harnesses are clearer than one combined check.
- **Crate visibility**: Kani harnesses can call `pub(crate)` functions directly,
  avoiding the need to widen the public API for verification purposes.

### `cfg(kani)` configuration and crate visibility

Kani harnesses are gated behind `#[cfg(kani)]`, which is only defined when Kani
compiles the crate. Under the Rust 2024 edition, any `cfg` name not registered
with the compiler triggers an `unexpected_cfgs` lint warning. To suppress this,
register `cfg(kani)` in the crate's `Cargo.toml`:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }
```

This tells `rustc` that `kani` is an expected configuration name, so normal
`cargo check` and `cargo clippy` runs do not emit spurious warnings. The entry
lives in `common/Cargo.toml` for the decomposition harnesses and in
`crates/whitaker_clones_core/Cargo.toml` for the clone-detector harnesses.

Kani harnesses verify private helpers that are not part of the public API.
Rather than making these helpers fully public, the following items are promoted
to `pub(crate)` visibility:

- **`community` module** (`common/src/decomposition_advice/mod.rs`): Promoted
  from `mod community` to `pub(crate) mod community` so that
  `test_support::decomposition` helpers and unit tests can import
  `SimilarityEdge` and `build_adjacency`.
- **`build_adjacency` function**
  (`common/src/decomposition_advice/community.rs`): Promoted from `fn` to
  `pub(crate) fn` so that colocated Kani harnesses and the test-support
  adjacency report can call it directly without widening the crate's public API
  surface.
- **`SimilarityEdge::new(left, right, weight)`**
  (`common/src/decomposition_advice/community.rs`): A `pub(crate)` constructor
  added to allow Kani harnesses and test-support modules to create edge values
  without exposing a public constructor on the production type. It is used
  internally by `adjacency_report` to convert validated `EdgeInput` values into
  `SimilarityEdge` instances before delegating to `build_adjacency`. External
  callers should use `adjacency_report` rather than constructing
  `SimilarityEdge` directly.

This pattern keeps the runtime API narrow while giving verification and test
code the access it needs.

### Test-support APIs for adjacency testing

The `common::test_support::decomposition` module provides declarative helpers
for integration and behaviour-driven tests:

- **`adjacency_report(node_count, edges)`**: Validates edge input (canonical
  order, in-bounds, positive weights), builds adjacency lists via
  `build_adjacency`, and returns `Result<AdjacencyReport, AdjacencyError>`.
  Callers can `match` on the result, `.expect(...)` in tests, or propagate the
  error upward when invalid declarative input should fail the caller.
- **`AdjacencyError`**: Typed validation failure for the `Err` branch. The
  shipped variants are `NonCanonicalEdge { index, left, right }` when
  `left >= right`, `EndpointOutOfRange { index, right, node_count }` when an
  endpoint exceeds the graph size, and `ZeroWeight { index }` when a weight is
  non-positive for the production contract. Callers should inspect these
  variants when they need to assert a specific rejection path.
- **`AdjacencyReport`**: Wrapper around adjacency vectors on the `Ok` branch,
  with methods for testing properties:
  - `is_symmetric()`: Checks that all edges appear in both directions
  - `all_indices_in_bounds()`: Verifies neighbour indices are valid
  - `is_sorted()`: Confirms neighbours are sorted by index
  - `neighbours_of(node)`: Returns neighbours of a node (or `None` if
    out-of-bounds)
- **`EdgeInput`**: Declarative edge struct with `left`, `right`, `weight`
  fields, passed to `adjacency_report` and interpreted on the `Ok` branch as
  canonical-order, in-range, positive-weight edge input for behaviour-driven
  development (BDD) scenarios.

The test-support API validates input and delegates to the shipped
`build_adjacency` function, keeping raw adjacency vectors crate-internal while
providing a clean testing interface.

See
[`docs/execplans/6-4-5-use-kani-to-verify-build-adjacency-preserves-similarity-edges.md`](./execplans/6-4-5-use-kani-to-verify-build-adjacency-preserves-similarity-edges.md)
for the complete design rationale and implementation decisions.

### Test-support APIs for label-propagation testing

The `common::test_support::decomposition` module also provides a declarative
label-propagation helper for integration and behaviour-driven tests:

- **`label_propagation_report(method_names, edges, max_iterations)`**:
  Validates edge input by delegating to `validate_edges`, constructs minimal
  method vectors from `method_names`, runs deterministic label propagation for
  up to `max_iterations` passes, and returns
  `Result<LabelPropagationReport, AdjacencyError>`. The same `AdjacencyError`
  variants used by `adjacency_report` apply, because malformed declarative
  graph input is rejected before runtime propagation is called.
- **`LabelPropagationReport`**: Wrapper around the runtime propagation report
  on the `Ok` branch, with methods for testing properties:
  - `labels()`: Returns the final label vector
  - `label_of(node)`: Returns the propagated label for a node, or `None` when
    the node is out of bounds
  - `iteration_count()`: Returns the number of propagation passes performed
  - `has_active_nodes()`: Reports whether the validated input graph contains at
    least one non-isolated node
  - `all_labels_in_bounds()`: Verifies every final label is a valid node index

The test-support API validates input and delegates to the shipped label
propagation runtime, keeping raw adjacency vectors and runtime reports
crate-internal while providing a clean testing interface.

See
[`docs/execplans/6-4-6-kani-verification-propagate-labels-preserves-indices.md`](./execplans/6-4-6-kani-verification-propagate-labels-preserves-indices.md)
for the complete design rationale and implementation decisions.

## Installer release helper binaries

The `whitaker-installer` crate exposes several internal release-helper binaries
used by GitHub workflows and packaging scripts. These are part of the build
contract even though they are not user-facing CLI entry points.

### Why `autobins = false` is required

`installer/Cargo.toml` sets `autobins = false` and declares every binary target
explicitly. This is required because the release workflows invoke specific bin
names that do not always match Cargo's filename-derived defaults.

Current explicit targets:

- `whitaker-installer` from `src/main.rs`
- `whitaker-package-lints` from `src/bin/package_lints.rs`
- `whitaker-package-installer` from `src/bin/package_installer_bin.rs`
- `whitaker-package-dependency-binary` from
  `src/bin/package_dependency_binary.rs`

Without explicit declarations, Cargo would infer fallback names such as
`package_lints` and `package_installer_bin`. Those names do not match the
workflow invocations, so release and rolling-release builds would fail even
though the source files exist.

### Validation coverage and purpose

Workflow validation in `tests/workflows/` protects this contract from drift:

- `test_installer_packaging_bins_match_release_workflow_contract` asserts that
  the workflow-facing binary names exist in workspace metadata.
- The same test also asserts that filename-derived fallback target names are
  absent, proving the crate still relies on explicit target declarations rather
  than accidental Cargo defaults.
- `workflow_test_helpers.py` centralizes the `cargo metadata --no-deps` lookup
  used by these contract tests so packaging changes fail with one clear error
  path.

When modifying release helpers, keep the workflow YAML, `installer/Cargo.toml`,
and the metadata-based tests in lock-step.

### Workflow test support and local runner configuration

The rolling-release contract tests share YAML and shell-parsing helpers in
`tests/workflows/rolling_release_workflow_test_support.py`. Keep parsing and
failure messages centralized there when adding more rolling-release assertions,
instead of duplicating small YAML walkers or shell-branch extractors across
multiple test modules. In particular, `_workflow_dispatch_branch_body()`
returns only the matched branch body and excludes the closing `fi`, so
follow-on assertions can stay focused on the branch contents rather than shell
framing.

#### GitHub-expression helpers

Four helpers in `rolling_release_workflow_test_support.py` analyse `if` guard
expressions on workflow steps:

| Helper                                         | Signature                                    | Purpose                                                                                                                                                               |
| ---------------------------------------------- | -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `_github_operand_pattern`                      | `(operand: str) -> re.Pattern[str]`          | Builds a regex that matches the operand as a standalone token, preventing partial-name false positives (e.g. a prefix or suffix sharing characters with the operand). |
| `_github_expression_mentions_operand`          | `(expression: object, operand: str) -> bool` | Returns `True` when the normalized expression contains the operand as a whole token.                                                                                  |
| `_github_expression_negates_operand`           | `(expression: object, operand: str) -> bool` | Returns `True` when the expression contains `!operand` or `!(operand)`. Double negation (`!!operand`) is not flagged.                                                 |
| `_github_expression_compares_operand_to_false` | `(expression: object, operand: str) -> bool` | Returns `True` when the expression contains `operand == false` (or `false == operand`) in any quoting style. Strict-equality only; `!=` comparisons are not matched.  |

Use these helpers together in step-guard assertions to verify gating semantics
rather than exact expression strings, making tests resilient to harmless
formatting changes in the workflow YAML.

Local workflow tests use the Makefile variables `UV` and `WORKFLOW_TEST_VENV`:

- `UV` selects the `uv` executable used to create and populate the
  workflow-test virtual environment.
- `WORKFLOW_TEST_VENV` selects the virtual-environment path, defaulting to
  `.venv`.

Use `make workflow-test-deps` to create or refresh that environment, and
`make workflow-test` to run the opt-in `act` plus `pytest` workflow smoke tests
against it.

### Worked example: adding another packaging binary

When adding a new internal helper binary, make all of the following changes in
one patch:

1. Add the Rust entry point under `installer/src/bin/`.
2. Add a matching `[[bin]]` stanza in `installer/Cargo.toml`.
3. Keep `autobins = false` so Cargo does not expose unexpected fallback names.
4. Update any workflow or script that invokes the helper to use the explicit
   bin name.
5. Extend the workflow contract test so the new target is asserted alongside
   the existing helpers.

Example `Cargo.toml` entry:

```toml
[[bin]]
name = "whitaker-package-example"
path = "src/bin/package_example.rs"
```

If the workflow should invoke that helper, add an assertion to
`test_installer_packaging_bins_match_release_workflow_contract` before relying
on it in release automation. That keeps the breakage in unit-style Python tests
instead of the rolling-release pipeline.

## Dependency binary packaging

Whitaker publishes repository-hosted copies of `cargo-dylint` and `dylint-link`
for the installer's supported targets. The installer prefers these repository
assets before falling back to `cargo binstall` and then `cargo install`.

### TOML (Tom's Obvious, Minimal Language) manifest schema

The committed manifest at `installer/dependency-binaries.toml` declares each
required dependency binary as a TOML array-of-tables entry. Every entry must
contain the following fields:

Table: Manifest schema for crate entries

| Field        | Type   | Description                                                                        |
| ------------ | ------ | ---------------------------------------------------------------------------------- |
| `package`    | string | Cargo package name (must be unique across entries)                                 |
| `binary`     | string | Executable basename without platform suffix                                        |
| `version`    | string | Required upstream version                                                          |
| `license`    | string | SPDX (Software Package Data Exchange) licence expression for provenance disclosure |
| `repository` | string | Upstream source repository URL                                                     |

Example entry:

```toml
[[dependency_binaries]]
package = "cargo-dylint"
binary = "cargo-dylint"
version = "6.0.1"
license = "MIT OR Apache-2.0"
repository = "https://github.com/trailofbits/dylint"
```

The Rust domain model (`DependencyBinary`) enforces all fields as mandatory
during deserialization. The manifest parser also rejects duplicate `package`
values to prevent ambiguous resolution.

### Manifest and public APIs

The committed manifest lives at `installer/dependency-binaries.toml`. It is
loaded through `whitaker_installer::dependency_binaries`, which exposes:

- `DependencyBinary` for typed manifest entries
- `required_dependency_binaries()` for the full manifest set
- `find_dependency_binary()` for package lookup
- archive and binary naming helpers shared by packaging and installation
- repository-install traits for archive download and extraction

The packaging surface lives in `whitaker_installer::dependency_packaging`:

- `DependencyPackageParams` for one packaging request
- `package_dependency_binary()` for one deterministic `.tgz` or `.zip`
  archive
- `render_provenance_markdown()` and `write_provenance_markdown()` for the
  shared provenance and licence sidecar

### Control flow

The dependency-install path is split into focused modules under
`installer/src/dependency_binaries/install/`:

- `metadata.rs` computes target-specific archive names, binary names, and the
  exact archive member path
- `downloader.rs` resolves rolling-release asset URLs, downloads the
  archive, re-opens it through the capability, and orchestrates the
  checksum-verification call
- `checksum.rs` fetches and parses the `.sha256` sidecar, then verifies the
  SHA-256 checksum: `compute_sha256` streams the archive in fixed-size chunks
  and renders the digest with `to_lower_hex`, and `verify_archive_checksum`
  compares it against the expected value
- `extractor.rs` extracts the exact packaged member into a temporary file and
  atomically renames it into the local bin directory
- `installer.rs` orchestrates directory discovery, download, extraction, and
  executable permission fixes

`downloader.rs` performs all archive filesystem I/O through a capability-scoped
`cap_std::fs_utf8::Dir` rather than ambient `std::fs`.
`open_download_destination` first converts the destination to a
`camino::Utf8Path`, rejecting a non-UTF-8 path up front with an
`io::ErrorKind::InvalidInput` error. It then calls `open_destination_dir`,
which opens the destination's parent via
`Dir::open_ambient_dir(parent, ambient_authority())`; this is the single point
where the `ambient_authority()` grant bootstraps the capability. Every
subsequent archive operation — create, write, and re-open for checksum
verification — goes through that `Dir` handle.

The crate-level `installer/src/hex.rs` module (not part of the `install/`
subdirectory above) provides `to_lower_hex`, which renders bytes as lowercase
hex. It exists because `sha2` 0.11 changed `Sha256::finalize()` to return
`hybrid_array::Array<u8, _>`, which does not implement `LowerHex`, and `Sha256`
no longer implements `io::Write`, so neither `format!("{:x}", digest)` nor
`io::copy(reader, &mut hasher)` compile against it any more. `to_lower_hex` is
shared by `downloader.rs`, `artefact/packaging.rs`, and the `sha256_hex` test
helper.

`installer/src/deps.rs` drives the high-level fallback order:

1. Attempt the repository-hosted dependency archive for the current target.
2. Verify the installed tool is now usable. `cargo-dylint` is checked by
   running `cargo dylint --version`. A repository-release `dylint-link` is
   accepted on the strength of its install pipeline and is never executed; see
   "Why `dylint-link` is never probed" below.
3. If the repository download reports `NotFound`, skip `cargo binstall` and
   fall back directly to `cargo install`.
4. For other repository failures, fall back to `cargo binstall` when available
   and then to `cargo install` if `cargo binstall` is absent or fails.

### Dependency fallback details

`installer/src/deps/install.rs` uses the `InstallOutcome` enum to describe how
each dependency tool was satisfied. `install_tool()` returns
`RepositoryRelease`, `CargoBinstall`, or `CargoInstall`, and
`update_status_after_install()` uses that outcome to decide whether it should
re-check for `dylint-link` after installing `cargo-dylint`.

The HTTP download layers in
`installer/src/dependency_binaries/install/downloader.rs` and
`installer/src/artefact/download.rs` both map
`ureq::Error::StatusCode(404 | 410)` to a semantic `NotFound` error variant.
All other `ureq` failures become `Download` or `HttpError`. Callers inspect
`error.is_not_found()` to distinguish the missing-asset source-build path from
the generic Cargo fallback path.

`install_missing_tools()` iterates across `DEPENDENCY_TOOLS`, calls
`should_install_tool()` to skip any tool already present in `remaining_status`,
then calls `install_tool()` and feeds the returned `InstallOutcome` into
`update_status_after_install()`. This keeps `remaining_status` accurate for
later iterations so a successful `cargo-dylint` install can suppress a redundant
`dylint-link` install.

After resolving the dependency metadata entry, `install_tool()` copies
`dependency.version()` into `CargoInstallPlan` before attempting the
repository-release install. That means every subsequent `run_cargo_install()`
fallback, including the direct missing-asset path, invokes
`cargo install --locked --version <version>`. This preserves the version
recorded in `installer/dependency-binaries.toml` instead of silently using the
latest upstream release.

`update_status_after_install()` delegates the local-install re-check decision to
`should_refresh_companions()`. That helper returns `true` only when the
install outcome was not `RepositoryRelease` and `dylint-link` is still missing,
so the code checks for a resolvable `dylint-link` binary only after local
`cargo-dylint` installs and does not re-check it when the pre-built repository
artefact was used or when `dylint-link` was already present.

#### Why `dylint-link` is never probed

`dylint-link` is a linker wrapper: it forwards its entire argument list to the
underlying linker. It therefore has no reliable self-reporting subcommand.
`--version` exits early, and `--help` succeeds only when a usable linker and
toolchain are present in the ambient environment, so it reports on the
environment rather than on the artefact. Executing it as a health check
rejected valid, checksum-verified release artefacts and forced a
`cargo install --locked --version 6.0.1 dylint-link` fallback that cannot
compile on toolchains below the crate's rustc floor, breaking consumers pinned
to older toolchains.

The trust boundary for a repository-release install is the install pipeline:
the release asset name pins the package and version, the `.sha256` sidecar
establishes integrity, extraction confirms the expected archive member, and the
permission step establishes launch eligibility. `repository_install_verified()`
in `installer/src/deps/install.rs` therefore accepts `dylint-link` as soon as
that pipeline reports success, and retains the generic version check for
`cargo-dylint`. Genuine pipeline failures still fall back to Cargo.

A Cargo-managed `dylint-link` already on `PATH` is checked by resolving an
executable file and comparing the version Cargo recorded for it, which needs no
execution either.

The `dylint-link` verification in `installer/src/deps.rs` is implemented by
five small private helpers:

- `find_binary_on_path(binary_name)` returns the first executable candidate so
  the installation check can validate the exact path it found.
- `find_binary_in_directory(directory, binary_name)` performs the per-directory
  search that `find_binary_on_path()` uses while walking `PATH`.
- `binary_candidates(directory, binary_name)` builds the ordered set of
  candidate paths that each directory contributes to the lookup.
- `is_executable_file(path)` applies the platform-specific file test:
  executable-bit plus regular-file checks on Unix, and `path.is_file()` on
  non-Unix targets where the executable suffix carries the meaning.
- `windows_path_extensions()` normalizes `PATHEXT` on Windows so
  `binary_candidates()` can expand extensionless names the same way the shell
  does.

These key helpers are covered by direct unit tests in
`installer/src/deps/path_tests.rs` for missing PATH values, empty PATH values,
multiple PATH directories, non-executable Unix files, executable Unix files,
broken PATH shims, and Windows `PATHEXT` resolution via both direct helper
tests and `check_dylint_tools()`.

Installer PATH-fixture helpers now live in
`installer/src/test_utils/dependency_binary_helpers.rs` instead of being
duplicated across multiple test modules. The key helpers are:

- `with_fake_binary_on_path(binary_name, run)`, which creates a temporary PATH
  entry containing one executable and runs the closure under `env_test_guard()`.
- `with_fake_path(setup, run)`, which provides two temporary PATH directories
  for tests that need to control PATH ordering or place binaries in later
  entries.
- `write_fake_binary(path, is_executable)`, which writes a fake binary and, on
  Unix, sets executable permissions explicitly for positive and negative tests.
- `AlwaysNotFoundRepositoryInstaller`, a repository-installer test double used
  by `installer/src/tests.rs` to force the direct Cargo fallback path without
  network access.

### CLI tool usage

Release automation uses the `whitaker-package-dependency-binary` helper in
`installer/src/bin/package_dependency_binary.rs`.

Package one executable:

```sh
cargo run -p whitaker-installer --bin whitaker-package-dependency-binary -- \
  package \
  --package cargo-dylint \
  --target x86_64-unknown-linux-gnu \
  --binary-path /tmp/cargo-dylint \
  --output-dir /tmp/dist
```

Generate the shared provenance document:

```sh
cargo run -p whitaker-installer --bin whitaker-package-dependency-binary -- \
  provenance \
  --output-dir /tmp/dist
```

The release workflows consume both artefact types:

- deterministic dependency archives named from the manifest and target triple
- `dependency-binaries-licences.md` for provenance and third-party licence
  disclosure

### Rolling-release manual recovery

`.github/workflows/rolling-release.yml` rebuilds dependency binaries
automatically on pushes to `main` when `installer/dependency-binaries.toml`
changes.

Manual runs now expose an explicit `force_dependency_binary_rebuild` boolean
input instead of rebuilding dependency binaries on every `workflow_dispatch`.

Use that input when the rolling release needs to recover from an earlier
dependency-binary build or publish failure, or when the current rolling release
is missing the expected `.tgz` or `.zip` dependency archives. Leave it set to
`false` for a manual republish that should reuse or restore the existing
dependency archives.

Figure: Rolling-release dependency-binary decision flow. The workflow checks
whether the event is a manual dispatch or a push to `main`, then decides
whether to set `should_build` and either rebuild dependency binaries or restore
the existing rolling-release archives.

```mermaid
flowchart TD
    A[Start workflow] --> B{Event type}

    B -->|workflow_dispatch| C{force_dependency_binary_rebuild input}
    B -->|push on main| D[Compare installer/dependency-binaries.toml between before and sha]
    B -->|other events| G[Preserve existing behaviour]

    C -->|true| E[Set should_build=true]
    C -->|false or unset| F[Set should_build=false]

    D -->|manifest changed| E
    D -->|manifest unchanged| F

    E --> H[Run build_dependency_binaries job]
    F --> I[Skip build_dependency_binaries job]

    H --> J[Publish job uses freshly built dependency archives]
    I --> K[Publish job restores dependency archives from existing rolling release]

    J --> L[End]
    K --> L[End]
    G --> L
```

### Continuous Integration (CI) manifest script

`installer/scripts/dependency_binaries_manifest.py` is a thin CI helper that
reads `installer/dependency-binaries.toml` and emits tab-separated rows for
shell consumption. Each output line contains three columns: package name,
binary name, and version.

Usage:

```sh
installer/scripts/dependency_binaries_manifest.py
installer/scripts/dependency_binaries_manifest.py custom.toml
installer/scripts/dependency_binaries_manifest.py -o matrix.tsv
```

The script validates manifest uniqueness (rejecting duplicate `package`
entries) and exits with code 1 on duplicates. It is invoked by the release and
rolling-release workflows to build the per-target build matrix.

Unit tests for this script live at
`tests/workflows/test_dependency_binaries_manifest.py` and cover argument
parsing, TOML loading, duplicate detection, TSV encoding, file output, and
error paths. Run them with:

```sh
python3 -m pytest tests/workflows/test_dependency_binaries_manifest.py -v
```

### Dependency binary Behaviour-Driven Development (BDD) tests

Dependency binary installation behaviour is specified in Gherkin feature files
and driven by rstest-bdd. The test architecture follows the same pattern used
across the project's behavioural tests.

#### File layout

- Feature file:
  `installer/tests/features/dependency_binaries.feature`
- Step definitions and scenario bindings:
  `installer/tests/behaviour_dependency_binaries.rs`
- Test helpers:
  `installer/src/test_utils/dependency_binary_helpers.rs`
- Shared test utilities:
  `installer/src/test_utils.rs`

#### Extending the BDD suite

To add a new scenario:

1. Append the scenario to the `.feature` file using existing Given/When/Then
   step vocabulary.
2. If the scenario requires new steps, add step functions annotated with
   `#[given(...)]`, `#[when(...)]`, or `#[then(...)]` in the behaviour test
   file.
3. Add a `#[scenario(path = "...", index = N)]` binding function at the
   bottom of the behaviour test file, where `N` is the zero-based scenario
   index.
4. Update `dependency_binary_helpers.rs` if the scenario introduces new
   expected command sequences.

#### Test infrastructure helpers

- **`StubExecutor`** records expected command invocations in sequence and
  returns predefined `Output` values. Call `assert_finished()` after the test
  body to verify all expected commands were consumed.
- **`ExpectedCallConfig`** drives `expected_calls()` to generate the full
  expected command sequence for a given tool and fallback configuration.
- **`StubDirs`** provides a minimal `BaseDirs` implementation that returns
  a configurable bin directory.
- **`StubRepositoryInstaller`** (in the behaviour test) implements
  `DependencyBinaryInstaller` with configurable success or failure behaviour
  for scenario isolation.

## Shared span recovery helpers

Whitaker provides reusable, macro-aware span recovery utilities split across
two layers.

### Policy layer — `whitaker-common`

`whitaker_common::rstest` exports a pure, `rustc`-independent policy:

The following table lists the reusable policy symbols exported from
`whitaker_common::rstest`.

| Symbol                                                        | Description                                                                                                                                           |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SpanRecoveryFrame<T>`                                        | A single frame in an ordered span chain, carrying the frame value and a `from_expansion: bool` flag.                                                  |
| `UserEditableSpan<T>`                                         | Enum result: `Direct(T)` — first frame is user-editable; `Recovered(T)` — a later frame is user-editable; `MacroOnly` — no user-editable frame found. |
| `recover_user_editable_span(frames: &[SpanRecoveryFrame<T>])` | Scans the ordered frame chain and returns the first non-expansion frame as `Direct` or `Recovered`.                                                   |

The policy layer has no dependency on `rustc_span` and can be unit-tested with
any `Clone + PartialEq` span type (e.g., `miette::SourceSpan`).

### Adapter layer — `whitaker` (feature `dylint-driver`)

`whitaker::hir` provides a thin adapter over `rustc_span::Span`:

The following table lists the `whitaker::hir` adapter functions that bridge from
`rustc_span::Span` into the shared recovery policy.

| Symbol                                                             | Description                                                                                                                                                   |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `span_recovery_frames(span: Span) -> Vec<SpanRecoveryFrame<Span>>` | Walks `source_callsite()` from `span`, building an ordered frame list. Stops on dummy spans, non-expansion frames, or when the walk makes no progress.        |
| `recover_user_editable_hir_span(span: Span) -> Option<Span>`       | Calls `span_recovery_frames` then delegates to `recover_user_editable_span`, returning the inner value of `Direct` or `Recovered`, or `None` for `MacroOnly`. |

Both functions are re-exported from `whitaker` under
`#[cfg(feature = "dylint-driver")]`.

### Consuming the helpers in a lint

```rust
use whitaker::recover_user_editable_hir_span;

// Recover a user-editable span for an attribute.
let user_span: Option<Span> = recover_user_editable_hir_span(attr.span);

// Discard macro-only glue — the user has no source location to edit.
if user_span.is_none() {
    return;
}
```

Use `recover_user_editable_hir_span` for both attribute spans and item spans.
When the item span itself is macro-only, treat any attribute comparison on that
item as vacuously in-bounds rather than emitting a spurious diagnostic.

### Extending the policy

To handle a new `T` (e.g., a custom span type in a test harness):

1. Implement `Clone` on `T`.
2. Construct `SpanRecoveryFrame<T>` values with
   `SpanRecoveryFrame::new(value, from_expansion)`.
3. Pass the ordered slice to `recover_user_editable_span`.

No adapter code is needed unless `T` is `rustc_span::Span`.

### Parsed attribute spans and item boundaries

`function_attrs_follow_docs` layers two further recovery rules on top of the
shared helpers, both in `crates/function_attrs_follow_docs/src/driver.rs`:

- **Parsed attribute spans.** rustc migrates built-in attributes from
  `Unparsed` to parsed `AttributeKind` variants nightly by nightly, and the
  parsed representation has no uniform span accessor. `parsed_attribute_span`
  therefore recovers the user-written span per kind through an explicit
  whitelist (`DocComment`, `Ignore`, `Inline`, `MustUse`, `Naked`, `NoMangle`,
  `Optimize`, `TargetFeature`, `TrackCaller`). Kinds outside the whitelist
  return `None` and drop out of the ordering check — some carry no span at all
  (`Cold`, `Used`), others carry one that is deliberately not recovered yet
  (`AllowInternalUnsafe`, `Deprecated`). When the pin advances and a variant
  changes shape, extend or adjust the whitelist rather than matching spanless
  kinds.
- **Item boundaries.** Modern nightlies exclude outer attributes from the
  item's HIR span, so `attribute_within_item` accepts an attribute span that is
  either contained within the item span (older behaviour, and inner attributes)
  or ends at or before the item's start (outer attributes on current
  nightlies). Dummy item spans are treated as vacuously in-bounds.

Unit coverage for both rules lives in
`crates/function_attrs_follow_docs/src/tests/order_detection.rs`
(`parsed_attribute_span_recovers_whitelisted_kinds` and
`attribute_within_item_span_boundaries`).

## Shared fingerprint helpers

`common::rstest` exposes two families of pure data model for deterministic
grouping of `rstest` call sites.

### Argument fingerprints

`ArgFingerprint` stores a positional sequence of `ArgAtom` values in call-site
order. Use it to group helper calls that have the same argument shape
regardless of which fixtures are bound at the call site.

```rust
use whitaker_common::{ArgAtom, ArgFingerprint};

let fp = ArgFingerprint::new([
    ArgAtom::fixture_local("db"),
    ArgAtom::const_lit("42"),
    ArgAtom::const_path("crate::defaults::TIMEOUT"),
    ArgAtom::unsupported(), // retained explicitly; never silently dropped
]);

assert_eq!(fp.atoms().len(), 4);
```

`ArgAtom` variants:

| Variant        | Constructor                     | Meaning                         |
| -------------- | ------------------------------- | ------------------------------- |
| `FixtureLocal` | `ArgAtom::fixture_local(name)`  | Fixture-local parameter         |
| `ConstLit`     | `ArgAtom::const_lit(text)`      | Stable literal value            |
| `ConstPath`    | `ArgAtom::const_path(def_path)` | Stable constant path            |
| `Unsupported`  | `ArgAtom::unsupported()`        | Explicit positional placeholder |

### Paragraph fingerprints

`ParagraphFingerprint` stores an ordered sequence of `StmtShape` values. Use
`ParagraphNormalizer` to assign deterministic `LocalSlot` indices (by
first-appearance order) before constructing the fingerprint, so paragraphs with
renamed locals compare equal when they are structurally equivalent.

```rust
use whitaker_common::{
    CalleeShape, ExprShape, LocalSlot, ParagraphFingerprint,
    ParagraphNormalizer, StmtShape,
};

let mut norm = ParagraphNormalizer::new();
let fp = ParagraphFingerprint::new([
    StmtShape::let_binding(ExprShape::call(
        CalleeShape::def_path("crate::load"),
        0,
    )),
    StmtShape::mutable_call(
        Some(norm.local_slot("result")),
        CalleeShape::def_path("crate::prepare"),
    ),
]);
```

`LocalSlot` indices are assigned in first-appearance order and are
deterministic across repeated normalization runs over the same name sequence.

### Test coverage

- **Unit tests** for the pure policy live in `common/src/rstest/tests.rs`.
- **BDD scenarios** live in
  `common/tests/rstest_span_recovery_behaviour.rs` and are driven by
  `common/tests/features/rstest_span_recovery.feature`.
- **Adapter tests** for `span_recovery_frames` and
  `recover_user_editable_hir_span` live in `src/hir/tests.rs`, loaded by
  `src/hir/mod.rs` under `#[cfg(test)]`.
- **Integration tests** for the first consumer are in
  `crates/function_attrs_follow_docs/src/tests/order_detection.rs`.

## Regression infrastructure

Two recent regression families rely on infrastructure that is easy to miss when
adding coverage or refactoring helpers.

### Async test harness detection

`no_expect_outside_tests` prefers source-level test attributes such as
`#[test]`, `#[rstest]`, and `#[tokio::test]`. In real `rustc --test`
compilations, async wrappers can lose that source-level marker and instead be
represented by a sibling `#[rustc_test_marker = "..."] const ...` descriptor.
The driver therefore falls back to matching that harness descriptor by symbol
name plus source range when direct attribute detection fails.

Keep the regression split aligned with that compiler boundary:

- Source-level attribute-shape coverage belongs in `ui/` fixtures.
- Regressions that need `--test`, example targets, or extra compiler flags
  belong in `crates/no_expect_outside_tests/src/lib_ui_tests.rs`.
- Real async framework regressions should use `examples/` targets when the bug
  depends on the same lowering path external consumers hit.

This separation exists so a proc-macro stub test cannot accidentally mask a
failure in the real harness-descriptor path.

### Test-context ancestry detection

`no_expect_outside_tests` decides whether an `.expect(..)` call sits in test
context by combining a HIR ancestry walk with attribute-shape matching.
`collect_context` traverses the ancestors of the call site, accumulates
`ContextEntry` items for modules, functions, impls, and blocks, and carries a
boolean `has_test_context_ancestry` alongside that list. On each step,
`has_test_ancestry` updates that boolean so the test-only decision can
propagate from outer ancestors into nested helper code. `summarise_context`
then combines the accumulated entries, the propagated boolean, and
`in_test_like_context_with(additional_test_attributes)` to produce the final
`ContextSummary.is_test` result. This pattern matters because user-configured
test markers such as `my_framework::test` must affect the whole ancestry chain,
not just the immediately enclosing function.

- `collect_context` starts at the `.expect(..)` call site and walks outward.
- `has_test_ancestry` returns `true` when any of these hold:
  - a prior ancestor already set `has_test_context_ancestry`
  - the current ancestor carries a `cfg(test)`-style attribute detected by
    `is_cfg_test_attribute`
  - the current ancestor is a function item whose attributes match Whitaker's
    built-in test list or `additional_test_attributes`
- `summarise_context` merges that ancestry flag with the collected
  `ContextEntry` values to derive the final `ContextSummary.is_test` decision.

Real `rstest` case expansion adds a second `--test` harness shape that the
attribute and direct sibling-descriptor paths do not see. For parameterized
cases, `rustc` lowers the user-written function into ordinary HIR and emits a
same-named sibling module that contains the synthesized harness functions plus
their `const` descriptors. In `src/hir/mod.rs`,
`collect_rstest_companion_test_functions()` extends the existing
`collect_harness_test_functions()` pass to catch that shape before
`no_expect_outside_tests` evaluates call-site context.

For example, this user-written test:

```rust
#[rstest]
#[case(Some("value"))]
fn accepts_rstest_case(#[case] input: Option<&str>) {
    input.expect("rstest case setup may use expect");
}
```

is represented as a module group shaped like this in the test harness:

```text
parent module
|-- fn accepts_rstest_case(...)
`-- mod accepts_rstest_case
    |-- fn case_1()
    `-- const case_1: test::TestDescAndFn
```

The important relationship is that the original function and the synthesized
module are siblings under the same parent module and share the same name. That
sibling module is the companion module; the parent module contents form the
module group that the helper scans.

This helper is architecturally significant for two reasons:

- It keeps compiler-lowering knowledge in the shared HIR module rather than in
  lint-specific ancestry logic, so other lints can reuse the same test-harness
  discovery rules if they need them later.
- It only marks a function when a same-named sibling module in the same module
  scope carries reliable rstest synthesis evidence, which prevents empty
  modules, arbitrary const-only sibling modules, and hand-authored `#[test]`
  modules from inheriting test status accidentally.

Two independent kinds of evidence qualify a sibling module as an rstest
companion:

1. **Explicit marker.** The module exposes a `RSTEST_HARNESS_DESCRIPTOR` const.
   This is unambiguous — no hand-authored test module emits that marker — so it
   qualifies the module on its own, and manual regression fixtures that cannot
   run the real proc-macro rely on it.
2. **Harness pair with expansion provenance.** The module contains the in-module
   `fn` / same-span (or adjacent split-span) `const` descriptor pair that
   `rustc --test` synthesizes, **and** the module item itself originates from
   macro expansion (`module_item.span.from_expansion()`).

The provenance guard in the second case is load-bearing. The `--test` harness
emits the same-named, same-span `const` descriptor for **every** `#[test]`
function, so the `fn`/`const` pair on its own is not rstest-specific: a
hand-authored `mod foo { extern crate test as t; #[test] fn bar() {} }` sitting
next to an ordinary `fn foo` has the identical HIR shape and would otherwise
wrongly exempt `fn foo`. The distinguishing rstest/`rustc` invariant is that
rstest generates the companion module through its attribute proc-macro, so the
module item comes from macro expansion, whereas a handwritten module sits at
the crate's root syntax context. The harness-pair heuristic is therefore only
trusted for modules that come from expansion.

The implementation follows three steps:

1. Walk each module group recursively, so nested `mod tests` blocks and inline
   modules are considered alongside crate-root items.
2. For each function item, look for a same-named sibling module in the same
   parent module.
3. Inspect that sibling module for rstest synthesis evidence before marking the
   original function as a test context, applying the two-tier evidence rule
   above. Empty modules, modules containing only unrelated items, and
   hand-authored `#[test]` modules that lack expansion provenance are never
   treated as companions.

That split lets the lint treat rstest companion modules as an extension of the
existing `--test` harness model instead of a separate policy path.

#### Known complexity limitation

`collect_companion_in_group()` and
`module_qualifies_as_rstest_companion_module()` in `src/hir/mod.rs` use nested
iteration, giving O(n²) complexity within each module scope. This is acceptable
in practice because module item counts are bounded at compile time and the
functions execute during lint analysis, not at runtime.

Issue [#225](https://github.com/leynos/whitaker/issues/225) tracks adding
complexity docstrings to those functions and evaluating a lookup-map
optimization.

### UI test harness helpers (`lib_ui_tests.rs`)

`crates/no_expect_outside_tests/src/lib_ui_tests.rs` provides the
infrastructure for regressions that require compiler flags, example targets, or
external crate dependencies that cannot be expressed with plain `ui/` source
fixtures.

#### Parameter structs

Two structs group the string parameters that describe a single regression run:

- **`ExampleHarnessRun`** — carries `name` (example binary name), `label`
  (used in panic messages), and `rustc_flags`. Use `ExampleHarnessRun::new` for
  the default `--test` flag, or `ExampleHarnessRun::with_flags` to supply a
  custom flag list.
- **`FixtureHarnessRun`** — carries `crate_name`, `directory` (top-level
  directory such as `"examples"` or `"ui"`), `fixture_name`, `label`,
  `rustc_flags`, and `extern_crates` (a list of additional crate names to wire
  as `--extern` dependencies).

#### Harness driver functions

Table: Harness driver functions and responsibilities.

| Function                         | Purpose                                                                                                                                 |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `run_example_under_test_harness` | Runs one example target under the dylint UI test harness using the flags in the `ExampleHarnessRun` spec.                               |
| `run_fixture_under_test_harness` | Prepares a fixture directory, assembles rustc flags (including extern-crate links), and delegates to `run_test_runner`.                 |
| `run_fixture_harness_test`       | End-to-end driver: calls `run_with_runner` and panics with a labelled message on failure. Used by parameterized `#[rstest]` test cases. |

#### Dependency Rust library (`rlib`) resolution

When a regression requires a real external crate (e.g. `tokio`), the harness
must locate the built rlib so it can pass `--extern tokio=…` to the compiler.
`dependency_directory()` finds the parent of the running test binary, and
`dependency_rlib()` scans that directory for `lib<crate>-*.rlib` files,
selecting the most recently built artefact. Ties in modification time are
broken by lexicographic path order for a stable, deterministic result.

Unit tests for this selection logic live in
`crates/no_expect_outside_tests/src/dependency_rlib_tests.rs`, loaded via a
`#[path]` module declaration in `lib_ui_tests.rs`.

#### `camino` dev-dependency

`lib_ui_tests.rs` uses [`camino`](https://docs.rs/camino)'s `Utf8Path` to pass
fixture directory paths to helper functions as UTF-8-guaranteed paths. This
avoids repeated `.to_str().unwrap()` calls on `std::path::Path` throughout the
harness. `camino` is a dev-dependency only; it is not present in the production
library.

### Staged-suite installer shortcut

Installer behavioural tests occasionally need the suite staging path without
recursively rebuilding the workspace from inside `nextest`. The debug-only
helper in `installer/src/staged_suite.rs` provides that shortcut.

- Behavioural tests opt in with `WHITAKER_INSTALLER_TEST_STAGE_SUITE=1`.
- The helper only activates for an exact suite-only request
  (`whitaker_suite` and nothing else).
- The helper returns `Ok(None)` in release binaries before reading the
  environment variable, so production installers never stage the placeholder
  artefact.

Use this hook only for installer orchestration tests. Release validation,
prebuilt-download coverage, and user-facing installation flows must continue to
exercise the real build or download paths.

### Test environment synchronization

Installer regression helpers that mutate process-wide environment variables
must coordinate through `installer/src/test_support.rs`.

- `env_test_guard()` acquires a shared `Mutex` before any test calls
  `temp_env::with_var` or `temp_env::with_var_unset`.
- Hold that guard for the full lifetime of the test setup so no parallel case
  can observe a half-applied environment change.
- `installer/src/staged_suite.rs` shows the intended pattern: acquire the
  guard, create the temporary target directory, then run the env-mutating test
  body.

For example:

```rust
let _guard = env_test_guard();
with_var(TEST_STAGE_SUITE_ENV, Some("1"), || {
    // exercise installer behaviour that reads the process environment
});
```

Keep the installer dev-dependencies aligned with that pattern when extending
the regression suite: `temp-env` provides scoped environment overrides,
`tempfile` provides isolated target directories, and `rstest` powers the
fixture-based test setup used by the staged-suite coverage.

#### Shared UI harness environment guards

Workspace-level UI harness tests that mutate process-wide environment variables
must use `whitaker_common::test_support::EnvVarGuard`. Use `EnvVarGuard::set`
to install a temporary value and `EnvVarGuard::remove` to make a variable
absent for the duration of a test. The guard acquires `env_test_guard()` only
while it captures, mutates, or restores the variable; it must not hold that
mutex while a runner callback executes, because the callback may need its own
guarded environment setup.

`whitaker::testing::ui::run_with_runner` applies a specialized guard before
invoking the Dylint UI runner. On every platform it clears `RUSTC_WRAPPER` only
while the runner needs bare `rustc` invocations for
`dylint_testing::Test::example`. On Windows it also sets `VCPKG_ROOT` to
`C:\vcpkg` when that directory exists and the variable is otherwise absent.
Restoration uses the same shared environment mutex, but the runner callback
itself executes without holding that mutex to avoid nested-lock deadlocks.

Example-based UI tests in `rstest_helper_should_be_fixture` also use a
cross-process directory lock under the system temporary directory. `nextest`
can run test binaries in separate processes, so a plain in-process `Mutex` is
not sufficient for those examples. A directory becomes eligible for stale
recovery only after it has aged past 30 minutes, and stale cleanup removes that
age-eligible lock directory only after successfully acquiring the lifetime
owner-liveness lock. If stale cleanup sees `NotFound`, it treats the directory
as already removed rather than as a failed recovery attempt, and the live lock
path still waits indefinitely in production while the tests keep the bounded
timeout. A sidecar advisory lock serializes ownership transitions, and the
directory's owner token prevents an original holder from deleting a successor
after stale recovery.

### Configuration constant patterns

Lint crates that expose configurable thresholds or defaults should centralize
the default value as a public constant. This prevents drift between code,
tests, configuration files, and documentation.

**Pattern:** Define a public constant in the analysis module and reference it
from `Settings::default()`, configuration parsing tests, and documentation.

**Example** (`bumpy_road_function`):

```rust
// src/analysis.rs
pub const DEFAULT_THRESHOLD: f64 = 2.5;

impl Default for Settings {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            // ... other fields
        }
    }
}
```

Add a regression test that validates the UI test configuration matches the
constant:

```rust
// tests/analysis_behaviour.rs
#[test]
fn ui_dylint_toml_threshold_matches_default() {
    let toml_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ui/dylint.toml");
    let contents = fs::read_to_string(&toml_path)
        .expect("ui/dylint.toml should exist");
    let parsed: toml::Value = toml::from_str(&contents)
        .expect("ui/dylint.toml should parse");

    let threshold = parsed
        .get("bumpy_road_function")
        .and_then(|t| t.get("threshold"))
        .and_then(toml::Value::as_float)
        .expect("bumpy_road_function.threshold should be a float");

    assert_eq!(
        threshold, DEFAULT_THRESHOLD,
        "ui/dylint.toml threshold must match DEFAULT_THRESHOLD constant"
    );
}
```

### Nextest filter validation

When adding nextest test-group filters (e.g., for serializing UI tests), add a
regression test that verifies the filter expression captures all intended test
binaries.

**Pattern:** Create a test that discovers relevant test files, parses the
nextest config, and asserts the filter contains the necessary clauses.

**Example** (`tests/nextest_ui_filter.rs`):

```rust
use rstest::{fixture, rstest};

#[fixture]
fn serial_dylint_ui_override() -> Value {
    let config = load_nextest_config();
    find_serial_dylint_ui_override(&config).clone()
}

#[rstest]
fn serial_dylint_ui_filter_captures_integration_ui_binaries(
    serial_dylint_ui_override: Value
) {
    let filter = extract_filter(&serial_dylint_ui_override);
    let crates = crates_with_integration_ui_test();

    assert!(
        filter.contains("(binary(ui) & test(=ui))"),
        "filter must capture integration test binaries: {crates:?}"
    );
}
```

This pattern prevents CI flakes where a new UI test is excluded from
serialization due to an incomplete filter expression.

## Using whitaker-installer

The `whitaker-installer` command-line interface (CLI) builds, links, and stages
Dylint lint libraries for local development. This avoids rebuilding from source
on each `cargo dylint` invocation.

### Basic usage

From the workspace root:

```sh
cargo run --release -p whitaker-installer
```

Or install it globally:

```sh
cargo install --path installer
whitaker-installer
```

By default, this builds the aggregated suite and stages it to a
platform-specific directory:

- Linux: `~/.local/share/dylint/lib/<toolchain>/release`
- macOS: `~/Library/Application Support/dylint/lib/<toolchain>/release`
- Windows: `%LOCALAPPDATA%\dylint\lib\<toolchain>\release`

When a prebuilt artefact is available, the installer extracts it to the
Whitaker data directory keyed by toolchain and target:

- Linux:
  `~/.local/share/whitaker/lints/<toolchain>/<target>/lib`
- macOS:
  `~/Library/Application Support/whitaker/lints/<toolchain>/<target>/lib`
- Windows:
  `%LOCALAPPDATA%\whitaker\lints\<toolchain>\<target>\lib`

### Configuration options

- `-t, --target-dir DIR` — Staging directory for built libraries
- `-l, --lint NAME` — Build specific lint (repeatable)
- `--individual-lints` — Build individual crates instead of the suite
- `--experimental` — Include experimental lints in the build. In suite mode
  this enables feature-gated experimental lints on `whitaker_suite`; in
  `--individual-lints` mode it adds crates from `EXPERIMENTAL_LINT_CRATES`.
- `--toolchain TOOLCHAIN` — Override the detected toolchain
- `--cranelift` — Install `rustc-codegen-cranelift` for the selected toolchain
- `-j, --jobs N` — Number of parallel build jobs
- `--dry-run` — Show what would be done without running
- `-v, --verbose` — Increase output verbosity (repeatable)
- `-q, --quiet` — Suppress output except errors
- `--skip-deps` — Skip `cargo-dylint`/`dylint-link` installation check
- `--skip-wrapper` — Skip wrapper script generation
- `--no-update` — Don't update existing repository clone

### Using installed lints

After installation, set `DYLINT_LIBRARY_PATH` to the staged directory:

```sh
export DYLINT_LIBRARY_PATH="$HOME/.local/share/dylint/lib/nightly-2025-01-15/release"
cargo dylint --all
```

For prebuilt installs, use the toolchain-and-target-specific directory:

```sh
export DYLINT_LIBRARY_PATH="$HOME/.local/share/whitaker/lints/nightly-2025-01-15/x86_64-unknown-linux-gnu/lib"
cargo dylint --all
```

Alternatively, configure workspace metadata to use the pre-built libraries
directly:

```toml
[workspace.metadata.dylint]
libraries = [
  { path = "/home/user/.local/share/whitaker/lints/nightly-2025-01-15/x86_64-unknown-linux-gnu/lib" }
]
```

This skips building entirely, providing faster lint runs during development.

### Installer internal architecture

`installer/src/main.rs` coordinates the installation workflow through a small
set of focused private helpers. Understanding them is useful when extending the
installation pipeline.


#### Public Git operation APIs

The `whitaker_installer::git` module exposes the following Git operations. They
all accept a UTF-8 repository path and return the installer's `Result` type;
each operation is bounded by the module's five-minute Git timeout. Use these
functions for the managed clone workflow, not to mutate a user's current
Whitaker checkout.

Triage: `type:docstyle`

*Table: Public Git operation APIs.*

| API | Purpose | Usage constraints |
| --- | --- | --- |
| `resolve_commit(repo: &Utf8Path, refspec: &str) -> Result<String>` | Resolves a local commit-ish (SHA, tag, or branch) to its full commit SHA and peels annotated tags. | Does not fetch. Call it when the ref is expected to exist locally, or after `fetch_ref` has populated the clone. |
| `fetch_ref(repo: &Utf8Path, refspec: &str) -> Result<String>` | Fetches the requested ref and tags from `origin`, records the result in the private `refs/whitaker/pinned-ref` ref, and returns its full commit SHA. | Use it to refresh a requested pin before checkout. It force-updates only the private pin ref; it does not move the current branch or check out the result. |
| `checkout_detached(repo: &Utf8Path, commit: &str) -> Result<()>` | Checks out exactly `commit` with a detached `HEAD`. | Use only for the installer-managed clone after resolving the requested ref. The workspace layer must reject pinning in the user's current Whitaker workspace first. |
| `ensure_default_branch(repo: &Utf8Path) -> Result<()>` | Reattaches a detached clone to the branch named by `origin/HEAD`; repairs a missing `origin/HEAD` with `git remote set-head origin --auto`. | Call before `update_repository` when a previous pin may have detached the managed clone. It is a no-op when `HEAD` already names a branch and does not pull changes itself. |

#### `resolve_additional_components`

```rust
fn resolve_additional_components(args: &InstallArgs) -> &'static [&'static str]
```

Translates CLI flags into the extra rustup component slice passed to
`ensure_toolchain_installed`. At present, the only flag it handles is
`--cranelift`, which adds `rustc-codegen-cranelift` to the component set.
Adding a new optional component requires a new CLI flag on `InstallArgs` and a
new arm in this function; no other callers need to change.

#### `FastPathContext`

```rust
struct FastPathContext<'a> {
    args: &'a InstallArgs,
    dirs: &'a dyn BaseDirs,
    requested_crates: &'a [CrateName],
    toolchain: &'a Toolchain,
    target_dir: &'a Utf8PathBuf,
}
```

A parameter-object struct that bundles the five immutable inputs consumed by
`try_fast_path_installation`. This follows the same idiom used elsewhere in the
codebase (`FinishInstallContext`, `PrebuiltInstallationContext`,
`MetricsWriteContext`) to keep function argument counts within the project
threshold of four. Construct it in `run_install` before calling
`try_fast_path_installation`.

#### `try_fast_path_installation`

```rust
fn try_fast_path_installation(
    context: &FastPathContext<'_>,
    stderr: &mut dyn Write,
) -> Result<Option<(Utf8PathBuf, InstallMode)>>
```

Attempts both fast paths in order and returns `Some((staging_path, mode))` if
either succeeds, or `None` if `run_install` should proceed to a full build:

1. **Prebuilt download** (`InstallMode::Download`) — delegates to
   `try_prebuilt_installation` inside `install_flow`.
2. **Staged-suite shortcut** (`InstallMode::Build`) — delegates to
   `staged_suite::try_test_staged_suite_installation`, which is only active when
   `WHITAKER_INSTALLER_TEST_STAGE_SUITE=1` is set (debug builds only).

When `try_fast_path_installation` returns `Some`, `run_install` constructs a
`FinishInstallContext` from the returned values and delegates to
`finish_install_and_record_metrics`, skipping the full build pipeline.

#### Crate resolution

`installer/src/resolution.rs` is the boundary for deciding which lint crates
the installer may build. `run_install` constructs `CrateResolutionOptions` from
the CLI flags before validating or resolving crate names:

- `individual_lints` switches resolution from the aggregated suite to the
  individual lint crate list.
- `experimental` is the explicit opt-in gate for experimental lints. In
  individual-lint mode it allows crates from `EXPERIMENTAL_LINT_CRATES`; in
  suite mode the build configuration maps it to suite feature flags.

Call `validate_crate_names` before `resolve_crates` whenever names come from
user input. Validation rejects unknown names and uses `is_experimental_crate`
to detect explicit experimental lint requests. If a user asks for an
experimental crate without `--experimental`, validation returns
`InstallerError::ExperimentalLintRequiresFlag`; callers should surface that
error unchanged so the CLI reports the missing opt-in rather than treating the
crate as unknown or silently building it.

## Standard vs Experimental Lints

Whitaker categorizes lints into two tiers:

- **Standard lints** are stable, well-tested, and included in the default suite.
  They have predictable behaviour with minimal false positives.
- **Experimental lints** are newer or more aggressive checks that may produce
  false positives or undergo breaking changes. They require explicit opt-in via
  the `--experimental` flag.

The current experimental set contains `rstest_helper_should_be_fixture`. It is
feature-gated in the suite as `experimental-rstest-helper-should-be-fixture`
and listed in `installer/src/resolution.rs` so the installer can derive the
matching suite feature automatically.

`rstest_helper_should_be_fixture` currently uses an in-crate collector rather
than a shared adapter. The collector stores passive call-site evidence in
deterministic `BTreeMap` order keyed by `tcx.def_path_str(callee_def_id)`,
deduplicates entries with a private `CallSiteLocation`, and preserves the raw
`DefId` in each record for later diagnostics. The late pass only records local
function or associated-function callees inside strict `#[rstest]` tests, drops
call sites whose spans cannot recover to user-editable source, and lowers
arguments conservatively to fixture-local, literal, constant path, or
unsupported atoms.

Future rstest lints should promote this adapter out of
`crates/rstest_helper_should_be_fixture/src/collector.rs` only when another
crate consumes the same HIR lowering policy. Until then, keep compiler-aware
HIR logic in the lint crate and keep the pure `ArgAtom`/`ArgFingerprint` model
in `whitaker_common::rstest`.

### Adding a new lint

New lints should typically start as experimental. To add a lint:

1. Create the lint crate under `crates/` (see
   [Creating a New Lint](#creating-a-new-lint))
2. Add the crate name to `EXPERIMENTAL_LINT_CRATES` in
   `installer/src/resolution.rs`
3. Add a feature flag for the lint in `suite/Cargo.toml` under `[features]`
4. Add an optional suite dependency and gate its descriptor, lint declaration,
   and combined pass entry behind that feature

### Promoting to standard

Once an experimental lint has been:

- Tested across multiple real-world codebases
- Refined to minimize false positives
- Stabilized with no breaking changes planned

It can be promoted to standard by:

1. Moving the crate name from `EXPERIMENTAL_LINT_CRATES` to `LINT_CRATES`
2. Adding the lint dependency to the suite `dylint-driver` feature in
   `suite/Cargo.toml`
3. Updating documentation to reflect the change

## Creating a New Lint

### Generating from the template

The `whitaker::LintCrateTemplate` helper generates both a `Cargo.toml` manifest
and a baseline `src/lib.rs`:

1. Create a directory for the lint under `crates/`.
2. Use the template to generate files:

   ```rust
   use cap_std::{ambient_authority, fs::Dir};
   use whitaker::LintCrateTemplate;

   fn main() -> Result<(), Box<dyn std::error::Error>> {
       let template = LintCrateTemplate::new("my_new_lint")?;
       let files = template.render();

       let root = Dir::open_ambient_dir(".", ambient_authority())?;
       root.create_dir_all("crates/my_new_lint/src")?;
       root.write("crates/my_new_lint/Cargo.toml", files.manifest())?;
       root.write("crates/my_new_lint/src/lib.rs", files.lib_rs())?;
       Ok(())
   }
   ```

3. Populate `ui/` fixtures for the lint. The generated `lib.rs` already declares
   the canonical `whitaker::declare_ui_tests!("ui")` test.

### Template options

`LintCrateTemplate::with_ui_tests_directory` targets alternative directories.
The helper:

- Normalizes Windows-style separators to forward slashes
- Rejects traversal via `..`
- Fails fast on empty names, uppercase characters, trailing separators, or
  absolute paths

### UI test fixtures

Create test fixtures under `crates/my_new_lint/ui/`:

- `pass_*.rs` - Code that should pass the lint
- `fail_*.rs` - Code that should trigger the lint
- `fail_*.stderr` - Expected diagnostic output

## Testing Lints from Git

To test lints directly from a Git repository without installing:

```sh
cargo dylint list --git https://github.com/leynos/whitaker --rev v0.1.0 --all
```

Swap `v0.1.0` for the tag to test, or omit `--rev` to use the current branch
tip.

## Localized Diagnostics

Whitaker supports multiple locales for diagnostic messages. Fluent resources
are bundled under `common/locales/`.

This layout is an architectural boundary rather than a convenience. The
`whitaker-common` crate is published independently, so `cargo package` only
ships files that live under `common/`. Keeping the Fluent bundles crate-local
ensures the published tarball contains the translations required by
`common::i18n` during package verification and downstream consumption.

### Available locales

- `en-GB` (default) - English
- `cy` - Welsh (Cymraeg)
- `gd` - Scottish Gaelic (Gàidhlig)

### Using the Localizer API

```rust
use common::i18n::{
    available_locales, Arguments, Localizer, FALLBACK_LOCALE, branch_phrase,
};
use common::i18n::FluentValue;
use std::borrow::Cow;
use std::collections::HashMap;

let preferred = "gd";
assert!(available_locales().contains(&preferred.to_string()));

let localizer = Localizer::new(Some(preferred));

let mut args: Arguments<'static> = HashMap::new();
let branch_count = 3;
let branch_limit = 2;
args.insert(Cow::Borrowed("name"), FluentValue::from("match on Foo"));
args.insert(Cow::Borrowed("branches"), FluentValue::from(branch_count));
args.insert(Cow::Borrowed("limit"), FluentValue::from(branch_limit));
let branch_phrase_text = branch_phrase(localizer.locale(), branch_count as usize);
args.insert(
    Cow::Borrowed("branch_phrase"),
    FluentValue::String(Cow::Owned(branch_phrase_text)),
);
let limit_phrase_text = branch_phrase(localizer.locale(), branch_limit as usize);
args.insert(
    Cow::Borrowed("limit_phrase"),
    FluentValue::String(Cow::Owned(limit_phrase_text)),
);

let message = localizer
    .message_with_args("conditional_max_n_branches", &args)?;
let note = localizer
    .attribute_with_args("conditional_max_n_branches", "note", &args)?;

if localizer.used_fallback() {
    eprintln!("Fell back to {FALLBACK_LOCALE}");
}
```

### Locale resolution

Language selection uses `common::i18n::available_locales()` to enumerate
compiled locales. When an unsupported locale is requested, the loader falls
back to `en-GB` and surfaces a missing message error if a slug is not
translated.

Locale can be set via:

1. `DYLINT_LOCALE` environment variable
2. `locale` entry in `dylint.toml`

The `common::i18n::resolve_localizer` helper combines explicit overrides with
environment and configuration, trimming whitespace and warning about
unsupported locales.

### Adding translations

Lints source messages directly from Fluent bundles at emission time. Each
diagnostic assembles structured arguments, so translations never depend on
hand-built strings. If a lookup fails, the lint records a delayed span bug and
falls back to deterministic English text.

To add a new locale:

1. Create a new directory under `common/locales/` (e.g.,
   `common/locales/fr/`)
2. Add `.ftl` files with translated messages
3. Update `common::i18n::available_locales()` to include the new locale

## Release tooling

Whitaker includes tooling for automating release-related tasks.

### `scripts/generate_checksums.py`

This script generates SHA-256 checksum files for release archives. It is
integrated into the rolling-release workflow to produce `.sha256` files for all
published archives.

#### Usage

Generate checksums for archives in the default `dist/` directory:

```sh
scripts/generate_checksums.py
```

Generate checksums for archives in a custom directory:

```sh
scripts/generate_checksums.py /path/to/archives
```

#### Public API

The script exposes the following functions for programmatic use:

- **`compute_sha256(path: Path) -> str`** — Computes the SHA-256 hex digest for
  a file using streaming reads to handle large files without memory pressure.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import compute_sha256

  digest = compute_sha256(Path("archive.tgz"))
  print(f"SHA-256: {digest}")
  ```

- **`find_archives(directory: Path) -> list[Path]`** — Discovers archive files
  matching the configured patterns (`*.tgz`, `*.zip`). Returns a sorted list of
  paths. Raises `NoArchivesFoundError` if no matching archives are found.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import find_archives

  archives = find_archives(Path("dist"))
  for archive in archives:
      print(f"Found: {archive.name}")
  ```

- **`generate_checksums(directory: Path) -> None`** — Generates `.sha256` files
  for all archives in the specified directory. Checksum files are written in
  the format `HASH  FILENAME\n` for compatibility with standard verification
  tools.

  ```python
  from pathlib import Path
  from scripts.generate_checksums import generate_checksums

  generate_checksums(Path("dist"))  # Creates dist/*.sha256 files
  ```

#### Exceptions

- **`NoArchivesFoundError`** — Raised when `find_archives()` or
  `generate_checksums()` cannot locate any archive files matching the
  configured patterns. This exception indicates either an empty directory or a
  path mismatch.

#### Integration with release workflow

The script is invoked by the rolling-release workflow after archives are
packaged. Checksum files are uploaded alongside archives as workflow artefacts,
allowing users to verify download integrity using standard tools (see the
[README](../README.md) for platform-specific verification instructions).

## Publishing

Before publishing, run the full validation suite:

```sh
make publish-check
```

This builds each package and every lint library in a production-like
environment, without the `prefer-dynamic` flag used during development, and
packages the crates for inspection. It runs no tests: the coverage job is the
single execution of the suite per pull request, as described in "One execution
of the test suite per pull request" above.

[issue-180]: https://github.com/leynos/whitaker/issues/180
[whitaker-run-33748602187]: https://github.com/leynos/whitaker/actions/runs/33748602187
[whitaker-run-33756048103]: https://github.com/leynos/whitaker/actions/runs/33756048103
[whitaker-run-33410178021]: https://github.com/leynos/whitaker/actions/runs/33410178021
[whitaker-run-33369228466]: https://github.com/leynos/whitaker/actions/runs/33369228466
[whitaker-run-33345742967]: https://github.com/leynos/whitaker/actions/runs/33345742967
[whitaker-run-33340945546]: https://github.com/leynos/whitaker/actions/runs/33340945546
[whitaker-run-33322310248]: https://github.com/leynos/whitaker/actions/runs/33322310248
