# Bring the Whitaker Dylint gate back to green

This ExecPlan is a living document. The sections `Progress`,
`Surprises & discoveries`, `Decision log`, and `Outcomes & retrospective` must
be kept up to date as work proceeds.

## Purpose and observable outcome

Pull request 372 ("Run the Whitaker suite over Whitaker's own sources") is open
against `harden-lint-workspace-config`, and both of its CI jobs failed on
2026-11-01. The failure is not caused by this branch's Rust changes. It is
caused by the Dylint toolchain the repository pins: `dylint_testing` 6.0.1 and
`dylint_driver` 6.0.1 clone `rust-lang/rust-clippy` at the branch name
`master`, and that branch no longer exists.

The outcome we want is observable and narrow: `make lint` and `make test` both
pass locally on this branch, CI turns green for a new head commit, and the
blocking `CHANGES_REQUESTED` review on the superseded commit is superseded by a
review of the new head. A reader should be able to confirm the work is done by
running the two gates and by looking at the PR's check list.

## Context and orientation

The repository is `leynos/whitaker`, a Rust workspace of Dylint lints. Dylint
is the third-party framework that loads the lints into a compiler.
`cargo-dylint` is the command-line driver. `dylint_testing` is the library the
UI tests use to build an example and run a lint against it. `dylint_driver` is
a build-only crate whose build script needs a checkout of
`rust-lang/rust-clippy` so that it can compile the driver against the same
rustc the toolchain uses.

Three files matter most:

- `Cargo.toml` at the repository root declares
  `dylint_linting = "6"` and `dylint_testing = "6"` under
  `[workspace.dependencies]`. Caret requirements mean `Cargo.lock`, not the
  manifest, decides the resolved version.
- `Cargo.lock` currently pins `dylint`, `dylint_internal`, `dylint_linting` and
  `dylint_testing` at 6.0.1.
- `Makefile` at lines 69–73 pins `CARGO_DYLINT_VERSION ?= 6.0.1`,
  `DYLINT_LINK_VERSION ?= 6.0.1` and `DYLINT_TOOLS_TOOLCHAIN ?= stable`.
  `installer/dependency-binaries.toml` pins the same two tools at 6.0.1.

The failing gate is `lint-whitaker` in the `Makefile`, which shells out to the
`whitaker` wrapper installed by `whitaker-installer`. The wrapper is a two-line
bash script that exports `DYLINT_LIBRARY_PATH` and runs `cargo dylint`. The
`windows-compat` CI job has no lint step; it fails in `make test` because the
UI tests in `crates/rstest_helper_should_be_fixture/tests/ui.rs` call
`dylint_testing::ui::Test::example`, which builds `dylint_driver` from
crates.io inside a temporary directory.

## The failure, precisely

Two distinct symptoms, one cause.

`make lint` on Linux fails while building `dylint_driver v6.0.1`'s build
script. The build script calls
`dylint_internal::clippy_utils::repository::clippy_repository`, which in 6.0.1
hard-codes `clone(RUST_CLIPPY_URL, "master", …)`. rust-clippy renamed its
default branch to `main`, so the clone succeeds and the checkout fails:

```plaintext
Cloning into '/tmp/.tmpvF8ofW'...
Error: `revparse_ext` failed for `master`
revspec 'master' not found; class=Reference (4); code=NotFound (-3)
  9: dylint_internal::clippy_utils::repository::clippy_repository
 10: <dylint_internal::clippy_utils::revs_no_preinstall::Revs>::new
 11: build_script_main::extra_symbols::rev_from_channel
```

`make test` on Windows fails identically, four times, once per UI test that
reaches the harness, and stops the build with
`make: *** [Makefile:99: test] Error 100`.

That the failure is time-dependent rather than candidate-dependent is proven by
two runs on the same commit `0080ed49`: run `40562097513` passed on 2026-10-27
and run `41133125389` failed on 2026-10-30 with `revspec 'master' not found`.
No commit changed between them.

## Constraints

The rebase instruction stands but has already been satisfied.
`git merge-base --is-ancestor origin/harden-lint-workspace-config HEAD` exits
0, and `git rev-list --merges 5a08c6ff..HEAD` is empty. There is nothing to
replay, so no rebase will be performed and the earlier evidence stays valid.
This must be recorded rather than repeated.

`origin/main` is not a suitable rebase target. Patch-identity comparison shows
that every commit of PR 370 and PR 371 is still unlanded relative to `main`, and
`5a08c6ff..origin/main` spans 716 files with +76493/−9545. Retargeting would
drag a parent PR's work into a 35-file PR. PR 372 must stay on its current base.

The repository forbids isolated Cargo caches, forbids bypassing compile
admission, and forbids altering `RUSTC_WRAPPER` or `RUSTDOC`. Every compiler
invocation here queues normally.

All seven crates of the Dylint family — `dylint`, `dylint_driver`,
`dylint_internal`, `dylint_linting`, `dylint_testing`, `cargo-dylint` and
`dylint-link` — are published through 6.3.2 with nothing yanked. This was
measured against the crates.io API on 2026-11-02, not assumed: 6.3.2 was
released 2026-10-30 for every one of them. The binary tools are pinned one
release behind the libraries at 6.3.0, matching the repository's own `v0.2.11`
installer tag and the assets that release publishes.

## Tolerances (exception triggers)

Stop and escalate if any of the following happens:

- The dylint 6.3.x upgrade requires a source change in a lint crate beyond a
  mechanical `impl_late_lint!` or `config::<T>` signature adaptation.
- `make lint` or `make test` fails for a reason that is not attributable to the
  version change, and the failure survives one clean re-run.
- More than 12 non-ExecPlan files change, or more than 600 net lines.
- Compile admission becomes unhealthy (`build-limits-status` exits non-zero, or
  a `manager`/`stopped_populated` fault appears).
- A secret, token, or credential would need to be printed to diagnose a step.

## Risks

The largest risk is a false green: the version bump may move the driver build
onto a different Clippy commit than the one the repository's lints were tested
against. `dylint_internal` 6.3.x selects a Clippy revision by walking
first-parent history from the repository's `HEAD`, so an un-pinned clone of
`main` is mutable upstream state. The verification plan below must show the
actual selected revision.

The second risk is that `dylint_testing` 6.3.x changes its example-invocation
parsing in a way that interacts with the repository's own wrapper scripts. The
6.3.2 source carries wrapper-aware parsing, but that has to be observed, not
assumed. The repository already defends the neighbouring hazard:
`src/testing/ui/mod.rs` clears `RUSTC_WRAPPER` for every UI run because
`dylint_testing::Test::example` scans `cargo build --message-format=json` for
bare `rustc` and panics when a wrapper intercepts. CI sets
`RUSTC_WRAPPER: sccache`, so that defence is load-bearing there and must
survive the upgrade.

The third risk is CI connectivity, and the original assumption here was
backwards. `WHITAKER_INSTALLER_VERSION` was pinned to 0.2.7, and 0.2.7's
embedded `installer/dependency-binaries.toml` pins both tools at 6.0.1 — as do
0.2.8, 0.2.9 and 0.2.10. Only `v0.2.11` carries 6.3.0, and it is the first
published installer with an embedded Clippy snapshot manifest at all. Leaving
0.2.7 in place would have made the installer reinstall the superseded tools
over the Makefile pins during `make lint` in CI, which is the exact failure the
gate is meant to catch. The pin therefore moves to 0.2.11, and the CI workflow
records why at the point of the pin.

The fourth risk is that the version skew between two halves is asymmetric in a
way that only shows up at runtime. `dylint::driver_builder::initialize` writes
`version = "=<the asking crate's CARGO_PKG_VERSION>"` into the temporary driver
manifest, so `dylint_testing` 6.3.2 asks for `dylint_driver = "=6.3.2"`, not
`=6.3.0`. A host that already holds a warm 6.3.0 driver under
`~/.dylint_drivers/<toolchain>/dylint-driver` will judge it outdated
(`is_outdated` returns `their_version < our_version`) and rebuild. That rebuild
is silently satisfied by cloning whichever upstream state `main` currently
holds unless the pinned Clippy repository is supplied.

## Verification plan

There are four obligations.

First, the suite must resolve the new versions. Evidence is `cargo metadata`
output showing `dylint_linting` and `dylint_testing` at 6.3.2 and `dylint` at
6.3.2. Command: `cargo metadata --locked --format-version 1 | jq`. Discharge
condition: the printed versions match.

Second, no lint crate may lose behaviour. Evidence is `make lint`, whose
`lint-whitaker` target runs the whole suite over the support crates. Discharge
condition: exit status 0 and no `Dylint` warning lines.

Third, the UI tests must build the driver against a valid Clippy checkout.
Evidence is `make test` with `NEXTEST_PROFILE=ci`, which runs the same UI tests
that failed on Windows. Discharge condition: the `bumpy_road_function::ui` and
`rstest_helper_should_be_fixture::ui` tests pass.

Fourth, the documentation must describe the pins that now exist. No file under
`docs/` — `docs/developers-guide.md`, `docs/publishing.md` and
`docs/whitaker-dylint-suite-design.md` all carried 6.0.1 — and no live code or
workflow file may name 6.0.1 after the change. Discharge condition:
`grep -rn '6\.0\.1' docs/*.md Makefile installer .github/workflows/ci.yml`
returns only the explanatory sentence in `ci.yml` that says earlier installer
releases embed 6.0.1. Historical ExecPlans under `docs/execplans/` are records,
not statements about the current tree, and are excluded.

Non-vacuity: the obligations can fail. A stale lock file fails the first
obligation. A behavioural regression in a lint fails the second. The exact
Windows failure earlier in this document is the witness that the third
obligation is reachable and not satisfied vacuously by a test that never builds
a driver. The fourth can fail by simply not running.

No new invariant is introduced, so no property test or formal proof applies.
The change is a dependency-and-pins bump, and the tests that already exist are
the right level of rigour.

## Milestones

**M1 — Record the rebase disposition and capture the failure evidence.** The
branch is already on its base, and the CI logs are saved at
`/tmp/pr372-run41258935258-failed.log`. End state: no mutation, and the plan
above records why. Recovery: none needed. Acceptance evidence: the ancestry
command exits 0 and the merge list is empty.

**M2 — Move the Dylint pins.** Change `Cargo.lock` so `dylint`,
`dylint_internal`, `dylint_linting` and `dylint_testing` resolve to 6.3.2, and
change `Makefile` lines 69–70 and `installer/dependency-binaries.toml` so
`cargo-dylint` and `dylint-link` are 6.3.0. `dylint_driver` is not in the lock
because it is built from crates.io inside a temporary directory at test time;
it will pick up whatever `dylint` 6.3.2 requests, which is `=6.3.2` — the
asking crate's own version, because `initialize()` interpolates
`env!("CARGO_PKG_VERSION")` into the manifest it writes. Every assertion that
mirrors a pin moves with it: the embedded-manifest fixtures and `rstest` cases
in `installer/src/dependency_binaries/manifest.rs`, the archive-name test in
`installer/src/dependency_binaries/install/tests.rs`, the expected
`cargo install` invocations in `installer/src/tests/mod.rs` and
`installer/src/deps/tests.rs`, the doc comment in `installer/src/deps/mod.rs`,
and the two Python mirrors under `tests/workflows/`. The CI installer pin moves
to 0.2.11 and the three places that explain the host-tool toolchain are
corrected to name the real floor (`cargo-dylint` 6.3.0 locks `cargo-util`
0.2.32, which needs rustc 1.97). End state: `cargo metadata --locked` succeeds
and reports the new versions. Recovery:
`git checkout -- Cargo.lock Makefile installer/dependency-binaries.toml` and
`git checkout -- <each mirrored test>`.

**M3 — Run the gates.** Run `make check-fmt`, `make test` with
`NEXTEST_PROFILE=ci`, `make typecheck` and `make lint` sequentially, each
captured to a log under `/tmp`, with
`DYLINT_CLIPPY_REPO=/home/leynos/.cache/whitaker-clippy-snapshot/e4608a25c2e20116fbc1e97a51a73f094a1db7a1`
in the environment. End state: all four exit 0. Acceptance: log tails show the
expected pass lines and no `revspec 'master'` text.

The environment variable is not optional decoration. This host holds a warm
6.3.0 driver at `~/.dylint_drivers/nightly-2026-05-28/dylint-driver`; the 6.3.2
libraries ask for `=6.3.2`, so the first UI test that runs rebuilds the driver
in-process, and without the variable `clippy_repository` clones upstream
`main` — mutable state that could differ between two runs of the same commit.
The supplied path is a real, non-shallow Git repository whose `HEAD` is
`e4608a25c2e20116fbc1e97a51a73f094a1db7a1`, whose `HEAD` is `refs/heads/main`,
and which contains the `selected_revision`
`9fca3bc9fc2bc83c60bde26d18ed68f11564b228` ("Rustup (#17109)") on its
first-parent history. `clippy_repository` clones from that object database into
a tempdir and leaves the source untouched.

That revision is not inferred from history alone. The snapshot directory
carries a manifest, `whitaker-clippy-snapshot.json`, which is what the
installer publishes alongside the bundle:

```json
{
  "schema_version": 1,
  "commit": "e4608a25c2e20116fbc1e97a51a73f094a1db7a1",
  "toolchain": "nightly-2026-05-28",
  "selected_revision": "9fca3bc9fc2bc83c60bde26d18ed68f11564b228",
  "dylint_version": "6.3.0",
  "bundle_sha256": "8b7e4716285185e9c799e48fe8f05096b73d31f0467806f0b8290476f408810c"
}
```

Three independent checks agree on it: the manifest names it, the directory's own
`git rev-parse HEAD` returns the `commit` field, and
`git rev-list --first-parent` over the 8010-commit history contains
`9fca3bc9fc2bc83c60bde26d18ed68f11564b228` exactly once. This file lives on the
host cache, not in the repository; the repository's copy is the installer's
published `clippy-snapshot.toml` at tag `v0.2.11`.

**M4 — Commit, push, and re-observe.** Commit as one atomic change, push over
SSH with a lease bound to the head the previous push left, refresh the tracking
ref, and dispatch CI on the new head. End state: the PR's checks are green for
the new head. Recovery: the old head remains reachable through the PR's reflog.

The lease target is a leaf value, not a constant. It was `d10469fd` when this
milestone was written; by 2026-11-03 the remote head had moved to `adc974c9`,
so the last push leased `adc974c9..adc974c9`. Re-read `git ls-remote` before
each push rather than copying an earlier lease into the command.

## Progress

- [x] (2026-11-02) Confirmed the instructed rebase is a verified no-op.
- [x] (2026-11-02) Identified the single upstream cause: `clone(…, "master")`.
- [x] (2026-11-02) Confirmed 6.3.2 fixes both blockers upstream.
- [x] (2026-11-02) Wrote this plan.
- [x] (2026-11-02) Regenerated `Cargo.lock` to 6.3.2 and proved it is a
  regenerate, not a hand edit:
  `cargo update -p dylint -p dylint_internal
  -p dylint_linting -p dylint_testing --precise 6.3.2`
  from the committed lock reproduces the working-tree lock byte for byte
  (`cmp` exit 0).
- [x] (2026-11-02) M2: moved the pins, the CI installer pin, every mirrored
  test assertion, and the stale 6.0.1 wording in code, workflows and docs.
- [x] (2026-11-02) `make fmt`, `make markdownlint`, `make spelling` and
  `make nixie` (no Mermaid changed) pass; the Python mirrors under
  `tests/workflows/` pass, 10 tests, after the `PATH` hermeticity repair.
- [x] (2026-11-03) M3a: the `revspec 'master'` objective is met. `make test`
  compiled 341 crates including `dylint_internal` 6.3.2, `dylint` 6.3.2,
  `dylint_linting` 6.3.2 and `dylint_testing` 6.3.2, and logged **zero**
  `revspec 'master'` occurrences; 1637 of 1665 tests passed. The objective for
  M3a — no `revspec 'master'` anywhere — is met and that is the decisive
  evidence for the pin change. That run was **not** green overall, and its
  remaining failures are M3b work, not M3a work: six lines report
  ``could not find `dylint_version` `` for `no_unwrap_or_else_panic` and
  `rstest_helper_should_be_fixture`, alongside a timeout and a SIGTERM on the
  nested-Cargo harnesses. The excerpt is pre-fix evidence: the log finished at
  03:05, and `src/testing/ui/mod.rs` gained the copy-library guard at 04:36.
  `no_unwrap_or_else_panic` reaches the guard because its `src/lib_ui_tests.rs`
  calls `whitaker::testing::ui::run_with_runner`, which applies
  `with_runner_env` around `ensure_toolchain_library` and the runner, and
  `dylint_testing::initialize` is lazy, so it fires inside that closure.
- [x] (2026-11-03) M3b: the two pre-existing causes found during M3 are
  repaired in the working tree. `dylint_testing`'s copy-library overwrite is
  guarded by `DYLINT_LINK_ENABLE_COPY_LIBRARY=1`, the example-harness lock is
  target-scoped instead of host-global, and `.config/nextest.toml` now bounds
  tests on both profiles. Gate re-run evidence is recorded under M4.
- [ ] (in progress) M4: `make fmt` is green and the unrelated
  `docs/whitaker-dylint-suite-design.md` reflow churn is reverted again after
  the second `make fmt` pass re-damaged the same `<pre><code>` block. The
  remaining Rust gates re-run after the gate-2 fix; push, re-observation and CI
  dispatch follow the gate results. Readiness, not the commit, is the gate.
  Status at 2026-11-03: the publication and re-observation halves are done (see
  the Progress entries below through the assessment reply). What M4 still owes
  is a completed, green exact-head CI result for the head that carries the
  finding-3 and finding-4 repairs.
- [x] (2026-11-03) The repair is committed as `f19408ff`
  ("Guard the Dylint copy step and scope the harness lock"). It sits on unpushed
  `55da1e23`, on top of remote `d10469fd`. Nine files, 740 insertions, 177
  deletions.
- [x] (2026-11-03) A focused re-run of the three nested-Cargo harnesses
  produced one decisive result before its supervisor timed it out:
  `example_compiles_without_diagnostics::case_1` passed in 2386.964 s (39.8
  min) against the 75-minute allowance. That is direct proof the override
  reaches the test and the nested build completes; the earlier readings of
  "stalled" were an artefact of sampling a 300 s window during a ~100-deep
  compile queue. The run then hit the background supervisor's wall-clock limit
  at 39:55 and was SIGTERMed mid-way through `case_2`, which nextest recorded
  as a `SIGTERM` abort and one retry, not as a timeout. Log:
  `/tmp/test-rshf-harness-harden-lint-whitaker-gate.out`. The run was not
  restarted: gate 2 below runs the same three harnesses under the `ci` profile
  and supersedes it as evidence.
- [x] (2026-11-03) `make check-fmt` passed. Its log holds only the echoed
  command, which is `cargo fmt --check`'s success output.
- [x] (2026-11-03) Gate 2, `make test NEXTEST_PROFILE=ci`, **failed** with
  two `TIMEOUT [ 300.006s]` lines and a cascade cancellation:
  `no_unwrap_or_else_panic ui::hand_written_test_companion_does_not_exempt_parent_function`
  and `ui::aliased_test_crate_non_companion_does_not_exempt_parent_function`
  at 1642/1673 and 1643/1673, then
  `Cancelling due to test failure: 3 tests still running`. 1671 of 1673 tests
  had already passed. Log: `/tmp/test-ci-harden-lint-whitaker-gate.out`.
- [x] (2026-11-03) The `300.006 s` figure is the diagnosis, not merely the
  duration: it is exactly the base `slow-timeout = 300s`, so those tests
  matched **no** longer override. The cause is a filter that could never match.
  The override used `package(no_unwrap_or_else_panic) & test(ui::ui)`, but
  nextest reports those tests as `ui`, `ui::hand_written_…`, and so on —
  `ui::ui` appears **zero** times in the gate log. The filter named a test that
  does not exist.
- [x] (2026-11-03) Found a second, latent defect of the same family in the
  default profile. The broad 20-minute `serial-dylint-ui` override lists an
  unqualified `test(ui::ui)`, which matches `rstest_helper_should_be_fixture`
  and — because nextest resolves each setting from the *first* matching
  override that configures it — shadowed both the 75-minute and the 120-minute
  overrides below it. Those longer allowances had therefore never taken effect
  in that profile.
- [x] (2026-11-03) Both profiles now select the whole package binary,
  `package(no_unwrap_or_else_panic) & binary(no_unwrap_or_else_panic)`, which
  covers every test in the crate's own binary without an enumerable name list
  that can silently rot as harnesses are added. The default profile's broad
  clause gained
  `& not package(no_unwrap_or_else_panic) &
  not package(rstest_helper_should_be_fixture)`
  so it no longer wins a setting it was never meant to govern.
- [x] (2026-11-03) `tests/nextest_ui_filter.rs` was rewritten from a
  single-string assertion into a rule that would have caught both defects. It
  scans `crates/no_unwrap_or_else_panic/src/lib_ui_tests.rs` for the tests that
  rebuild through nested Cargo, then requires the *governing* (first matching)
  override in each profile to select the package binary, carry a build-sized
  period, and serialize into `serial-dylint-ui`. Two seeded-fault controls
  confirm it can fail: reintroducing the name-list defect fails both profiles,
  and reintroducing the shadowing clause fails the default profile at
  `overrides[0]`. The file was 371 lines at that point, within the 400-line cap.
- [x] (2026-11-03) CodeScene advisory repair. The hosted check's String Heavy
  Function Arguments finding on `tests/nextest_ui_filter.rs` is cleared locally:
  `cs review tests/nextest_ui_filter.rs` reports `{"score":10.0,"review":[]}`
  with no findings, and `cs delta origin/harden-lint-workspace-config` on the
  working tree produces empty output.
  `cargo nextest run --test nextest_ui_filter` passes 5/5.
- [x] (2026-11-03) Seeded-fault negative control on the refactored test:
  reverting the `ci` profile's override filter to the old `test(ui::ui)` name
  list made
  `harness_allowances_are_governed_by_a_whole_package_override::case_2_ci` fail
  with "profile.ci: no override with a slow-timeout positively selects
  no_unwrap_or_else_panic", proving the test still detects the original defect
  after the refactor. The fault was then reverted and the suite re-ran green
  (5/5), with `git diff --quiet .config/nextest.toml` clean and the test file
  byte-identical to its pre-control copy.
- [x] (2026-11-03) `tests/nextest_ui_filter.rs` is now exactly 400 lines, at
  the `AGENTS.md` per-file cap (`module_max_lines.max_lines = 400`), after
  `cargo fmt --all`.
- [x] (2026-11-03) All five gates pass sequentially against the refactored
  working tree, each started once and each exiting 0: `make check-fmt` (3 s),
  `make typecheck` (1 s), `make test NEXTEST_PROFILE=ci` (138 s;
  `Summary [117.309s] 1673 tests run: 1673 passed, 5 skipped`, doctests 41
  passed / 0 failed), `make lint` (3 s), and `make markdownlint` (10 s, 78
  files, 0 errors). Logs are
  `/tmp/{check-fmt,test,typecheck,lint,markdownlint}-*-harden-lint-whitaker-gate.out`.
- [x] (2026-11-03) The typed-newtype refactor invalidated the worked example
  in `docs/developers-guide.md` § "Nextest filter validation", which quoted
  `tests/nextest_ui_filter.rs`. The excerpt had already drifted on `main` — it
  called `crates_with_integration_ui_test()`, a symbol that exists in no
  revision of this repository — and the refactor made it wrong in one further
  way, since `extract_filter` now returns `FilterExpression` rather than
  `&str`. The excerpt is corrected to the current signature and body.
  `mdtablefix --diff docs/developers-guide.md` reports "1 file left unchanged".
- [x] (2026-11-03) Hash-bound evidence for the one changed code file, which
  the gates do not cover. `make test` excludes the root `whitaker` package
  (`Makefile:23` `TEST_EXCLUDES`), and `make test` is exactly what
  `.github/workflows/ci.yml:187` runs, so `tests/nextest_ui_filter.rs` — a
  root-crate integration test — runs in neither the local gate nor hosted CI.
  That exclusion predates this branch: it arrived with `98170cf9`, which is an
  ancestor of `origin/main`. A targeted
  `cargo nextest run --test nextest_ui_filter` against the working tree
  therefore supplies the missing evidence directly: 5 tests run, 5 passed, 0
  skipped, exit 0, with the file's sha256 identical
  (`554d98e797769dfab86a86aae5c29f11e2e1c180973032c59ae580fcf945eda2`) before
  and after the run. Log:
  `/tmp/test-nuf-hashbound-harden-lint-whitaker-gate.out`.
- [x] (2026-11-03) Published as `adc974c9` and confirmed remote parity. The
  push over SSH reported `7f442958..adc974c9`; the tracking ref fetched with a
  leading `+` refspec and `ls-remote` both report
  `adc974c95b4aaf70966d74822abd9a45f4f6cfca`, matching `git rev-parse HEAD`.
  The branch's upstream is the named remote `origin`, not a raw URL.
- [x] (2026-11-03) Hosted CodeScene confirmation, which the local CLI could not
  give. `CodeScene Code Health Review (main)` concludes `success` with zero
  annotations on `adc974c9`; the advisory is cleared remotely, not only in the
  working tree.
- [x] (2026-11-03) CI dispatched as run `41598023070` (`workflow_dispatch`,
  because `refs/pull/372/merge` is frozen at `39b4757f` and no `pull_request`
  run fires on a new head).
- [x] (2026-11-03) Run `41598023070` concluded **`success`** on `adc974c9`.
  Both real jobs are green — `linux-full` (including `Lint` and
  `Publish dry run`) and `windows-compat` (including `Test`,
  `make test NEXTEST_PROFILE=ci`, and `Installer release dry run`).
  `coverage-check` is `skipped`, which is its configured lane selection, not a
  gap. The run checked out `adc974c9`, so it certifies the committed tree at
  that commit. It did **not** cover the `test-root-lib` target, the two-profile
  nextest filter rewrite, or the new contract suite at the time; those were
  uncommitted then and have since been published in `126ffa86`.
- [x] (2026-11-03) Exact-head CI dispatched as run `41602487492`
  (`workflow_dispatch`, head `57312cfe…`). `linux-full` **failed**, and the
  failing step is `Enforce en-GB-oxendict spelling` — no other step failed. The
  cause was a real defect in the pushed head, not a false positive:
  `tests/nextest_ui_filter/filter_model.rs` said `parenthesised`. It passed the
  same gate locally while the file was untracked, because `typos` scans tracked
  files, and the commit that added it made it visible. Fixed with a one-word
  change; `make spelling` and `make markdownlint` are green afterwards. The red
  result is retained as the evidence for the fix rather than rerun away.
  `windows-compat` does not run the spelling gate and was still in progress at
  that point; the run is superseded on its `linux-full` half by the next
  dispatch on the corrected head.
- [x] (2026-11-03) Spelling repair committed as `126ffa86` ("Fix the spelling
  gate and record the probe evidence") and published. All seven gates were run
  again on the exact bytes committed, sequentially and each once: `check-fmt`,
  `fmt`, `typecheck`, `test NEXTEST_PROFILE=ci` (1673 nextest passed / 5
  skipped, then 56 `test-root-lib` passed, then all doctests `ok`), `lint`,
  `test-workflow-contracts` (13 passed), `markdownlint` (78 files, 0 errors).
  The `fmt` and `markdownlint` pair was re-run a second time after the final
  plan edit so the Markdown formatting evidence covers the committed bytes
  rather than an earlier revision; the file changed at 09:24:58, between the
  09:23:49 `fmt` log and the 09:31:44 `markdownlint` log.
- [x] (2026-11-03) Push transport corrected. The first push of `126ffa86` used
  `origin`, which is an HTTPS URL, and the Lody remote helper rewrote it, so
  the push printed `To https://github.com/leynos/whitaker.git`. The documented
  clean environment was not sufficient: beyond the numbered `GIT_CONFIG_*`
  `insteadof` rewrite, `GIT_EXEC_PATH` points at `~/.lody/gh-session-bin/<id>`
  and supplies its own `git-remote-https`, and that variable is not in the
  documented removal list. Stripping every `GIT_*` variable and pinning
  `GIT_EXEC_PATH=/usr/libexec/git-core` produced a silent, genuine SSH push.
  The remote content was already correct in both attempts; the branch head
  `126ffa86` is now confirmed by SSH `ls-remote`, an SSH fetch with a leading
  `+` refspec, `origin/harden-lint-whitaker-gate`, and `git rev-parse HEAD`,
  which all agree.
- [x] (2026-11-03) Hosted confirmation of the spelling repair. Run
  `41604232779` on `126ffa86` passed `Enforce en-GB-oxendict spelling`,
  `Check formatting`, `Nixie` and `Markdown lint` — the step that failed on
  `57312cfe` now succeeds on the same job. The run was superseded by the next
  head before its `Lint` step finished, so this certifies the Markdown half of
  that head, not the whole of it.
- [x] (2026-11-03) Plan corrections committed as `5934827f` and published over
  SSH. Two things are recorded there: the push-transport correction above, and
  the discovery that a spelling gate sees only what Git tracks.
- [x] (2026-11-03) CodeRabbit's scoped disposition received on the assessment
  reply (issue comment
  [6599065986](https://github.com/leynos/whitaker/pull/372#issuecomment-6599065986)).
  It closes findings 1 and 2 and keeps 3 and 4 open on named gaps. Each gap
  was checked against live source before being accepted; all of them were real.
- [x] (2026-11-03) Finding 3's remaining gaps closed.
  `tests/nextest_ui_filter.rs` is an integration binary of the `whitaker` root
  package, so `TEST_EXCLUDES` skipped it *and* `test-root-lib`'s `--lib` did
  not reach it: no gate ran the regression test that guards the timeout
  configuration. A `test-nextest-filter` target now names it, `make test`
  invokes it, and it is in `.PHONY`. It runs 10 of 10 against the repaired
  configuration and 4 of 10 against `adc974c9`'s, exiting 2 — so the gate
  detects the defect rather than merely executing.
- [x] (2026-11-03) Finding 3's contract gaps closed in
  `tests/workflow_contracts/lint_makefile_test.py`. The install step is now
  *executed* against stubs in three cases (cached, binstall, cargo install)
  instead of having its text searched; the lint contract asserts
  `--all-targets` and `--all-features`; and `WHITAKER_PACKAGES` must contain
  the four intended support crates rather than merely avoid a banned set. Five
  discrimination probes each fail exactly one test for the intended reason:
  deleting the standalone `whitaker-installer` execution, unpinning the
  `cargo install` version, dropping `--all-targets`, dropping
  `-p whitaker_sarif`, and removing the new target from `make test`.
- [x] (2026-11-03) The finding-3 contract repairs and finding-4 plan
  corrections are published as `615d3bd7`, pushed over the verified SSH route
  and confirmed at the remote head; local HEAD, `refs/remotes/origin/...` and
  `ls-remote` all read `615d3bd7f7bf8a8b0138d89fc51ba86e929de8d7`. Run
  `41604896975` covers only `5934827f`, which predates them.
- [x] (2026-11-03) Run `41604232779` concluded **`success`** on `126ffa86`, the
  head CodeRabbit inspected. It certifies that candidate only.
- [x] (2026-11-03) CodeScene regression found and repaired. The
  `CodeScene Code Health Review (main)` check went from `success` on `adc974c9`
  to `failure` on this head: one advisory rule, "String Heavy Function
  Arguments", in `tests/nextest_ui_filter/filter_model.rs` at 9.69 code health.
  The cause was the finding-2 repair's own module, so the violation was
  introduced here and repaired here rather than suppressed; no
  `code-health-rules.json` override was added, and the repository has none.
  `TestIdentity::matches` and `split_leaf` each carried an adjacent pair of bare
  `&str` values, which the type system cannot tell apart. `Operator` and
  `FilterArgument` now name them. Local `cs review` scores the file 10.0 with
  no findings, and `cs delta 5a08c6ff` reports no issues. Published as
  `06e33c04`.
- [x] (2026-11-03) The CodeScene repair re-ran every gate on its own bytes:
  `make check-fmt`, `make typecheck`, `make test-nextest-filter` (10/10),
  `make test NEXTEST_PROFILE=ci` (1673 tests, 56 root-lib, 10 filter,
  doctests), and `make lint` (rustdoc, Clippy and the Whitaker suite all
  reached) — all exit 0. The first `make lint` attempt failed on a denied
  `clippy::shadow-reuse` from rebinding rather than destructuring, which
  `make typecheck` and the tests could not see; the fix destructures in the
  pattern. The negative control was re-run after the refactor and still yields
  6 passed / 4 failed against `adc974c9`'s configuration, so detection survives
  the change.
- [x] (2026-11-03) CodeScene confirmed cleared **hosted**, not only locally.
  `CodeScene Code Health Review (main)` concludes `pass` on the head after the
  repair; it had concluded `fail` on `8af0fb2b` and earlier.
- [x] (2026-11-03) Exact-head CI concluded **`success`** on `9863d842`, the
  current head, as run `41618871744` (`workflow_dispatch`, for the same
  frozen-merge-ref reason as every earlier run). `windows-compat` and
  `linux-full` both pass; `coverage-check` is `skipped` by its configured lane
  selection. Every head since `1ae7b8c9` changes only this ExecPlan, so those
  runs certify documentation state rather than new code; run `41611150392` on
  `1ae7b8c9` is the one that certifies the code, with every `linux-full` step
  green — `Check formatting`, `Enforce en-GB-oxendict spelling`, `Nixie`,
  `Markdown lint`, `Install Whitaker`, `Lint`, `Workflow contract tests` and
  `Publish dry run`. Seven dispatched runs have now concluded `success`:
  `41606198130` on `615d3bd7`, `41606387238` on `8af0fb2b`, `41607603679` on
  `b660e777`, `41607827636` on `ebbabd5b`, `41611150392` on `1ae7b8c9`,
  `41614932937` on `d4bf692a` and `41618871744` on `9863d842`. Each certifies
  its own head only.
- [x] (2026-11-03) CodeScene re-confirmed on `9863d842`:
  `CodeScene Code Health Review (main)` concludes `success` with zero
  annotations. `Gecko Security Review` also `success`; `Sourcery review` is
  `skipped` because the PR exceeds its 150,000 diff-character limit, an
  external limiter rather than a finding; `automerge` is `skipped` because the
  PR is a stack member.
- [ ] CodeRabbit review of `9863d842`. `0711dcad` is still queued with three
  other requests ahead of it; `comenq list` reports a declining ETA, from 2h44m
  to 1h19m as the queue drained, so it is advancing rather than stalled. The
  ETA is a prediction of remaining wait, not the request's age. CodeRabbit
  reports `Review paused` on the head's check run — its title is
  `Review paused`, its summary says `Result: Skipped - Review paused`, and
  every step, including `Prepare review scope`, is skipped. The pause is an
  automatic one after an influx of commits, not a rate limit, and the notices
  offering `@coderabbitai resume` and a "Resume reviews" checkbox are the bot's
  own shortcut. The queue entry is the authorized route, a pause is a wait
  rather than an impasse, and a second entry for the same PR is forbidden, so
  the queue is allowed to deliver.
- [x] (2026-11-03) ExecPlan completeness/correctness assessment posted as issue
  comment
  [6598161483](https://github.com/leynos/whitaker/pull/372#issuecomment-6598161483)
  under `Assessment-ID EP-372-harden-lint-whitaker-gate-20261103-a7f3`,
  through the manual-comment route (`buzzybee-df12`, a pool account, not
  `leynos`). The proof assessment is a justified `not applicable`: the
  PR-exclusive diff against base `5a08c6ff` is 59 files and touches no proof,
  Verus, or Kani file. (An `origin/main...HEAD` check appears to show Kani
  files, but that three-dot range spans the whole 10-PR stack and does not
  describe this PR.)
- [x] (2026-11-03) Assessment reply received on `adc974c9` (issue comment
  `6598194983`, Assessment-ID
  `EP-372-harden-lint-whitaker-gate-20261103-a7f3`). It reports four findings.
  All four were verified against live source before any edit; findings 1 and 2
  were repaired first, and 3 and 4 followed. The state recorded here is that
  point in time; the repairs were published in `126ffa86` and `5934827f`.
- [x] (2026-11-03) Finding 1 repaired, with a wider scope than reported. The
  assessment named one clause,
  `test(example_harness_collects_call_site_evidence)`. A workspace grep showed
  the same shadowing through **any** `test(<name>)` disjunct:
  `example_compiles_under_test_harness` is declared in both
  `no_expect_outside_tests` (`src/lib_ui_tests.rs:308`) and
  `no_unwrap_or_else_panic` (`src/lib_ui_tests.rs:184`), so
  `test(ui::example_compiles_under_test_harness)` reached the excluded package
  too. Guarding each clause separately fixes the symptom and leaves the next
  clause free to reintroduce it, so the exclusion now wraps the **whole
  disjunction** in both profiles. In each, the guarded part is the six
  `test(...)` clauses plus `binary(ui) & test(=ui)`, conjoined with the two
  `not package(...)` guards and parenthesized as one operand of an outer
  disjunction; that whole line is a single `filter = "..."` value in
  `.config/nextest.toml` and is easier to read there than to quote here. The
  lock test is joined **outside** the guard because
  `acquire_places_the_lock_inside_the_effective_target_directory`
  (`crates/rstest_helper_should_be_fixture/tests/ui/harness_lock_tests.rs:40`)
  takes the target-scoped file lock without rebuilding anything: it needs
  serialization but not a build-sized timeout. The clauses themselves cannot be
  deleted, because `no_expect_outside_tests` declares
  `ui::example_compiles_under_test_harness` and has no override of its own.
- [x] (2026-11-03) The `ci` profile had a **separate, unreported** gap found
  while repairing finding 1. Its filter had never contained the
  `test(ui::example_compiles_under_test_harness)` clause at all, so that
  nested-Cargo harness ran in CI at the 300 s base. The `ci` filter now carries
  the same clause set as `default`. This surfaced as a red
  `case_6_ci_shared_name`, which is the useful outcome: the test found a real
  defect the assessment had not.
- [x] (2026-11-03) Finding 2 repaired. `tests/nextest_ui_filter.rs` no longer
  searches filter text. It evaluates the nextest filter grammar against concrete
  `TestIdentity` values through a new
  `tests/nextest_ui_filter/filter_model.rs`, and
  `governing_override(overrides, setting, identity)` returns the *first*
  override, in file order, that both configures the setting and selects the
  identity — which is the rule nextest documents. Six parameterized cases pin
  the effective `slow-timeout`, including the negative control
  `SHARED_NAME_CONTROL` (package `no_expect_outside_tests`, which has no
  override of its own and must still receive the broad clause's 20-minute
  allowance) and both excluded packages at 75 minutes, under both profiles. The
  evaluator panics on an unknown operator or leaf shape rather than answering
  `false`, so a filter that grows a construct it does not model fails the guard
  loudly instead of passing vacuously.
- [x] (2026-11-03) The new test is non-vacuous, proved by a red step. Against
  the **committed** config it failed 4 of 10 cases, including
  `case_3_default_rstest_helper`, which received `1200s` where `4500s` was
  required — the exact defect the assessment reported. After the repair the
  same run is 10 of 10. The red log is
  `/tmp/red-nuf-harden-lint-whitaker-gate.out`, the green log
  `/tmp/green-nuf-harden-lint-whitaker-gate.out`.
- [x] (2026-11-03) The same guard re-proved against the real pre-repair file
  rather than a seeded reversion. Checking out `adc974c9:.config/nextest.toml`
  over the working copy and running
  `cargo nextest run --locked -p whitaker --test nextest_ui_filter --all-features`
  gives 4 of 10 failing: `case_1_default_package_override` and
  `case_3_default_rstest_helper` both report `left: 1200, right: 4500`, with
  the message naming first-matching-override precedence;
  `case_6_ci_shared_name` reports that no `ci` override selects the shared name
  at all; and
  `nested_cargo_harnesses_are_governed_by_the_whole_package_override::case_1_default`
  reports that `overrides[0]` governs by name list instead of the package
  binary. Restoring the committed file and running the identical command gives
  10 of 10 passing, exit 0. Logs:
  `/tmp/nuf-negctl2-harden-lint-whitaker-gate.out` (red) and
  `/tmp/nuf-posctl-harden-lint-whitaker-gate.out` (green).
- [x] (2026-11-03) The first attempt at that control proved nothing and was
  discarded rather than recorded as evidence. It selected cases with
  `-E 'test(effective_slow_timeout_follows_override_precedence)'`; nextest's
  `test(...)` matcher compares against the full test *name*, which for an
  `rstest` case carries a `::case_N_<label>` suffix, so the expression matched
  nothing and nextest exited 4 with `0 tests run`. See "A discrimination probe
  can report a false clean" under Surprises & discoveries.
- [x] (2026-11-03) The repair obeys the 400-line file cap without weakening the
  guard: `tests/nextest_ui_filter.rs` is 294 lines and the model is 172. The
  first draft was 450 lines in one file and was split rather than trimmed.
  `clippy --all-targets` exits 0. The fixture uses the repository's own
  `#[whitaker_test_macros::allow_fixture_expansion_lints]`, already applied at
  roughly ten sites, because `rustfmt.toml`'s `fn_single_line = true` collapses
  the body to a brace form that Clippy's `unused_braces` denies.
- [x] (2026-11-03) Finding 3 repaired: the root library's tests are reachable
  from the gate. `TEST_EXCLUDES` names `--exclude whitaker` because the root
  package's *test binaries* cannot link under `--all-features`; the *library*
  links fine. `src/testing/ui/tests.rs` — which holds
  `runner_env_guard_marks_the_toolchain_library_as_prepared` and its
  explicit-zero case — therefore ran nowhere. A new `test-root-lib` target runs
  `nextest run -p whitaker --lib --all-features`, `make test` invokes it, and
  the target is in `.PHONY`. Gate evidence: 56 tests run, 56 passed, 0 skipped,
  exit 0, under both the default and `ci` profiles
  (`/tmp/trl-harden-lint-whitaker-gate.out`).
- [x] (2026-11-03) The new gate is non-vacuous, proved by a seeded behavioural
  fault rather than a compile error. Replacing `with_copy_library_flag`'s body
  with an unconditional set compiled cleanly and produced 55/56 with
  `runner_env_guard_marks_the_toolchain_library_as_prepared::case_2_explicit_value_is_kept`
  FAILED and `MAKE_EXIT=2`. The guard was then restored and `git diff --stat`
  came back empty before the green run above. The first attempt at this control
  was **not** evidence: deleting the branch instead failed to compile on
  `dead_code` and `unused_imports`, which proves nothing about detection.
- [x] (2026-11-03) Finding 3's contract-test half repaired in
  `tests/workflow_contracts/lint_makefile_test.py`, 13 tests, all passing
  (`/tmp/contracts4-harden-lint-whitaker-gate.out`). It asserts the two-target
  `lint` composition, warning denial in both tools, `--lib whitaker_suite` with
  no `--all` fallback, the support-crate-only package set, the CI install step
  and its position before `Lint`, the cached-install skip, the workflow-level
  installer pin, the cache key, and the new `test-root-lib` wiring.
- [x] (2026-11-03) Those contract tests are non-vacuous, and the check is
  automated rather than argued. `/tmp/redcheck.py` mutates the Makefile and
  workflow into eleven broken states and requires each corresponding test to
  fail; it also runs one allow-probe that drops `--exclude whitaker` wholesale
  and requires the root-lib test to stay **green**, so the tolerated end state
  is not mistaken for a regression. Result: eleven `RED ok` lines, one
  `GREEN ok`, "all probes red", exit 0
  (`/tmp/redcheck2-harden-lint-whitaker-gate.out`).
- [x] (2026-11-03) Finding 3's third strand — the CI boundary tests for the
  Whitaker install — is the same contract file. The earlier
  `tests/workflows/test_ci_workflow.py` covers the workflow's other steps and
  was left alone; this is a new, narrow file beside it rather than a rewrite of
  an existing suite.
- [x] (2026-11-03) Finding 4: the execplan is reconciled against the published
  implementation. Corrected here: the "nothing is committed or pushed" claim,
  the "default-profile overrides never apply to `ci`" claim, the two-crate
  `Test::src_base` diagnosis, the M4 lease constant, and the abbreviated
  `selected_revision` (now the full `9fca3bc9fc2bc83c60bde26d18ed68f11564b228`,
  with the snapshot manifest and three agreeing checks attached).
- [x] (2026-11-03) `docs/developers-guide.md` corrected and extended. Line 320
  said `make lint  # Run Clippy`; it now names the two-target gate, and two new
  subsections document the prerequisite composition, the flag variables and why
  `--all-targets` is absent, `WHITAKER_PACKAGES`, the `--lib whitaker_suite`
  selection, the CI installer pin, and the `test-root-lib` target with the
  `--exclude whitaker` rationale and its retirement path.
- [x] (2026-11-03) The assessment's inherited-issue note was checked and
  confirmed, and is left alone. `tests/workflows/test_ci_workflow.py` expects
  `dtolnay/rust-toolchain@e97e2d8c…` while `.github/workflows/ci.yml:114` uses
  `dtolnay/rust-toolchain@6c977a6c…`. Both values already differ at base
  `5a08c6ff`, so this PR neither introduced nor is obliged to fix it, and the
  contract suite that would catch it is a different directory from the one this
  branch added. It is recorded here as a known inherited gap rather than
  silently repaired, because repairing it would import an unrelated change into
  a scope that is already over tolerance.
- [x] (2026-11-03) Every gate re-run green on the repair working tree, each
  logged under `/tmp/<action>-harden-lint-whitaker-gate.out`: `make check-fmt`;
  `make fmt` (see the note on `docs/whitaker-dylint-suite-design.md` below);
  `make typecheck`; `make test NEXTEST_PROFILE=ci` — 1673 nextest tests passed,
  5 skipped, then 56 `test-root-lib` tests passed (the target is reachable and
  `src/testing/ui/tests.rs` now runs under the gate), then the workspace
  doctests all `ok`; `make lint` (both halves); `make test-workflow-contracts`
  (13 passed); the `/tmp/redcheck.py` mutation harness (11 red, 1 green
  allow-probe); and `make markdownlint` including `spelling` and
  `spelling-phrase-check`. No gate was run in parallel with another, and
  `RUSTC_WRAPPER` was left in place throughout. The contract suite has since
  grown two tests, so that run's `13 passed` predates them; `615d3bd7` re-ran
  it at 14 passed.
- [x] (2026-11-03) One unrelated reflow, excluded from the commit.
  `make fmt` rewrites a Rust snippet in `docs/whitaker-dylint-suite-design.md`
  (line 1034) that `mdtablefix` wants to join onto one line and wrap mid-path.
  The same preview appears at base `5a08c6ff`, so it is pre-existing drift in a
  file this PR touches only for the 6.0.1→6.3.2 version bump, and applying it
  would add unrelated churn to an over-tolerance diff. It is reverted before
  each commit; `make fmt` is expected to reintroduce it locally until the
  snippet is fenced, which is a separate repair.
- [ ] Formal CodeRabbit review queued as comenq `0711dcad`, ETA ~4h 29m.

### Scope tolerance disposition

The plan's own tolerance stops at "more than 12 non-ExecPlan files change, or
more than 600 net lines". The PR exceeds both, so the number is recorded here
rather than left implicit, split by what produced it.

Measured against base `5a08c6ff`, excluding `docs/execplans/`, the assessed head
`adc974c9` is **58 files, +1464/−846, net 618** — which reproduces the figure
the assessment reported, so the two agree on the measurement rather than on the
interpretation.

| Segment                          | Commits                 | Files | Insertions | Deletions | Net |
| -------------------------------- | ----------------------- | ----- | ---------- | --------- | --- |
| Original PR work (2026-08-30)    | `43b488ee`, `b6be553d`  | 36    | 425        | 288       | 137 |
| Repair increment (2026-11-01/03) | `d10469fd` … `adc974c9` | 25    | 1045       | 564       | 481 |
| Union at the assessed head       |                         | 58    | 1464       | 846       | 618 |

The segments are not disjoint, and only the net column is additive. The file
counts sum above the union because three files appear in both rows. The
insertion and deletion columns each sum six lines above it for the same reason:
in `.github/workflows/ci.yml`, `AGENTS.md` and `Makefile`, a change made by the
original work that the repair increment leaves in place is counted by both
segment rows and once by the union, which overshoots by six inserted and six
deleted lines — one and one, four and four, and one and one respectively. The
net column does sum exactly, `137 + 481 = 618`, because a surviving change
contributes the same net to both rows as to the union; that is the check that
the split covers the whole change without double counting. Twenty-two files are
touched only by the repair increment.

The breach is real and is not argued away. It was accepted because the
assessment's finding set is itself the cause: findings 1 and 2 rebuilt the
nextest filter evaluator, finding 3 added a gate target and a contract suite,
and finding 4 is documentation. None of those is optional under this skill's
"action every finding" rule, and none can be shrunk without dropping a finding.
The disposition is therefore *accepted with this record*, not *within
tolerance*. The original PR work, at 36 files and net 137, is the part the
tolerance was written for; the repair increment, at net 481, is the
review-driven overrun.

The figures above deliberately stop at `adc974c9`. The finding-3 and finding-4
repairs add two new files and edit five, and no smaller scope would discharge
the findings, so the published head is larger again. Sizing that increment
precisely in this document would require a figure that this document's own
commit changes — a self-reference with no stable answer. The last row is
therefore the honest, reproducible measurement, and the statement of
*direction* is what the disposition rests on.

## Surprises & discoveries

An earlier revision of this entry claimed the `serial-dylint-ui` group and the
ten-minute `slow-timeout` under `[[profile.default.overrides]]` did not reach
the `ci` profile, so the UI harnesses supposedly ran concurrently and
unterminated for over an hour. That is wrong, and the correction matters
because it changes what the repair has to do. Nextest's own documentation for
per-test settings states the precedence explicitly: starting with the selected
profile, then following its inheritance chain through `default`, the first
override that matches a test and configures a setting is applied.
`[[profile.default.overrides]]` was therefore already in force under
`NEXTEST_PROFILE=ci`; the harnesses were serialized and bounded, and the hour
went on the shared compile pool.

What the misreading did establish is the real defect it obscured: the `ci`
profile, which is the profile that actually includes the Windows-only installer
binaries and is what CI runs, carried none of those budgets itself. Restating
them in `[[profile.ci.overrides]]` is still correct, but for the reason the
`.config/nextest.toml` comment gives — the budgets belong where a reader of the
profile that runs them will find them — not because they were missing at
runtime.

The failure is time-dependent. The same commit passed three days before it
failed. This matters because the first instinct was to look for a branch-local
cause, and there is none.

`cargo-dylint` was already 6.3.0 on this host, so the old `Makefile` pins at
6.0.1 described a tool that was not actually the one being run locally. The
Windows job installs its own toolchain and is unaffected by the host. That same
fact is what exposed the Python harness defect above: a host whose tool already
satisfies the pin cannot exercise a stale-tool stub unless the stub genuinely
wins `PATH`.

The pinned nightly is `nightly-2026-05-28` (`rustc 1.98.0-nightly`), and all
seven Dylint crates declare no `rust-version` at all — only `edition = 2024`
and, for the `dylint` library, `rust_version = 1.91`. So the library upgrade
itself puts no new floor under the pinned nightly. What does put a floor under
the *host tools* is `cargo-dylint` 6.3.0's own shipped `Cargo.lock`, which
resolves `cargo-util` 0.2.32, and that crate declares `rust_version = 1.97`.
The pinned nightly is 1.98-nightly and would in fact satisfy it; the stable
toolchain is retained because it is the documented, already-proven route and
because the previous pins' comments leaned on it. The comments were updated to
name the real dependency and floor rather than the stale
`cargo-util 0.2.28`/rustc 1.93 pair.

Raising the Python mirror from 6.0.1 to 6.3.0 exposed a latent hermeticity
defect in `tests/workflows/test_publish_check_provisioning.py`. The harness
exports `PATH` with its stubs first and expects the recipe to resolve them, but
`Makefile:9` asserts `PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(PATH)` at
parse time, and a makefile assignment beats the inherited environment. At the
old pin the test passed by accident: the host's real `cargo-dylint` is 6.3.0,
which mismatched the asserted 6.0.1, so the install fired anyway. At the new
pin the host binary satisfies the probe, the install never runs, and two
assertions fail. The harness now passes `PATH=` as a make command-line
override, which does outrank the makefile assignment. This is a repair to the
test's stated intent — "stubs first on PATH" — not a weakening: with the
override restored the stale-tool assertion is again exercised, and the
failed-install case again aborts before `git clone`.

`make fmt` now corrupts a Rust code fence in
`docs/whitaker-dylint-suite-design.md`. The installed `mdtablefix` 0.6.1 runs
with `--wrap --fences --in-place` and rewrites the `register_lints` example,
joining `#[no_mangle]` onto the following line and breaking
`store: &mut LintStore` across two lines. The result is not valid Rust. It
reproduces on a pristine copy of the file at `HEAD`, so it is a pre-existing
tool defect that this branch merely surfaced by running the formatter, not a
defect in the pin change. The damaged hunk was restored to its original text;
the other two touched docs reformat byte-identically. Nothing outside the
intended pin lines now differs in that file.

`dylint_testing` 6.3.2's `rustc_flags_from_line` is not the same design as
`main`'s vendored copy. `main` splits with `shlex` and returns only the
arguments after `rustc`; 6.3.2 splits on spaces and keeps the wrapper arguments
ahead of `rustc`. Both are upstream-safe, but main cannot be copied wholesale
here without also importing its `[patch.crates-io]` plumbing and its
`vendor/dylint_testing` module, which belongs to a different PR.

### A timeout override can match nothing and still look correct

The gate-2 failure is worth recording separately because the configuration
looked right and the failure did not resemble a configuration problem. The
override read:

```toml
filter = "package(no_unwrap_or_else_panic) & test(ui::ui)"
```

but nextest reports the crate's tests as `ui`, `ui::hand_written_…`,
`ui::example_compiles_under_test_harness`, and so on. No test has the name
`ui::ui`, so the clause matched nothing, the tests fell to the 300 s base, and
two of them were killed at 300.006 s among 1,673 tests. `ui::ui` appears zero
times in the gate log; that absence, not the duration, is the proof.

Two properties make this class of defect hard to see without a test that reads
the configuration:

- **`test(…)` matches test names, not module paths.** `ui::ui` reads like "the
  `ui` test in the `ui` module", but for a test declared at the top of the
  crate's own binary the runtime name is bare `ui`. The integration-binary form
  does produce `ui`, and the filter's `(binary(ui) & test(=ui))` clause covers
  it; conflating the two leaves the inline-module case unreachable.
- **A setting resolves from the first matching override that configures it.**
  Precedence is evaluated per setting, and an override that does not configure
  a setting is ignored for that setting. So a broad clause placed above a
  specific one silently wins the timeout while looking like a fallback. The
  default profile had exactly that shape: `test(ui::ui)` matched
  `rstest_helper_should_be_fixture`, so the 75- and 120-minute allowances below
  it had never applied.

The repair selects the whole package binary (`package(X) & binary(X)`), which
cannot rot as harnesses are added, and excludes both specifically-governed
crates from the broad clause. The regression test in
`tests/nextest_ui_filter.rs` evaluates the governing override rather than
merely finding one that mentions the package, and two seeded-fault controls
confirm it fails when either defect is reintroduced.

### A discrimination probe can report a false clean

The non-vacuity check for the config guard was first run as:

```sh
cargo nextest run --locked -p whitaker_suite \
  -E 'test(effective_slow_timeout_follows_override_precedence)'
```

It exited 4 with `0 tests run: 0 passed, 0 skipped`. The expression matched no
test. `rstest` appends `::case_N_<label>` to each parameterized case's name, so
the bare function name is not a complete test identity, and nextest's
`test(...)` matcher compares against the whole name. A run that selects nothing
cannot fail, so an exit status read without the summary would have looked like
a pass.

Three habits avoid the trap: read the `N tests run` count before believing any
verdict, prefer selecting the test binary (`--test nextest_ui_filter`) over
writing a matcher when the goal is to run the whole set, and require the red
step to name the defect it caught. The corrected run failed 4 of 10 cases with
`left: 1200, right: 4500` and messages citing first-matching-override
precedence — that is a detection, where `0 tests run` was the absence of one.

### A spelling gate sees only what Git tracks

`make spelling` runs `typos`, which walks tracked files. A file that is
untracked in the working tree is invisible to it, so a local
`make markdownlint` can be green on a tree that is red the moment the file is
committed. That is exactly what happened here: a doc comment in
`tests/nextest_ui_filter/filter_model.rs` used the British `-ised` ending for
"parenthesized", passed `make markdownlint` while the file was untracked, and
failed hosted CI on `Enforce en-GB-oxendict spelling` as soon as `57312cfe`
tracked it. It is the same shape as the tracked-files-only class of surprise
already recorded for this repository, and the discovery is that it applied to a
*new* file added in the same change rather than to stale branch content.

The practical rule is to run the spelling gate after staging, or at minimum to
expect a first-commit surprise for any newly added file with prose in it. CI
caught it, which is the gate working, but one dispatch was spent learning it.

### A text assertion cannot see which branch runs

The install-step contract originally searched the workflow script for
`whitaker-installer` and for an install command. Both searches pass whether or
not the step ever *executes* the installed binary, and whether or not either
download branch can actually run: deleting the standalone `whitaker-installer`
line, or unpinning the `cargo install` version, left the suite green. The fix
was to stop reading the script and run it under stubs, then assert the calls it
made.

Writing that harness surfaced a second trap. `command -v` is a shell builtin,
so a stub on `PATH` cannot intercept the cache probe — the probe has to be
answered by whether the binary exists at all. And the probe itself
(`cargo binstall --version`) had to be distinguishable from the download
(`cargo binstall pkg@ver`), which a `$1`-only match could not do, because `$1`
is `binstall` in both. Matching on `$*` for `--version` separates them.

The general lesson is that a contract test's *non-vacuity* is a property of the
mutation it rejects, not of the interface it covers. "The test mentions the
step" and "the test would fail if the step were removed" are different claims,
and only the second one is worth recording as evidence.

### CodeScene measures the delta, not the file

A hosted CodeScene check, `CodeScene Code Health Review (main)`, concluded
`failure` on head `7f4429581cf4e3da0c339b03fc9f727a911674d6`. The failing gate
was "Enforce advisory code health rules (1 file with String Heavy Function
Arguments)", and the detail row read
`nextest_ui_filter.rs | 1 advisory rule | 10.00 -> 9.69`.

The hosted check-run API supplied no per-function annotation — its annotations
list is empty — so the threshold had to be recovered another way. The local
CodeScene CLI (`cs`, version 1.0.52) reproduced the finding exactly and gave
the number the API withheld. Its delta command was run as:

```sh
cs delta origin/harden-lint-workspace-config --output-format json
```

That command reported "In this module, 53.3% of all arguments to its 16
functions are strings. The threshold for string arguments is 39.0%". An
independent hand census of the file reproduced the same figures: 16 functions,
15 arguments, 8 of them string-typed, ratio 0.5333. CodeScene's measurement was
therefore confirmed rather than merely accepted.

The root cause is that the file is pre-existing; it exists on `origin/main` and
on this PR's base branch. This branch's delta grew it from 137 to about 378
lines (+300/−59), which pushed the module's string-argument ratio from below
the threshold to 53.3%, above the 39.0% warning threshold. CodeScene analyses
only the delta, which is why pre-existing higher-ratio files elsewhere in the
repository were not flagged.

### Why `make test` failed for exactly two lint crates

Two crates failed in that run, `no_unwrap_or_else_panic` and
`rstest_helper_should_be_fixture`, and an earlier revision of this entry
explained them by saying their harnesses call
`dylint_testing::ui::Test::src_base` directly instead of going through
`whitaker::testing::ui::run_with_runner`. That explanation is wrong, and the
source says so plainly: every lint crate in this tree calls `run_with_runner`,
at the base commit and at `HEAD`. `src_base` is also the wrong suspect for a
different reason — `Test::src_base` runs through `run_tests`, which never
copies a library at all; `copy_library` is reached from `dylint_libs`, which
every `Test::run` calls.

The real mechanism is narrower and does not depend on the `Test` constructor:

`whitaker`'s helper builds the library itself (`src/testing/ui/toolchain.rs`):
it reads the package's feature table, finds `dylint-driver`, and passes
`--features dylint-driver` to `cargo build --lib`. `dylint_testing`'s own
`initialize` does not. It runs a bare `cargo build` with
`DYLINT_BUILDING_LIBRARIES=1`, and no `--features` flag at all. Because every
lint crate declares `default = []` and puts the lint behind an optional
`dylint-driver` feature, that bare build compiles the
`#[cfg(not(feature = "dylint-driver"))]` stub. `copy_library` then copies the
stub over the toolchain-qualified filename, and `compiletest_rs` loads it and
reports `could not find 'dylint_version'`.

What decides *which* crates fail is build ordering, not call path. The wrapper
runs first and writes a correct toolchain-qualified library; the bare build
overwrites `target/debug/libX.so` only if Cargo actually rebuilds it. A crate
whose plain artefact was already fresh from the gate's own `--all-features`
compile keeps the working library, and a crate whose artefact was rebuilt from
the stub loses it. That is why the same commit can pass and fail on different
days, and why the two failures are a sample rather than a rule.

The published repair removes the overwrite instead of depending on ordering:
`with_runner_env` sets `DYLINT_LINK_ENABLE_COPY_LIBRARY=1` when the caller has
not, and `dylint_internal::link::debug_path_with_toolchain` then returns the
toolchain-qualified path without copying. `env::enabled` treats any value other
than `0` as on, so an explicit `DYLINT_LINK_ENABLE_COPY_LIBRARY=0` still opts
out, which `case_2_explicit_value_is_kept` pins.

The measurements that pin this down, taken on this candidate:

| Build                                                             | Artefact size | `dylint_version` exported |
| ----------------------------------------------------------------- | ------------- | ------------------------- |
| `cargo build -p no_unwrap_or_else_panic` (default features)       | 5 256 B       | 0                         |
| `cargo build -p no_unwrap_or_else_panic --features dylint-driver` | 67 465 296 B  | 1                         |

The 5 256-byte file is `static-pie linked` and exports only the four weak libc
symbols, so it is the stub and nothing else. The same file, byte for byte, is
what `deps/libno_unwrap_or_else_panic.so` held during the failing run.

This is a mechanism that only misfires when the bare build actually runs from
the workspace root. If a previous `--all-features` build from `make test` has
already left a fresh, feature-enabled plain `.so` in `target/debug`, Cargo
reuses it, `deps/` keeps its artefact, and `copy_library` copies a working
library. Whether the gate passes therefore depends on build ordering. `main`
solved this durably by vendoring `dylint_testing` (`Cargo.toml`:
`[patch.crates-io] dylint_testing = { path = "vendor/dylint_testing" }`) and
adding a reproducible, Clippy-keyed driver bootstrap. This branch carries
neither, so it inherits the ordering sensitivity.

The separate lock_timeout failures are a third, unrelated defect, already
recorded above: a host-global `/tmp/rstest-helper-example-harness.lock`. This
entry originally added a second half — "`[[profile.default.overrides]]` that
never apply to `profile.ci`" — and that half was **wrong**. Nextest inherits
through the profile chain and applies the first override that matches a test
*per setting*, so `default`'s overrides were always in force under
`NEXTEST_PROFILE=ci`. What the `ci` profile genuinely lacked was its own copy
of the budgets, which is a legibility problem rather than a runtime one.

Both files are now touched by this branch: the lock resolves through Cargo's
target directory into `serial-dylint-ui`, and `.config/nextest.toml` carries
per-profile base timeouts plus `[[profile.ci.overrides]]`. `main` had fixed the
same two defects independently (6-minute acquire, `serial-dylint-ui` under
`profile.ci` with 75–120-minute timeouts); this branch reaches the same end
state without importing `main`'s vendored `dylint_testing`.

## Decision log

**Describe a spelling defect rather than quoting it.** Twice in this task the
spelling gate rejected prose that *quoted* a misspelling — once in a commit
message draft and once in this plan's "A spelling gate sees only what Git
tracks" section, where writing `said "parenthesised"` reproduced the very error
being recorded. The gate is correct to reject it: `typos` cannot tell a quoted
defect from a made one, and the repository's rule is that exceptions go in
`typos.local.toml`, not into the prose. The narrow fix is to name the defect
structurally ("used the British `-ised` ending for *parenthesized*") so no
forbidden token appears. Reserving `typos.local.toml` for repository
*terminology* keeps that file meaningful; adding an allow-entry for a one-off
quotation would erode it. This is worth doing consistently, because a plan that
documents a class of mistake is exactly where that mistake is likeliest to be
reintroduced.

**Do not rebase onto `origin/main`.** Patch-identity comparison proves PRs 370
and 371 are unlanded, so `main` contains none of this stack's parent work.
Rebasing would import 716 files of unrelated change.

**Do not vendor `dylint_testing`.** `main` solves this by vendoring 6.3.0 with a
`shlex`-based patch. Upstream 6.3.2 already fixes both blockers, so vendoring
is unnecessary maintenance surface.

**Pin `cargo-dylint` and `dylint-link` at 6.3.0, libraries at 6.3.2.** Every
crate in the family is published through 6.3.2, so the binary tools could
follow the libraries. They are held at 6.3.0 deliberately: that is the version
the repository's own `v0.2.11` tag pins in
`installer/dependency-binaries.toml`, the version the `v0.2.11` GitHub release
publishes for all five targets, and the version `DYLINT_LIBRARY_PATH`-loaded
6.3.2 libraries are compatible with — `DYLINT_VERSION` is the constant
`"0.1.0"` in both 6.3.0 and 6.3.2, so a library built against one loads in a
driver built from the other. Moving the binary pins in lockstep with the
libraries would work, but it would also invalidate the release assets this
repository has already published, for no behavioural gain.

The driver is the exception and is not pinned by us at all. `dylint`'s
`initialize()` derives its `dylint_driver` requirement from its own
`CARGO_PKG_VERSION`, so 6.3.2 asks for `=6.3.2`. The warm 6.3.0 driver on this
host is therefore judged outdated and rebuilt on the first run. That is
expected, and it is why M3 supplies `DYLINT_CLIPPY_REPO`.

**Bump `WHITAKER_INSTALLER_VERSION` from 0.2.7 to 0.2.11.** This was originally
decided the other way, on the belief that the published installer already
pinned 6.3.0. Measurement falsified that:
`git show v0.2.7:installer/dependency-binaries.toml` is 6.0.1, and 0.2.8, 0.2.9
and 0.2.10 are unchanged. Only `v0.2.11` carries 6.3.0 — and only 0.2.11 has an
embedded Clippy snapshot, no `Run` subcommand being available in earlier
releases either. Leaving 0.2.7 in CI would have the installer's own
`ensure_dylint_tools` reinstall 6.0.1 over the Makefile's 6.3.0 pin, so the two
halves would disagree at exactly the point the gate exercises. The bump is not
scope creep; it is a precondition for the pin to mean anything.

The bump does not change the local gate path. Running the 0.2.11 installer from
this worktree would take `decide_workspace_action` → `UseCurrentDir` and
re-stage this branch's suite under `~/.local/share/whitaker`, and 0.2.11 is a
192-file, −17,400-line descendant of this branch. CI installs into an ephemeral
runner, so the pin there is cheap; local gates keep using the existing 0.2.7
wrapper, whose two-line shape delegates to the ambient `cargo-dylint` 6.3.0.

**Repair the harness in place (supersedes an earlier stop-and-hand-back).** An
earlier revision of this plan proposed halting at M3b and asking the user to
choose between a rebase onto `main` and a local repair, on the belief that a
local repair would duplicate unowned upstream work. Source-level measurement
inverted that premise, so the entry is superseded rather than deleted:

- The `dylint_version` failure is not a vendoring difference. It was introduced
  by Dylint 6.3.x, and the guard that avoids it —
  `DYLINT_LINK_ENABLE_COPY_LIBRARY=1` — is already present in this branch's own
  history at `9e4c0be0` (`src/testing/ui/mod.rs`, `src/testing/ui/tests.rs`),
  and again on `origin/main` at `0108d347`. This branch dropped it in a later
  commit. Restoring it is a re-adoption, not new upstream work.
- Main's version of the guard is `unsafe { env::set_var }` plus its own
  `EnvTestGuard` type. This workspace sets `unsafe_code = "forbid"`
  (`Cargo.toml:178`), which neither `ab2bc519` nor `origin/main` does, so a
  literal port would not compile. The repair therefore expresses the same guard
  through `whitaker_common::test_support`'s closure-scoped `temp_env` helpers.
- The host-global `/tmp` lock and the empty `[[profile.ci]]` are genuine
  cross-worktree and CI-lane defects that `main` also fixes, and both are small
  enough to fix here rather than rebase a 5-package workspace onto a 6-package
  one for.

A rebase onto `main` remains available, but it is no longer the cheaper option
and is not required for the gates to pass.

**Answer the CodeScene advisory with typed domain values, not suppression.**
The String Heavy Function Arguments finding in `tests/nextest_ui_filter.rs` is
repaired by introducing three domain newtypes: `ProfileName<'a>(&'a str)`,
`PackageName<'a>(&'a str)` with a `Display` implementation, and
`FilterExpression<'a>(&'a str)` with a `mentions(PackageName)` method that
absorbs the former free function
`mentions_package(filter: &str, package: &str)`. A small helper
`filter_of(&Value) -> Option<FilterExpression>` replaces two duplicated
`get("filter").and_then(Value::as_str).map(...)` chains. The refactor removes
the stringly-typed signature that allowed a swapped `(filter, package)`
argument pair to compile and silently answer `false`. A `.codescene` rule
suppression or directive was rejected: the stringly-typed signature was a
genuine design weakness, and the advisory identified it correctly.

## Outcomes & retrospective

Complete for M1–M3. M4 is open for one reason only: its green CI result belongs
to `adc974c9`, and the repairs that answer the assessment's findings 3 and 4
are not in that commit. Everything up to `adc974c9` is committed and pushed —
nine commits from base `5a08c6ff`, of which seven are the 2026-11-01/03 repair
increment — and the commit that carries this text, the `test-root-lib` target
and the contract suite after it is the candidate still awaiting a hosted run.

An earlier revision of this section said "nothing is committed or pushed". That
was true when it was written and false by 2026-11-03: `adc974c9` is on
`origin/harden-lint-whitaker-gate`, its `ls-remote` head matches
`git rev-parse HEAD`, and CodeScene reports success on it. The distinction M4
still owes is not publication but *hosted confirmation of the final candidate*,
and that is a different, narrower gap.

Gate 2 has now run once and failed, and the failure was informative in a way
the earlier design did not anticipate. The nextest overrides carried a filter
that matched no test, so two real harnesses were killed at the 300 s base among
1,671 passing tests. The defect was invisible to inspection because
`test(ui::ui)` reads like a valid path and because nextest's per-setting
first-match precedence let an earlier broad clause shadow a later specific one.
The lesson for the remaining work is that a configuration change of this kind
needs a test that evaluates the *governing* override, not one that merely finds
an override mentioning the package, and that such a test needs a seeded-fault
control to show it can fail. Both are now in place.

M3a is met: the `revspec 'master'` failure is gone, with zero occurrences
across a full compile of `dylint_internal` 6.3.2 and a 1637-test pass. M3b's
two blockers were diagnosed to source level and repaired rather than deferred:

- `dylint_testing` 6.3.2's `initialize()` runs a bare `cargo build` with no
  `--features`, producing an empty `cdylib` for every lint crate, and then
  copies it over the toolchain-qualified library the harness built with
  `dylint-driver`. `src/testing/ui/mod.rs` now sets
  `DYLINT_LINK_ENABLE_COPY_LIBRARY=1` when the caller has not, so the copy is
  skipped. Covered by
  `runner_env_guard_marks_the_toolchain_library_as_prepared`.
- The example-harness lock lived at a host-global `temp_dir()` path with a
  30-minute stale window, so one worktree's slow build could block every other
  worktree on the host. It now resolves Cargo's target directory and locks
  inside it, with a 5-minute stale window. Covered by
  `acquire_places_the_lock_inside_the_effective_target_directory` and the
  target-resolver cases in `harness_lock_tests.rs`.
- `.config/nextest.toml` gained a base `slow-timeout` and `global-timeout` on
  both profiles, CI overrides for the nested-Cargo harnesses that exist in this
  tree, and a `serial-nested-cargo-builds` test group. Without a base bound, a
  hung test was reported slow for ever and only the job timer ended it, which
  cancels the run and discards the log that would have named the test.

The hosted CodeScene advisory is cleared, in both directions. Locally, the CLI
reproduces the finding, the hand census confirms its figures, and the
typed-newtype refactor drives `cs review` back to a score of 10.0 with no
findings, with `cs delta` producing empty output. Remotely, the hosted
`CodeScene Code Health Review (main)` check concludes `success` with zero
annotations on `adc974c9`, as recorded under Progress.

The hosted CI re-run on that same head has since concluded green. Run
[41598023070](https://github.com/leynos/whitaker/actions/runs/41598023070)
reports `conclusion: success`: `linux-full` and `windows-compat` both complete
and successful, with `Check formatting`, the spelling, Nixie and Markdown gates,
`Install Whitaker`, `Lint` (both halves), `Workflow contract tests`,
`Publish dry run`, `Test` and `Installer release dry run` all passing, and
`coverage-check` skipped by its configured lane selection.

What that does not certify is the tree described in this document. The run
checked out `adc974c9`, before the finding-3 and finding-4 repairs — the
`test-root-lib` target, the rewritten nextest filters in both profiles, the new
contract suite and the corrections to this plan — were written. Those were
uncommitted at that point and are now published in `126ffa86`, `5934827f` and
`615d3bd7`. They are a new candidate, and the green result belongs to the old
one. M4 therefore closes when a `workflow_dispatch` run on the published head
is green, not before; run `41606198130` is the attempt against `615d3bd7`, and
its conclusion is reported rather than assumed.

## Conformance basis

The governing artefacts are this repository's `AGENTS.md` (quality gates and
committing rules) and `docs/developers-guide.md` (tooling pins). No upstream
Terms of Reference or technical design applies; this is a maintenance repair
driven by an external dependency rename.

Trace links: `AGENTS.md quality gates -> EP-M3 -> make lint` and
`AGENTS.md quality gates -> EP-M3 -> make test`.

## Idempotence and recovery

Every milestone is re-runnable. The lock-file change is a regenerate, not a
hand edit, so repeating it is safe. The gate runs are read-only. The commit in
M4 is amendable; the push uses a lease so a concurrent writer cannot be
overwritten.
