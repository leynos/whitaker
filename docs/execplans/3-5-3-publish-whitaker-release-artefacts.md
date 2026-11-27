# Publish `whitaker` release artefacts with `cargo-binstall` metadata (3.5.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting expert review and user approval). Implementation must
not begin until the user explicitly approves this plan.

Branch: `3-5-3-publish-whitaker-release-artefacts`

Roadmap item: 3.5.3 in `docs/roadmap.md` ("Publish `whitaker` release artefacts
with `cargo-binstall` metadata that mirror the existing installer packaging
flow"). Requires 3.5.1, 4.2.1, and 4.3.1, all of which are done.

## Purpose / big picture

Whitaker ships two user-facing executables. `whitaker-installer` is the legacy
installer; `whitaker` is the root command-line interface (CLI) added by roadmap
item 3.5.1, whose `whitaker check` subcommand (3.5.2) is now the default
linting path. The design's headline workflow is:

```console
cargo binstall whitaker
whitaker check
```

`cargo binstall` (from the `cargo-binstall` project) installs a Rust binary by
downloading a prebuilt archive instead of compiling the crate. It reads a
`[package.metadata.binstall]` table from the crate's manifest. That table holds
templates such as `pkg-url` (where the archive lives) and `bin-dir` (where the
binary sits inside the archive), and binstall fills their placeholders
(`{name}`, `{version}`, `{target}`, `{bin}`, `{binary-ext}`) by literal
substitution.

Today the second half of that workflow works and the first half does not do
what it says. The root `Cargo.toml` already carries binstall metadata for
`whitaker`, and `whitaker` 0.3.0 is on crates.io, but no release ever publishes
the archive that metadata names. The v0.3.0 GitHub release carries
`whitaker-installer-<target>-v0.3.0.{tgz,zip}` for five targets and no
`whitaker-*` archive at all. So `cargo binstall whitaker` requests
`https://github.com/leynos/whitaker/releases/download/v0.3.0/whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz`,
receives a 404, and silently falls back to compiling the crate from source:
several minutes of build time, and a hard failure for anyone running binstall
with `--disable-strategies compile`.

After this change, every `v<version>` release built by `release.yml` also
publishes, for each of the five supported targets, a
`whitaker-<target>-v<version>.tgz` (`.zip` on Windows) archive and its
`.sha256` sidecar, packaged by the same tool, on the same runners, under the
same glibc baseline, partial-release policy, and verification gates as the
installer. The release workflow publishes `whitaker` to crates.io after
`whitaker-installer`, and the post-publication `verify-binstall` job proves on
Linux, macOS, and Windows that:

```console
cargo binstall --no-confirm --disable-strategies compile,quick-install whitaker@<version>
whitaker --version
```

installs the released archive's binary rather than building from source. Before
any release exists, a new pull-request check runs the real `cargo-binstall`
against archives produced by the real packager and served from a local HTTPS
server, using the real binstall tables from both manifests, so a naming or
layout regression is caught before a tag is pushed.

Preserved behaviour: every `whitaker-installer` asset name, archive layout,
checksum, binstall table, and verification step stays byte-for-byte the same.
No user-facing command, flag, or diagnostic of either executable changes.

Deliberately changed behaviour: the release-only helper
`whitaker-package-installer` gains a required `--package` argument naming which
executable it packages, and rejects a version string that could escape the
output directory. Both are internal release tooling behind the `release-tools`
feature.

This item closes the "no `whitaker` GitHub release archives until 3.5.3"
limitation recorded in `docs/adrs/adr-0012-whitaker-cli-boundary.md` (§Decision
outcome and §Known risks and limitations), in `docs/users-guide.md` (§The
shortest path and §Root CLI), and in `docs/publishing.md`.

## Definitions

- **Release artefact.** A file attached to a GitHub release, here an archive
  and its `.sha256` sidecar.
- **Release asset namespace.** The flat set of file names attached to one
  GitHub release. Two assets with one name cannot coexist; the release
  workflow's re-run path uses `gh release upload --clobber`, which replaces a
  same-named asset silently.
- **Packager.** The release-only binary `whitaker-package-installer`
  (`installer/src/bin/package_installer_bin.rs`), a thin wrapper around
  `whitaker_installer::installer_packaging::package_installer`.
- **Supported target.** One of the five triples in `SUPPORTED_TARGETS`
  (`installer/src/artefact/target.rs:15`), validated by `TargetTriple`.
- **Release leg.** One matrix entry of the `build-installer` job in
  `.github/workflows/release.yml`: one target on one native runner.
- **Lockstep version.** The rule in `docs/publishing.md` that
  `whitaker-common`, `whitaker-installer`, and `whitaker` share one version,
  and that the release tag is `v` followed by it.

## Constraints

1. Implementation begins only after the user explicitly approves this plan.
2. `whitaker-installer` release behaviour must not change: its asset names,
   archive layout (`whitaker-installer-<target>-v<version>/whitaker-installer`
   or `.exe`), `[package.metadata.binstall]` table in `installer/Cargo.toml`,
   checksum format, and every existing verification step stay as they are. The
   `ci.yml` pin `WHITAKER_INSTALLER_VERSION` and the lint jobs that binstall it
   are out of scope.
3. The root binstall table (`Cargo.toml` `[package.metadata.binstall]`) is
   already published inside `whitaker` 0.3.0 on crates.io, and binstall reads
   that published copy for every future `whitaker@<version>` install. The
   archives must therefore be shaped to match the table, not the reverse. Any
   change to the root table needs escalation.
4. Supported targets and runners are exactly those of `build-installer` today
   (`release.yml:30-39`). Adding or dropping a target is out of scope.
5. x86_64 Linux release binaries must not require a glibc symbol newer than
   `GLIBC_2.35` (ADR 0001; `docs/publishing.md`); the `whitaker` binary joins
   the existing `scripts/check_glibc_baseline.py` check.
6. No new runtime dependency. Authorized dev-dependency additions for the
   `whitaker-installer` package: `googletest` and `pretty_assertions`, both
   already declared in `[workspace.dependencies]`. Python scripts follow
   `docs/scripting-standards.md` (`uv run --script`, Python 3.13) and may use
   only the standard library plus dependencies already used by sibling scripts.
7. Interfaces that may change, with every caller updated in the same commit
   and no compatibility shim (all are pre-1.0 and release-internal): the
   `whitaker_installer::installer_packaging` functions and
   `InstallerPackageParams`, and the packager's command line. No compatibility
   is owed to any external consumer of these: the packager is gated behind
   `release-tools` and is excluded from `cargo install` and `cargo binstall`.
8. Identifier names that say "installer" (`installer_packaging`,
   `whitaker-package-installer`, the `build-installer` job, the
   `installer-<target>` workflow artefact, `release-installer-dry-run`) are
   kept, and their documentation widened. Renaming them belongs with the
   removal of installer-first references in roadmap item 3.9.3 (see D-2).
9. No file under `docs/execplans/` other than this one may be edited;
   completed plans are historical. Accepted ADR decisions are not rewritten:
   ADR 0012 receives a dated implementation note.
10. Code files stay under 400 lines (`AGENTS.md`).
    `installer/src/installer_packaging_tests.rs` is 329 lines today, so new
    tests go in new modules rather than growing it past the limit.
11. Tests do not mutate the process environment (`AGENTS.md` §Testing). A test
    that needs a variable passes it to a child process with `Command::env`.
12. Every Rust module begins with a `//!` comment; every function carries a
    `///` comment; every `assert!`-family call carries a diagnostic message.
    Python definitions under `scripts` and `tests` carry docstrings (the
    `interrogate` gate inside `make lint`).
13. The workflow is not run against GitHub during implementation: no tag is
    pushed, no release is created, and no crate is published. Release-time
    behaviour is evidenced by contract tests, the Makefile dry run, and the
    local binstall check; the first real evidence of `verify-binstall` for
    `whitaker` arrives with the next release (see `Risks`).

## Tolerances (exception triggers)

Comparison base: `git merge-base origin/main HEAD` at the start of
implementation (expected `755c1f2c` unless `main` advances). This ExecPlan file
is excluded from every count. Count with:

```sh
base=$(git merge-base origin/main HEAD)
git diff --numstat "$base"..HEAD -- . ':(exclude)docs/execplans/**'
```

Classification uses a whole-file rule. Code: `*.rs`, `*.py`, `*.sh`,
`*.feature`, `Makefile`, and `.github/workflows/*.yml` (the workflow additions
are predominantly embedded shell, so the whole file is counted as code).
Configuration: `Cargo.toml` files, `Cargo.lock`, `.config/nextest.toml`.
Documentation: `*.md` outside `docs/execplans/`. Limits measure net lines
(additions minus deletions) per category and are enforced independently.

- Files: more than 36 changed non-ExecPlan files.
- Code: more than 2,000 net lines.
- Configuration: more than 120 net lines.
- Documentation: more than 450 net lines.
- Interface: any public API change beyond Constraint 7, any change to a
  user-facing command or flag of `whitaker` or `whitaker-installer`, or any
  change to either binstall table.
- Dependencies: any dependency beyond Constraint 6.
- Architecture: building `whitaker` in a separate job or with a separate
  packager, changing the target matrix, or changing asset names.
- Iterations: a gate still failing after three fix attempts for the same
  cause.
- Time: a milestone taking more than one working day of elapsed effort.
- Ambiguity: any point where `docs/whitaker-cli-design.md`, ADR 0012, and
  this plan disagree in a way that changes behaviour.

Budget survey. The estimate traces each affected file. Packaging library and
binary: about 160 lines of production change. Rust tests (unit,
behaviour-driven, property, archive integration): about 550. Two Python scripts
with their contract tests: about 600. Workflow and Makefile changes: about 250.
Workflow contract tests: about 250. That totals roughly 1,810 lines of code,
leaving a 10% contingency inside 2,000. Configuration is two dev-dependency
lines plus any nextest grouping (estimate under 30). Documentation covers seven
files at about 280 lines.

Historical evidence. `4-2-1-add-cargo-binstall-metadata.md` set 10 files and
finished at 9, with no revision.
`4-3-1-release-workflow-for-whitaker-installer.md` set 15 files and 800 net
lines and records no revision, although later work grew the workflow
considerably outside that plan. `3-5-1-root-whitaker-binary.md` broke its
70-file budget and raised its net-line ceiling to 7,000 (D-12). It finished at
186 files and +13,009/−1,222.
`3-5-2-whitaker-check-as-the-default-linting-path.md` set 30 files and 2,500
net lines for code. It finished at 73 files and 9,001 net lines after five
review rounds. In both overruns the growth came mostly from test and
verification scope added during review (missed scope), not from production
code. This plan therefore budgets tests as the larger share of code, and states
the verification scope fully before approval. Any review round that adds
obligations must re-estimate against these limits and escalate before it
breaches them.

## Risks

- Risk: `cargo-binstall` 1.16.6 (the version CI pins) may lack
  `--root-certificates` or may treat a synthesized manifest differently from
  1.22.0, which the planning probe used. Severity: medium. Likelihood: low.
  Mitigation: `EP-M0` re-runs the probe against 1.16.6 before any code depends
  on it. If it fails, the local check pins the newest version that passes, and
  the decision is recorded.
- Risk: the first real proof that `cargo binstall whitaker` resolves a
  published archive arrives only with the next tag, after merge. Severity:
  medium. Likelihood: low. Mitigation: the local HTTPS check exercises the real
  binstall against the real tables and the real packager output. The remaining
  difference is the URL origin and GitHub's asset hosting, which the
  installer's identical, already-verified pattern covers.
- Risk: publishing `whitaker` to crates.io from CI could fail if the index has
  not yet propagated `whitaker-installer`. Severity: medium. Likelihood: low.
  Mitigation: Cargo (1.66 and later) waits for the published crate to appear in
  the index before `cargo publish` returns, and the steps run sequentially. The
  publish step is still the last mutating step, so a failure leaves the GitHub
  release complete and the crate publishable by hand (`docs/publishing.md`).
- Risk: a re-dispatched release with `--clobber` rebuilds and replaces
  existing installer assets. Severity: low. Likelihood: low. Mitigation: this
  is existing behaviour and is unchanged. The plan does not backfill v0.3.0
  (D-5).
- Risk: the `whitaker` release binary adds build time to each release leg.
  Severity: low. Likelihood: high. Mitigation: one `cargo build` invocation
  with both `-p` flags shares the dependency graph; `whitaker` already depends
  on the installer library. Record the leg duration before and after in
  `Artefacts and notes` from the dry run.
- Risk: the root manifest's version is a literal (`version = "0.3.0"`), not
  `version.workspace = true`, so it can drift from the installer's version.
  Severity: high (a drifted version yields an archive whose name binstall never
  requests). Likelihood: low. Mitigation: `INV-LOCKSTEP` is checked in the
  release build leg before packaging and again in `publish`.
- Risk: the `tests/workflows` suite is opt-in and never runs in CI, so
  structural guards placed there are not guards. Severity: medium. Likelihood:
  medium. Mitigation: every new workflow invariant is pinned in
  `tests/workflow_contracts`, which `make test-workflow-contracts` runs in
  `linux-full`. Existing `tests/workflows` files are updated only to stay
  correct.

## Progress

- [x] (2026-11-27T22:10Z) Branch renamed and Lody session titled.
- [x] (2026-11-27T22:30Z) Wyvern reconnaissance of the release workflow,
  root CLI and design, and test and verification infrastructure.
- [x] (2026-11-27T22:40Z) crates.io and GitHub release state confirmed;
  binstall HTTPS feasibility probe and negative control run with 1.22.0.
- [x] (2026-11-27T23:00Z) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M0` feasibility probe against `cargo-binstall` 1.16.6.
- [ ] `EP-M1` packager packages either executable.
- [ ] `EP-M2` release pipeline builds, packages, checks, and publishes
  `whitaker`.
- [ ] `EP-M3` binstall resolution verified before and after publication.
- [ ] `EP-M4` documentation, ADR note, and roadmap closure.

## Surprises & discoveries

- Observation: the root package already carries binstall metadata, and
  `whitaker` 0.3.0 is already on crates.io, published by hand on 2026-11-16.
  `release.yml` publishes only `whitaker-common` and `whitaker-installer`.
  Evidence: `Cargo.toml:131-138`; the crates.io API reports `max_version` 0.3.0
  for `whitaker`; `release.yml:520-527`. Impact: 3.5.3 is about producing the
  archives and automating the publish, not about writing metadata. Constraint 3
  follows from this.
- Observation: the v0.3.0 release exists but has no `whitaker-*` asset.
  Evidence: `gh release view v0.3.0 --json assets`. Impact: the PR-time
  `verify-binstall-manifest` job skips only when a release is missing
  (`scripts/check-release-published.sh`). Pointed at the root manifest today,
  it would fail. The probe must ask whether the asset exists (see D-6).
- Observation: `cargo-binstall` refuses plain `http://` package URLs
  (`reqwest` `BadScheme`). It accepts `https://127.0.0.1` with a self-signed
  certificate passed through `--root-certificates`. Evidence: planning probe,
  `Artefacts and notes`. Impact: the local end-to-end check uses HTTPS.
- Observation: `installer_packaging` and `dependency_packaging` duplicate the
  tar and zip writers. Only the dependency copy uses
  `tar::HeaderMode::Deterministic` and a fixed zip timestamp. Evidence:
  `installer/src/installer_packaging.rs:230-276`,
  `installer/src/dependency_packaging.rs:150-192`. Impact: none for this item
  (see D-7).
- Observation: the `4-2-1` and `4-3-1` plans diverged from what shipped
  (literal `.tgz` instead of `{archive-format}`, native ARM runners, later
  gates), and record none of it. Impact: this plan describes the current
  workflow from source, not from those plans.

## Decision log

- D-1. Decision: build and package `whitaker` in the existing
  `build-installer` release leg, with one `cargo build` invocation naming both
  packages, and upload both archives in the same `installer-<target>` workflow
  artefact. Rationale: "mirror the existing installer packaging flow" is met
  most directly by the same runner, toolchain, packager, checksum step, glibc
  check, partial-release policy, and download pattern. A shared invocation
  compiles the shared dependency graph once. The alternative was a separate
  `build-whitaker` job: it duplicates about 100 lines of YAML, doubles runner
  start-up, and lets the two executables drift onto different runners. Its one
  advantage is independent failure, which is worth little because both
  executables fail for the same reasons. Date/Author: 2026-11-27, plan author.
- D-2. Decision: keep the "installer" identifiers (Constraint 8) and widen
  their documentation; defer renames to roadmap item 3.9.3. Rationale: renaming
  the module, packager, job, artefact, and Makefile target touches 74
  references in 28 files, which roughly doubles this item's churn without
  changing behaviour. Item 3.9.3 already owns removing installer-first
  references, so the renames have a home. The rejected alternative was renaming
  now to `release_packaging` and `whitaker-package-release`. That is more
  precise naming, at the cost of a large mechanical diff, and it would have to
  be done as a separate refactor commit anyway. Date/Author: 2026-11-27, plan
  author.
- D-3. Decision: the packager selects the executable through a closed enum,
  `ReleaseBinary { Whitaker, WhitakerInstaller }`, exposed as a required
  `--package <whitaker|whitaker-installer>` `clap::ValueEnum` argument. The
  `CRATE_NAME` constant is removed. Rationale: a closed set makes the asset
  namespace finite and checkable, and rules out typos at parse time. For both
  executables the package name equals the binary name, which is what both
  binstall tables assume (`{name}` in the directory and `{bin}` for the file).
  Making the argument required, rather than defaulting to `whitaker-installer`,
  avoids a silent default that would mislabel an archive. The rejected
  alternative was a free-form `--name` string, which is open-ended and
  reintroduces typos. Date/Author: 2026-11-27, plan author.
- D-4. Decision: `release.yml` publishes `whitaker` to crates.io after
  `whitaker-installer`, under the same `CARGO_REGISTRY_TOKEN` gate. Rationale:
  binstall reads `whitaker@<version>`'s metadata from crates.io, so an archive
  without the matching crate version is unreachable, and the installer flow
  being mirrored already publishes its crate. This is an outward-facing change
  that takes effect on the next tag; it is called out for explicit approval.
  Alternative: keep the manual publish described in `docs/publishing.md`. That
  leaves a window in which the archives exist and binstall cannot see them.
  Date/Author: 2026-11-27, plan author; requires user approval.
- D-5. Decision: do not backfill v0.3.0. The first release carrying
  `whitaker-*` archives is the next tag. Rationale: a backfill means
  dispatching the new workflow against the old tag, which rebuilds and
  `--clobber`s the published installer assets, changing bytes under existing
  checksums and installers. Users of 0.3.0 keep the source-build fallback,
  which is documented. Date/Author: 2026-11-27, plan author; the user may
  override at approval.
- D-6. Decision: the PR-time `verify-binstall-manifest` job covers both
  manifests. It skips a package when the named release, or the specific asset
  binstall would request, is absent. The probe is
  `scripts/check-release-published.sh`, extended with an optional asset-name
  argument and keeping its "only a confirmed absence skips" rule. Rationale:
  the root manifest names v0.3.0, whose release exists without the asset
  (`Surprises`). The release-level probe would fail it. Making the job
  root-unaware would leave a manifest change untested. Date/Author: 2026-11-27,
  plan author.
- D-7. Decision: do not merge the duplicated tar and zip writers in
  `installer_packaging` and `dependency_packaging`, and do not change the
  installer archives' header mode, in this item. Rationale: merging would
  change installer archive bytes (header mode and timestamps), and Constraint 2
  forbids that. It is a separate improvement to record as a follow-up issue.
  The new `whitaker` archives use the installer writer, so the two executables'
  archives stay identical in construction. Date/Author: 2026-11-27, plan author.
- D-8. Decision: no Verus proof and no Kani harness. Verification uses
  exhaustive `rstest` tables over the finite package-by-target domain,
  `proptest` over versions with an independent oracle, archive integration
  tests through the real packager, Python property tests for the version check,
  and an end-to-end check against the real `cargo-binstall`. The reasoning is in
  `Verification plan` §Formal verification decision. Date/Author: 2026-11-27,
  plan author.
- D-9. Decision: record the release-artefact design in a new subsection of
  `docs/whitaker-cli-design.md` §Compatibility and migration, and add a dated
  implementation note to ADR 0012. No new ADR. Rationale: the decisions apply
  ADR 0012 and the existing 4.2.1 and 4.3.1 patterns; none is hard to reverse
  or architecturally new. The crates.io automation (D-4) is the closest to an
  ADR-level choice. It is recorded in the design document and
  `docs/publishing.md`, and can be promoted to an ADR if review asks.
  Date/Author: 2026-11-27, plan author.
- D-10. Decision: the packager rejects a version string that is empty or
  contains a character outside ASCII alphanumerics, `.`, `-`, and `+`.
  Rationale: the version is interpolated into an output path and an archive
  member path. The workflow derives it from `cargo metadata`, but the packager
  must not rely on its caller to keep `/`, `\`, or `..` out. This allows every
  SemVer string, including pre-release and build metadata, and needs no new
  dependency. Alternative: parse with the `semver` crate, which is not a
  dependency of `whitaker-installer` (Constraint 6). Date/Author: 2026-11-27,
  plan author.

## Outcomes & retrospective

Not started. Populate at each milestone boundary and at completion.

## Context and orientation

Inspected base revision: `755c1f2cce7dafe1e9fb7ceda826e0854569538b` (`main`,
"Fold the Phase 17 DOMAIN amendment into the roadmap (#612)"). Line numbers
below refer to that revision; relocate symbols with `rg -n '<symbol>'`.

### Packages and executables

- Root package `whitaker` (`Cargo.toml:85-138`). Version is the literal
  `0.3.0`. Its single binary `whitaker` comes from `src/main.rs`, with
  `autobins = false`. It depends on the `whitaker-installer` library and
  carries binstall metadata at `Cargo.toml:131-138`. It is published on
  crates.io as 0.3.0.
- `whitaker-installer` (`installer/Cargo.toml`). Version is the literal
  `0.3.0`. It has the user binary `whitaker-installer` and four release-only
  helper binaries behind `required-features = ["release-tools"]`, including the
  packager `whitaker-package-installer`. Its binstall metadata is at
  `installer/Cargo.toml:121-128`.
- `whitaker-cli` (`cli/`) is `publish = false`, has no binary, and is not
  involved.

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

- `installer/src/installer_packaging.rs`. `CRATE_NAME` (line 31) is
  hard-coded to `"whitaker-installer"`. The module defines `ArchiveFormat`,
  `InstallerPackageParams`, `InstallerPackageOutput`, and
  `InstallerPackagingError` (`Io`, `BinaryNotFound`, `Zip`). It also has the
  pure naming functions `archive_filename` (98), `inner_dir_name` (127),
  `binary_filename` (151), and `archive_format` (179), the orchestrator
  `package_installer` (198), and the private `create_tgz_archive` (230,
  `sparse(false)` so binstall sees a regular file) and `create_zip_archive`
  (253). Unit tests are in `installer/src/installer_packaging_tests.rs` (329
  lines) and `installer/src/archive_entry_type_tests.rs`.
- `installer/src/bin/package_installer_bin.rs`. The packager CLI takes
  `--crate-version`, `--target`, `--binary-path`, and `--output-dir`, and prints
  `Created <path>`. Its tests are inline (lines 87-181).
- `installer/src/binstall_metadata.rs` holds the template constants
  `PKG_URL_TEMPLATE`, `WINDOWS_PKG_URL_TEMPLATE`, `BIN_DIR_TEMPLATE`,
  `DEFAULT_PKG_FMT`, `WINDOWS_PKG_FMT`, and `WINDOWS_OVERRIDE_TARGET`. It also
  has `BinstallRelease`, `expand_pkg_url_for`, and `expand_bin_dir_for`, plus
  the test-support loaders `load_cargo_toml_from` and `extract_binstall_table`.
- `installer/src/dependency_packaging.rs` is a separate packager for
  `cargo-dylint` and `dylint-link`. It is not changed (D-7).
- `installer/src/version.rs` defines `Version`, an unvalidated `String`
  newtype.

### Release workflow (`.github/workflows/release.yml`)

Triggers are `push` of `v*` tags and `workflow_dispatch` with a `tag` input.

- `build-installer` (lines 21-125): a five-target matrix on native runners.
  It reads the installer version with `cargo metadata | jq`, runs
  `cargo build -p whitaker-installer --release --target`, builds the packager,
  packages into `dist/`, checks the glibc baseline on x86_64 Linux, runs
  `scripts/generate_checksums.py dist/`, and uploads the artefact
  `installer-<target>`.
- `build-dependency-binaries` (127-223) packages the Dylint tools.
- `release-compatibility` (225-309) extracts the x86_64 Linux archives,
  requires exactly one installer archive, checks glibc, and runs `--version`.
- `clippy-snapshot` (311-346).
- `publish` (348-527) checks that the tag matches the installer version
  only (374-392). It downloads `installer-*` and `dependency-binaries-*`,
  refuses an empty release, warns on missing targets, uploads
  `dist/*.{tgz,zip,bundle,sha256,md}` (`--clobber` on re-run), then runs
  `cargo publish -p whitaker-common` and `-p whitaker-installer`.
- `verify-binstall` (529-545) calls `.github/workflows/verify-binstall.yml`.
- `verify-published-bootstrap` (547-587).

`.github/workflows/verify-binstall.yml` runs `verify-binstall` (post-publish,
on three OSes, installing `whitaker-installer@<tag>` with `cargo-binstall`
1.16.6 and compile and quick-install disabled) and `verify-binstall-manifest`
(pull requests touching the metadata; Linux and macOS;
`--manifest-path installer/Cargo.toml`, skipped by
`scripts/check-release-published.sh` when the release is absent).

`ci.yml` runs `make test-workflow-contracts` and `make package-root-cli` in
`linux-full`, and `make install-smoke` and `make release-installer-dry-run` in
`windows-compat`. The dry run (`Makefile:775-832`) builds and packages the
installer for the host triple and checks for an archive and a checksum.

### Tests and gates

- Rust: `rstest` 0.28, `rstest-bdd` 0.6 (feature files under
  `installer/tests/features/`, including `installer_release.feature` and
  `binstall_metadata.feature`), `proptest` 1, `insta` 1, `googletest` 0.14.3,
  `pretty_assertions` 1.4.1 (the last two are workspace dependencies not yet
  used by `whitaker-installer`). The root test `tests/behaviour_binstall.rs`
  checks the root table against the constants.
- Python: `tests/workflow_contracts/*_test.py`, run by
  `make test-workflow-contracts` with `pytest`, `pyyaml`, `ruamel.yaml`, and
  `hypothesis`. Shared helpers include `ubicloud_workflow_support.py`
  (`load_workflow`, `load_job`, `steps_by_name`). Release-relevant files:
  `release_binstall_contract_test.py`, `check_release_published_test.py`,
  `root_package_contract_test.py`, and `helper_binaries_feature_test.py`.
- Gates: `make check-fmt`, `make lint` (interrogate, skill manifests, rustdoc,
  Clippy, the Whitaker suite), `make test` (nextest; the default profile skips
  `behaviour_cli` and `behaviour_toolchain`, which CI runs with
  `NEXTEST_PROFILE=ci`), `make test-workflow-contracts`, `make markdownlint`
  (includes `make spelling`), and `make nixie`. Verus and Kani targets exist
  (`make verus`, `make kani`) but this plan adds no proofs (D-8).

### Signposts

Documentation: `docs/whitaker-cli-design.md` (§Public CLI surface,
§`whitaker check`, §Compatibility and migration),
`docs/adrs/adr-0012-whitaker-cli-boundary.md`,
`docs/adrs/adr-0001-prebuilt-dylint-library-distribution.md` (glibc baseline),
`docs/publishing.md`, `docs/users-guide.md`, `docs/developers-guide.md` (helper
binaries, `verify-binstall`), `docs/whitaker-dylint-suite-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/scripting-standards.md`, `docs/documentation-style-guide.md`. The
clone-detector design (`docs/whitaker-clone-detector-design.md`) has no bearing
on this item.

Skills: `rust-router`, then `rust-unit-testing` (fixtures, tables,
`googletest`), `proptest`, `domain-cli-and-daemons` (packager CLI),
`rust-errors` (new error variant), `nextest`, `python-testing` and `hypothesis`
(script tests), `pr-creation`, `commit-message`. Use `codegraph-mcp` or `leta`
for call-site discovery.

## Conformance basis

Upstream artefacts, at revision `755c1f2c`:

- `CLI-DESIGN`: `docs/whitaker-cli-design.md`. §Public CLI surface (the root
  package owns the real executable; `whitaker-installer` remains functional).
  §`whitaker check` (the workflow is `cargo binstall whitaker`, then
  `whitaker check`). §Compatibility and migration, step 1 (copy the working
  binstall metadata pattern from `whitaker-installer` onto `whitaker`).
- `ADR-0012`: `docs/adrs/adr-0012-whitaker-cli-boundary.md`, accepted
  2026-10-17. Its §Decision outcome says GitHub release archives for the root
  executable are not published until 3.5.3. Its §Known risks and limitations
  lists the absence of archives.
- `ADR-0001`: the glibc 2.35 baseline for x86_64 Linux release binaries.
- `ROADMAP-3.5.3` and its prerequisites 4.2.1 (binstall metadata) and 4.3.1
  (release workflow).
- No Terms of Reference document exists for this project; none is invented.

Requirements derived for this item:

- `REQ-ARCHIVES`: every release `v<V>` built by `release.yml` publishes,
  for each target whose leg succeeds, `whitaker-<target>-v<V>.tgz` (`.zip` for
  `x86_64-pc-windows-msvc`) containing exactly one regular file,
  `whitaker-<target>-v<V>/whitaker` (`whitaker.exe` on Windows), plus a
  `.sha256` sidecar.
- `REQ-BINSTALL`:
  `cargo binstall --disable-strategies compile,quick-install whitaker@<V>`
  installs that binary on Linux, macOS, and Windows.
- `REQ-MIRROR`: the `whitaker` archives use the installer's targets,
  runners, packager, checksum step, glibc check, partial-release policy,
  publication step, and verification gates.
- `REQ-LOCKSTEP`: a release proceeds only when the tag equals `v` followed by
  the versions of both `whitaker` and `whitaker-installer`.
- `REQ-CRATES`: the release workflow publishes `whitaker@<V>` to crates.io
  after `whitaker-installer@<V>` (D-4).
- `REQ-INSTALLER-UNCHANGED`: Constraint 2.
- `REQ-DOCS`: user, publishing, developer, design, and ADR documentation
  describe the shipped state.

Observed gaps:

- `GAP-PACKAGER`: `CRATE_NAME` is hard-coded (`installer_packaging.rs:31`).
- `GAP-BUILD`: no workflow builds or packages `-p whitaker`.
- `GAP-COMPAT`: `release-compatibility` neither requires nor runs a `whitaker`
  archive.
- `GAP-LOCKSTEP`: `publish` checks only the installer's version.
- `GAP-PUBLISH`: `release.yml` does not publish `whitaker` to crates.io.
- `GAP-VERIFY`: `verify-binstall` installs only `whitaker-installer`.
- `GAP-PROBE`: `verify-binstall-manifest` skips on a missing release, not on a
  missing asset, so it cannot cover the root manifest.
- `GAP-DOCS`: `docs/users-guide.md:66-70` and `:316-322`,
  `docs/publishing.md:162-166`, and ADR 0012 (§Decision outcome, §Known risks
  and limitations) describe archives as pending.

Trace links:

```plaintext
CLI-DESIGN §whitaker check -> REQ-BINSTALL -> EP-M3
  -> scripts/verify_binstall_local.py (CI: verify-binstall-local) + verify-binstall.yml matrix
CLI-DESIGN §Compat step 1 -> REQ-MIRROR -> EP-M1, EP-M2 -> release_cli_artefacts_contract_test.py + make release-installer-dry-run
ADR-0012 §Decision outcome -> REQ-ARCHIVES, GAP-BUILD -> EP-M2
  -> release.yml build-installer (whitaker steps) + INV-NAME, INV-MEMBER
ADR-0001 glibc baseline -> REQ-MIRROR, GAP-COMPAT -> EP-M2 -> release-compatibility (whitaker glibc + --version)
publishing.md lockstep -> REQ-LOCKSTEP, GAP-LOCKSTEP -> EP-M2 -> scripts/check_release_versions.py + INV-LOCKSTEP tests
D-4 -> REQ-CRATES, GAP-PUBLISH -> EP-M2 -> publish step contract test
D-6 -> GAP-PROBE -> EP-M3 -> check_release_published_test.py asset cases
GAP-PACKAGER -> EP-M1 -> INV-NAME, INV-MEMBER, INV-VERSION-SAFE
GAP-DOCS -> REQ-DOCS -> EP-M4 -> make markdownlint, make nixie, stale-claim search
```

## Verification plan

### Abstract model

Let `P = {whitaker, whitaker-installer}` be the release packages. Let `T` be
the five supported targets, and `W ⊂ T` the singleton
`{x86_64-pc-windows-msvc}`. A release version `v` is a non-empty string over
`[0-9A-Za-z.+-]`. The functions are:

- `ext(t)`: `zip` if `t ∈ W`, else `tgz`.
- `exe(t)`: `.exe` if `t ∈ W`, else empty.
- `dir(p, t, v) = p ++ "-" ++ t ++ "-v" ++ v`.
- `asset(p, t, v) = dir(p, t, v) ++ "." ++ ext(t)`.
- `member(p, t, v) = dir(p, t, v) ++ "/" ++ p ++ exe(t)`.

The specification side is binstall's view. Write `R(s, p, t, v)` for literal
substitution in template string `s` of `{name}` by `p`, `{version}` by `v`,
`{target}` by `t`, `{bin}` by `p`, and `{binary-ext}` by `exe(t)`. Write
`url_p(t)` and `bindir_p` for the template strings in package `p`'s manifest
(`url_p(t)` uses the override when `t ∈ W`). Binstall downloads
`R(url_p(t), p, t, v)` and expects `R(bindir_p, p, t, v)` inside it as a
regular file.

The implementation side is the packager: `package(p, t, v, bytes)` writes one
archive named `archive_filename(p, v, t)` containing one entry at
`inner_dir_name(p, v, t) ++ "/" ++ binary_filename(p, t)`.

A release is the set of assets uploaded under tag `g`. It proceeds only when
`g = "v" ++ version(whitaker) = "v" ++ version(whitaker-installer)`.

### Invariants and lemmas

- `INV-NAME` (naming correspondence; `REQ-ARCHIVES`, `REQ-BINSTALL`): for
  every `p ∈ P`, `t ∈ T`, and admissible `v`, `archive_filename(p, v, t)`
  equals the last path segment of `R(url_p(t), p, t, v)`. The URL prefix equals
  `https://github.com/leynos/whitaker/releases/download/v` followed by `v` and
  `/`. The packaged entry path equals `R(bindir_p, p, t, v)`.
- `INV-MEMBER` (archive layout; `REQ-ARCHIVES`): every packaged archive holds
  exactly one entry. That entry is a regular file at `member(p, t, v)` whose
  bytes equal the input binary's. On `tgz` it keeps the input's executable mode
  bits.
- `INV-NAMESPACE` (no clobbering; `REQ-ARCHIVES`, `REQ-INSTALLER-UNCHANGED`):
  for a fixed `v`, the ten names `asset(p, t, v)` are pairwise distinct. None
  equals a dependency-binary asset name or the Clippy snapshot bundle name in
  the same release. No glob that `release-compatibility` uses to select one
  package's archive matches another package's archive.
- `INV-INSTALLER-STABLE` (`REQ-INSTALLER-UNCHANGED`): for
  `p = whitaker-installer`, `archive_filename`, `inner_dir_name`,
  `binary_filename`, and `archive_format` return exactly what they returned
  before this change.
- `INV-VERSION-SAFE` (D-10): the packager refuses any version outside the
  admissible alphabet, or empty, before touching the file system. It accepts
  every SemVer 2.0.0 string.
- `INV-LOCKSTEP` (`REQ-LOCKSTEP`): the version check accepts exactly when the
  tag starts with `v` and its remainder equals the version of every named
  package. It fails on a missing package, a non-`v` tag, or any mismatch.
- `INV-RELEASE-COMPLETE` (`REQ-MIRROR`): when any x86_64 Linux artefact
  exists, `release-compatibility` requires exactly one
  `whitaker-x86_64-unknown-linux-gnu-v*.tgz`. It checks that archive's binary
  against `GLIBC_2.35` and runs `whitaker --version` successfully.
- `L-SUBST` (lemma connecting `INV-NAME` to `REQ-BINSTALL`): if the manifest's
  binstall table is byte-equal to the strings rendered in `INV-NAME`, and
  binstall renders by literal substitution (`AXIOM-BINSTALL-SUBST`), then
  binstall requests exactly `asset(p, t, v)` and looks for exactly
  `member(p, t, v)`. This is the argument that lets a test-local renderer stand
  in for binstall, and `INV-E2E` checks it against the real tool.
- `INV-E2E` (`REQ-BINSTALL`, pre-publication): the real `cargo-binstall`
  1.16.6 installs both `p ∈ P` for the host target. It uses archives produced
  by the real packager from real release builds, binstall tables copied
  verbatim from the real manifests (with only the URL origin
  `https://github.com/leynos/whitaker/releases/download/` replaced by a local
  HTTPS origin), and compile and quick-install disabled. The installed binary
  then reports version `v`.
- `INV-PUBLISHED` (`REQ-BINSTALL`, post-publication): `verify-binstall`
  installs `whitaker@<tag>` and `whitaker-installer@<tag>` from GitHub and
  crates.io on Linux, macOS, and Windows, and both report `--version`.

### Axioms (trusted, not verified here)

- `AXIOM-BINSTALL-SUBST`: binstall renders `pkg-url` and `bin-dir` by literal
  substitution of the placeholders above, and an exact-target override replaces
  the base keys for that target (`cargo-binstall` `SUPPORT.md`). Boundary: both
  manifests. Exercised against the real tool by `INV-E2E`.
- `AXIOM-BINSTALL-REGULAR`: binstall skips tar entries that are neither
  regular files nor directories (#461). Boundary: `create_tgz_archive`.
- `AXIOM-BINSTALL-HTTPS`: binstall refuses `http://` package URLs and accepts
  `https://` with an extra trust root from `--root-certificates`. Observed with
  1.22.0 during planning; `EP-M0` confirms it for 1.16.6. Boundary:
  `scripts/verify_binstall_local.py`.
- `AXIOM-BINSTALL-MANIFEST`: with `--manifest-path`, binstall takes metadata
  from that file and does not consult crates.io. Without it, binstall reads
  `whitaker@<V>`'s published manifest from crates.io. Boundary: D-4 and
  `INV-PUBLISHED`.
- `AXIOM-GH-ASSETS`: a GitHub release's asset names form one flat namespace,
  and `gh release upload --clobber` replaces a same-named asset.
- `AXIOM-CARGO-PUBLISH`: `cargo publish` returns only after the crate is
  visible in the index (Cargo 1.66 and later), so a dependent publish that
  follows it resolves.
- `AXIOM-RUNNER-GLIBC`: `ubuntu-22.04` provides glibc 2.35, so binaries built
  there pass the baseline check.

### Methods per obligation

`INV-NAME`:

- Method: exhaustive `rstest` table over all ten `(p, t)` pairs, combined with
  a `proptest` property over generated versions.
- Rationale: `P × T` is a finite closed domain, so enumerating it is complete.
  The version is unbounded, but it enters only by concatenation, which a
  property over the admissible alphabet exercises cheaply.
- Domain: `P × T` exhaustively. Versions are drawn from
  `(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})(-[0-9A-Za-z.-]{1,12})?(\+[0-9A-Za-z.-]{1,12})?`
  plus the fixed edge cases `0.0.0`, `0.3.0`, and `10.20.30-rc.1+build.5`.
- Oracle: a test-local renderer that reads the `pkg-url`, override, and
  `bin-dir` strings from the real root and installer manifests through
  `load_cargo_toml_from`. It applies `str::replace` per `L-SUBST`. It does not
  call `expand_pkg_url_for` or the packager, so it is independent of both.
- Artefact: `installer/tests/release_naming_properties.rs` (new). It needs
  the root manifest path, which is
  `Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml")`.
- Evidence:
  `cargo nextest run -p whitaker-installer --test release_naming_properties`.
  Red: the test does not compile until `ReleaseBinary` exists. Green: 10 table
  cases plus the property (256 cases by default) pass.
- Non-vacuity: the table covers both formats and both packages. The property
  classifies generated versions with `prop_assert` counters (pre-release,
  build-metadata, plain) and fails if any class is empty. Seeded faults:
  restore a hard-coded `"whitaker-installer"` in `archive_filename`, drop
  `.exe` from `binary_filename`, or swap `ext` for Windows. Each must fail the
  `whitaker` or Windows rows. Run each fault once, record the failure, and
  revert it.

`INV-MEMBER`:

- Method: integration tests through the real packager binary
  (`CARGO_BIN_EXE_whitaker-package-installer`) for all ten pairs, plus
  `rstest-bdd` scenarios for the user-visible cases.
- Rationale: the binary is the boundary the workflow calls. The packager
  writes either format on any host, so all ten pairs run everywhere.
- Artefact: new cases in `installer/tests/behaviour_packaging_cli.rs`, or a
  new sibling module if it would pass 400 lines. New scenarios go in
  `installer/tests/features/installer_release.feature` with steps in
  `installer/tests/behaviour_installer_release.rs`.
  `installer/src/archive_entry_type_tests.rs` is parametrized over `p`.
- Evidence: red until `--package` exists (clap rejects an unknown argument,
  `exit 2`). Green: every archive has exactly one entry, and it is regular with
  the expected path and bytes, checked with `googletest`'s `elements_are!` and
  `eq`.
- Non-vacuity: the fake binary has distinctive content and mode `0o755`. A
  fault that writes the entry under the `whitaker-installer` directory for
  `p = whitaker` must fail. So must re-enabling sparse entries (already covered
  by `archive_entry_type_tests`, now for both packages).

`INV-NAMESPACE`:

- Method: exhaustive `rstest` table, with versions drawn from the same
  property generator.
- Artefact: `installer/tests/release_naming_properties.rs`.
- Domain: the ten CLI asset names; the dependency-binary asset names, built
  through `dependency_binaries::archive_filename` from the real
  `dependency-binaries.toml`; the bundle name pattern
  `whitaker-clippy-snapshot-*.bundle`; and the `release-compatibility` globs,
  as fixed strings mirrored in the test.
- Evidence: all names are distinct, and each glob matches exactly the
  intended asset (a small test-local `fnmatch`).
- Non-vacuity: the test includes a positive control. The pair `whitaker` and
  `whitaker-installer` share the prefix `whitaker-`, and the test asserts that
  the naive glob `whitaker-*-x86_64-unknown-linux-gnu-v*.tgz` does match both,
  so the matcher is shown to detect overlap. The `release-compatibility` glob,
  `whitaker-x86_64-unknown-linux-gnu-v*.tgz`, must then match only one of them.

`INV-INSTALLER-STABLE`:

- Method: the existing tests in `installer_packaging_tests.rs`,
  `binstall_metadata_tests.rs`, `installer/tests/behaviour_binstall.rs`, and
  `installer_release.feature` stay green, with call sites changed only to pass
  `ReleaseBinary::WhitakerInstaller`. In addition, an `insta` snapshot records
  the full naming tuple for `whitaker-installer` across the five targets at
  version `0.3.0`, taken before the refactor and asserted after it.
- Non-vacuity: the snapshot is created on the unmodified tree in the red
  step. Any change to an installer name fails it.

`INV-VERSION-SAFE`:

- Method: `proptest`. One property generates strings containing at least one
  character outside the alphabet and expects `InvalidVersion`. One generates
  admissible SemVer strings (the `INV-NAME` generator) and expects acceptance.
  A named `rstest` table covers `""`, `"../0.3.0"`, `"0.3.0/x"`, `"0.3.0\\x"`,
  and `"0.3.0 "`.
- Artefact: `installer/src/release_version_tests.rs` (new; the predicate
  lives in `installer_packaging.rs` or a small sibling module if the file would
  pass 400 lines).
- Evidence: the CLI exits 1 with
  `error: invalid release version "../0.3.0": ...` and writes no file. The test
  checks that the output directory stays empty.
- Non-vacuity: the rejection property's generator inserts the bad character
  at a random position, and a classification counter confirms that `/`, `\`,
  whitespace, and non-ASCII each occur.

`INV-LOCKSTEP`:

- Method: `pytest` with named cases plus a `hypothesis` property, against
  `scripts/check_release_versions.py`. The script reads
  `cargo metadata --no-deps --format-version 1 --locked` JSON (injected as a
  file in tests), takes `--tag` and repeated `--package`, and exits non-zero
  with a named reason on mismatch.
- Artefact: `scripts/check_release_versions.py` (new) and
  `tests/workflow_contracts/check_release_versions_test.py` (new).
- Evidence: named cases are accepted, a missing `v`, a mismatched root, a
  mismatched installer, and an absent package. The property: for generated
  version triples, the script accepts exactly when all are equal to the tag's.
- Non-vacuity: the property generator is biased so that roughly half the
  cases are equal. It records `hypothesis.event` for "accepted" and "rejected",
  and the test asserts that both events occur.

`INV-RELEASE-COMPLETE`, `REQ-MIRROR`, `REQ-CRATES`, and D-6 workflow wiring:

- Method: workflow contract tests in
  `tests/workflow_contracts/release_cli_artefacts_contract_test.py` (new),
  following `release_binstall_contract_test.py`'s pattern of a violations
  function plus parametrized refusals. They check that:
  - the build leg builds both packages and packages each with the right
    `--package`;
  - the glibc check names both binaries;
  - `release-compatibility` requires exactly one `whitaker` x86_64 archive
    and runs it;
  - `publish` calls `check_release_versions.py` with both packages before
    upload;
  - the crates.io step publishes `whitaker` after `whitaker-installer`;
  - `verify-binstall.yml` installs both packages.
- Non-vacuity: each refusal case mutates a loaded copy of the workflow, for
  example by deleting the `whitaker` package step or reordering the publishes,
  and expects a named violation, as the existing contract test does.

`INV-E2E`:

- Method: an end-to-end script, `scripts/verify_binstall_local.py` (new,
  Cyclopts and plumbum per `docs/scripting-standards.md`). It takes a `dist/`
  directory and the two manifest paths. For each package it writes a
  synthesized manifest with `[package]` name and version, one `[[bin]]`, and
  the real `[package.metadata.binstall]` table copied verbatim. The only change
  is that the exact origin
  `https://github.com/leynos/whitaker/releases/download/` becomes
  `https://127.0.0.1:<port>/`; if that origin is absent, the script fails. It
  serves `dist/` under `v<version>/` over HTTPS with a throwaway self-signed
  certificate, runs `cargo binstall` with `--no-confirm`, `--no-track`,
  `--disable-strategies compile,quick-install`, `--root-certificates <cert>`,
  `--manifest-path <synth>`, `--root <tmp>`, `--install-path <tmp>/bin`, and
  `--targets <host>` for `<p>@<v>`, and checks that `<p> --version` reports
  `<v>`.
- Pure parts (manifest synthesis and origin rewrite) have `pytest` and
  `hypothesis` tests in
  `tests/workflow_contracts/verify_binstall_local_test.py`. The script runs for
  real in a new `verify-binstall-local` job in `verify-binstall.yml` on
  `ubuntu-latest` and `macos-latest`. That job is triggered by pull requests
  touching the packager, either manifest, `release.yml`, `verify-binstall.yml`,
  the script, or the Makefile, and it builds through
  `make release-installer-dry-run DIST_DIR=…`.
- Non-vacuity: the planning probe showed that a wrong `bin-dir` makes binstall
  exit 94 ("Fallback to cargo-install is disabled"). The script's tests include
  a `--negative-control` mode that corrupts `bin-dir` in the synthesized
  manifest and requires that failure. The CI job runs it once after the
  positive run.
- Residual gap: Windows is not covered before publication, because the
  script's OpenSSL and Python TLS server dependencies are not portable to the
  `windows-compat` job without new tooling. Windows `zip` naming and layout are
  covered by `INV-NAME` and `INV-MEMBER` on every host, and Windows binstall
  resolution by `INV-PUBLISHED`.

`INV-PUBLISHED`:

- Method: `verify-binstall.yml`'s `verify-binstall` job gains a package
  matrix dimension (`whitaker-installer`, `whitaker`). The `whitaker` cell
  installs and runs `whitaker --version`. The bootstrap steps stay
  installer-only.
- Evidence: available only on the next tag. Record the run URL in
  `Outcomes & retrospective` when it exists; until then, this obligation
  remains open and is stated so in the PR description.

### Formal verification decision

No obligation here warrants Verus or Kani, and none is added (D-8).

The contractual logic is a naming function over a closed, finite domain (two
packages by five targets) concatenated with an unbounded version string, and a
version-equality check. `INV-NAME` is a correspondence between two
string-rendering procedures. The exhaustive table discharges the finite part
completely. The version enters both procedures only by concatenation, so the
property test's broad generator, with an independent oracle, gives strong
evidence for the unbounded part, and `INV-E2E` pins the oracle to the real tool.

A Verus proof of injectivity over all versions would establish `INV-NAMESPACE`
without bound. The 3.5.1 plan rejected an injectivity proof of exactly this
kind, because nothing inverts an asset name. Here the non-colliding property is
load-bearing (`--clobber`), but it is fully decided by prefix-freeness of a
ten-element set at a fixed version. The exhaustive table checks that, and it
holds for every version because every asset in a release shares the version.

A proof would also model `str::replace` and `format!` rather than the code
binstall actually runs, so it would leave the real boundary (binstall's
renderer, archive extraction, GitHub hosting) unverified. The end-to-end check
covers that boundary. Kani adds nothing over an exhaustive table for a
ten-element domain, and symbolic strings would make it slow.

If review identifies an obligation that genuinely ranges over an unbounded
structure the tests cannot enumerate, the plan returns to this section before
implementation.

### Suggested BDD features

Proposed additions to `installer/tests/features/installer_release.feature`, run
by `installer/tests/behaviour_installer_release.rs` (`rstest-bdd` 0.6), tracing
to `REQ-ARCHIVES`, `INV-NAME`, `INV-MEMBER`, and `EP-M1`:

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

  Scenario: Root CLI archive matches the root manifest's binstall templates
    Given package "whitaker", version "0.4.0" and target "aarch64-apple-darwin"
    And a fake binary exists
    When the binary is packaged
    Then the root manifest's pkg-url ends with the archive filename
    And the root manifest's bin-dir names the archive's only entry

  Scenario: Packaging refuses a version that escapes the output directory
    Given package "whitaker", version "../0.4.0" and target "x86_64-unknown-linux-gnu"
    And a fake binary exists
    When packaging is attempted
    Then an invalid-version error is returned
    And the output directory is empty
```

The existing scenarios change their `Given` wording to name
`"whitaker-installer"` explicitly; their expected strings do not change. A
proposed acceptance scenario for the end-to-end check, exercised by
`verify-binstall-local` rather than an `rstest-bdd` runner, is:

```gherkin
Feature: Installing the root CLI from its release archive
  Scenario: cargo-binstall installs whitaker from the packaged archive
    Given the release archives for the host target are served over HTTPS
    And a manifest carrying the root package's binstall table
    When cargo binstall installs "whitaker" with compile and quick-install disabled
    Then the installed "whitaker --version" reports the manifest version

  Scenario: a broken bin-dir cannot fall back to a source build
    Given the same archives and a manifest whose bin-dir names a missing path
    When cargo binstall installs "whitaker" with compile and quick-install disabled
    Then cargo binstall exits with status 94
```

## Plan of work

### `EP-M0` feasibility probe (prototype, deleted afterwards)

Question: does `cargo-binstall` 1.16.6 accept `--root-certificates` and install
from a synthesized manifest whose package URL points at `https://127.0.0.1`?
And does a host release build of `-p whitaker` succeed with `--locked`?

Run the planning probe from `Artefacts and notes` with `cargo binstall` 1.16.6.
Install it into a scratch root with
`cargo binstall --root /tmp/binstall-1.16.6 cargo-binstall@1.16.6`. Use
`cargo-binstall` itself, not a workspace build, and never `~/.cargo`. Then run
`cargo build --locked --release -p whitaker-installer -p whitaker` and record
the wall time.

Go if the positive run installs and the `bin-dir` negative control exits 94.
No-go if 1.16.6 lacks the flag. In that case pin the local check to the oldest
version that passes and record a decision; do not change the `verify-binstall`
pin. The scratch files live under `/tmp` and are deleted.

### `EP-M1` the packager packages either executable

Assigned: `GAP-PACKAGER`, `INV-NAME`, `INV-MEMBER`, `INV-NAMESPACE`,
`INV-INSTALLER-STABLE`, `INV-VERSION-SAFE`; D-2, D-3, D-10.

Red:

1. Add the `insta` snapshot for `INV-INSTALLER-STABLE` on the unmodified tree
   and accept it. It passes now and guards the refactor.
2. Add `installer/tests/release_naming_properties.rs`,
   `installer/src/release_version_tests.rs`, the new BDD scenarios and steps,
   and the parametrized packager CLI cases. They fail to compile or fail because
   `ReleaseBinary`, the `--package` flag, and `InvalidVersion` do not exist.
   Record the failures.

Green:

1. In `installer/src/installer_packaging.rs`, remove `CRATE_NAME` and add:

   ```rust
   /// An executable that the release workflow packages for cargo-binstall.
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
   pub enum ReleaseBinary {
       /// The root `whitaker` command-line interface.
       Whitaker,
       /// The legacy `whitaker-installer` executable.
       WhitakerInstaller,
   }

   impl ReleaseBinary {
       /// Return the package name, which is also the binary name.
       #[must_use]
       pub const fn package_name(self) -> &'static str { /* … */ }
   }
   ```

   `clap` (with `derive`) is already an unconditional dependency of the
   `whitaker-installer` library (`installer/Cargo.toml:80`), so deriving
   `ValueEnum` adds no dependency edge. The `clap` value names must be exactly
   `whitaker` and `whitaker-installer`; a unit test asserts this.
2. Thread `ReleaseBinary` through `archive_filename(binary, version, target)`,
   `inner_dir_name(binary, version, target)`, and
   `binary_filename(binary, target)`. Add `pub binary: ReleaseBinary` to
   `InstallerPackageParams`. `archive_format` is unchanged. Update every call
   site: `installer/src/installer_packaging_tests.rs`,
   `installer/src/archive_entry_type_tests.rs`,
   `installer/tests/behaviour_installer_release.rs`, and the doctests.
3. Add the variant `InvalidVersion { version: String, reason: &'static str }`
   to `InstallerPackagingError`, and a predicate
   `validate_release_version(&Version) -> Result<(), InstallerPackagingError>`.
   Call it first in `package_installer`.
4. In `installer/src/bin/package_installer_bin.rs`, add a required
   `#[arg(long, value_enum)] package: ReleaseBinary` and pass it through.
5. Update the two existing callers in the same commit, with no behaviour
   change: `release.yml` "Package archive" and the Makefile
   `release-installer-dry-run` both pass `--package whitaker-installer`.
6. Add `googletest` and `pretty_assertions` to `[dev-dependencies]` in
   `installer/Cargo.toml` (workspace references).

Refactor: widen the module, binary, and function documentation (Constraint 8),
and keep every file under 400 lines.

Focused commands:

```sh
cargo nextest run -p whitaker-installer --all-features \
  -E 'test(installer_packaging) | test(archive_entry_type) | test(release_version)
      | binary(release_naming_properties) | binary(behaviour_installer_release)
      | binary(behaviour_packaging_cli)'
cargo test --doc -p whitaker-installer --all-features installer_packaging
```

Then the milestone gates (see `Concrete steps`). Run the seeded faults from
`INV-NAME` and `INV-MEMBER` once each, record them in `Artefacts and notes`,
and revert them.

### `EP-M2` the release pipeline builds, checks, and publishes `whitaker`

Assigned: `GAP-BUILD`, `GAP-COMPAT`, `GAP-LOCKSTEP`, `GAP-PUBLISH`,
`REQ-MIRROR`, `REQ-LOCKSTEP`, `REQ-CRATES`, `INV-LOCKSTEP`,
`INV-RELEASE-COMPLETE`; D-1, D-4.

Red: add `tests/workflow_contracts/release_cli_artefacts_contract_test.py`,
`scripts/check_release_versions.py` as a stub that exits 2, and
`tests/workflow_contracts/check_release_versions_test.py`. Run
`make test-workflow-contracts` and record the named violations and failures.

Green:

1. `scripts/check_release_versions.py`: implement per `INV-LOCKSTEP`, with
   numpy-style docstrings and a usage example in the module docstring.
2. `release.yml` `build-installer`:
   - Replace "Read version from Cargo.toml" with a call to
     `scripts/check_release_versions.py --tag "$RELEASE_TAG" --package
     whitaker-installer --package whitaker --github-output`. It writes
     `version=<v>` once all packages agree and fails the leg otherwise.
   - Build with
     `cargo build --locked -p whitaker-installer -p whitaker --release --target`.
   - Determine both binary paths.
   - Package each with its `--package`.
   - Extend the glibc check to both binaries.
   - Add a comment above the job explaining that it builds both executables
     and why the "installer" name stays (D-2).
3. `release-compatibility`: select
   `dist/whitaker-x86_64-unknown-linux-gnu-v*.tgz` as a separate array, require
   exactly one when any x86_64 artefact exists, add its binary to the glibc
   check, and run `whitaker --version`.
4. `publish`: replace the installer-only tag check with
   `check_release_versions.py --tag … --package whitaker-installer --package
   whitaker`.
   Append `cargo publish -p whitaker` after
   `cargo publish -p whitaker-installer`.
5. Makefile `release-installer-dry-run`: build both packages, package both,
   and generate checksums. Check that exactly two archives and two sidecars
   exist, extract the `whitaker` archive, and run the extracted binary's
   `--version`. Accept an optional `DIST_DIR` (default: the existing temporary
   directory) so `EP-M3`'s job can reuse the output. Update the recipe help
   text to say it packages both executables.
6. Update `tests/workflows/test_release_installer_dry_run_makefile.py` and
   `tests/workflows/test_release_workflow_healing.py` so they stay correct.
   They are opt-in, so the guards live in the contract tests.

Focused commands:

```sh
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make release-installer-dry-run 2>&1 | tee /tmp/release-dry-run-whitaker-3-5-3-publish-whitaker-release-artefacts.out
```

Expected dry-run tail on Linux:

```plaintext
Created /tmp/…/dist/whitaker-installer-x86_64-unknown-linux-gnu-v0.3.0.tgz
Created /tmp/…/dist/whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz
whitaker 0.3.0
```

### `EP-M3` binstall resolution is verified before and after publication

Assigned: `GAP-VERIFY`, `GAP-PROBE`, `REQ-BINSTALL`, `INV-E2E`,
`INV-PUBLISHED`; D-6.

Red: add `tests/workflow_contracts/verify_binstall_local_test.py` and the new
asset cases in `tests/workflow_contracts/check_release_published_test.py`. The
cases are: the release exists and the asset exists, which is published; the
release exists and the asset is missing, which is not published; the release is
missing, which is not published; any other failure fails. Extend
`release_cli_artefacts_contract_test.py` with the `verify-binstall.yml`
expectations. All fail first.

Green:

1. `scripts/check-release-published.sh VERSION [ASSET]`: with `ASSET`, use
   `gh api repos/$GITHUB_REPOSITORY/releases/tags/v$VERSION --jq
   '.assets[].name'`
   and match the name exactly. A missing asset prints `published=false`. The
   existing two-argument behaviour is unchanged.
2. `scripts/verify_binstall_local.py`: implement per `INV-E2E`, including
   `--negative-control`.
3. `verify-binstall.yml`:
   - `verify-binstall` gains `package: [whitaker-installer, whitaker]` in its
     matrix. The `whitaker` cell runs the install and `whitaker --version`
     only; the installer's bootstrap steps are conditioned on `package ==
     'whitaker-installer'`.
   - `verify-binstall-manifest` gains the same package dimension, using
     `--manifest-path Cargo.toml` for `whitaker`. It probes the expanded asset
     name for the runner's target, computed by the script from the manifest,
     not hand-written.
   - Add a `verify-binstall-local` job (pull requests; Linux and macOS) that
     runs `make release-installer-dry-run DIST_DIR=…`, then the local script,
     then its negative control.
   - Extend the `pull_request.paths` list to the packager sources,
     `release.yml`, `Makefile`, and the two scripts.
   - Install `cargo-binstall` with the same pinned
     `taiki-e/install-action` step and version.
4. Record the `EP-M0` decision on the binstall version in the workflow
   comment.

Focused commands:

```sh
make test-workflow-contracts 2>&1 | tee /tmp/test-workflow-contracts-whitaker-3-5-3-publish-whitaker-release-artefacts.out
make release-installer-dry-run DIST_DIR=/tmp/whitaker-3-5-3-dist
scripts/verify_binstall_local.py --dist /tmp/whitaker-3-5-3-dist \
  --root-manifest Cargo.toml --installer-manifest installer/Cargo.toml
scripts/verify_binstall_local.py --dist /tmp/whitaker-3-5-3-dist \
  --root-manifest Cargo.toml --installer-manifest installer/Cargo.toml --negative-control
```

Expected: the positive run prints both installed versions and exits 0; the
negative control reports binstall's exit 94 and itself exits 0.

### `EP-M4` documentation and delivery closure

Assigned: `GAP-DOCS`, `REQ-DOCS`; D-9.

1. `docs/whitaker-cli-design.md`: add a "Release artefacts" subsection under
   §Compatibility and migration. It covers the archive naming and layout table
   for both executables, the shared build leg, lockstep versioning, the
   crates.io order, the verification gates, and the deferred renames (D-2,
   pointing at 3.9.3). Extend footnote 7 if needed.
2. `docs/adrs/adr-0012-whitaker-cli-boundary.md`: add a dated implementation
   note ("2026-MM-DD, roadmap 3.5.3") recording that release archives now exist
   from the next tag onward and that v0.3.0 is not backfilled. The accepted
   decision text stays as it is.
3. `docs/users-guide.md`:
   - Rewrite §The shortest path's closing paragraph and §Root CLI. Give
     `cargo binstall whitaker` as the primary install route, keep
     `cargo install --path .` for checkouts, and say that 0.3.0 still builds
     from source.
   - Add the archive names to any table of release assets.
4. `docs/publishing.md`:
   - Replace the "does not publish GitHub release archives" paragraph.
   - Document that `release.yml` publishes all three crates in order, and
     that the tag must match both versions.
   - Add `whitaker` checks to "After publishing" (`verify-binstall` for both
     packages).
5. `docs/developers-guide.md`: in the helper-binaries and `verify-binstall`
   sections, document `--package`, `ReleaseBinary`, version validation,
   `scripts/check_release_versions.py`, `scripts/verify_binstall_local.py`, and
   the `verify-binstall-local` job. Record the reuse policy:
   `installer_packaging` is the only packager for the two Whitaker executables,
   and dependency binaries keep `dependency_packaging` until the follow-up from
   D-7.
6. `docs/repository-layout.md`: mention the two new scripts if it lists
   `scripts/` contents.
7. `docs/contents.md`: add this ExecPlan.
8. `docs/roadmap.md`: mark 3.5.3 done (`[x]`) only after every acceptance
   item below passes, except `INV-PUBLISHED`, which is post-release by
   construction and stated as such.
9. Open a follow-up issue for D-7, deduplicating the archive writers. Do not
   implement it here.

Stale-claim search (exclude `docs/execplans/`):

```sh
rg -n '3\.5\.3|binstall archives are pending|not published until' \
  docs README.md --glob '!docs/execplans/**'
rg -n 'fall(s)? back to (a )?(building|source)' docs README.md --glob '!docs/execplans/**'
```

Every hit is either updated or justified in `Decision log`.

## Milestones and plateaus

`EP-M0`, feasibility probe:

- Outcome: binstall 1.16.6 behaviour and the host build time are known.
- Requirements: `AXIOM-BINSTALL-HTTPS`.
- Acceptance evidence: the probe transcript in `Artefacts and notes`.
- Conformance check: no repository change.
- Recovery: none needed.
- Remaining gaps: everything.
- Compatibility decision: none.

`EP-M1`, the packager packages either executable:

- Outcome: `whitaker-package-installer --package whitaker …` writes a
  binstall-shaped `whitaker` archive. The installer archive is unchanged, and
  release and dry-run callers pass `--package whitaker-installer`.
- Requirements: `GAP-PACKAGER`; `INV-NAME`, `INV-MEMBER`, `INV-NAMESPACE`,
  `INV-INSTALLER-STABLE`, `INV-VERSION-SAFE` discharged.
- Acceptance evidence: the focused tests above pass, with red evidence and
  seeded-fault evidence recorded; `make check-fmt`, `make lint`, and
  `make test` pass.
- Conformance check:
  - `REQ-INSTALLER-UNCHANGED` holds (the snapshot is unchanged).
  - The design is still followed (D-1 to D-3).
  - No upstream assumption is falsified.
  - The public API changes only within Constraint 7.
  - There is no trust-boundary or persisted-format change.
  - Trace links are current.
- Recovery: revert the milestone's commits; the release workflow is still
  installer-only and correct.
- Remaining gaps: nothing builds `whitaker` in the release yet.
- Compatibility decision: none (Constraint 7; all callers are updated in the
  same commit).

`EP-M2`, the release pipeline builds, checks, and publishes `whitaker`:

- Outcome: a tag would produce, check, upload, and publish `whitaker`
  artefacts and crate. The Makefile dry run proves it on the host.
- Requirements: `GAP-BUILD`, `GAP-COMPAT`, `GAP-LOCKSTEP`, `GAP-PUBLISH`;
  `INV-LOCKSTEP` and `INV-RELEASE-COMPLETE` discharged.
- Acceptance evidence: `make test-workflow-contracts` passes, including the
  refusal cases; `make release-installer-dry-run` passes as shown;
  `make check-fmt`, `make lint`, and `make test` pass.
- Conformance check:
  - The installer steps are unchanged except for `--package`.
  - D-4 is approved.
  - The glibc baseline is unchanged.
  - No new dependency is added.
- Recovery: revert; `EP-M1`'s plateau is intact.
- Remaining gaps: binstall resolution is not yet exercised.
- Compatibility decision: none.

`EP-M3`, binstall resolution verified:

- Outcome: a pull request that breaks either table, the packager, or the
  dry run fails `verify-binstall-local`. The next release proves `whitaker`
  binstall on three operating systems.
- Requirements: `GAP-VERIFY`, `GAP-PROBE`; `INV-E2E` discharged;
  `INV-PUBLISHED` wired (evidence pending release).
- Acceptance evidence: the local positive and negative-control transcripts;
  `make test-workflow-contracts`, `make check-fmt`, `make lint`, and
  `make test` pass; the PR's `verify-binstall-local` job is green on both
  runners.
- Conformance check: the verification gates mirror the installer's
  (`REQ-MIRROR`).
- Recovery: revert; `EP-M2` is intact (post-publication verification remains
  installer-only).
- Remaining gaps: `INV-PUBLISHED` evidence.
- Compatibility decision: none.

`EP-M4`, documentation and closure:

- Outcome: documentation describes the shipped state, and the roadmap item
  is done.
- Requirements: `GAP-DOCS`, `REQ-DOCS`.
- Acceptance evidence: `make markdownlint`, `make nixie`, and
  `make check-fmt` pass; the stale-claim search is clean; roadmap 3.5.3 is
  `[x]`.
- Conformance check: ADR 0012's decision is preserved and a note is added;
  every `Surprises` entry is reconciled.
- Recovery: documentation only.
- Remaining gaps: `INV-PUBLISHED` evidence; the D-7 follow-up issue.
- Compatibility decision: none.

## Concrete steps

Working directory: the repository root of this worktree. Enable `pipefail` in
every shell that pipes a gate through `tee`. Log names follow
`/tmp/<action>-whitaker-3-5-3-publish-whitaker-release-artefacts.out`.

Gate sequence after each milestone, run sequentially (sub-agents do not run
full gates; `scrutineer` may):

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

`make test-doc` also runs after `EP-M1`, because doctests on the naming
functions change. `make test NEXTEST_PROFILE=ci` runs once before the PR is
marked ready, for parity with CI. Compiles queue behind the shared
`build-limits` admission pool: a waiting build is not hung. Run long gates from
the main session in the background with a generous timeout, and check
`build-limits-status` before blaming the candidate for scattered failures.

Red, green, and refactor evidence per milestone is recorded in
`Artefacts and notes` as it happens.

## Validation and acceptance

The work is accepted when:

- `whitaker-package-installer --package whitaker --crate-version 0.4.0
  --target x86_64-pc-windows-msvc --binary-path <fake.exe> --output-dir d`
  creates `d/whitaker-x86_64-pc-windows-msvc-v0.4.0.zip`, whose only entry is
  `whitaker-x86_64-pc-windows-msvc-v0.4.0/whitaker.exe`.
- The same command with `--crate-version ../0.4.0` exits 1 with an
  invalid-version error and leaves `d` empty.
- `make release-installer-dry-run` on Linux creates both archives and both
  sidecars, and the extracted `whitaker --version` prints `whitaker 0.3.0`.
- `scripts/verify_binstall_local.py` installs both packages from the dry
  run's output with the pinned `cargo-binstall`, and its negative control
  observes exit 94.
- The new and changed tests pass: `release_naming_properties`,
  `release_version_tests`, the new `installer_release.feature` scenarios, the
  packager CLI cases, `release_cli_artefacts_contract_test.py`,
  `check_release_versions_test.py`, `verify_binstall_local_test.py`, and
  `check_release_published_test.py`. Each failed for the intended reason before
  its implementation.
- Every pre-existing installer packaging, binstall, and release test passes
  unchanged in its expectations.
- `make check-fmt`, `make lint`, `make test`, `make test-workflow-contracts`,
  `make markdownlint`, and `make nixie` pass on the final tree.
- On the PR, `verify-binstall-local` and `verify-binstall-manifest` are green.
  The latter skips `whitaker` with a notice naming the missing v0.3.0 asset.

Quality criteria:

- Tests: as above, under both the default and `ci` nextest profiles.
- Verification: `INV-NAME`, `INV-MEMBER`, `INV-NAMESPACE`,
  `INV-INSTALLER-STABLE`, `INV-VERSION-SAFE`, `INV-LOCKSTEP`,
  `INV-RELEASE-COMPLETE`, and `INV-E2E` are discharged with non-vacuity
  evidence. `INV-PUBLISHED` is wired, and its evidence follows the next release.
- Lint: `make lint` is clean, with no new `#[expect]` unless justified
  in-line.
- Security: the packager rejects path-escaping versions; the local check
  binds only to `127.0.0.1` and deletes its throwaway key; no token is added to
  any new job beyond `GITHUB_TOKEN` read access.

## Idempotence and recovery

Every milestone is a sequence of ordinary commits. Re-running a gate is safe.
The dry run writes only to a temporary directory or to the given `DIST_DIR`.
The local binstall check installs into a temporary `--root` with `--no-track`,
binds `127.0.0.1` on an ephemeral port, and removes its directory and key on
exit, including on failure (`try`/`finally`). Nothing in this plan pushes a
tag, creates a release, or publishes a crate. If a milestone fails its gates
after three attempts at the same cause, stop, record the state in `Progress` and
`Decision log`, and escalate.

## Artefacts and notes

Planning probe (2026-11-27, `cargo-binstall` 1.22.0, scratch directory under
`/tmp`, since deleted). A fake `whitaker` script was packaged as
`whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz` with the real `bin-dir` layout,
and a synthesized manifest pointed `pkg-url` at a local server.

Plain HTTP:

```plaintext
WARN Fail to resolve the future err=Download(Remote(Http(HttpError { method: GET,
  url: "http://127.0.0.1:18765/whitaker-x86_64-unknown-linux-gnu-v0.3.0.tgz", … source: BadScheme })))
ERROR Fatal error: For crate whitaker: Fallback to cargo-install is disabled
```

HTTPS with `--root-certificates cert.pem`:

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

Release state on 2026-11-27: crates.io `whitaker` `max_version` 0.3.0. The
v0.3.0 GitHub release has the installer, dependency-binary, and Clippy-snapshot
assets, and no `whitaker-*` asset.

## Interfaces and dependencies

In `installer/src/installer_packaging.rs` (`whitaker_installer`), at the end of
`EP-M1`:

```rust
pub enum ReleaseBinary { Whitaker, WhitakerInstaller }
impl ReleaseBinary {
    pub const ALL: [ReleaseBinary; 2];
    pub const fn package_name(self) -> &'static str;
}
pub struct InstallerPackageParams {
    pub binary: ReleaseBinary,
    pub version: Version,
    pub target: TargetTriple,
    pub binary_path: PathBuf,
    pub output_dir: PathBuf,
}
pub fn archive_filename(binary: ReleaseBinary, version: &Version, target: &TargetTriple) -> String;
pub fn inner_dir_name(binary: ReleaseBinary, version: &Version, target: &TargetTriple) -> String;
pub fn binary_filename(binary: ReleaseBinary, target: &TargetTriple) -> String;
pub fn archive_format(target: &TargetTriple) -> ArchiveFormat; // unchanged
pub fn validate_release_version(version: &Version) -> Result<(), InstallerPackagingError>;
pub fn package_installer(params: &InstallerPackageParams)
    -> Result<InstallerPackageOutput, InstallerPackagingError>;
pub enum InstallerPackagingError {
    Io(std::io::Error),
    BinaryNotFound(PathBuf),
    Zip(zip::result::ZipError),
    InvalidVersion { version: String, reason: &'static str },
}
```

Ownership and reuse policy: `installer_packaging` is the single owner of
release-archive naming and layout for the two Whitaker executables. The
workflow and Makefile reach it only through the packager binary. Tests may call
the library directly. No workflow step may compute an archive name by hand
except the `release-compatibility` selection globs, which `INV-NAMESPACE` pins.
`binstall_metadata` remains the owner of the template constants. Dependency
binaries keep `dependency_packaging` (D-7).

Packager command line at the end of `EP-M1`:

```plaintext
whitaker-package-installer --package <whitaker|whitaker-installer> \
  --crate-version <VERSION> --target <TRIPLE> --binary-path <PATH> --output-dir <DIR>
```

New scripts, both `uv run --script`, Python 3.13 or later:

- `scripts/check_release_versions.py`, taking `--tag <vX>`, a repeated
  `--package <name>`, and optional `--metadata <json-file>` and
  `--github-output`. It exits 0 and prints `version=<X>` when consistent. It
  exits 1 with `release version mismatch: …` otherwise, and exits 2 on usage
  errors.
- `scripts/verify_binstall_local.py`, taking `--dist <dir>`,
  `--root-manifest <path>`, `--installer-manifest <path>`, and optional
  `--target <triple>`, `--binstall <path>`, and `--negative-control`.

Dependencies: no new runtime dependency. `googletest` and `pretty_assertions`
become dev-dependencies of `whitaker-installer` (workspace versions 0.14.3 and
1.4.1). Python scripts use the standard library plus `cyclopts` and `plumbum`,
as `docs/scripting-standards.md` prescribes and as
`scripts/check_dependency_binary_assets.py` already does. CI tools are
`cargo-binstall` 1.16.6 through the pinned `taiki-e/install-action`, and the
runner images' `openssl`.

## Revision note

- 2026-11-27: initial draft from Wyvern reconnaissance, crates.io and GitHub
  release checks, and the binstall HTTPS probe. Awaiting expert review.
