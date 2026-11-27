# Publish `whitaker` release artefacts with `cargo-binstall` metadata (3.5.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (expert review applied; awaiting user approval). Implementation
must not begin until the user explicitly approves this plan. Decisions D-4
(crates.io publication from CI), D-5 (no v0.3.0 backfill), and D-11 (verified
multi-package packaging in CI) are called out for explicit approval.

Branch: `3-5-3-publish-whitaker-release-artefacts`

Roadmap item: 3.5.3 in `docs/roadmap.md` ("Publish `whitaker` release artefacts
with `cargo-binstall` metadata that mirror the existing installer packaging
flow"). Requires 3.5.1, 4.2.1, and 4.3.1, all of which are done.

## Purpose / big picture

Whitaker ships two user-facing executables. `whitaker-installer` is the legacy
installer. `whitaker` is the root command-line interface (CLI) that roadmap
item 3.5.1 added, and its `whitaker check` subcommand (3.5.2) is now the
default linting path. The design's headline workflow is:

```console
cargo binstall whitaker
whitaker check
```

`cargo binstall` (from the `cargo-binstall` project) installs a Rust binary by
downloading a prebuilt archive instead of compiling the crate. It downloads the
requested version's crate from crates.io and reads the
`[package.metadata.binstall]` table in that version's `Cargo.toml`. The table
holds templates: `pkg-url` says where the archive lives, and `bin-dir` says
where the binary sits inside it. Binstall fills their placeholders (`{name}`,
`{version}`, `{target}`, `{bin}`, `{binary-ext}`) by literal substitution.

Today the second half of that workflow works, but the first half does not do
what it says. The root `Cargo.toml` already carries a binstall table for
`whitaker`, and `whitaker` 0.3.0 is on crates.io, but no release has ever
published the archive that table names. The v0.3.0 GitHub release carries
`whitaker-installer-<target>-v0.3.0.{tgz,zip}` for five targets and no
`whitaker-*` archive. `cargo binstall whitaker` therefore requests a
`whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz` under the v0.3.0 release and
gets a 404. It then silently falls back to compiling the crate from source.
That costs several minutes of build time, and it is a hard failure for anyone
running binstall with `--disable-strategies compile`.

After this change, every `v<version>` release built by `release.yml` from 0.4.0
onwards also publishes, for each of the five supported targets, a
`whitaker-<target>-v<version>.tgz` archive (`.zip` on Windows) and its
`.sha256` sidecar. The `whitaker` archives use the same packager, runners,
glibc baseline, and verification gates as the installer's. The release workflow
publishes `whitaker-common`, `whitaker-installer`, and `whitaker` to crates.io
with one verified, re-runnable multi-package publish. A post-publication job
proves on Linux, macOS, and Windows that the following installs the released
archive's binary rather than building from source:

```console
cargo binstall --no-confirm --disable-strategies compile,quick-install whitaker@<version>
whitaker --version
```

Before any release exists, a new pull-request check runs the real
`cargo-binstall` against archives that the real packager produced, served from
a local HTTPS server, with the real binstall tables. A naming or layout
regression is caught before a tag is pushed.

Preserved behaviour:

- Every `whitaker-installer` asset name, archive layout, binstall table, and
  verification step stays the same.
- The installer binary is built exactly as today, by a `cargo build` that
  names only `whitaker-installer`, so its resolved feature set does not change.
- No user-facing command, flag, or diagnostic of either executable changes.

Deliberately changed behaviour:

- The release-only helper `whitaker-package-installer` gains a required
  `--package` argument naming the executable it packages. It also rejects a
  version string outside the SemVer character set.
- The public `whitaker_installer::installer_packaging` module is renamed and
  generalized. This is a source-breaking change to a published 0.x library, so
  it is recorded in the 0.4.0 migration guide.
- The root, installer, and common crates inherit their version from
  `[workspace.package]`.
- A failed crates.io publication can now be completed by re-running the job.

This item closes the "no `whitaker` GitHub release archives until 3.5.3"
limitation. That limitation is recorded in
`docs/adrs/adr-0012-whitaker-cli-boundary.md` (§Decision outcome and §Known
risks and limitations), in `docs/users-guide.md` (§The shortest path and §Root
CLI), and in `docs/publishing.md`.

## Definitions

- **Release artefact.** A file attached to a GitHub release, here an archive
  and its `.sha256` sidecar.
- **Release asset namespace.** The flat set of file names attached to one
  GitHub release. Two assets cannot share a name. The release workflow's
  re-dispatch path uses `gh release upload --clobber`, which silently replaces
  an asset of the same name.
- **Packager.** The release-only binary `whitaker-package-installer`
  (`installer/src/bin/package_installer_bin.rs`). It is a thin wrapper around
  the packaging library.
- **Supported target.** One of the five triples in `SUPPORTED_TARGETS`
  (`installer/src/artefact/target.rs:15`), validated by `TargetTriple`.
- **Release leg.** One matrix entry of the `build-installer` job in
  `.github/workflows/release.yml`, which builds one target on one native runner.
- **Lockstep version.** The rule in `docs/publishing.md` that
  `whitaker-common`, `whitaker-installer`, and `whitaker` share one version,
  and that the release tag is `v` followed by it.
- **First CLI-archive version.** The first version whose release carries
  `whitaker-*` archives: 0.4.0, the next release. A release below it is not
  expected to carry them.
- **Feature unification.** Cargo's resolver 2 merges the features that every
  package in one build command requests for a shared dependency. Building two
  packages together can therefore compile one of them differently from building
  it alone.

## Constraints

1. Implementation begins only after the user explicitly approves this plan.
2. `whitaker-installer` release behaviour must not change. This covers its
   asset names, its archive layout
   (`whitaker-installer-<target>-v<version>/whitaker-installer`, or `.exe`), its
   `[package.metadata.binstall]` table in `installer/Cargo.toml`, its checksum
   format, and every existing verification step. It also covers the resolved
   dependency feature set of the installer binary: the installer is built by a
   `cargo build` that names only `-p whitaker-installer`, never together with
   `-p whitaker`. The planning check found that a combined build adds `clap`'s
   `env` and `string`, `proc-macro2`'s `span-locations`, and `syn`'s `visit`.
   Archive bytes are not reproducible today (D-7), so the constraint is about
   names, layout, and feature set, not checksums. The `ci.yml` pin
   `WHITAKER_INSTALLER_VERSION` and the lint jobs that binstall it are out of
   scope.
3. The root binstall table stays identical to the installer's apart from the
   package name, as `tests/behaviour_binstall.rs` already pins. Binstall reads
   the table from the manifest of the version it resolves, so each release
   publishes its own table. The reason for keeping it identical is the
   mirroring requirement, not compatibility with 0.3.0. Changing it needs a
   `Decision log` entry and a matching change to `INV-NAME`.
4. Supported targets and runners are exactly those of `build-installer` today
   (`release.yml:30-39`). Adding or dropping a target is out of scope.
5. x86_64 Linux release binaries must not require a glibc symbol newer than
   `GLIBC_2.35` (ADR 0001; `docs/publishing.md`). The `whitaker` binary joins
   the existing `scripts/check_glibc_baseline.py` check.
6. No new runtime dependency. The authorized dev-dependency additions for the
   `whitaker-installer` package are `googletest` and `pretty_assertions`, both
   already declared in `[workspace.dependencies]`. Python scripts follow
   `docs/scripting-standards.md` (`uv run --script`, Python 3.13), with one
   restriction: every new script uses the standard library only
   (`dependencies = []`, `argparse`). Release-path siblings such as
   `scripts/generate_checksums.py` already do this, so a release never
   downloads a Python package.
7. Interfaces that may change, with every caller updated in the same commit
   and no compatibility shim:
   - The public `whitaker_installer::installer_packaging` module, its types,
     and its functions. It is renamed and generalized (D-12). The release
     must be 0.4.0, which the existing `docs/migrations/0.4.0.md` already
     targets, and the break is listed there.
   - The packager's command line, which is behind `release-tools` and
     excluded from `cargo install` and `cargo binstall`.
   - The `release-installer-dry-run` recipe's outputs.

   No other public API of `whitaker-installer` or `whitaker` changes.
8. Identifiers visible outside Rust keep their "installer" names: the
   packager binary `whitaker-package-installer`, the `build-installer` job, the
   `installer-<target>` workflow artefact, and the `release-installer-dry-run`
   target. Their documentation is widened. A tracked issue owns the renames
   (D-2).
9. No file under `docs/execplans/` other than this one may be edited;
   completed plans are historical. Accepted ADR decisions are not rewritten.
   ADR 0012 gains an entry in its existing `## Implementation note` section.
10. Code files stay under 400 lines (`AGENTS.md`). Renamed test modules that
    would grow past the limit are split.
11. Tests do not mutate the process environment (`AGENTS.md` §Testing).
12. Every Rust module begins with a `//!` comment, and every function carries
    a `///` comment. Every `assert!`-family call carries a diagnostic message.
    Python definitions carry numpy-style docstrings (the `interrogate` gate
    inside `make lint`).
13. Nothing in implementation pushes a tag, creates a release, or publishes a
    crate. Release-time behaviour is evidenced by contract tests, harness runs
    of the new scripts, the Makefile dry run, local multi-package packaging,
    and the local binstall check. The first real evidence of the
    post-publication job for `whitaker` arrives with the 0.4.0 release (see
    `Risks`).

## Tolerances (exception triggers)

Comparison base: `git merge-base origin/main HEAD` at the start of
implementation (expected `755c1f2c` unless `main` advances). This ExecPlan file
is excluded from every count. Count with rename detection, so a moved file
counts only its edited lines:

```sh
base=$(git merge-base origin/main HEAD)
git diff -M --numstat "$base"..HEAD -- . ':(exclude)docs/execplans/**'
```

Classification uses a whole-file rule:

- Code: `*.rs`, `*.py`, `*.sh`, `*.feature`, `Makefile`, and
  `.github/workflows/*.yml`. The workflow additions are mostly embedded shell,
  so the whole file counts as code.
- Configuration: `Cargo.toml` files, `Cargo.lock`, and `.config/nextest.toml`.
- Documentation: `*.md` outside `docs/execplans/`.

Limits measure net lines (additions minus deletions) per category and are
enforced independently:

- Files: more than 40 changed non-ExecPlan files.
- Code: more than 2,200 net lines.
- Configuration: more than 60 net lines.
- Documentation: more than 500 net lines.
- Interface: any public API change beyond Constraint 7; any change to a
  user-facing command or flag of `whitaker` or `whitaker-installer`; or any
  change to either binstall table.
- Dependencies: any dependency beyond Constraint 6.
- Architecture: building the installer together with `whitaker`, using a
  separate packager for `whitaker`, changing the target matrix, or changing
  asset names.
- CI cost: the `windows-compat` job's wall time growing by more than 10
  minutes, or `linux-full`'s by more than 8 minutes, against the median of the
  three most recent green `main` runs. The new `verify-binstall-local` job is
  limited to 30 minutes by `timeout-minutes`.
- Iterations: a gate still failing after three fix attempts for the same
  cause.
- Time: a milestone taking more than one working day of elapsed effort.
- Ambiguity: any point where `docs/whitaker-cli-design.md`, ADR 0012, and
  this plan disagree in a way that changes behaviour.

Budget survey. The estimate traces each affected file. Production Rust (two new
modules, the renamed packaging module, and the packager CLI) comes to about 220
lines. Rust tests (unit, behaviour-driven, property, packager CLI, and root
naming and namespace) come to about 600. Three standard-library Python scripts
(`release_assets.py`, `publish_crates.py`, and `verify_binstall_local.py`) come
to about 450, and their contract and harness tests to about 450. Workflow and
Makefile changes come to about 250. That totals roughly 1,970, leaving about
10% contingency inside 2,200. Configuration is three `version.workspace` lines
and two dev-dependency lines. Documentation covers nine files at about 350
lines.

Historical evidence.

- `4-2-1-add-cargo-binstall-metadata.md` set 10 files and finished at 9, with
  no revision.
- `4-3-1-release-workflow-for-whitaker-installer.md` set 15 files and 800 net
  lines and records no revision. Later work grew that workflow considerably
  outside the plan.
- `3-5-1-root-whitaker-binary.md` broke its 70-file budget and raised its
  net-line ceiling to 7,000 (D-12). It finished at 186 files and +13,009/−1,222.
- `3-5-2-whitaker-check-as-the-default-linting-path.md` set 30 files and
  2,500 net lines for code. It finished at 73 files and 9,001 net lines after
  five review rounds.

In both overruns most of the growth was test and verification scope added
during review, which is missed scope, not production code. This plan therefore
states the full verification scope before approval, and the expert review that
preceded approval has already been folded in (D-13). A later review round that
adds obligations must re-estimate against these limits and escalate before
breaching them.

## Risks

- Risk: `cargo-binstall` 1.16.6 (the version CI pins) may lack
  `--root-certificates`, or may treat a synthesized manifest differently from
  1.22.0, which the planning probe used. Severity: medium. Likelihood: low.
  Mitigation: `EP-M0` re-runs the probe against 1.16.6 before any code depends
  on it. If 1.16.6 fails, the local check pins the oldest version that passes,
  and the decision is recorded.
- Risk: the first real proof that `cargo binstall whitaker` resolves a
  published archive arrives only with the 0.4.0 tag, after merge. Severity:
  medium. Likelihood: low. Mitigation: the local HTTPS check runs the real
  binstall against the real tables and the real packager output, and checks the
  exact URL binstall requests. The remaining difference is the URL origin and
  GitHub's asset hosting, which the installer's identical, already verified
  pattern covers. Roadmap 3.5.3 is marked done with `INV-PUBLISHED` stated as
  post-release evidence.
- Risk: today's partial-release policy is fail-closed rather than partial.
  `release-compatibility` downloads the x86_64 installer artefact only when the
  whole `build-installer` matrix succeeded (`release.yml:254`). Any failed leg
  therefore fails compatibility and blocks `publish`. Adding `whitaker` to
  every leg makes a `whitaker`-only failure release-blocking too. Severity:
  medium. Likelihood: low. Mitigation: this is the desired lockstep behaviour,
  because a release with an installer and no CLI archive is incomplete. It is
  documented truthfully in `REQ-ARCHIVES` and `docs/publishing.md`. Repairing
  the existing download condition is out of scope and goes in a follow-up issue.
- Risk: a crates.io publication fails part-way. Severity: high. Likelihood:
  low. Mitigation: `scripts/publish_crates.py` (D-4) skips every package whose
  version is already in the registry. It then publishes the remainder with one
  `cargo publish -p … -p …`, which packages and verifies every named crate
  against a local staging registry before uploading any. A re-run therefore
  completes a partial publication.
- Risk: the root crate's full verification has so far happened only at
  publish time (`make package-root-cli` uses `--no-verify`). Severity: medium.
  Likelihood: medium. Mitigation: D-11 makes pull requests run a verified
  multi-package `cargo package` for the three crates.
- Risk: the `whitaker` release build lengthens each release leg and the
  Windows dry run. Severity: low. Likelihood: high. Mitigation: the CI-cost
  tolerance. Record leg and job durations before and after in
  `Artefacts and notes`.
- Risk: the `tests/workflows` suite is opt-in and never runs in CI, so a
  structural guard placed there is not a guard. Severity: medium. Likelihood:
  medium. Mitigation: every new invariant is pinned in
  `tests/workflow_contracts`, which `make test-workflow-contracts` runs in
  `linux-full`. Existing `tests/workflows` files are updated only to stay
  correct.

## Progress

- [x] (2026-11-27T22:10Z) Branch renamed and Lody session titled.
- [x] (2026-11-27T22:30Z) Wyvern reconnaissance of the release workflow,
  root CLI and design, and test and verification infrastructure.
- [x] (2026-11-27T22:40Z) crates.io and GitHub release state confirmed;
  binstall HTTPS feasibility probe and negative control run with 1.22.0.
- [x] (2026-11-27T23:00Z) Initial draft written and committed.
- [x] (2026-11-28T00:10Z) Community-of-experts design review run as four
  independent reviewers. Feature unification confirmed locally with
  `cargo tree`. Findings applied (D-13).
- [ ] Plan approved by the user.
- [ ] `EP-M0` feasibility probe against `cargo-binstall` 1.16.6, and the
  `whitaker --version` format.
- [ ] `EP-M1` the packaging library and packager package either executable.
- [ ] `EP-M2` the release pipeline builds, checks, and publishes `whitaker`.
- [ ] `EP-M3` binstall resolution verified before and after publication.
- [ ] `EP-M4` documentation, migration guide, ADR note, and roadmap closure.

## Surprises & discoveries

- Observation: the root package already carries binstall metadata, and
  `whitaker` 0.3.0 was published to crates.io by hand on 2026-11-16.
  `release.yml` publishes only `whitaker-common` and `whitaker-installer`.
  Evidence: `Cargo.toml:131-138`; the crates.io API reports `max_version` 0.3.0
  for `whitaker`; `release.yml:520-527`. Impact: 3.5.3 is about producing the
  archives and automating the publication, not about writing metadata.
- Observation: the v0.3.0 release exists but has no `whitaker-*` asset.
  Evidence: `gh release view v0.3.0 --json assets`. Impact: release-level
  probes cannot tell "not yet expected" from "missing". The first CLI-archive
  version decides this (D-6).
- Observation: `cargo-binstall` refuses plain `http://` package URLs
  (`reqwest` `BadScheme`). It accepts `https://127.0.0.1` with a self-signed
  certificate passed through `--root-certificates`. Evidence: planning probe,
  `Artefacts and notes`. Impact: the local end-to-end check uses HTTPS.
- Observation: building `-p whitaker-installer -p whitaker` together changes
  the installer's dependency features. Evidence:
  `cargo tree --offline -e features` run with and without `-p whitaker`
  (`Artefacts and notes`). Impact: separate build commands (Constraint 2, D-1).
- Observation: `installer_packaging` and `dependency_packaging` duplicate the
  tar and zip writers. Only the dependency copy uses
  `tar::HeaderMode::Deterministic` and a fixed zip timestamp. Evidence:
  `installer/src/installer_packaging.rs:230-276`,
  `installer/src/dependency_packaging.rs:150-192`. Impact: D-7.
- Observation: `verify-binstall-manifest` reads the version with
  `sed … | head -n 1`. On the root `Cargo.toml` that finds the
  `[workspace.package]` version before `[package]`. Evidence:
  `verify-binstall.yml:185`, `Cargo.toml:15` and `:88`. Impact: D-3a makes the
  two values one, and the job reads it through
  `release_assets.py manifest-version`.
- Observation: the `4-2-1` and `4-3-1` plans diverged from what shipped
  (literal `.tgz` instead of `{archive-format}`, native ARM runners, later
  gates), and record none of it. Impact: this plan describes the current
  workflow from source, not from those plans.

## Decision log

- D-1. Decision: build and package `whitaker` inside the existing
  `build-installer` release leg, in steps that follow the installer's unchanged
  steps. The leg runs `cargo build --locked -p whitaker-installer` first and
  packages the installer, then runs `cargo build --locked -p whitaker` and
  packages the CLI. Both archives go into `dist/` and upload as the leg's single
  `installer-<target>` artefact. Rationale: this mirrors the installer flow on
  the same runner, toolchain, packager, checksum step, and glibc check.
  Separate commands keep the installer's feature set unchanged (Constraint 2),
  and the second build reuses most of the first's compiled dependencies. The
  alternative was a separate `build-whitaker` job. It duplicates about 100
  lines of YAML, doubles runner start-up, and protects nothing, because the
  existing compatibility gate is already fail-closed on any leg failure
  (`Risks`). Separate per-package workflow artefacts were also considered and
  rejected for the same reason. Date/Author: 2026-11-28, plan author, revised
  after review.
- D-2. Decision: keep the identifiers visible outside Rust (Constraint 8)
  and widen their documentation. Open a tracked GitHub issue in `EP-M4` that
  owns their renames, linked from the design document. Rust-internal names are
  renamed now (D-12). Rationale: renaming the packager binary, job, artefact,
  and Makefile target touches release, CI, harness, and documentation files for
  no behaviour gain. Roadmap 3.9.3 covers user-facing installer-first
  references and does not clearly own internal tooling names, so an explicit
  issue is needed to keep the renames from being lost. Date/Author: 2026-11-28,
  plan author, revised after review.
- D-3. Decision: the executable is a closed enum,
  `ReleaseBinary { Whitaker, WhitakerInstaller }`, in a new module
  `whitaker_installer::release_binary`. It provides `ALL`, `package_name()`,
  `bin_name()`, and a conversion to `binstall_metadata::BinstallRelease`. The
  packager exposes it as a required `--package <whitaker|whitaker-installer>`
  (`clap::ValueEnum`; `clap` is already an unconditional library dependency,
  `installer/Cargo.toml:80`). Rationale: a closed set makes the asset namespace
  finite and checkable, and catches typos at parse time. A separate module gives
  `binstall_metadata` and the packaging module one shared identity without a
  dependency cycle. Keeping `bin_name()` apart from `package_name()` lets tests
  compare each against the real `[package].name` and `[[bin]].name`. A required
  argument avoids a silent default that would mislabel an archive. The rejected
  alternative was a free-form `--name` string. Date/Author: 2026-11-28, plan
  author, revised after review.
- D-3a. Decision: make lockstep structural. `Cargo.toml` (root `[package]`),
  `installer/Cargo.toml`, and `common/Cargo.toml` set
  `version.workspace = true`. The release checks that the tag equals the
  version of each of the three packages, read through `cargo metadata`.
  Rationale: a structural rule cannot drift, and `cli/Cargo.toml` already
  inherits. The planned standalone version-check script is dropped. The
  `[workspace.dependencies]` path entries still name a version, and Cargo
  rejects a mismatch at resolution. Date/Author: 2026-11-28, plan author, from
  review.
- D-4. Decision: `release.yml` publishes `whitaker-common`,
  `whitaker-installer`, and `whitaker` through `scripts/publish_crates.py`. The
  script queries crates.io for each `<name>/<version>`: HTTP 200 means
  published and is skipped; 404 means unpublished; anything else fails. It then
  runs one `cargo publish --locked -p <each unpublished>`, under the existing
  `CARGO_REGISTRY_TOKEN` gate. Rationale: binstall reads `whitaker@<version>`
  from crates.io, so an archive without the matching crate version is
  unreachable, and the installer flow being mirrored already publishes its
  crate. Multi-package publication (stable since Cargo 1.90; the pinned nightly
  is 1.98) verifies all named packages before uploading any. Skipping published
  versions makes the step safe to re-run. This is an outward-facing change that
  takes effect on the next tag, so it is called out for approval. The
  alternative was to keep the manual root publication in `docs/publishing.md`.
  That leaves a window in which archives exist that binstall cannot see.
  Date/Author: 2026-11-28, plan author; requires user approval.
- D-5. Decision: do not backfill v0.3.0. The first release carrying
  `whitaker-*` archives is 0.4.0. Rationale: the only automated route is a
  re-dispatch, which rebuilds and `--clobber`s every published v0.3.0 asset,
  changing the bytes under existing checksums. A non-clobbering backfill needs
  either a new "upload missing assets only" mode, or binaries for five targets
  built by hand from the v0.3.0 tag; macOS and Windows cannot be built on this
  host. 0.4.0 is the next release, so the value of a backfill is small. Users
  of 0.3.0 keep the documented source-build fallback. The user may override
  this at approval, in which case the backfill becomes an explicitly authorized
  post-merge action with its own plan. Date/Author: 2026-11-28, plan author,
  rationale corrected after review.
- D-6. Decision: the first CLI-archive version, 0.4.0, is a named constant in
  one place, `scripts/release_assets.py`, exposed as the
  `cli-archives-expected VERSION` subcommand. Both binstall verification jobs
  ask it before handling `whitaker`. Below 0.4.0 they skip with a notice. At or
  above it, a missing `whitaker` archive fails the job. Rationale: a skip
  whenever the asset is absent would hide every future incomplete release,
  which is the regression these jobs exist to catch. A version floor allows
  only the known historical gap. Date/Author: 2026-11-28, plan author, revised
  after review.
- D-7. Decision: do not merge the duplicated tar and zip writers, and do not
  change the installer archives' header mode, in this item. Open a follow-up
  issue. Rationale: merging would change installer archive bytes (header mode
  and timestamps), which is a separate behaviour change. The new `whitaker`
  archives use the installer writer, so the two executables' archives stay
  identical in construction. Date/Author: 2026-11-27, plan author.
- D-8. Decision: no Verus proof and no Kani harness. Verification uses:
  exhaustive tables over the finite package-by-target domain; a prefix-freeness
  check that, through the lemma `L-PREFIX`, settles collision-freedom for every
  version; `proptest` over versions with an independent oracle; archive
  integration tests through the real packager; harness tests of the new
  scripts; and an end-to-end check against the real `cargo-binstall`. The
  reasoning is in `Verification plan` §Formal verification decision.
  Date/Author: 2026-11-28, plan author, rationale corrected after review.
- D-9. Decision: record the release-artefact design in a new "Release
  artefacts" subsection of `docs/whitaker-cli-design.md` §Compatibility and
  migration, and add an entry to ADR 0012's existing `## Implementation note`
  section. No new ADR. Rationale: the decisions apply ADR 0012 and the 4.2.1
  and 4.3.1 patterns. The documentation style guide reserves ADRs for decisions
  of their own, and D-4 extends the order already documented in
  `docs/publishing.md:173-175`. Date/Author: 2026-11-28, plan author, confirmed
  by review.
- D-10. Decision: versions are a validated newtype, `ReleaseVersion`. It can
  only be built through `TryFrom<&str>`, which accepts a non-empty string over
  ASCII alphanumerics, `.`, `-`, and `+`. The naming functions and parameters
  take `&ReleaseVersion`, so no unchecked version reaches a path. Rationale:
  the version is interpolated into an output path and an archive member path.
  This character set contains every SemVer 2.0.0 string and excludes path
  separators and whitespace. A version such as `..` is still admissible, but it
  is harmless, because every name puts a package and target prefix before it
  and a separator never follows it. The rejected alternatives were a free
  validation function, which callers could forget, and the `semver` crate,
  which is not a dependency of `whitaker-installer`. Date/Author: 2026-11-28,
  plan author, revised after review.
- D-11. Decision: change `make package-root-cli` from
  `cargo package -p whitaker --no-verify` plus `patch.crates-io` overrides to
  one verified
  `cargo package --locked -p whitaker-common -p whitaker-installer -p whitaker`.
  Multi-package packaging resolves the siblings through a local staging
  registry. Rationale: this is the same verification `cargo publish` performs
  at release time, moved to pull requests, so a packaging failure in the root
  crate cannot first appear during a release. The cost is a verified build of
  three packaged crates in `linux-full`, bounded by the CI-cost tolerance. If
  the tolerance is breached, fall back to running it only on pull requests that
  touch a manifest or `src/`, and record the decision. Date/Author: 2026-11-28,
  plan author, from review; requires user approval.
- D-12. Decision: rename the Rust packaging surface now, in the same commit
  that changes every caller:
  - the module `installer_packaging` becomes `release_packaging`;
  - `InstallerPackageParams` becomes `ReleasePackageParams`;
  - `InstallerPackageOutput` becomes `ReleasePackageOutput`;
  - `InstallerPackagingError` becomes `ReleasePackagingError`, now
    `#[non_exhaustive]`;
  - `package_installer` becomes `package_release_binary`.

  Rationale: every call site is already edited to pass `ReleaseBinary`, and the
  API break is already forced into 0.4.0. One break with precise names is
  better than two. `#[non_exhaustive]` lets later variants arrive without
  another break. The packager binary keeps its name (D-2). Date/Author:
  2026-11-28, plan author, from review.
- D-13. Decision: review disposition. Four independent reviewers covered
  structure and contracts, failure modes and a pre-mortem, alternatives and
  viability, and verification and cost.
  - Accepted and applied: separate builds; structural lockstep including
    `whitaker-common`; a re-runnable multi-package publication; a version
    floor in place of "skip if absent"; `cargo metadata` version reads;
    standard-library-only scripts; `ReleaseBinary` in its own module with
    `bin_name()`; `ReleaseVersion`; renamed Rust items and
    `#[non_exhaustive]`.
  - Also accepted and applied: an oracle that takes `{name}`, `{bin}`, and
    override keys from the manifests; `L-PREFIX` in place of the incorrect
    "shared version" argument; non-vacuity mechanisms that work in proptest
    and Hypothesis; and globs extracted from `release.yml` and checked in
    Python.
  - Further applied: harness tests for the new release scripts; a per-target
    completeness gate in `publish`; an end-to-end check that asserts requested
    paths and binary hashes and installs a pre-release version; a Linux-only,
    path-narrowed, time-limited local job; a separate post-publication CLI
    job; `--version` output comparison; root-level placement of the
    root-manifest tests; and the full `[[bin]]` and `[features]` copy in
    synthesized manifests.
  - Considered and rejected: a v0.3.0 backfill (D-5); renaming the externally
    visible identifiers now (D-2); `cargo-dist`, which is a migration rather
    than a mirror and would change asset names and the glibc and
    dependency-binary gates; one combined archive holding both binaries,
    which breaks Constraint 2; and dropping the local HTTPS check in favour of
    a real pre-release tag, which would need a mutating release action per
    change and would give no pull-request-time signal.
  - Deferred to follow-up issues: the fail-closed download condition, the
    `--clobber` re-dispatch semantics, and the writer deduplication (D-7).

  Date/Author: 2026-11-28, plan author.

## Outcomes & retrospective

Not started. Populate at each milestone boundary and at completion.

## Context and orientation

Inspected base revision: `755c1f2cce7dafe1e9fb7ceda826e0854569538b` (`main`,
"Fold the Phase 17 DOMAIN amendment into the roadmap (#612)"). Line numbers
below refer to that revision; relocate symbols with `rg -n '<symbol>'`.

### Packages and executables

- Root package `whitaker` (`Cargo.toml:85-138`):
  - Its version is the literal `0.3.0`, and `[workspace.package]` also says
    `0.3.0` (`Cargo.toml:15`).
  - It has one binary, `whitaker`, from `src/main.rs`, with
    `autobins = false`.
  - It depends on the `whitaker-installer` library and on `ortho_config`
    0.11, and carries binstall metadata at `Cargo.toml:131-138`.
  - It is published on crates.io as 0.3.0.
- `whitaker-installer` (`installer/Cargo.toml`):
  - Its version is the literal `0.3.0`.
  - It has the user binary `whitaker-installer` and four release-only helper
    binaries behind `required-features = ["release-tools"]`, among them the
    packager `whitaker-package-installer`.
  - Its binstall metadata is at `installer/Cargo.toml:121-128`.
  - Its self dev-dependency enables `test-support` and `release-tools`, so
    `CARGO_BIN_EXE_whitaker-package-installer` is available to its
    integration tests (`installer/tests/behaviour_packaging_cli.rs:54`).
- `whitaker-common` (`common/Cargo.toml`) has the literal version `0.3.0`.
- `whitaker-cli` (`cli/`) is `publish = false`, inherits the workspace
  version, and is not involved.

Both binstall tables are identical apart from the package name:

```toml
[package.metadata.binstall]
pkg-url = "https://github.com/leynos/whitaker/releases/download/v{version}/{name}-{target}-v{version}.tgz"
bin-dir = "{name}-{target}-v{version}/{bin}{binary-ext}"
pkg-fmt = "tgz"

[package.metadata.binstall.overrides.x86_64-pc-windows-msvc]
pkg-url = "https://github.com/leynos/whitaker/releases/download/v{version}/{name}-{target}-v{version}.zip"
pkg-fmt = "zip"
```

### Packaging code

- `installer/src/installer_packaging.rs` (public module, `lib.rs:56`):
  - `CRATE_NAME` (line 31) is hard-coded to `"whitaker-installer"`.
  - Types: `ArchiveFormat`, `InstallerPackageParams` (whose `version` is a
    `Version`, an unvalidated `String` newtype from `installer/src/version.rs`),
    `InstallerPackageOutput`, and `InstallerPackagingError` (`Io`,
    `BinaryNotFound`, and `Zip`; not `#[non_exhaustive]`).
  - Pure naming functions: `archive_filename` (98), `inner_dir_name` (127),
    `binary_filename` (151), and `archive_format` (179).
  - The orchestrator `package_installer` (198), and the private writers
    `create_tgz_archive` (230) and `create_zip_archive` (253). The tgz writer
    uses `sparse(false)` so binstall sees a regular file.
  - Tests live in `installer_packaging_tests.rs` (329 lines) and
    `archive_entry_type_tests.rs`.
- `installer/src/bin/package_installer_bin.rs`. The packager CLI takes
  `--crate-version`, `--target`, `--binary-path`, and `--output-dir`, and prints
  `Created <path>`. Its tests are inline (lines 87-181), including a
  missing-argument table (lines 130-131) and `Cli` literals (lines 145 and 166).
- `installer/src/binstall_metadata.rs` (public) holds:
  - the template constants;
  - `BinstallRelease { package_name: &str, version: &str, target: &str }`;
  - `expand_pkg_url_for` and `expand_bin_dir_for`, plus the two-argument
    forms that hard-code `"whitaker-installer"` (lines 65-67 and 114-116);
  - the test-support loaders `load_cargo_toml_from` and
    `extract_binstall_table`.
- `installer/src/dependency_packaging.rs` re-exports only `ArchiveFormat` and
  `TargetTriple` from `installer_packaging` (line 16), and has its own naming
  and writers. It is not otherwise changed (D-7).
- Other references to update: `installer/tests/behaviour_installer_release.rs`
  and its feature file, `installer/tests/behaviour_packaging_cli.rs:241-252`,
  `tests/workflows/cargo_locked_stubs.py:47`,
  `tests/workflows/test_rolling_release_workflow.py:75-100`,
  `docs/whitaker-dylint-suite-design.md:1971`, and `docs/developers-guide.md`.
  Find any others with
  `rg -n 'installer_packaging|InstallerPackage|package_installer\b'`.

### Release workflow (`.github/workflows/release.yml`)

Triggers are `push` of `v*` tags and `workflow_dispatch` with a `tag` input.

- `build-installer` (lines 21-125): a five-target matrix on native runners.
  Each leg:
  - reads the installer version with `cargo metadata | jq`;
  - runs `cargo build -p whitaker-installer --release --target`;
  - builds the packager and packages into `dist/`;
  - checks the glibc baseline on x86_64 Linux;
  - runs `scripts/generate_checksums.py dist/`;
  - uploads the artefact `installer-<target>`.
- `build-dependency-binaries` (127-223) packages the Dylint tools.
- `release-compatibility` (225-309) runs only when the x86_64 artefacts were
  downloaded, and that download is conditioned on the whole matrix result. It
  extracts the archives, requires exactly one installer archive, checks glibc,
  and runs `--version`.
- `clippy-snapshot` (311-346).
- `publish` (348-527):
  - checks that the tag matches the installer version only (374-392);
  - downloads `installer-*` and `dependency-binaries-*`;
  - refuses an empty release and warns on missing targets;
  - uploads `dist/*.{tgz,zip,bundle,sha256,md}`, with `--clobber` on a
    re-run;
  - runs `cargo publish -p whitaker-common` and then `-p whitaker-installer`
    in one `set -e` block.
- `verify-binstall` (529-545) calls `.github/workflows/verify-binstall.yml`.
- `verify-published-bootstrap` (547-587).

`.github/workflows/verify-binstall.yml` has two jobs:

- `verify-binstall` runs after publication on three operating systems. It
  installs `whitaker-installer@<tag>` with `cargo-binstall` 1.16.6, with
  compile and quick-install disabled, then runs the bootstrap checks and uploads
  `installed-bootstrap-${{ matrix.os }}`.
- `verify-binstall-manifest` runs on pull requests that touch the metadata,
  on Linux and macOS, using `--manifest-path installer/Cargo.toml`.
  `scripts/check-release-published.sh` skips it when the release is absent.
  Neither job sets up Rust or `uv`.

`ci.yml` runs `make test-workflow-contracts`, `make package-root-cli`, and
`make publish-check` in `linux-full`, and `make install-smoke` and
`make release-installer-dry-run` in `windows-compat`. The dry run
(`Makefile:775-832`) builds and packages the installer for the host triple and
checks that an archive and a checksum exist.

### Tests and gates

- Rust test tools:
  - `rstest` 0.28 and `rstest-bdd` 0.6, with feature files under
    `installer/tests/features/`, including `installer_release.feature` and
    `binstall_metadata.feature`;
  - `proptest` 1 and `insta` 1;
  - `googletest` 0.14.3 and `pretty_assertions` 1.4.1, workspace
    dependencies already used by the root package and not yet by
    `whitaker-installer`.

  The root test `tests/behaviour_binstall.rs` checks the root table against the
  constants. New `installer/tests/*` binaries run under the nextest `default`
  profile, the coverage filter, and `ci`; the default profile skips only
  `behaviour_cli`, `behaviour_toolchain`, and examples.
- Python tests live in `tests/workflow_contracts/*_test.py`, run by
  `make test-workflow-contracts` in `linux-full` with `pytest`, `pyyaml`,
  `ruamel.yaml`, and `hypothesis`. Shared helpers:
  - `ubicloud_workflow_support.py` provides `load_workflow`, `load_job`, and
    `steps_by_name`;
  - `release_script_harness.py` runs real scripts under PATH stubs, records
    `calls.log`, and uses `FAKE_*` toggles.

  The release-relevant contract tests are `release_binstall_contract_test.py`,
  `check_release_published_test.py`, `root_package_contract_test.py`, and
  `helper_binaries_feature_test.py`.
- Gates:
  - `make check-fmt`;
  - `make lint`, which covers interrogate, skill manifests, rustdoc, Clippy,
    and the Whitaker suite;
  - `make test`, which runs nextest (CI uses `NEXTEST_PROFILE=ci`);
  - `make test-doc` and `make test-workflow-contracts`;
  - `make markdownlint`, which includes `make spelling`, and `make nixie`.

  Verus and Kani targets exist (`make verus`, `make kani`), but this plan adds
  no proofs (D-8).

### Signposts

Documentation:

- `docs/whitaker-cli-design.md`: §Public CLI surface, §`whitaker check`, and
  §Compatibility and migration.
- `docs/adrs/adr-0012-whitaker-cli-boundary.md`, and
  `docs/adrs/adr-0001-prebuilt-dylint-library-distribution.md` for the glibc
  baseline.
- `docs/publishing.md`, `docs/users-guide.md`, and `docs/migrations/0.4.0.md`.
- `docs/developers-guide.md`: helper binaries and `verify-binstall`.
- `docs/whitaker-dylint-suite-design.md`.
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/scripting-standards.md` and `docs/documentation-style-guide.md`.

`docs/whitaker-clone-detector-design.md` has no bearing on this item.

Skills: `rust-router`, then:

- `rust-unit-testing` for fixtures, tables, and `googletest`;
- `proptest`, `rust-types-and-apis` for the newtype and enum, and
  `rust-errors` for the renamed error;
- `domain-cli-and-daemons` for the packager CLI, and `nextest`;
- `python-testing` and `hypothesis` for the script tests;
- `pr-creation` and `commit-message`.

Use `codegraph-mcp` or `leta` for call-site discovery.

## Conformance basis

Upstream artefacts, at revision `755c1f2c`:

- `CLI-DESIGN`: `docs/whitaker-cli-design.md`.
  - §Public CLI surface: the root package owns the real executable, and
    `whitaker-installer` remains functional.
  - §`whitaker check`: the workflow is `cargo binstall whitaker`, then
    `whitaker check`.
  - §Compatibility and migration, step 1: copy the working binstall metadata
    pattern from `whitaker-installer` onto `whitaker`.
- `ADR-0012`: `docs/adrs/adr-0012-whitaker-cli-boundary.md`, accepted
  2026-10-17. §Decision outcome says GitHub release archives for the root
  executable are not published until 3.5.3, and §Known risks and limitations
  lists their absence.
- `ADR-0001`: the glibc 2.35 baseline for x86_64 Linux release binaries.
- `ROADMAP-3.5.3`, and its prerequisites 4.2.1 (binstall metadata) and 4.3.1
  (release workflow).
- No Terms of Reference document exists for this project; none is invented.

Requirements derived for this item:

- `REQ-ARCHIVES`: every release `v<V>` with `V ≥ 0.4.0` that `publish`
  uploads carries, for each target present, both
  `whitaker-installer-<target>-v<V>` and `whitaker-<target>-v<V>` archives
  (`.zip` for `x86_64-pc-windows-msvc`, otherwise `.tgz`), each with a
  `.sha256` sidecar. Each `whitaker` archive contains exactly one regular file,
  `whitaker-<target>-v<V>/whitaker` (`whitaker.exe` on Windows). A failed leg
  blocks publication, which is the existing fail-closed behaviour.
- `REQ-BINSTALL`:
  `cargo binstall --disable-strategies compile,quick-install whitaker@<V>`
  installs that binary on Linux, macOS, and Windows.
- `REQ-MIRROR`: the `whitaker` archives use the installer's targets,
  runners, packager, checksum step, glibc check, publication step, and
  verification gates.
- `REQ-LOCKSTEP`: the three crates share the workspace version, and a release
  proceeds only when the tag is `v` followed by it.
- `REQ-CRATES`: the release workflow publishes the three crates through a
  verified, re-runnable multi-package publication (D-4).
- `REQ-INSTALLER-UNCHANGED`: Constraint 2.
- `REQ-DOCS`: user, publishing, developer, design, migration, and ADR
  documentation describe the shipped state.

Observed gaps:

- `GAP-PACKAGER`: `CRATE_NAME` is hard-coded (`installer_packaging.rs:31`),
  and versions are unvalidated.
- `GAP-BUILD`: no workflow builds or packages `-p whitaker`.
- `GAP-COMPAT`: `release-compatibility` neither requires nor runs a
  `whitaker` archive, and `publish` checks no per-target completeness.
- `GAP-LOCKSTEP`: versions are literals, and `publish` checks only the
  installer's.
- `GAP-PUBLISH`: `release.yml` does not publish `whitaker`, and a partial
  publication cannot be completed by re-running.
- `GAP-PACKAGE-VERIFY`: the root crate is packaged with `--no-verify` on
  pull requests.
- `GAP-VERIFY`: neither binstall verification job covers `whitaker`.
- `GAP-DOCS`: `docs/users-guide.md:66-70` and `:316-322`,
  `docs/publishing.md:162-166`, and ADR 0012 (§Decision outcome, §Known risks
  and limitations) describe archives as pending.

Trace links:

```plaintext
CLI-DESIGN §whitaker check -> REQ-BINSTALL -> EP-M3
  -> verify_binstall_local.py (CI: verify-binstall-local) + verify-binstall-cli job
CLI-DESIGN §Compat step 1 -> REQ-MIRROR -> EP-M1, EP-M2
  -> release_cli_artefacts_contract_test.py + make release-installer-dry-run
ADR-0012 §Decision outcome -> REQ-ARCHIVES, GAP-BUILD, GAP-COMPAT -> EP-M2
  -> build-installer whitaker steps + release_assets.py check-complete + INV-NAME, INV-MEMBER
ADR-0001 glibc baseline -> REQ-MIRROR -> EP-M2 -> release-compatibility (whitaker glibc + --version)
publishing.md lockstep -> REQ-LOCKSTEP, GAP-LOCKSTEP -> EP-M2 -> INV-LOCKSTEP contract tests
D-4 -> REQ-CRATES, GAP-PUBLISH -> EP-M2 -> publish_crates.py harness tests
D-11 -> GAP-PACKAGE-VERIFY -> EP-M2 -> make package-root-cli (verified)
D-6 -> GAP-VERIFY -> EP-M3 -> release_assets.py cli-archives-expected tests
GAP-PACKAGER -> EP-M1 -> INV-NAME, INV-MEMBER, INV-NAMESPACE, INV-VERSION-SAFE
GAP-DOCS -> REQ-DOCS -> EP-M4 -> make markdownlint, make nixie, stale-claim search
```

## Verification plan

### Abstract model

The model uses these sets:

- `P = {whitaker, whitaker-installer}`, the release packages.
- `T`, the five supported targets, and `W ⊂ T`, the singleton
  `{x86_64-pc-windows-msvc}`.
- Admissible release versions: non-empty strings over `[0-9A-Za-z.+-]`.
- `D`, the dependency-binary packages from `installer/dependency-binaries.toml`
  (`cargo-dylint` and `dylint-link`). Their versions are their own, not the
  release version.

The functions are:

- `ext(t)`: `zip` if `t ∈ W`, else `tgz`.
- `exe(t)`: `.exe` if `t ∈ W`, else empty.
- `pre(p, t) = p ++ "-" ++ t ++ "-v"`, the version-free prefix.
- `asset(p, t, v) = pre(p, t) ++ v ++ "." ++ ext(t)`.
- `member(p, t, v) = pre(p, t) ++ v ++ "/" ++ p ++ exe(t)`.
- Dependency assets have the prefix `pre(d, t)` for `d ∈ D`. The Clippy
  snapshot bundle has the prefix `whitaker-clippy-snapshot-`.

The specification side is binstall's view. Write `R(s, n, b, t, v)` for the
result of literally substituting, in the template string `s`:

- `{name}` by `n`, the manifest's `[package].name`;
- `{bin}` by `b`, the manifest's `[[bin]].name`;
- `{version}` by `v` and `{target}` by `t`;
- `{binary-ext}` by `exe(t)`.

Write `url_m(t)` for manifest `m`'s `pkg-url`, using the override whose key
equals `t` when one exists, and `bindir_m` for its `bin-dir`. Binstall downloads
`R(url_m(t), …)` and expects `R(bindir_m, …)` inside it as a regular file.

The implementation side is the packager: `package(p, t, v, bytes)` writes one
archive named `archive_filename(p, v, t)`, containing one entry at
`inner_dir_name(p, v, t) ++ "/" ++ binary_filename(p, t)`.

A release under tag `g` proceeds only when `g = "v" ++ V`, where `V` is the
workspace version that all three crates inherit.

### Invariants and lemmas

- `INV-NAME` (naming correspondence; `REQ-ARCHIVES`, `REQ-BINSTALL`): for
  every `p ∈ P`, every `t ∈ T`, and every admissible `v`, with `m` the manifest
  paired with `p`:
  - `archive_filename(p, v, t)` equals the last path segment of
    `R(url_m(t), name_m, bin_m, t, v)`;
  - that URL's prefix is
    `https://github.com/leynos/whitaker/releases/download/v` followed by `v`
    and `/`;
  - the packaged entry path equals `R(bindir_m, name_m, bin_m, t, v)`;
  - `ReleaseBinary::package_name(p) = name_m` and
    `ReleaseBinary::bin_name(p) = bin_m`.
- `INV-MEMBER` (archive layout; `REQ-ARCHIVES`): every packaged archive holds
  exactly one entry. That entry is a regular file at `member(p, t, v)`, and its
  bytes equal the input binary's. On `tgz` it also keeps the input's executable
  mode bits.
- `L-PREFIX` (lemma): let `S` be a finite set of strings in which no element
  is a proper prefix of another. If `x ++ y = x' ++ y'` with `x, x' ∈ S`, then
  `x = x'` and `y = y'`.

  Proof sketch: one of `x` and `x'` is a prefix of the other, because both are
  prefixes of the same string. Prefix-freeness then forces equality, and
  cancelling the common prefix gives `y = y'`. The lemma is standard and short.
  It is stated here because it turns a finite check into a guarantee for all
  versions.
- `INV-NAMESPACE` (no clobbering; `REQ-ARCHIVES`, `REQ-INSTALLER-UNCHANGED`):
  the set of version-free prefixes is prefix-free. That set is
  `{pre(p, t) | p ∈ P, t ∈ T}`, together with `{pre(d, t) | d ∈ D, t ∈ T}` and
  the bundle prefix. By `L-PREFIX`, assets from different `(package, target)`
  pairs never collide, whatever versions they carry. Within one pair, the suffix
  `v ++ "." ++ ext(t)` determines `v`. Separately, every glob that
  `release.yml` uses to select one package's archive matches only that
  package's archive.
- `INV-INSTALLER-STABLE` (`REQ-INSTALLER-UNCHANGED`): for
  `p = whitaker-installer`, the naming functions return exactly what the
  pre-change functions returned. Separately, the release leg's installer build
  command names no package other than `whitaker-installer`.
- `INV-VERSION-SAFE` (D-10): `ReleaseVersion::try_from` accepts exactly the
  admissible strings. It accepts every SemVer 2.0.0 string and rejects every
  string that is empty or contains any other character.
- `INV-LOCKSTEP` (`REQ-LOCKSTEP`):
  - The three manifests inherit `version.workspace`.
  - The `publish` job and every build leg compare the tag with each of the
    three packages' `cargo metadata` versions, and fail on any difference or
    on a tag without a leading `v`.
- `INV-RELEASE-COMPLETE` (`REQ-ARCHIVES`, `REQ-MIRROR`):
  - `release_assets.py check-complete dist/ --version V` fails unless every
    target that has any archive has both packages' archives and all their
    sidecars, and every sidecar verifies against its archive.
  - `release-compatibility` requires exactly one
    `whitaker-x86_64-unknown-linux-gnu-v*.tgz` whenever any x86_64 artefact
    is present. It checks that binary against `GLIBC_2.35`, and its
    `--version` output must equal `whitaker V`.
- `INV-PUBLISH-RERUN` (`REQ-CRATES`): `publish_crates.py` publishes exactly
  the named packages whose `<name>/<version>` the registry does not have. If
  every package is already published, it exits 0 without calling `cargo`. Any
  registry answer other than 200 or 404 fails the run without publishing.
- `INV-CLI-FLOOR` (D-6): `release_assets.py cli-archives-expected V` prints
  `expected=false` exactly when `V < 0.4.0` under SemVer precedence.
- `L-SUBST` (lemma connecting `INV-NAME` to `REQ-BINSTALL`): suppose the
  manifest's binstall strings are the ones `INV-NAME` renders, and binstall
  renders by literal substitution (`AXIOM-BINSTALL-SUBST`). Then binstall
  requests exactly `asset(p, t, v)` and looks for exactly `member(p, t, v)`.
  `INV-E2E` checks this against the real tool by recording the requested paths.
- `INV-E2E` (`REQ-BINSTALL`, before publication): the real `cargo-binstall`,
  at the `EP-M0` version, installs each `p ∈ P` for the host target when:
  - the archives were produced by the real packager from real builds;
  - each manifest is synthesized from the real one, with its `[package]`
    name, version, and `autobins`, all `[[bin]]` entries, `[features]`, and
    the binstall table copied verbatim, and only the URL origin
    `https://github.com/leynos/whitaker/releases/download/` replaced by a
    local HTTPS origin;
  - compile and quick-install are disabled.

  The server must record exactly one successful GET of
  `/v<v>/asset(p, host, v)`, and the installed binary's SHA-256 must equal the
  packaged binary's.
- `INV-PUBLISHED` (`REQ-BINSTALL`, after publication): for tags at or above
  0.4.0, `verify-binstall-cli` installs `whitaker@<tag>` from GitHub and
  crates.io on Linux, macOS, and Windows. The installed `whitaker --version`
  must print `whitaker <V>`.

### Axioms (trusted, not verified here)

- `AXIOM-BINSTALL-SUBST`: binstall renders `pkg-url` and `bin-dir` by literal
  substitution of the placeholders above, and an exact-target override replaces
  the base keys for that target (`cargo-binstall` `SUPPORT.md`). Boundary: both
  manifests. `INV-E2E` exercises it against the real tool.
- `AXIOM-BINSTALL-VERSIONED`: without `--manifest-path`, binstall reads the
  binstall table from the crate manifest of the resolved version on crates.io.
  Boundary: D-4 and Constraint 3.
- `AXIOM-BINSTALL-REGULAR`: binstall skips tar entries that are neither
  regular files nor directories (#461). Boundary: the tgz writer.
- `AXIOM-BINSTALL-HTTPS`: binstall refuses `http://` package URLs and accepts
  `https://` with an extra trust root from `--root-certificates`. Observed with
  1.22.0; `EP-M0` confirms it for 1.16.6. Boundary:
  `scripts/verify_binstall_local.py`.
- `AXIOM-CARGO-MULTI`: `cargo publish` and `cargo package` with several `-p`
  flags package and verify every named crate against a local staging registry
  before uploading any, then upload in dependency order and wait for each to
  appear in the index. Stable since Cargo 1.90. Boundary: D-4 and D-11. `EP-M2`
  exercises the local packaging half.
- `AXIOM-CARGO-FEATURES`: resolver 2 unifies features across the packages
  named in one build command. This is observed (`Artefacts and notes`), and the
  reason behind Constraint 2.
- `AXIOM-GH-ASSETS`: a GitHub release's asset names form one flat namespace,
  and `gh release upload --clobber` replaces a same-named asset.
- `AXIOM-CRATES-API`: `GET https://crates.io/api/v1/crates/<name>/<version>`
  returns 200 for a published version and 404 for an unpublished one, and
  requires a `User-Agent` header.
- `AXIOM-RUNNER-GLIBC`: `ubuntu-22.04` provides glibc 2.35.

### Methods per obligation

#### `INV-NAME`

- Method: an exhaustive `rstest` table over all ten `(p, t)` pairs, plus a
  `proptest` property over versions.
- Rationale: `P × T` is finite and closed, so enumerating it is complete. The
  version enters only by concatenation, and a broad generator with an
  independent oracle exercises that.
- Domain: `P × T` exhaustively. Versions come from a `prop_oneof!` with one
  arm per class: plain `MAJOR.MINOR.PATCH` with components up to 99999,
  pre-release, build metadata, and both together. Fixed rows `0.0.0`, `0.4.0`,
  and `10.20.30-rc.1+build.5` are added.
- Oracle: a test-local renderer that reads, through `load_cargo_toml_from`:
  - `[package].name`, `pkg-url`, `bin-dir`, and the `overrides` keys from
    the real root and installer manifests;
  - the `[[bin]].name` of the one `[[bin]]` entry without
    `required-features`, which is the user-facing binary (the installer's
    four helper binaries all require `release-tools`);
  - then applies `str::replace` per `L-SUBST`.

  The test literals are the pairing between the manifest and the enum
  (`Whitaker ↔ Cargo.toml`, `WhitakerInstaller ↔ installer/Cargo.toml`) and the
  `exe(t)` table. The oracle never calls `ReleaseBinary::package_name`,
  `expand_*`, or the packager.
- Artefact: `tests/release_naming_properties.rs`, a new file in the root
  package next to `tests/behaviour_binstall.rs`, so no installer test reads
  `../Cargo.toml`.
- Evidence: `cargo nextest run -p whitaker --test release_naming_properties`.
  Red: it does not compile until `ReleaseBinary` and `ReleaseVersion` exist.
  Green: 10 table rows pass, plus 256 property cases for each class arm.
- Non-vacuity: every class arm is its own `proptest!` test, so no class can
  go unexercised. Seeded faults, run once each, recorded, and reverted:
  - map `Whitaker` to `"whitaker-cli"` in `package_name`;
  - drop `.exe` from `binary_filename`;
  - swap `ext` for Windows;
  - restore a hard-coded `"whitaker-installer"`.

  Each must fail a `whitaker` row or a Windows row.

#### `INV-MEMBER`

- Method: integration tests through the real packager binary
  (`CARGO_BIN_EXE_whitaker-package-installer`) for all ten pairs, plus
  `rstest-bdd` scenarios.
- Artefacts:
  - new cases in `installer/tests/behaviour_packaging_cli.rs`, split into a
    sibling `installer/tests/behaviour_release_packaging_cli.rs` if the file
    would pass 400 lines;
  - new scenarios in `installer/tests/features/installer_release.feature`,
    with steps in `installer/tests/behaviour_installer_release.rs`;
  - `installer/src/archive_entry_type_tests.rs`, parametrized over `p`.
- Evidence: red until `--package` exists (clap rejects the unknown argument
  with exit 2). Green: each archive has exactly one entry, a regular file with
  the expected path and bytes, asserted with `googletest`'s `elements_are!` and
  `eq`.
- Non-vacuity: the fake binary has distinctive content and mode `0o755`. A
  seeded fault must fail: writing the `whitaker` entry under the installer's
  directory. Re-enabling sparse entries must also fail, for both packages.

#### `INV-NAMESPACE`

- Method: an exhaustive prefix-freeness check in Rust, and a glob check in
  Python.
- Rust artefact: `tests/release_naming_properties.rs`. It computes every
  `pre(p, t)` by stripping `v ++ "." ++ ext(t)` from `archive_filename` at a
  fixed version, after first asserting the name ends with that suffix. It
  computes every `pre(d, t)` from `dependency_binaries::parse_manifest` over
  the real `installer/dependency-binaries.toml`, adds the bundle prefix, and
  asserts that no element is a proper prefix of another and that all are
  distinct.
- Python artefact:
  `tests/workflow_contracts/release_cli_artefacts_contract_test.py`. It
  extracts every `dist/...` glob from `release.yml` through `load_workflow`.
  Using `fnmatch.fnmatchcase`, it asserts that each matches exactly the
  intended package's names for all five targets, at version `0.4.0` and at
  `0.4.0-rc.1+b.5`.
- Non-vacuity:
  - Positive control: the prefix check must reject the set
    `{"whitaker-", "whitaker-installer-"}`, which is not prefix-free.
  - The glob check must report that the naive glob
    `whitaker-*-x86_64-unknown-linux-gnu-v*.tgz` matches both packages.
  - Seeded fault: a target string starting with `installer-` must fail the
    prefix check.

#### `INV-INSTALLER-STABLE`

- Method: the existing tests in `installer_packaging_tests.rs` (renamed with
  the module), `binstall_metadata_tests.rs`,
  `installer/tests/behaviour_binstall.rs`, and `installer_release.feature` stay
  green, with call sites changed only to pass
  `ReleaseBinary::WhitakerInstaller`.
- Extra snapshot: an `insta` snapshot taken on the unmodified tree in the red
  step records the tuple
  `(archive_filename, inner_dir_name, binary_filename, archive_format)` for
  `whitaker-installer` across the five targets at `0.3.0`, and is asserted
  after the refactor.
- Contract test: `release_cli_artefacts_contract_test.py` asserts that the
  build leg's installer `cargo build` command names only
  `-p whitaker-installer`. A refusal case injects `-p whitaker` and expects a
  named violation.
- Non-vacuity: the snapshot exists before the change, so any change to an
  installer name fails it.

#### `INV-VERSION-SAFE`

- Method: `proptest`, plus an `rstest` table with one row per rejection
  class.
- Properties: generated SemVer strings (the `INV-NAME` arms) are accepted.
  For rejection, one `proptest!` test per bad class inserts a character from
  that class at a random position in a valid version: `/`, `\`, ASCII
  whitespace, ASCII punctuation outside `.+-`, and non-ASCII.
- Table: `""`, `"../0.4.0"`, `"0.4.0/x"`, `"0.4.0\\x"`, and `"0.4.0 "` are
  rejected, and `".."` is accepted (D-10).
- Artefact: `installer/src/release_version_tests.rs`, beside the new
  `installer/src/release_version.rs`.
- CLI evidence: `--crate-version ../0.4.0` exits 1 with
  `error: invalid release version "../0.4.0": …`, and the output directory
  stays empty.

#### `INV-LOCKSTEP`

- Method: Python contract tests.
- Checks:
  - parse the three manifests with `tomllib` and require
    `version = { workspace = true }`;
  - require that `publish`'s tag check and every build leg's version step
    name all three packages;
  - refusal cases drop one package from the check, and restore a literal
    version in a copied manifest.
- Artefact: `release_cli_artefacts_contract_test.py`.

#### `INV-RELEASE-COMPLETE` and `INV-CLI-FLOOR`

- Method: `pytest` over `scripts/release_assets.py`'s pure functions, plus
  subprocess runs against a scratch `dist/` with fake archives and sidecars.
- `check-complete` cases:
  - a complete set passes;
  - a missing `whitaker` archive, a missing sidecar, and a sidecar whose
    digest does not match its archive each fail with a named reason;
  - a target with neither package passes.
- `cli-archives-expected` cases: `0.3.0`, `0.3.9`, and `0.4.0-rc.1` give
  `false`; `0.4.0`, `0.4.1`, and `1.0.0` give `true`.
- Hypothesis property: generated SemVer pairs agree with a reference
  comparator written in the test from the SemVer precedence rules.
- `release-compatibility` glob logic: covered by `INV-NAMESPACE`'s extraction
  test. The step itself calls `release_assets.py check-archive` for the
  `whitaker` archive's single member.
- Artefacts: `scripts/release_assets.py` and
  `tests/workflow_contracts/release_assets_test.py`.
- Non-vacuity: a Hypothesis strategy draws `equal = st.booleans()` and builds
  equal or unequal pairs from it. `@example` pins one of each.

#### `INV-PUBLISH-RERUN`

- Method: harness tests of `scripts/publish_crates.py`. They follow
  `release_script_harness.py`'s pattern, with stub `cargo` and a local HTTP
  server standing in for the crates.io API, whose URL is overridden by a
  `--registry-api` option that defaults to crates.io.
- Cases:
  - none published: one `cargo publish --locked -p whitaker-common -p
    whitaker-installer -p whitaker`;
  - `whitaker-common` published: `cargo` is called with the other two;
  - all published: no `cargo` call, exit 0;
  - a 500 or a network error: exit 1, no `cargo` call;
  - a missing `User-Agent`: the stub server answers 403, which also fails.
- Artefact: `tests/workflow_contracts/publish_crates_test.py`.
- Non-vacuity: `calls.log` must contain exactly the expected argument vector,
  checked in order.

#### `INV-E2E`

- Method: `scripts/verify_binstall_local.py`, standard library only, run in a
  new `verify-binstall-local` job.
- What the script does:
  - takes `--dist`, the two manifest paths, and an optional `--binstall`;
  - refuses a `--dist` that does not contain exactly the expected archives
    and sidecars;
  - synthesizes the manifests as described in `INV-E2E`;
  - starts a threaded `http.server` on `127.0.0.1:0` (an ephemeral port),
    wrapped in TLS with a throwaway certificate (`openssl req -x509` with
    `subjectAltName=IP:127.0.0.1` and `basicConstraints=critical,CA:FALSE`),
    serving `dist/` under `v<version>/` and logging every request path;
  - runs `cargo binstall` with `--no-confirm`, `--no-track`,
    `--disable-strategies compile,quick-install`,
    `--root-certificates <cert>`, `--manifest-path <synth>`, `--root <tmp>`,
    `--install-path <tmp>/bin`, and `--targets <host>` for `<p>@<v>`;
  - asserts the request log and the binary hash.

  A `try`/`finally` removes the temporary directory and key.
- A second run repackages the same binaries under version
  `0.4.0-rc.1+b.5`, so `+` and `-` pass through binstall's rendering. It
  asserts install success and the requested path, not `--version`.
- Pure parts (manifest synthesis, origin rewrite, request-log assertions)
  are tested in `tests/workflow_contracts/verify_binstall_local_test.py`.
- Non-vacuity: `--negative-control` corrupts `bin-dir` in the synthesized
  manifest. It requires binstall to exit non-zero (94 in the planning probe)
  after the server has logged a successful GET of the right asset, so the
  failure is shown to be about layout, not TLS or reachability. The CI job runs
  the negative control after the positive run.
- Job: Linux only, with `timeout-minutes: 30`, `setup-rust` with its
  sccache, and `setup-uv`. It builds the dev profile, because naming and layout
  do not depend on the build profile. It runs on pull requests that touch
  `installer/src/release_*.rs`, `installer/src/bin/package_installer_bin.rs`,
  `installer/src/binstall_metadata.rs`, either manifest, `verify-binstall.yml`,
  or `scripts/verify_binstall_local.py`. It does not run on Makefile changes.
- Residual gap: macOS and Windows are not covered before publication.
  `INV-NAME` and `INV-MEMBER` cover their naming and layout on every host, and
  `INV-PUBLISHED` covers their binstall resolution.

#### `INV-PUBLISHED`

- Method: a separate `verify-binstall-cli` job in `verify-binstall.yml` with
  the same three-OS matrix and `cargo-binstall` pin.
- It first asks `release_assets.py cli-archives-expected` and skips with a
  notice below 0.4.0. Otherwise it installs `whitaker@<V>` and compares
  `whitaker --version` with `whitaker <V>`.
- `release.yml` calls it after `publish`, as it already calls
  `verify-binstall`. The installer's `verify-binstall` job and its
  `installed-bootstrap-*` artefacts are not touched.
- `verify-binstall-manifest` gains a root cell, gated the same way. It reads
  the version from `[workspace.package]` with `tomllib`, after asserting that
  the package inherits it. `cargo metadata` would make rustup install the
  pinned nightly on a runner without Rust set up.
- Evidence: available only with the 0.4.0 tag. Record the run URL in
  `Outcomes & retrospective` when it exists. Until then the obligation stays
  open, and the PR description says so.

### Formal verification decision

No obligation here warrants Verus or Kani, and none is added (D-8).

The contractual logic consists of three things:

- a naming function over a closed, finite domain (two packages by five
  targets), concatenated with an unbounded version string;
- a version floor;
- a set difference over three crate names.

The unbounded part of the naming argument is collision-freedom for all
versions, including the dependency assets, which carry their own versions. That
is settled by `L-PREFIX`, a three-line lemma, together with an exhaustive
finite check of its premise (prefix-freeness of about twenty strings). The
lemma is standard, and its premise is decided by enumeration.

A Verus development would restate `L-PREFIX` over `Seq<char>` and assume a
model of `format!` and of binstall's renderer. It would add proof plumbing
without checking any code the release actually runs, and it would leave the
real boundary unverified: binstall's renderer, archive extraction, and GitHub
hosting. `INV-E2E` covers that boundary.

The naming correspondence is two procedures over the same ten-element domain,
and the exhaustive table discharges it completely for fixed versions. The
version enters only by concatenation, which the class-partitioned property
exercises against an oracle that is independent of the code. Kani adds nothing
over an exhaustive table for ten elements, and symbolic strings would make it
slow.

The 3.5.1 plan reached the same conclusion for a similar injectivity proof.
This plan differs in one respect: collision-freedom is load-bearing here,
because of `--clobber`, and that is why `L-PREFIX` and its finite check are
stated and tested explicitly.

If implementation exposes an obligation that genuinely ranges over an unbounded
structure the tests cannot enumerate, the plan returns to this section before
continuing.

### Suggested BDD features

These are proposed additions to
`installer/tests/features/installer_release.feature`, run by
`installer/tests/behaviour_installer_release.rs` (`rstest-bdd` 0.6). They trace
to `REQ-ARCHIVES`, `INV-NAME`, `INV-MEMBER`, `INV-VERSION-SAFE`, and `EP-M1`:

```gherkin
  Scenario: Root CLI archive filename uses tgz for Linux target
    Given package "whitaker", version "0.4.0" and target "x86_64-unknown-linux-gnu"
    When the archive filename is computed
    Then the archive filename is "whitaker-x86_64-unknown-linux-gnu-v0.4.0.tgz"

  Scenario: Root CLI Windows archive contains the exe binary
    Given package "whitaker", version "0.4.0" and target "x86_64-pc-windows-msvc"
    And a fake binary exists
    When the binary is packaged
    Then the archive contains "whitaker-x86_64-pc-windows-msvc-v0.4.0/whitaker.exe"

  Scenario: Installer archive is unchanged by the root CLI support
    Given package "whitaker-installer", version "0.4.0" and target "aarch64-apple-darwin"
    And a fake binary exists
    When the binary is packaged
    Then the archive contains "whitaker-installer-aarch64-apple-darwin-v0.4.0/whitaker-installer"

  Scenario: A version that escapes the output directory is refused
    Given package "whitaker" and target "x86_64-unknown-linux-gnu"
    When the release version "../0.4.0" is parsed
    Then an invalid release version error names "../0.4.0"
```

The existing scenarios change their `Given` wording to name
`"whitaker-installer"` explicitly. Their expected strings do not change. The
binstall correspondence for the root manifest is asserted in the root-package
property test, not in this installer feature file, so the installer package
does not read the workspace root manifest.

The end-to-end check is exercised by `verify-binstall-local` rather than an
`rstest-bdd` runner. Its proposed acceptance scenarios are:

```gherkin
Feature: Installing the root CLI from its release archive
  Scenario: cargo-binstall installs whitaker from the packaged archive
    Given the release archives for the host target are served over HTTPS
    And a manifest carrying the root package's binstall table
    When cargo binstall installs "whitaker" with compile and quick-install disabled
    Then the server logged one GET of the whitaker archive for the host target
    And the installed binary is byte-identical to the packaged binary

  Scenario: a broken bin-dir cannot fall back to a source build
    Given the same archives and a manifest whose bin-dir names a missing path
    When cargo binstall installs "whitaker" with compile and quick-install disabled
    Then the server logged a successful GET of the whitaker archive
    And cargo binstall exits non-zero
```

## Plan of work

### `EP-M0` feasibility probe (prototype, deleted afterwards)

Questions:

1. Does `cargo-binstall` 1.16.6 accept `--root-certificates` and install from
   a synthesized manifest whose package URL points at `https://127.0.0.1`?
2. What exactly does `whitaker --version` print?
3. Does
   `cargo package --locked -p whitaker-common -p whitaker-installer -p whitaker`
   verify on this tree, and how long does it take?

Steps:

- Install 1.16.6 into a scratch root:
  `cargo binstall --root /tmp/binstall-1.16.6 cargo-binstall@1.16.6`. Never
  install into `~/.cargo`.
- Rerun the planning probe from `Artefacts and notes` with that binary,
  including the negative control.
- Run `cargo run -q -p whitaker -- --version`.
- Run the multi-package `cargo package` with `tee` to `/tmp`, and record the
  wall time.

Go if the positive probe installs, the negative control exits non-zero after a
successful GET, and packaging verifies. If 1.16.6 lacks the flag, pin the local
check to the oldest version that passes and record a decision; the
`verify-binstall` pin does not change. If multi-package packaging fails, record
why and escalate D-11 before `EP-M2`. Scratch files live under `/tmp` and are
deleted.

### `EP-M1` the packaging library and packager package either executable

Assigned: `GAP-PACKAGER`, `INV-NAME`, `INV-MEMBER`, `INV-NAMESPACE`,
`INV-INSTALLER-STABLE`, `INV-VERSION-SAFE`; D-2, D-3, D-10, D-12.

Red:

1. On the unmodified tree, add and accept the `insta` snapshot for
   `INV-INSTALLER-STABLE`. It passes now and guards the refactor. Commit it on
   its own.
2. Add the following, then record the failures. They fail to compile, or
   fail because the types and the flag do not exist.
   - `tests/release_naming_properties.rs`;
   - `installer/src/release_version_tests.rs`;
   - the new BDD scenarios and steps;
   - the parametrized packager CLI cases;
   - a `missing_package` row in the packager's missing-argument table.

Green:

1. Add `installer/src/release_binary.rs`:

   ```rust
   //! The executables the release workflow packages for cargo-binstall.

   /// An executable that the release workflow packages for cargo-binstall.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
   pub enum ReleaseBinary {
       /// The root `whitaker` command-line interface.
       Whitaker,
       /// The legacy `whitaker-installer` executable.
       WhitakerInstaller,
   }

   impl ReleaseBinary {
       /// Every release executable, in release order.
       pub const ALL: [Self; 2] = [Self::WhitakerInstaller, Self::Whitaker];
       /// Return the crates.io package name.
       #[must_use]
       pub const fn package_name(self) -> &'static str { /* … */ }
       /// Return the Cargo binary target name.
       #[must_use]
       pub const fn bin_name(self) -> &'static str { /* … */ }
       /// Describe this executable's release for binstall template expansion.
       #[must_use]
       pub fn binstall_release<'a>(self, version: &'a ReleaseVersion, target: &'a TargetTriple)
           -> BinstallRelease<'a> { /* … */ }
   }
   ```

   A unit test asserts that the `clap` value names are exactly `whitaker` and
   `whitaker-installer`.
2. Add `installer/src/release_version.rs` with `ReleaseVersion`. It is
   validated through `TryFrom<&str>` and `TryFrom<String>`, provides
   `as_str()`, implements `Display` and `AsRef<str>`, and reports failure as
   `InvalidReleaseVersion { version: String, reason: &'static str }`
   (`thiserror`).
3. Rename `installer_packaging.rs` to `release_packaging.rs`, and
   `installer_packaging_tests.rs` to `release_packaging_tests.rs`, with
   `git mv`. Then apply D-12's renames:
   - remove `CRATE_NAME`;
   - thread `ReleaseBinary` and `&ReleaseVersion` through the naming
     functions;
   - add `pub binary: ReleaseBinary` to `ReleasePackageParams`, and make its
     `version` a `ReleaseVersion`;
   - mark `ReleasePackagingError` `#[non_exhaustive]`.

   Update every caller: `lib.rs`, `dependency_packaging.rs:16`,
   `archive_entry_type_tests.rs`, `behaviour_installer_release.rs`,
   `behaviour_packaging_cli.rs`, the packager binary and its tests, the
   doctests, and anything else `rg` finds.
4. In the packager, add a required
   `#[arg(long, value_enum)] package: ReleaseBinary`. Parse `--crate-version`
   into `ReleaseVersion` and add a `CliError::Version` variant.
5. Update the two existing callers in the same commit, with no behaviour
   change: `release.yml` "Package archive" and the Makefile
   `release-installer-dry-run` both pass `--package whitaker-installer`. Update
   `tests/workflows/cargo_locked_stubs.py` and
   `tests/workflows/test_rolling_release_workflow.py` to match.
6. Add `googletest` and `pretty_assertions` to `[dev-dependencies]` in
   `installer/Cargo.toml` as workspace references.

Refactor: widen the module, binary, and function documentation (Constraint 8),
keep every file under 400 lines, and make `binstall_metadata`'s two-argument
`expand_*` forms delegate through `ReleaseBinary::WhitakerInstaller` rather
than a string literal.

Focused commands:

```sh
cargo nextest run -p whitaker-installer --all-features \
  -E 'test(release_packaging) | test(archive_entry_type) | test(release_version)
      | test(release_binary) | binary(behaviour_installer_release)
      | binary(behaviour_packaging_cli)'
cargo nextest run -p whitaker --test release_naming_properties --test behaviour_binstall
cargo test --doc -p whitaker-installer --all-features release_packaging
```

Then run the milestone gates (see `Concrete steps`). Run the seeded faults for
`INV-NAME`, `INV-MEMBER`, and `INV-NAMESPACE` once each, record them in
`Artefacts and notes`, and revert them.

### `EP-M2` the release pipeline builds, checks, and publishes `whitaker`

Assigned: `GAP-BUILD`, `GAP-COMPAT`, `GAP-LOCKSTEP`, `GAP-PUBLISH`,
`GAP-PACKAGE-VERIFY`, `REQ-MIRROR`, `REQ-LOCKSTEP`, `REQ-CRATES`,
`INV-LOCKSTEP`, `INV-RELEASE-COMPLETE`, `INV-PUBLISH-RERUN`,
`INV-INSTALLER-STABLE` (the build-command half); D-1, D-3a, D-4, D-11.

Red: add `tests/workflow_contracts/release_cli_artefacts_contract_test.py`,
`release_assets_test.py`, and `publish_crates_test.py`, plus stub scripts that
exit 2. Run `make test-workflow-contracts` and record the named violations and
failures.

Green:

1. Manifests: set `version.workspace = true` in the root `[package]`,
   `installer/Cargo.toml`, and `common/Cargo.toml` (D-3a). Run
   `cargo metadata --locked` to confirm that nothing else changed.
2. `scripts/release_assets.py` (standard library, `argparse`) has four
   subcommands, each with numpy-style docstrings and a usage example. The
   fourth, `manifest-version`, is used in `EP-M3`.
   - `check-archive ARCHIVE --member PATH` asserts exactly one regular-file
     member at `PATH` and verifies the `.sha256` sidecar;
   - `check-complete DIST --version V` implements `INV-RELEASE-COMPLETE` and
     derives the expected names from the documented scheme;
   - `cli-archives-expected V` implements `INV-CLI-FLOOR`, with
     `FIRST_CLI_ARCHIVE_VERSION = "0.4.0"` as its single source of truth.
3. `scripts/publish_crates.py` (standard library, `urllib` with a generic
   `User-Agent`, and `subprocess`) implements `INV-PUBLISH-RERUN`.
4. `release.yml` `build-installer`:
   - The version step reads all three packages' versions through
     `cargo metadata --locked` and fails unless all equal `${RELEASE_TAG#v}`.
   - The existing installer build step is unchanged apart from `--locked`.
     The packaging step gains `--package whitaker-installer`.
   - New steps follow it: `cargo build --locked -p whitaker --release
     --target …`; the binary path with `.exe` on Windows; packaging with
     `--package whitaker`; and the glibc check widened to both binaries.
   - `generate_checksums.py dist/` and the upload stay as they are, after
     both packages.
   - A comment above the job explains why the job builds both executables,
     why the builds are separate (Constraint 2), and why the name stays
     (D-2).
5. `release-compatibility`:
   - add a `whitaker_archives` array from
     `dist/whitaker-x86_64-unknown-linux-gnu-v*.tgz`, requiring exactly one
     when any x86_64 artefact exists;
   - run `release_assets.py check-archive` on it;
   - extract it, add its binary to the glibc check, and compare
     `whitaker --version` with `whitaker ${version}`.

   The installer lines are untouched apart from the same exact `--version`
   comparison.
6. `publish`:
   - replace the installer-only tag check with the three-package check;
   - after the downloads and before the upload, run `release_assets.py
     check-complete dist --version "${RELEASE_TAG#v}"`;
   - replace the two `cargo publish` lines with
     `scripts/publish_crates.py --package whitaker-common --package
     whitaker-installer --package whitaker --version "${RELEASE_TAG#v}"`.
7. Makefile `release-installer-dry-run`:
   - build the installer, then the root CLI, as separate commands;
   - package both and run `generate_checksums.py`;
   - run `release_assets.py check-archive` on each archive, which works on
     Windows without `unzip` because it uses Python's `zipfile` and
     `tarfile`, and `check-complete`;
   - run the built `whitaker` binary's `--version`.

   It accepts an optional `RELEASE_DIST_DIR`, a separate variable that the
   cleanup trap does not delete; if the directory is not empty, the recipe
   fails. Update its help text.
8. Makefile `package-root-cli` becomes the verified multi-package
   `cargo package` (D-11). Drop the `patch.crates-io` overrides and update
   `tests/workflow_contracts/root_package_contract_test.py` to match.
9. Update `tests/workflows/test_release_installer_dry_run_makefile.py` and
   `tests/workflows/test_release_workflow_healing.py` so they stay correct.

Focused commands:

```sh
set -o pipefail
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make release-installer-dry-run 2>&1 | tee /tmp/release-dry-run-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make package-root-cli 2>&1 | tee /tmp/package-root-cli-whitaker-3-5-3-publish-whitaker-release-artefacts.out
```

Expected dry-run tail on Linux (version per `EP-M0`):

```plaintext
Created /tmp/…/dist/whitaker-installer-x86_64-unknown-linux-gnu-v0.3.0.tgz
Created /tmp/…/dist/whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz
release_assets: complete (1 target, 2 archives, 2 checksums)
whitaker 0.3.0
```

### `EP-M3` binstall resolution is verified before and after publication

Assigned: `GAP-VERIFY`, `REQ-BINSTALL`, `INV-E2E`, `INV-CLI-FLOOR` (wiring), and
`INV-PUBLISHED` (wiring); D-6.

Red: add `tests/workflow_contracts/verify_binstall_local_test.py`, and extend
`release_cli_artefacts_contract_test.py` with the `verify-binstall.yml`
expectations:

- the `verify-binstall-cli` job, its floor gate, and its `--version`
  comparison;
- the root cell of `verify-binstall-manifest` and its
  `release_assets.py manifest-version` read;
- the `verify-binstall-local` job's paths, timeout, Linux-only runner, and
  negative control;
- `release.yml` calling `verify-binstall-cli` after `publish`.

All fail first.

Green:

1. `scripts/verify_binstall_local.py`: implement per `INV-E2E`, including the
   pre-release version run and `--negative-control`.
2. `verify-binstall.yml`:
   - Add `verify-binstall-cli` (`workflow_call` and `workflow_dispatch` with
     `tag`), with the three-OS matrix, the pinned `taiki-e/install-action`
     with `cargo-binstall@1.16.6`, and `GITHUB_TOKEN`. It checks out
     `release_assets.py` at the tag or `scripts-ref` and sets up `uv`.
   - Extend `verify-binstall-manifest` with a `package` dimension. Each cell
     reads the version from `[workspace.package]` through a
     `release_assets.py manifest-version MANIFEST` subcommand (`tomllib`,
     run with `uv`, no Rust setup). The subcommand fails unless the package
     inherits `version.workspace`.
   - Add `verify-binstall-local` per `INV-E2E`.
   - Extend `pull_request.paths` with the paths `INV-E2E` names.
3. `release.yml`: add a `verify-binstall-cli` caller job after `publish`,
   alongside `verify-binstall`.
4. Record the `EP-M0` binstall-version decision in a workflow comment.

Focused commands:

```sh
set -o pipefail
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make release-installer-dry-run RELEASE_DIST_DIR=/tmp/whitaker-3-5-3-dist
scripts/verify_binstall_local.py --dist /tmp/whitaker-3-5-3-dist \
  --root-manifest Cargo.toml --installer-manifest installer/Cargo.toml
scripts/verify_binstall_local.py --dist /tmp/whitaker-3-5-3-dist \
  --root-manifest Cargo.toml --installer-manifest installer/Cargo.toml --negative-control
```

Expected: the positive run reports one GET per package and matching hashes, for
both the release version and the pre-release version, and exits 0. The negative
control reports a successful GET followed by binstall's non-zero exit, and
itself exits 0.

### `EP-M4` documentation and delivery closure

Assigned: `GAP-DOCS`, `REQ-DOCS`; D-2, D-7, D-9.

1. `docs/whitaker-cli-design.md`: add a "Release artefacts" subsection under
   §Compatibility and migration. It covers:
   - the naming and layout table for both executables;
   - the shared leg with separate builds, and the feature-unification
     reason;
   - structural lockstep, the multi-package publication, and the first
     CLI-archive version;
   - the verification gates;
   - the deferred renames, with a link to their issue.
2. `docs/adrs/adr-0012-whitaker-cli-boundary.md`: add a dated entry to the
   existing `## Implementation note` section ("2026-MM-DD, roadmap 3.5.3"). It
   records that release archives exist from 0.4.0 onwards and that v0.3.0 is
   not backfilled. The decision text is unchanged.
3. `docs/migrations/0.4.0.md`: add the `installer_packaging` to
   `release_packaging` rename, the `ReleaseBinary` and `ReleaseVersion`
   arguments, and `#[non_exhaustive]`, with before-and-after snippets. Note that
   `cargo binstall whitaker` now installs a prebuilt binary.
4. `docs/users-guide.md`:
   - rewrite §The shortest path's closing paragraph and §Root CLI, making
     `cargo binstall whitaker` the primary install route from 0.4.0;
   - keep `cargo install --path .` for checkouts;
   - state that 0.3.0 still builds from source.
5. `docs/publishing.md`:
   - replace the "does not publish GitHub release archives" paragraph;
   - document the workspace version, the three-crate tag check, the
     re-runnable publication, the verified `package-root-cli`, the
     completeness gate, and the truthful fail-closed behaviour;
   - add the `verify-binstall-cli` check to "After publishing".
6. `docs/developers-guide.md`: in the helper-binaries and `verify-binstall`
   sections, document:
   - `--package`, `ReleaseBinary`, and `ReleaseVersion`;
   - the three new scripts and the `verify-binstall-local` job;
   - the reuse policy in `Interfaces and dependencies`.
7. `docs/whitaker-dylint-suite-design.md:1971`: update the module name.
8. `docs/repository-layout.md`: mention the new scripts if it lists
   `scripts/`. `docs/contents.md`: add this ExecPlan.
9. Open follow-up issues for:
   - the externally visible renames (D-2);
   - writer deduplication and reproducible installer archives (D-7);
   - the fail-closed `release-compatibility` download condition;
   - the `--clobber` re-dispatch semantics.

   Link each from the design subsection.
10. `docs/roadmap.md`: mark 3.5.3 done (`[x]`) only after every acceptance
    item below passes. `INV-PUBLISHED` is post-release by construction and is
    stated as such.

Stale-claim search, excluding `docs/execplans/`:

```sh
rg -n '3\.5\.3|binstall archives are pending|not published until|installer_packaging' \
  docs README.md --glob '!docs/execplans/**'
rg -n 'fall(s)? back to (a )?(building|source)' docs README.md --glob '!docs/execplans/**'
```

Every hit is either updated or justified in `Decision log`.

## Milestones and plateaus

### `EP-M0`, feasibility probe

- Outcome: binstall 1.16.6's behaviour, the `--version` format, and the
  verified multi-package packaging time are known.
- Requirements: `AXIOM-BINSTALL-HTTPS` and `AXIOM-CARGO-MULTI`.
- Acceptance evidence: the probe transcripts in `Artefacts and notes`.
- Conformance check: no repository change.
- Recovery: none needed.
- Remaining gaps: everything.
- Compatibility decision: none.

### `EP-M1`, the packaging library and packager package either executable

- Outcome: `whitaker-package-installer --package whitaker …` writes a
  binstall-shaped `whitaker` archive. The installer archive is unchanged, and
  the release and dry-run callers pass `--package whitaker-installer`.
- Requirements: `GAP-PACKAGER`. Discharges `INV-NAME`, `INV-MEMBER`,
  `INV-NAMESPACE` (the Rust half), `INV-INSTALLER-STABLE` (the naming half), and
  `INV-VERSION-SAFE`.
- Acceptance evidence: the focused tests pass, with red and seeded-fault
  evidence recorded. `make check-fmt`, `make lint`, `make test`, and
  `make test-doc` pass.
- Conformance check:
  - `REQ-INSTALLER-UNCHANGED` holds: the snapshot is unchanged.
  - D-3, D-10, and D-12 are followed.
  - The public API changes only within Constraint 7.
  - There is no trust-boundary or persisted-format change, and trace links
    are current.
- Recovery: revert the milestone's commits. The release workflow is then
  installer-only and correct.
- Remaining gaps: nothing builds `whitaker` in the release yet.
- Compatibility decision: none. The change is pre-1.0 and goes into the
  0.4.0 migration guide, and every caller is updated in the same commit.

### `EP-M2`, the release pipeline builds, checks, and publishes `whitaker`

- Outcome: a 0.4.0 tag would produce, check, upload, and publish the
  `whitaker` artefacts and crate. The dry run proves it on the host, and pull
  requests verify the three-crate package.
- Requirements: `GAP-BUILD`, `GAP-COMPAT`, `GAP-LOCKSTEP`, `GAP-PUBLISH`,
  and `GAP-PACKAGE-VERIFY`. Discharges `INV-LOCKSTEP`, `INV-RELEASE-COMPLETE`,
  `INV-PUBLISH-RERUN`, and `INV-NAMESPACE` (the glob half).
- Acceptance evidence: `make test-workflow-contracts` passes, including the
  refusal cases. `make release-installer-dry-run` and `make package-root-cli`
  pass as shown, and `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check:
  - The installer build command names only the installer.
  - D-4 and D-11 are approved.
  - The glibc baseline is unchanged, and no new dependency is added.
  - The CI-cost tolerance is measured on the PR.
- Recovery: revert. `EP-M1`'s plateau is intact.
- Remaining gaps: binstall resolution is not yet exercised.
- Compatibility decision: none.

### `EP-M3`, binstall resolution verified

- Outcome: a pull request that breaks either table, the packager, or the
  synthesized-manifest path fails `verify-binstall-local`. The 0.4.0 release
  will prove `whitaker` binstall on three operating systems.
- Requirements: `GAP-VERIFY`. Discharges `INV-E2E` and `INV-CLI-FLOOR`, and
  wires `INV-PUBLISHED`, whose evidence waits for the release.
- Acceptance evidence:
  - the local positive and negative-control transcripts;
  - `make test-workflow-contracts`, `make check-fmt`, `make lint`, and
    `make test` pass;
  - the PR's `verify-binstall-local` job is green, and
    `verify-binstall-manifest` skips the root cell with the floor notice.
- Conformance check: the verification gates mirror the installer's
  (`REQ-MIRROR`), and the installer's `verify-binstall` job is untouched.
- Recovery: revert. `EP-M2` is intact.
- Remaining gaps: the `INV-PUBLISHED` evidence.
- Compatibility decision: none.

### `EP-M4`, documentation and closure

- Outcome: documentation describes the shipped state, the follow-up issues
  exist, and the roadmap item is done.
- Requirements: `GAP-DOCS` and `REQ-DOCS`.
- Acceptance evidence: `make markdownlint`, `make nixie`, and `make check-fmt`
  pass; the stale-claim search is clean; the four issues are linked; and
  roadmap 3.5.3 is `[x]`.
- Conformance check: ADR 0012's decision is preserved and a note is added,
  and every `Surprises` entry is reconciled.
- Recovery: documentation only.
- Remaining gaps: the `INV-PUBLISHED` evidence.
- Compatibility decision: none.

## Concrete steps

Working directory: the repository root of this worktree. Enable `pipefail` in
every shell that pipes a gate through `tee`. Log names follow
`/tmp/<action>-whitaker-3-5-3-publish-whitaker-release-artefacts.out`.

After each milestone, run the gate sequence sequentially. Sub-agents do not run
full gates; `scrutineer` may.

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make lint 2>&1 | tee /tmp/lint-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make test 2>&1 | tee /tmp/test-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make markdownlint 2>&1 | tee /tmp/markdownlint-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make nixie 2>&1 | tee /tmp/nixie-whitaker-3-5-3-publish-whitaker-release-artefacts.out
```

Additional runs:

- `make test-doc` after `EP-M1`, because the naming doctests change.
- `make test NEXTEST_PROFILE=ci` once, before the PR is marked ready, for
  parity with CI.

Compiles queue behind the shared `build-limits` admission pool, and a waiting
build is not hung. Run long gates from the main session in the background with
a generous timeout. Check `build-limits-status` before blaming the candidate
for scattered failures.

Record red, green, and refactor evidence for each milestone in
`Artefacts and notes` as it happens.

## Validation and acceptance

The work is accepted when all of the following hold:

- Packaging a fake binary with `whitaker-package-installer`, passing
  `--package whitaker`, `--crate-version 0.4.0`,
  `--target x86_64-pc-windows-msvc`, `--binary-path <fake.exe>`, and
  `--output-dir d`, creates `d/whitaker-x86_64-pc-windows-msvc-v0.4.0.zip`. Its
  only entry is `whitaker-x86_64-pc-windows-msvc-v0.4.0/whitaker.exe`.
- The same command with `--crate-version ../0.4.0` exits 1 with an
  invalid-release-version error and leaves `d` empty.
- `make release-installer-dry-run` on Linux creates both archives and both
  sidecars. `release_assets.py` reports the set complete, and the built
  `whitaker --version` prints `whitaker <workspace version>`.
- `make package-root-cli` verifies all three crates.
- `scripts/verify_binstall_local.py` installs both packages from the dry
  run's output with the pinned `cargo-binstall`, at the release version and at
  a pre-release version. It shows one GET of each expected path and matching
  hashes, and its negative control fails for the layout reason.
- These new and changed tests pass, and each failed for the intended reason
  before its implementation:
  - `release_naming_properties`, `release_version_tests`, and the
    `release_binary` tests;
  - the new `installer_release.feature` scenarios and the packager CLI cases;
  - `release_cli_artefacts_contract_test.py`, `release_assets_test.py`,
    `publish_crates_test.py`, `verify_binstall_local_test.py`, and
    `root_package_contract_test.py`.
- Every pre-existing installer packaging, binstall, and release test passes,
  with unchanged expectations.
- `make check-fmt`, `make lint`, `make test`, `make test-doc`,
  `make test-workflow-contracts`, `make markdownlint`, and `make nixie` pass on
  the final tree.
- On the PR, `verify-binstall-local` and `verify-binstall-manifest` are green.
  The latter skips the root cell with the floor notice for 0.3.0.

Quality criteria:

- Tests: as above, under both the default and `ci` nextest profiles.
- Verification: `INV-NAME`, `INV-MEMBER`, `INV-NAMESPACE`,
  `INV-INSTALLER-STABLE`, `INV-VERSION-SAFE`, `INV-LOCKSTEP`,
  `INV-RELEASE-COMPLETE`, `INV-PUBLISH-RERUN`, `INV-CLI-FLOOR`, and `INV-E2E`
  are discharged with non-vacuity evidence. `INV-PUBLISHED` is wired, and its
  evidence follows the 0.4.0 release.
- Lint: `make lint` is clean, with no new `#[expect]` unless justified in
  place.
- Security:
  - the packager rejects path-escaping versions;
  - the local check binds only to `127.0.0.1` and deletes its throwaway key;
  - `publish_crates.py` sends no credential to the registry API and passes
    `CARGO_REGISTRY_TOKEN` only to `cargo`;
  - no new job receives more than read access through `GITHUB_TOKEN`.

## Idempotence and recovery

Every milestone is a sequence of ordinary commits, and re-running a gate is
safe. The dry run writes only to a temporary directory, or to an empty
`RELEASE_DIST_DIR` that the caller provides. The local binstall check:

- installs into a temporary `--root` with `--no-track`;
- binds `127.0.0.1` on an ephemeral port;
- removes its directory and key on exit, including on failure.

At release time, `publish_crates.py` makes the crates.io step safe to re-run.
Nothing in this plan pushes a tag, creates a release, or publishes a crate. If
a milestone fails its gates after three attempts at the same cause, stop,
record the state in `Progress` and `Decision log`, and escalate.

## Artefacts and notes

Planning probe (2026-11-27, `cargo-binstall` 1.22.0, in a scratch directory
under `/tmp`, since deleted). A fake `whitaker` script was packaged as
`whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz` with the real `bin-dir` layout,
and a synthesized manifest pointed `pkg-url` at a local server.

Plain HTTP:

```plaintext
WARN Fail to resolve the future err=Download(Remote(Http(HttpError { method: GET,
  url: "http://127.0.0.1:18765/whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz", … source: BadScheme })))
ERROR Fatal error: For crate whitaker: Fallback to cargo-install is disabled
```

HTTPS with `--root-certificates cert.pem`. The certificate was made with
`openssl req -x509 -newkey rsa:2048 -nodes -subj "/CN=127.0.0.1" -addext
"subjectAltName=IP:127.0.0.1" -addext "basicConstraints=critical,CA:FALSE"`.

```plaintext
WARN The package whitaker v0.3.0 (x86_64-unknown-linux-gnu) has been downloaded from 127.0.0.1
INFO   - whitaker => /tmp/binstall-probe-…/root/bin/whitaker
INFO Done in 833.482997ms
$ root/bin/whitaker
probe-whitaker 0.3.0
```

Negative control (`bin-dir = "{name}-{target}/{bin}{binary-ext}"`):

```plaintext
ERROR Fatal error: For crate whitaker: Fallback to cargo-install is disabled
exit=94
```

Feature unification (2026-11-28,
`cargo tree --offline -e features --prefix
none --target x86_64-unknown-linux-gnu`).
This is the difference in the installer's feature set between running with
`-p whitaker-installer` alone and running with `-p whitaker` added:

```plaintext
> clap feature "env"
> clap feature "string"
> clap_builder feature "env"
> clap_builder feature "string"
> proc-macro2 feature "span-locations"
> syn feature "visit"
> toml v0.8.23
```

Release state on 2026-11-27:

- crates.io reports `whitaker` `max_version` 0.3.0.
- The v0.3.0 GitHub release has the installer, dependency-binary, and
  Clippy-snapshot assets, and no `whitaker-*` asset.
- `cargo publish --help` on the pinned toolchain (cargo 1.98.0-nightly)
  lists `-p, --package [<SPEC>]  Package(s) to publish`.

## Interfaces and dependencies

At the end of `EP-M1`, the `whitaker_installer` crate exposes:

```rust
// installer/src/release_binary.rs
pub enum ReleaseBinary { Whitaker, WhitakerInstaller }
impl ReleaseBinary {
    pub const ALL: [ReleaseBinary; 2];
    pub const fn package_name(self) -> &'static str;
    pub const fn bin_name(self) -> &'static str;
    pub fn binstall_release<'a>(self, version: &'a ReleaseVersion, target: &'a TargetTriple)
        -> binstall_metadata::BinstallRelease<'a>;
}

// installer/src/release_version.rs
pub struct ReleaseVersion(String);
impl TryFrom<&str> for ReleaseVersion { type Error = InvalidReleaseVersion; }
impl TryFrom<String> for ReleaseVersion { type Error = InvalidReleaseVersion; }
impl ReleaseVersion { pub fn as_str(&self) -> &str; }
pub struct InvalidReleaseVersion { pub version: String, pub reason: &'static str }

// installer/src/release_packaging.rs (renamed from installer_packaging.rs)
pub struct ReleasePackageParams {
    pub binary: ReleaseBinary,
    pub version: ReleaseVersion,
    pub target: TargetTriple,
    pub binary_path: PathBuf,
    pub output_dir: PathBuf,
}
pub struct ReleasePackageOutput { pub archive_path: PathBuf, pub archive_name: String }
#[non_exhaustive]
pub enum ReleasePackagingError { Io(std::io::Error), BinaryNotFound(PathBuf), Zip(zip::result::ZipError) }
pub fn archive_filename(binary: ReleaseBinary, version: &ReleaseVersion, target: &TargetTriple) -> String;
pub fn inner_dir_name(binary: ReleaseBinary, version: &ReleaseVersion, target: &TargetTriple) -> String;
pub fn binary_filename(binary: ReleaseBinary, target: &TargetTriple) -> String;
pub fn archive_format(target: &TargetTriple) -> ArchiveFormat; // unchanged
pub fn package_release_binary(params: &ReleasePackageParams)
    -> Result<ReleasePackageOutput, ReleasePackagingError>;
```

Ownership and reuse policy:

- `release_binary` owns the identity of the two Whitaker executables.
- `release_version` owns release-version validation.
- `release_packaging` owns archive naming and layout for those executables.
- `binstall_metadata` owns the template constants and expansion, and takes
  its identity from `release_binary`.

The workflow and Makefile reach packaging only through the packager binary;
tests may call the library directly. No workflow step computes an archive name
by hand, with two exceptions:

- the `release-compatibility` selection globs, which `INV-NAMESPACE` pins;
- `release_assets.py`, whose expected-name scheme is pinned against the Rust
  library by its tests. The tests compare its output for all ten pairs with
  names produced by the packager in a fixture.

Dependency binaries keep `dependency_packaging` (D-7).

Packager command line at the end of `EP-M1`:

```plaintext
whitaker-package-installer --package <whitaker|whitaker-installer> \
  --crate-version <VERSION> --target <TRIPLE> --binary-path <PATH> --output-dir <DIR>
```

New scripts. All use `uv run --script`, Python 3.13 or later,
`dependencies = []`, and `argparse`:

- `scripts/release_assets.py` with four subcommands:
  - `check-archive ARCHIVE --member PATH`;
  - `check-complete DIST --version V`;
  - `cli-archives-expected V`, which prints `expected=true|false`;
  - `manifest-version MANIFEST`, which prints `version=V` from
    `[workspace.package]` and fails unless the manifest's package inherits
    it.

  All exit 1 with a named reason on failure, and 2 on usage errors.
- `scripts/publish_crates.py --package NAME… --version V`, with an optional
  `--registry-api URL` (tests only) and `--dry-run`.
- `scripts/verify_binstall_local.py`. It takes `--dist DIR`,
  `--root-manifest PATH`, and `--installer-manifest PATH`, with optional
  `--target TRIPLE`, `--binstall PATH`, and `--negative-control`.

Dependencies: no new runtime dependency. `googletest` and `pretty_assertions`
become dev-dependencies of `whitaker-installer`, at workspace versions 0.14.3
and 1.4.1. The CI tools are `cargo-binstall` 1.16.6 through the pinned
`taiki-e/install-action` (or the version `EP-M0` decides on for the local
check), the runner image's `openssl`, `setup-rust`, and `setup-uv`.

## Revision note

- 2026-11-27: initial draft from Wyvern reconnaissance, crates.io and GitHub
  release checks, and the binstall HTTPS probe.
- 2026-11-28: community-of-experts review applied (D-13). Main changes:
  - separate installer and CLI builds, because feature unification was
    confirmed locally;
  - structural lockstep through `version.workspace`, which replaces the
    planned version script;
  - a re-runnable multi-package crates.io publication (D-4), and verified
    multi-package packaging on pull requests (D-11, new);
  - a version floor in place of "skip if absent" (D-6);
  - new `release_binary` and `release_version` modules, and the Rust
    packaging surface renamed for 0.4.0 (D-12, new);
  - `L-PREFIX` replaces the incorrect shared-version argument in D-8;
  - oracles that read names from the manifests, non-vacuity mechanisms that
    work in proptest and Hypothesis, and harness-tested release scripts that
    use only the standard library;
  - a Linux-only, path-narrowed local binstall job that asserts request
    paths and hashes, and a separate post-publication CLI job.

  The milestone order is unchanged. The scope grows by D-11, the renames, and
  three scripts in place of two, and the code budget rises from 2,000 to 2,200
  lines accordingly. Awaiting user approval.
