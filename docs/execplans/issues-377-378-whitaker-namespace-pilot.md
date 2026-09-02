# Restore portable Whitaker installation and pilot Namespace runners

Status: IN PROGRESS — EP-M1 and EP-M2 complete and reviewed; EP-M3 in progress.

This ExecPlan delivers three reviewable changes as one GitHub stacked pull
request chain. The bottom layer restores a cold `whitaker-installer` source
installation under its declared minimum supported Rust version (MSRV). The
middle layer makes the x86_64 Linux release artefacts portable to Ubuntu 22.04.
The top layer migrates compatible Whitaker continuous-integration jobs from
UbiCloud to bounded Namespace profiles with one shared cache volume. Success is
visible when the installer builds under Rust 1.85, release artefacts require no
glibc symbol newer than `GLIBC_2.35`, and Namespace admits and completes the
migrated pull-request jobs.

## Conformance basis

Issue [#377](https://github.com/leynos/whitaker/issues/377) defines the cold
installer contract. Issue [#378](https://github.com/leynos/whitaker/issues/378)
defines the Linux glibc contract. Issue
[#387](https://github.com/leynos/whitaker/issues/387) defines the Namespace
pilot. `docs/adr-001-prebuilt-dylint-libraries.md` requires Linux release
portability through a conservative glibc baseline.
`docs/whitaker-dylint-suite-design.md` and `docs/publishing.md` define the
installer and publishing flows. Issue
[#386](https://github.com/leynos/whitaker/issues/386) tracks publication of the
separately maintained estate adoption procedure at a stable URL; its
workstation-local source was last verified with `nsc` 0.0.561 on 2026-09-01.

The trace links are:

- ISSUE-377 -> EP-M1 -> `make installer-msrv-check`;
- ISSUE-378 -> ADR-001 -> EP-M2 -> release workflow contracts and an Ubuntu
  22.04 packaged-artefact smoke test; and
- ISSUE-387 -> EP-M3 -> workflow contracts, `actionlint`, GitHub Actions, and
  `nsc github job list` evidence.

## Constraints

The three pull requests must remain a linear stack and each layer must be a
coherent, independently validated repository state. Branches and worktrees are
created with Git Donkey and linked with `gh stack`; stack branches are pushed
with `gh stack push` or `gh stack submit`, never an unleased force push.

The installer remains a Rust 2024 binary crate. Its MSRV is Rust 1.85 unless
evidence proves that an existing public requirement makes that impossible. A
dependency pin is permitted only when it is required to uphold that declared
MSRV and remains inside the dependency's compatible public API range.

The x86_64 Linux release target remains `x86_64-unknown-linux-gnu`. The release
fix must change the build baseline and verification, not silently substitute a
musl target or remove an artefact. The oldest supported baseline for this work
is Ubuntu 22.04 with glibc 2.35.

The Namespace pilot now uses Ubuntu 24.04 amd64 profiles capped at 4 vCPUs and
8 GB. `coverage-check` uses the 2-vCPU/4-GB `rust-linux-light` profile;
`linux-full` uses the 4-vCPU/8-GB `rust-linux-ci` profile. Both attach the same
20-GB cache volume through cache tag `whitaker-linux-amd64-v1`, and only trusted
`main` runs may populate it. Windows, macOS, release publication, rolling
release, and externally selected runners remain on their current platforms
unless a checked-in job contract proves that migration is both compatible and
within this plan.

The repository's action SHA pinning, permission model, documentation style,
Rust lint policy, and generated-file ownership rules remain intact.
`Cargo.lock` is regenerated through Cargo rather than edited by hand.

## Tolerances (exception triggers)

Stop and request direction if Rust 1.85 compatibility requires removing a
feature, changing the installer's public command-line interface, or pinning a
dependency outside a SemVer-compatible range. A deliberate MSRV newer than 1.85
also requires approval because it changes ISSUE-377's proposed outcome.

Stop if Ubuntu 22.04 cannot build the existing x86_64 GNU artefacts without a
new cross-compilation system, privileged container, third-party binary mirror,
or new signing/trust boundary. A simple explicit runner label, package install,
or repository-owned verification script is within tolerance.

For the Namespace layer, retain a job on its current runner and document the
reason when a required executable, Docker privilege, architecture, secret, or
performance contract is unavailable. Stop if the existing profile cannot admit
Whitaker jobs, if a migration would weaken permissions, or if three consecutive
pilot runs fail for the same Namespace-specific reason.

No single layer may absorb unrelated formatting, dependency upgrades, roadmap
work, or pre-existing failures. Any changed code file must remain below 400
lines.

## Risks

The main MSRV risk is that a transitive dependency raises its compiler floor
without an obvious direct manifest change. Mitigate this with an explicit
`rust-version`, a locked install target, and a CI job that runs the real Cargo
installation under Rust 1.85.

The release risk is a false portability result from checking only the installer
while downloaded `cargo-dylint` or `dylint-link` still require glibc 2.39.
Mitigate this by inspecting every packaged x86_64 Linux executable and running
the installed dependency binaries inside Ubuntu 22.04.

The pilot risk is confusing runner admission, image prerequisites, cache
behaviour, and command execution. Mitigate this with structural workflow tests,
one cache owner per path, cache-hit and sccache telemetry, and correlation of
GitHub job timestamps with `nsc github job list`.

Stacking creates a review dependency: the Namespace PR cannot merge before the
portability layers beneath it. This is intentional because the top layer uses
the contracts established below and GitHub requires lower stack layers to meet
merge requirements first.

## Verification plan

The first invariant, INV-MSRV, is that the installer's locked dependency graph
is accepted by Rust 1.85 and produces a runnable `whitaker-installer`. The
external axiom is Cargo's enforcement of package `rust-version` and dependency
MSRVs. A focused real installation discharges the invariant. The negative
control is the current main-branch lock graph, which the Cuprum run
demonstrated Cargo rejects because `time`, `time-core`, and `zip` require Rust
1.88.

The second invariant, INV-GLIBC, is that every packaged x86_64 Linux executable
requires no glibc symbol newer than 2.35. The lemma is that a binary built and
executed against the oldest supported runtime cannot acquire an unobserved
newer symbol requirement after packaging. A repository script will parse ELF
version references, and an Ubuntu 22.04 smoke job will execute the packaged
installer, `cargo-dylint`, and `dylint-link`. Existing v0.2.7 assets are the
negative control: their ELF tables contain `GLIBC_2.39` and fail to start on
Ubuntu 22.04.

The third invariant, INV-RUNNER, is that each repository-owned migrated Linux
job uses `namespace-profile-default`, while jobs with retained platform
contracts keep their existing runner expressions. Deterministic workflow tests
enumerate the expected assignments and reject a GitHub-hosted, UbiCloud, or
wrong Namespace label at a migrated site. `actionlint` validates the
intentional self-hosted label. A live pull-request run plus `nsc` admission
evidence discharges the external runner axiom.

These are finite configuration partitions rather than unbounded algorithms, so
parameterized example tests and real boundary execution provide proportionate
rigour. Property tests, model checking, and formal proofs would not strengthen
the relevant guarantees. Non-vacuity comes from testing each named executable,
each migrated job, each retained runner class, and the known failing v0.2.7
artefacts.

## Milestones

### EP-M1: Declare and enforce installer MSRV

Start in the Git Donkey worktree for `fix-installer-msrv`, based exactly on the
observed `origin/main` tip. Add the smallest failing workflow or Makefile
contract that requires a real locked installer build under Rust 1.85. Run it
against the current lock graph and record the expected dependency-MSRV failure.

Declare `rust-version = "1.85"` for `whitaker-installer`. Use Cargo metadata to
identify the narrowest compatible direct or transitive dependency constraints,
then regenerate `Cargo.lock`. Do not hand-edit the lockfile. Add an
`installer-msrv-check` target and CI step that install into a temporary root,
execute `whitaker-installer --version`, and remove the temporary installation.
Update the users' guide, publishing guide, design document, and repository
layout only where the new target or policy needs a durable signpost.

The milestone is complete when the focused check fails on the original graph,
passes on the corrected graph, and the repository's formatting, linting, test,
documentation, and publish gates pass. Commit the layer with issue #377 in the
subject and body.

Recovery is to revert the manifest constraint, regenerated lockfile, test, and
documentation together. Remaining work is ISSUE-378 and the Namespace pilot.

### EP-M2: Enforce the x86_64 Linux glibc baseline

Create the `conservative-linux-release` stack layer and Git Donkey worktree on
top of EP-M1. First add workflow-contract tests that reject `ubuntu-latest` for
the two x86_64 Linux release matrix entries and require the declared Ubuntu
22.04 baseline. Add a compatibility check that rejects a fixture or downloaded
binary naming `GLIBC_2.39`; record the expected red result.

Change both x86_64 release matrix entries to an explicit Ubuntu 22.04 runner.
Add a small repository-owned script that determines the maximum required glibc
symbol version for ELF executables and fails above 2.35. Apply it to the
packaged installer and dependency binaries before upload, then add an Ubuntu
22.04 end-to-end smoke job or release dry-run path that extracts and executes
all three tools. Keep architecture-specific non-Linux runners and artefact
names unchanged. Update ADR-001, the design document, publishing guide, and
repository layout for the owned script and enforced baseline.

The milestone is complete when the existing v0.2.7 Linux assets fail the new
check for `GLIBC_2.39`, newly built assets pass at or below 2.35, release
workflow contracts pass, and all repository gates pass. Commit the layer with
issue #378 in the subject and body.

The glibc checker is owned by `scripts/` and is reusable only at release
boundaries that already hold built ELF files. Callers select the supported
baseline and pass explicit files; the checker reads ELF version metadata and
reports compatibility but does not build, package, extract, download, or run
artefacts. Workflow jobs compose it after building and before uploading, while
script tests supply deterministic tool output. Keep packaging and end-to-end
execution in their existing owners rather than expanding this checker into a
general release orchestrator.

Recovery is to revert the release runner and verification additions as one
layer. Remaining work is the Namespace pilot.

### EP-M3: Add Whitaker to the Namespace pilot

Create the `adopt-namespace-runners` stack layer and Git Donkey worktree on top
of EP-M2. Inventory every workflow job, reusable workflow, composite action,
permission, cache, artefact hand-off, architecture, and assumed executable.
Record a GitHub/UbiCloud baseline from recent successful runs before changing
runner labels.

Migrate only compatible repository-owned pull-request Linux jobs whose current
4-vCPU resource contract fits the deployed 4-vCPU/16-GB profile. Provision
missing tools explicitly under existing version pins. Add
`.github/actionlint.yaml`, update deterministic workflow contracts for migrated
and retained assignments, and document the deployed uncached profile and every
exception in the developers' guide. Do not change actions/cache, sccache,
artefacts, release publication, Windows, macOS, mutation testing, or Docker
contracts in this baseline layer.

Push the stack and create draft linked PRs. Use the pull-request run to prove
admission and provisioning with
`nsc github job list --repository leynos/whitaker`. Compare queue time,
execution time, and outcome with the recorded baseline. If a job fails,
classify admission, provisioning, image prerequisite, command execution, cache,
and teardown separately before editing. Update the stable estate adoption
procedure once issue [#386](https://github.com/leynos/whitaker/issues/386) has
published it with a reviewable URL.

The milestone is complete when structural tests and all repository gates pass,
the expected jobs appear in `nsc`, and representative migrated jobs complete
successfully. Commit the layer, submit the three-PR draft stack, and record the
issue, pull-request, run, and job URLs.

Recovery is a revert of the runner-placement layer; the two portability fixes
beneath it remain independently useful.

#### Cache-optimization revision (2026-09-02)

The initial uncached placement proved admission but consumed too many Namespace
unit-minutes. The optimization revision replaces that measurement-only
configuration with bounded cached profiles. It must keep each expensive
dependency, tool, and compiler output under one cache owner, install tools only
from checksum-verified prebuilt releases, cap nextest at the profile's vCPU
count, prevent pull requests from writing the shared cache, and emit both the
Namespace cache result and sccache JSON statistics. `coverage-check` is omitted
from `main`, so a manual main-branch dispatch runs only `linux-full` and makes
that job the single cache-population owner.

## Validation commands

Run focused checks during each red-green-refactor cycle, followed by the full
repository gates before committing a layer:

```bash
make check-fmt
make lint
make test
make typecheck
make markdownlint
make nixie
make test-workflow-contracts
make publish-check PUBLISH_PACKAGES="whitaker-common whitaker-installer"
make release-installer-dry-run
```

Validate workflow syntax with the repository's installed `actionlint` policy.
Run the new MSRV and glibc targets explicitly because they exercise external
toolchains and runtime boundaries. Use `git diff --check` and inspect each
layer's diff from its immediate base before committing.

## Progress

- [x] 2026-09-01: Confirmed the Cuprum cache-hit glibc failure and cold source
  installation MSRV failure.
- [x] 2026-09-01: Inspected Whitaker v0.2.7 release artefacts and confirmed
  `GLIBC_2.39` in both `whitaker-installer` and `cargo-dylint`.
- [x] 2026-09-01: Filed issues #377 and #378 with acceptance criteria.
- [x] 2026-09-01: Created the `fix-installer-msrv` Git Donkey worktree from
  `origin/main`.
- [x] 2026-09-01: Obtained approval for this ExecPlan and began EP-M1.
- [x] 2026-09-01: Reproduced the EP-M1 negative control with
  `cargo +1.85.0 install --locked --path installer`: `time` 0.3.53, `time-core`
  0.1.9, and `zip` 8.6.0 require Rust 1.88.
- [x] 2026-09-01: Confirmed every published `zip` 8.x release requires Rust
  1.88, while `zip` 7.2.0 supports Rust 1.83.
- [x] 2026-09-01: Obtained approval to downgrade `zip` from 8.x to 7.2.0 and
  preserve the Rust 1.85 installer MSRV.
- [x] 2026-09-01: Completed EP-M1 implementation and deterministic validation
  in commits `f7dec85` and `b9846ff`.
- [x] 2026-09-01: Re-ran `publish-check` against committed `HEAD`; all 1,665
  CI-profile tests and both selected package verifications passed.
- [x] 2026-09-01: Extended `installer-msrv-check` to package, extract, and
  install the crate boundary with Rust 1.85, matching issue #377's publication
  contract.
- [x] 2026-09-01: Ran `coderabbit review --agent` through the scrutineer for
  EP-M1; it completed with zero findings and no rate-limit event.
- [x] 2026-09-01: Created the `conservative-linux-release` Git Donkey worktree
  from EP-M1 and registered both branches as a two-layer `gh stack`.
- [x] 2026-09-01: Added the read-only ELF/glibc checker and established the
  negative control: all three downloaded v0.2.7 x86_64 executables require
  `GLIBC_2.39` and are rejected above the 2.35 baseline.
- [x] 2026-09-01: Added Ubuntu 22.04 runner, pre-upload checker, packaged smoke,
  and partial-publication workflow contracts for tagged and rolling releases.
- [x] 2026-09-01: Built the installer and dependency tools inside Ubuntu 22.04;
  all three require at most `GLIBC_2.34`, below the `GLIBC_2.35` ceiling.
- [x] 2026-09-01: Passed the EP-M2 formatting, documentation, workflow,
  type-check, lint, full test, release dry-run, audit, Makefile, actionlint,
  and diff-hygiene gates. Nextest reported 1,652 passed and 5 skipped.
- [x] 2026-09-01: Committed EP-M2 as `3456839` and passed the committed-HEAD
  publication check: 1,665 CI-profile tests, all ten lint libraries, and both
  selected packages verified.
- [x] 2026-09-01: Ran `coderabbit review --agent` through the scrutineer for
  EP-M2; it completed with zero findings and no rate-limit event.
- [x] 2026-09-01: Confirmed the deployed `namespace-profile-default` with
  `nsc github profile describe --profile_id ghpf_d442h2l2nj56q -o json`: Ubuntu
  22.04, amd64, 4 vCPUs, 16,384 MB, remote builder, and no
  `cache_volume_settings` field. No profile was mutated.
- [x] 2026-09-01: Captured the pre-migration GitHub baseline from five
  successful CI runs with `gh run view`. Median queue/execution times were
  9s/24m11s for `linux-full` and 21s/13m29s for `coverage-check`; the complete
  run/job sample is recorded in `docs/developers-guide.md`.
- [x] 2026-09-01: Ran
      `nsc github job list --repository leynos/whitaker --since 7d` with
      `--max_entries 100 -o json`; it returned `null`, so there
      were no pre-migration Whitaker Namespace jobs to compare.
- [x] 2026-09-01: Documented the pilot's two intended migrated PR jobs and
  the retained Windows, main-branch coverage, release, rolling-release, and
  externally selected mutation exceptions. Cache volumes remain disabled and no
  Namespace cache persistence is claimed.
- [x] 2026-09-01: Migrated only `CI`'s `coverage-check` and `linux-full` jobs,
  added intentional actionlint labels, and added structural contracts for both
  migrated and retained runner assignments.
- [x] 2026-09-01: Diagnosed the Namespace-only coverage `E0463` as an omitted
  scheduling contract for the five active nested-Cargo Dylint UI tests;
  extended the existing `serial-dylint-ui` group without changing production
  lint code or the no-blanket-retry policy.
- [x] 2026-09-02: Isolated the build-script integration fixtures from the
  outer LLVM coverage target. Concurrent fixtures intentionally share a
  temporary package identity but validate different workspace manifests, so
  each nested Cargo command now receives the target directory owned by its
  `TempDir`. Ten focused concurrent coverage repetitions passed; no production
  build-script behaviour changed.
- [x] 2026-09-02: Resolved the five verified inline CodeRabbit findings from
  PR #381 review `5083956521` in `dfdc84f`: sentence-case debugging headings,
  table captions, capability-scoped test filesystem access, diagnosable
  coverage assertions, and `.yaml` workflow discovery. `make check-fmt`,
  `make typecheck`, `make lint`, `make test` (1,653 passed, 5 skipped),
  `make markdownlint`, `make nixie`, `make test-workflow-contracts`, and the
  focused workflow contracts all passed.
- [x] 2026-09-02: Resolved the current review's coverage-boundary pre-merge
  error in functional head `b6d2871`. The new isolated Makefile test runs fake
  coverage, recursive Make, and nested Cargo processes, proving that both
  target variables contain the same absolute directory; the developers' guide
  documents the corresponding override rule. The same deterministic gate set
  passed before commit.
- [x] 2026-09-02: After #382 squash-merged as `8ac23d1`, rebased the twelve
  Namespace-only commits from `691773c` onto that exact `origin/main` tip. The
  rebase retained the #382 baseline and replayed every Namespace commit
  one-to-one; a workspace compile passed after each replayed commit.
- [x] 2026-09-02: Replaced the uncached 4-vCPU/16-GB pilot placement with the
  bounded `rust-linux-light` and `rust-linux-ci` profiles. Both jobs use cache
  tag `whitaker-linux-amd64-v1`; `coverage-check` is capped at two nextest
  workers and `linux-full` at four.
- [x] 2026-09-02: Made the Namespace cache volume the sole owner of Rust, uv,
  Bun, prebuilt-tool, and local sccache paths. The workflows disable
  overlapping shared-action GitHub caches, require binary-only installers, and
  publish cache-hit plus sccache JSON evidence.
- [x] 2026-09-02: Passed the cache revision's focused 17 workflow contracts,
  formatting, lint, full test (1,653 passed and 5 skipped), Markdown, and
  Mermaid gates. A first local test attempt was invalidated when a concurrent
  target-directory sweep removed its executable; the clean rerun passed.
- [ ] Keep merge gated on exact-head GitHub checks and no blocking CodeRabbit
  concerns. Do not merge until the result for the rebased, pushed head is green.
- [ ] Complete EP-M3 and monitor Namespace jobs.

## Surprises & discoveries

The original pilot documentation attributed the failure to prebuilt
`cargo-dylint`, but the first process that failed was the cached
`whitaker-installer` executable. Direct ELF inspection established that both
published binaries require `GLIBC_2.39`, so the durable release fix must cover
the installer and dependency tools.

Cache isolation exposed rather than solved the second failure. Namespace had no
`cargo-binstall`, so the fallback compiled from crates.io with Cuprum's Rust
1.85 project compiler. Cargo then rejected the locked dependency graph before
building any Whitaker code.

The local EP-M1 negative control reproduced that exact failure from the
checked-in lockfile. Cargo metadata alone accepted the graph, so the permanent
gate must perform the real locked install rather than relying on metadata or a
resolver-only check.

The full CI workflow contract module also failed unchanged `main` because two
action-pin expectations had not moved with the deployed workflow pins. EP-M1
touches this same contract module, so the stale expected SHAs are aligned with
the already-pinned workflow values as a test-only prerequisite correction; no
workflow behaviour changes as a result.

The complete workflow test suite exposed another unchanged-main failure when
the developer already has the pinned `cargo-dylint` in `~/.cargo/bin`. The
provisioning fixtures put stubs first on `PATH`, but the Makefile deliberately
prepends the real Cargo bin directory again. Give those subprocess fixtures an
isolated home so their stale-tool and failed-install scenarios remain
deterministic; this is a test-only correction with no production effect.

Namespace `coverage-check` subsequently reported `E0463` from nested-Cargo UI
cases, while a fresh local full `make coverage` passed all 1,652 selected
tests. The first failure identified three absent
`rstest_helper_should_be_fixture` clauses. A fresh run later failed in a mapped
`no_unwrap_or_else_panic` case, proving that its two unmapped sibling example
harnesses could still race it. Extend the narrow group for all five active
shared-target harnesses; retain the existing scoped Windows retry rather than
adding a blanket retry or serializing the suite. The configuration contract now
keeps every active nested-Cargo clause present.

The complete five-entry `no_unwrap_or_else_panic` set was then selected under a
fresh LLVM coverage target directory and passed. Nextest resolves it to seven
executions because `example_compiles_under_test_harness` has three bounded
cases, alongside the four single negative cases.

Run `33562381054` disproved a concurrency-only explanation: the serial
aliased-companion example passed directly before the first harness case failed
three times. The outer LLVM coverage command uses `target/llvm-cov-target`, but
the nested Dylint Cargo command rebuilt the example and `rstest` under the
ordinary shared `target/debug`. The narrow fix is a per-runner
`CARGO_TARGET_DIR` binding in Whitaker's test harness. It keeps nested Cargo
artefacts self-consistent without modifying lint production code or the wider
test schedule.

Run `33564463057` falsified the shared UI helper mapping. The CI process has no
`CARGO_LLVM_COV_TARGET_DIR`; cargo-llvm-cov 0.6.24's `show-env` emits that
name, but its normal Nextest command supplies only `--target-dir`. The
correction is to define the exact coverage directory at the `make coverage`
boundary through both `CARGO_LLVM_COV_TARGET_DIR` and `CARGO_TARGET_DIR`,
rather than requiring test-helper code to infer a driver-private target path. A
fresh verbose local coverage run used that exact target directory for Nextest
and passed all three `example_compiles_under_test_harness` cases without
`E0463`.

A documentation-only rerun later exposed a separate race in
`build_script_integration`: the exact- and loose-parser fixtures inherit the
same outer coverage target while declaring the same temporary package identity.
One of three concurrent focused runs let the loose fixture's nested
`cargo check` succeed, so its rejection assertion failed. The test-only repair
passes each fixture's own `TempDir/target` through `--target-dir`; ten repeated
concurrent coverage runs then passed. The production build script was already
correct and remains unchanged.

All published `zip` 8.x versions declare Rust 1.88. A trial with `zip` 6.0.0,
its default features narrowed to the capabilities Whitaker uses, and `time`
0.3.45 resolved a Rust-1.85-compatible dependency graph. Compilation then
reached Whitaker source and exposed let-chains in `installer/src/list.rs` and
`installer/src/main.rs` that were not stabilized until after Rust 1.85.
Rewriting those expressions does not require a feature or command-line change,
but selecting `zip` 7.2.0 crosses the current direct dependency's major-version
boundary and therefore required the approval mandated by this plan's tolerance
section.

The first Rust-1.85-compatible lock graph selected `time` 0.3.45 through
`zip`'s optional `time` feature. `cargo audit` reported RUSTSEC-2026-0009,
while the patched `time` 0.3.47 requires Rust 1.88. Whitaker uses
`zip::DateTime`'s built-in representation rather than the external time-crate
conversions, so removing that unused feature eliminates the vulnerable
transitive dependency without changing archive behaviour.

`publish-check` clones the repository's committed `HEAD` for its Dylint
artefact phase, so a pre-commit invocation validated that phase at the
preceding plan-only commit while its `cargo package --allow-dirty` phase
validated the current manifests. Re-run the complete target after committing
EP-M1 so every phase exercises the milestone commit before requesting review.

Repository-wide `actionlint` currently reports the intentional UbiCloud runner
label because no custom-label configuration exists, plus pre-existing SC2193
findings in the release workflows. An invocation ignoring only those known
categories passes. EP-M3 already owns `.github/actionlint.yaml`; the release
script findings are assessed alongside the release workflow in EP-M2 rather
than obscured by an EP-M1 change.

The release workflow's SC2193 findings come from ShellCheck evaluating GitHub
matrix expressions before Actions substitutes their runtime values. EP-M2 keeps
the comparisons and adds narrowly scoped inline directives with the
runtime-expansion rationale; `actionlint` then passes both touched release
workflows without a global ignore.

The Ubuntu 22.04 positive control produced `whitaker-installer`, `cargo-dylint`
6.0.1, and `dylint-link` 4.0.0 with a maximum required symbol of `GLIBC_2.34`
for each executable. This leaves one minor-version margin beneath the declared
`GLIBC_2.35` ceiling and demonstrates that the explicit build baseline corrects
the published v0.2.7 assets' `GLIBC_2.39` requirement.

## Decision log

- 2026-09-01: Use three PR layers rather than combining release engineering
  and runner migration. This preserves independent rollback and keeps each
  issue's acceptance evidence reviewable.
- 2026-09-01: Set the proposed installer MSRV to Rust 1.85 because it is the
  first stable Rust 2024 compiler and matches the discovered consumer. Treat a
  higher floor as a user-approved deviation rather than an incidental
  dependency outcome.
- 2026-09-01: Target glibc 2.35 rather than merely replacing a runner label.
  An explicit executable compatibility gate protects future release workflow
  changes from recreating the defect.
- 2026-09-01: Reuse the existing uncached Namespace default profile. Creating
  or mutating remote profiles is unnecessary for the initial pilot and would
  confound runner-placement measurements.
- 2026-09-01: Begin implementation after the user explicitly approved this
  ExecPlan and requested the complete three-layer rollout.
- 2026-09-01: Pause EP-M1 before accepting the experimental `zip` downgrade.
  The Rust 1.85 outcome requires either an approved move from `zip` 8.x to
  7.2.0 or a user-approved MSRV increase to Rust 1.88.
- 2026-09-01: Preserve Rust 1.85 after the user approved the `zip` 7.2.0
  downgrade. Keep the dependency feature set narrow and rewrite the post-1.85
  let-chain without changing behaviour or the public command line.
- 2026-09-01: Verify Rust 1.85 against the packaged crate rather than the
  workspace path. This matches the published-consumer boundary named by issue
  #377 and keeps the package artefact isolated from stale build output.
- 2026-09-01: Apply the Ubuntu 22.04 and glibc 2.35 contract to both tagged and
  rolling x86_64 artefact builders. Rolling lint libraries and dependency tools
  share the same consumer boundary as tagged installer assets, so leaving them
  on `ubuntu-latest` would preserve the compatibility defect in another release
  channel.
- 2026-09-01: Invoke the glibc checker directly from each workflow rather than
  add a Make target. The explicit ELF list differs by job, and a
  variable-driven wrapper would obscure rather than strengthen the release
  boundary. The validation instruction therefore means running the checker CLI
  explicitly.
- 2026-09-01: Keep the glibc checker's extracted helpers private to its release
  boundary. Repository search found no other ELF-inspection abstraction, and
  the helpers exist only to separate version-needs parsing, process execution,
  result validation, and contextual error reporting. They are not a general
  subprocess or ELF API and must not be called from packaging or build code.
- 2026-09-01: Use the deployed `namespace-profile-default` unchanged for the
  pilot. The read-only profile description proves the required Ubuntu 22.04,
  amd64, 4-vCPU, 16-GB shape and absence of cache-volume settings; creating or
  mutating a profile would invalidate the baseline and is out of scope.
- 2026-09-01: Treat the empty `nsc github job list` result as the UbiCloud /
  Namespace pre-migration baseline rather than inventing Namespace timing data.
  Compare queue and execution durations only after migrated jobs have completed
  successfully.
- 2026-09-01: Migrate only `coverage-check` and `linux-full` in the pilot.
  Both are repository-owned pull-request Linux jobs whose prior UbiCloud
  standard-4 resource class matches the deployed Namespace shape. Retain the
  main-branch coverage, release, Windows, mutation, and reusable-workflow
  assignments to preserve their distinct event, platform, or ownership
  boundaries.
- 2026-09-02: Rebase the Namespace-only layer onto #382's `8ac23d1` squash
  rather than retaining the obsolete stacked parent. This preserves #382's
  released baseline while keeping the runner pilot's independently reviewed
  commits and contracts intact.
- 2026-09-02: Supersede the uncached measurement baseline after the user halted
  the estate rollout for cost control. Use 2 vCPUs/4 GB for the coverage gate,
  4 vCPUs/8 GB for the full build gate, and one shared cache tag rather than
  independent per-job caches. This keeps the ceiling at four cores while a
  single trusted main-branch job prevents cache-population stampedes.

## Outcomes & retrospective

EP-M1 now declares Rust 1.85 in the installer manifest, enforces a real locked
packaged-crate install in the Makefile and Linux CI, and retains only the `zip`
7.2 Deflate feature. Removing the unused `time` integration also leaves
`cargo audit` with no known vulnerabilities. The focused MSRV install,
formatting, Markdown, Mermaid, workflow-contract, type-check, Clippy, full
test, release archive, Makefile, and scoped actionlint gates pass. The full
Nextest result is 1,652 passed and 5 skipped. `cargo audit` retains four
pre-existing allowed warnings but reports no vulnerabilities. Whitaker has no
`doc-coverage` target, so that Netsuke-specific gate is not applicable.

The post-commit publication gate also passes: its CI-profile Nextest run
reported 1,665 passed and 5 skipped, the cloned-HEAD Dylint library build
listed all ten expected libraries, and both `whitaker-common` and
`whitaker-installer` packages verified. CodeRabbit reported no high-, medium-,
or low-severity concerns for the committed milestone.

Revision note (2026-09-01): The MSRV verification now packages, extracts, and
installs `whitaker-installer` under Rust 1.85 so it exercises the published
crate boundary. The Namespace adoption procedure is tracked in issue #386 until
its owner publishes a stable URL; this does not change the remaining EP-M2 or
EP-M3 implementation scope.

EP-M2 now fixes both tagged and rolling x86_64 release builders to Ubuntu
22.04, rejects ELF requirements above `GLIBC_2.35` before upload, and adds a
tagged packaged-artefact compatibility job that checks and executes all three
release tools before publication. The downloaded v0.2.7 negative control failed
at `GLIBC_2.39`; clean Ubuntu 22.04 builds of the installer and both dependency
tools passed at `GLIBC_2.34`. All deterministic milestone gates pass with 1,652
Nextest cases successful and 5 skipped. The committed-HEAD publication check
then passed 1,665 CI-profile tests with 5 skipped, built and listed all ten
expected Dylint libraries from a clone of `3456839`, and verified the
`whitaker-common` and `whitaker-installer` packages.

EP-M3 now moves the repository-owned `coverage-check` and `linux-full` jobs to
the deployed, uncached `namespace-profile-default` while retaining platform,
release, main-branch coverage, and externally owned reusable-workflow runner
boundaries. Structural contracts and actionlint cover both migrated and
retained assignments. Namespace run `33566432251` admitted both migrated jobs
to 4-vCPU, 16-GB Linux instances within about two seconds of workflow creation;
the jobs began after about 12 seconds. `coverage-check` completed successfully
at 22:36:10 UTC and `linux-full` at 22:38:28 UTC. The retained Windows job also
passed at 22:41:12 UTC. A post-gate `coderabbit review --agent` reported zero
high-, medium-, or low-severity findings at `e858760`.

The stable coverage result required one explicit build boundary beyond runner
placement: `make coverage` now gives outer LLVM coverage and nested Dylint
Cargo the same absolute target directory. A syscall trace proved the nested
example build inherited the outer target, and the fresh Namespace run no longer
reported `E0463`. The pilot therefore distinguishes fast runner admission from
build-tool target isolation rather than attributing the earlier test failure to
Namespace contention.

The build-script integration tests add a second, narrower boundary: their
temporary workspaces now own nested Cargo output even when the outer coverage
job deliberately shares one target directory. This prevents fixture-specific
manifest validation from reusing another fixture's build-script result.

The cache-optimization revision retains those behavioural boundaries while
changing the execution substrate. Both migrated jobs use Ubuntu 24.04, one
shared 20-GB Namespace cache, checksum-verified prebuilt tools, and bounded
nextest concurrency. Cache-hit output and sccache JSON make cold-versus-warm
performance observable; pull requests are readers, while the main-branch
`linux-full` job is the sole cache writer.
