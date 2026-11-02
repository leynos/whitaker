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

    Cloning into '/tmp/.tmpvF8ofW'...
    Error: `revparse_ext` failed for `master`
    revspec 'master' not found; class=Reference (4); code=NotFound (-3)
      9: dylint_internal::clippy_utils::repository::clippy_repository
     10: <dylint_internal::clippy_utils::revs_no_preinstall::Revs>::new
     11: build_script_main::extra_symbols::rev_from_channel

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
and which contains the `selected_revision` `9fca3bc9…` on its first-parent
history. `clippy_repository` clones from that object database into a tempdir
and leaves the source untouched.

**M4 — Commit, push, and re-observe.** Commit as one atomic change, push over
SSH with a lease bound to `d10469fd`, refresh the tracking ref, and dispatch CI
on the new head. End state: the PR's checks are green for the new head.
Recovery: the old head remains reachable through the PR's reflog.

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
- [ ] (in progress) M3: run the four Rust gates sequentially under
  `DYLINT_CLIPPY_REPO`. `make check-fmt` is green. `make test` compiled 341
  crates including `dylint_internal` 6.3.2, `dylint` 6.3.2,
  `dylint_linting` 6.3.2 and `dylint_testing` 6.3.2, and logged **zero**
  `revspec 'master'` occurrences; 1636 of 1665 tests passed with zero
  failures. The remaining 29 are the UI harnesses that build the Dylint
  driver against the pinned Clippy snapshot, which is the decisive evidence
  for this change.
- [ ] (pending) M4: commit, push, re-observe CI.

## Surprises & discoveries

The `serial-dylint-ui` test group and the ten-minute `slow-timeout` live
under `[[profile.default.overrides]]` in `.config/nextest.toml`, so they do
not apply to the `ci` profile that `make test` uses through
`NEXTEST_PROFILE=ci`. The UI harnesses therefore run concurrently and are
not terminated, each blocked on the shared compile pool rather than on its
own work. This is pre-existing configuration, not something this change
touches, and it explains why the gate took over an hour without failing.

The failure is time-dependent. The same commit passed three days before it
failed. This matters because the first instinct was to look for a branch-local
cause, and there is none.

`cargo-dylint` was already 6.3.0 on this host, so the old `Makefile` pins at
6.0.1 described a tool that was not actually the one being run locally. The
Windows job installs its own toolchain and is unaffected by the host. That
same fact is what exposed the Python harness defect above: a host whose tool
already satisfies the pin cannot exercise a stale-tool stub unless the stub
genuinely wins `PATH`.

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
joining `#[no_mangle]` onto the following line and breaking `store: &mut
LintStore` across two lines. The result is not valid Rust. It reproduces on a
pristine copy of the file at `HEAD`, so it is a pre-existing tool defect that
this branch merely surfaced by running the formatter, not a defect in the pin
change. The damaged hunk was restored to its original text; the other two
touched docs reformat byte-identically. Nothing outside the intended pin lines
now differs in that file.

`dylint_testing` 6.3.2's `rustc_flags_from_line` is not the same design as
`main`'s vendored copy. `main` splits with `shlex` and returns only the
arguments after `rustc`; 6.3.2 splits on spaces and keeps the wrapper arguments
ahead of `rustc`. Both are upstream-safe, but main cannot be copied wholesale
here without also importing its `[patch.crates-io]` plumbing and its
`vendor/dylint_testing` module, which belongs to a different PR.

## Decision log

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

## Outcomes & retrospective

Not yet complete. To be filled in after M4.

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
