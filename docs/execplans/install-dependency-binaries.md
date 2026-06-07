# Publish installer dependency binaries from repository releases

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Whitaker currently assumes `cargo-dylint` and `dylint-link` can be installed on
the end-user machine with `cargo binstall` or `cargo install`. That keeps the
installer simple, but it leaves three gaps:

1. the installer always depends on the end user's Cargo network path for these
   two tools;
2. GitHub Releases do not currently expose prebuilt copies of the exact
   dependency-tool versions Whitaker expects; and
3. the repository does not publish a committed, auditable record of which
   dependency-tool versions are required.

After this change, the repository will publish prebuilt `cargo-dylint` and
`dylint-link` binaries for the same five supported targets already used for
`whitaker-installer` and rolling lint artefacts. The required versions will be
committed to `main` in a repository-owned manifest, release pages will attach
those binary archives plus a companion licence-and-provenance document, and the
installer will attempt to download those prebuilt dependency binaries before it
falls back to `cargo binstall` or `cargo install`.

Observable outcome:

1. A maintainer changing the committed dependency-binary version manifest on
   `main` triggers fresh rolling-release builds for `cargo-dylint` and
   `dylint-link`.
2. A tagged release also uploads the same dependency-binary archives and the
   shared licence/provenance asset to the GitHub Release page alongside the
   existing `whitaker-installer` archives.
3. Running `whitaker-installer` on a supported platform prefers the repository
   asset path for missing `cargo-dylint` and `dylint-link`, and only invokes
   `cargo binstall` or `cargo install` if the repository-hosted install path is
   unavailable or invalid.
4. Unit tests and `rstest-bdd` 0.5.0 behavioural tests cover success, fallback,
   and failure cases.
5. The design document records the final decisions and the roadmap tracks the
   work as done.
6. `make check-fmt`, `make lint`, and `make test` all succeed before the
   implementation is considered complete.

## Constraints

- Keep the scope focused on repository-hosted dependency binaries for
  `cargo-dylint` and `dylint-link`. Do not expand this task into a general
  package manager or arbitrary third-party binary installer.
- Preserve the current installer contract: repository download first, then
  `cargo binstall`, then `cargo install` as the last resort. Any repository
  failure must degrade cleanly into the existing Cargo-based path.
- The canonical required versions for both dependency tools must live in the
  repository on `main`, not only in workflow YAML or release notes.
- Use the existing five supported targets from
  `installer/src/artefact/target.rs`: `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
  `x86_64-pc-windows-msvc`.
- Keep each Rust source file under 400 lines. If new code or tests would
  exceed that limit, split into sibling modules using the existing pattern.
- Every Rust module must begin with a `//!` module comment. Public APIs need
  rustdoc examples where practical.
- Use `rstest-bdd` v0.5.0 for new behavioural coverage. Step functions must
  respect the workspace `clippy::too_many_arguments` threshold of 4.
- Use en-GB-oxendict spelling in docs and comments.
- Do not mark the roadmap entry done until implementation, documentation, and
  all gates are green.
- Because `docs/roadmap.md` does not currently contain a dedicated unchecked
  item for this feature, implementation must add one before marking it done.
  The recommended placement is a new `4.3.2` entry under
  `## 4.3. Release workflow`, since this work extends both the release system
  and the installer dependency path.

## Tolerances (exception triggers)

- Scope: if the cleanest implementation requires touching more than 18 files or
  adding more than 900 net new lines, stop and review whether the work should
  be split into two roadmap items.
- Dependencies: if this requires more than two new third-party crates beyond
  what the workspace already carries, stop and justify them explicitly.
- Workflow complexity: if supporting dependency-binary publication would force a
  second, materially different target matrix from the existing release and
  rolling-release matrices, stop and consolidate the design before coding.
- Installer API: if an existing public installer API must change in a breaking
  way, stop and document the trade-off before proceeding.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail after
  three focused repair attempts per gate, stop and escalate with the saved logs.
- Upstream ambiguity: if `cargo-dylint` and `dylint-link` versioning or binary
  names cannot be consumed reproducibly from Cargo metadata alone, stop and
  choose an explicit repository-owned manifest format before any partial
  implementation lands.

## Risks

- Risk: the repository already has two release workflows with different
  triggers, and adding dependency binaries could duplicate packaging logic.
  Severity: medium. Likelihood: high. Mitigation: introduce one shared
  repository-owned dependency-binary manifest plus one packaging helper module,
  then have both workflows consume the same naming and metadata rules.

- Risk: upstream package names and executable names differ in an awkward way
  (`cargo-dylint` package, `cargo dylint` invocation; `dylint-link` binary).
  Severity: low. Likelihood: certain. Mitigation: capture package name, binary
  filename, and install verification command separately in the committed
  manifest and in the Rust domain model.

- Risk: installing a downloaded binary into the user's Cargo bin directory can
  fail because of permissions, missing directory, or non-executable file mode.
  Severity: medium. Likelihood: medium. Mitigation: route installation through
  the existing `BaseDirs::bin_dir()` abstraction, ensure the directory exists,
  set executable bits on Unix, and treat repository-install failure as a normal
  fallback trigger rather than a fatal path.

- Risk: release pages need licence and provenance information for third-party
  binaries, but the workflow currently only uploads archives and checksum
  sidecars. Severity: medium. Likelihood: certain. Mitigation: publish one
  shared asset generated from the committed manifest that includes package
  names, versions, licences, and repository URLs for both tools.

- Risk: there is no dedicated roadmap entry yet. Severity: low. Likelihood:
  certain. Mitigation: add a new roadmap item at implementation start, then
  mark it done only after the feature ships.

## Progress

- [x] (2026-03-30) Reviewed the current installer dependency installation path,
  `release.yml`, `rolling-release.yml`, the existing release/prebuilt
  ExecPlans, and the relevant design-document sections.
- [x] (2026-03-30) Confirmed that `rstest-bdd` 0.5.0 is already the workspace
  standard and that installer behaviour tests already use the required mutable
  world pattern.
- [x] (2026-03-30) Identified a roadmap gap: `docs/roadmap.md` has no dedicated
  unchecked item for dependency binaries, so the implementation must add one.
- [x] (2026-03-30) Drafted this ExecPlan in
  `docs/execplans/install-dependency-binaries.md`.
- [x] (2026-03-30) Implementation approved by user.
- [x] (2026-03-30) Added `installer/dependency-binaries.toml` plus
  `installer/src/dependency_binaries/` for manifest parsing, archive naming,
  local installation, and host-target detection.
- [x] (2026-03-30) Extended `release.yml` and `rolling-release.yml` to build
  and package dependency binaries, and to publish
  `dependency-binaries-licences.md`.
- [x] (2026-03-30) Updated `installer/src/deps.rs` to prefer repository-hosted
  dependency binaries before `cargo binstall` or `cargo install`, with
  verification and fallback logging.
- [x] (2026-03-30) Added unit tests plus
      `installer/tests/behaviour_dependency_binaries.rs`
  and `installer/tests/features/dependency_binaries.feature`.
- [x] (2026-03-30) Updated the design doc, added and completed roadmap item
  `4.3.2`, and ran `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, and `make test` successfully.

## Surprises & Discoveries

- `docs/roadmap.md` currently marks installer CLI (`3.2.1`) and release
  workflow (`4.3.1`) as complete, but there is no remaining roadmap item that
  directly tracks repository-published dependency binaries. The implementation
  therefore needs to add a new roadmap line rather than overloading an already
  completed one.

- The installer already has a clean seam for this work in
  `installer/src/deps.rs`, which currently decides only between
  `cargo binstall` and `cargo install`. That is the narrowest place to add
  repository-first dependency installation without disturbing the rest of the
  build pipeline.

- The repository already has two strong precedents that this feature should
  follow instead of inventing a new pattern: `installer_packaging` for archive
  creation and `prebuilt` for download-first/fallback behaviour.

- `rstest-bdd` step macros expect `std::result::Result<_, StepError>` under the
  hood; importing the installer's `Result<T>` alias into a behaviour test file
  causes the generated wrappers to fail type-checking. Keep behaviour harnesses
  on plain `std::result::Result` or omit the alias entirely.

## Decision Log

- Decision: track the required dependency-tool versions in a new
  repository-owned manifest rather than hard-coding them only in Rust or only
  in workflow YAML. Rationale: the user explicitly requires the versions to be
  published to `main`, the release workflows need machine-readable input, the
  installer needs the same values at runtime, and tests should be able to parse
  the actual source of truth to detect drift. Proposed shape: add
  `installer/dependency-binaries.toml` containing one entry per tool with
  fields for package name, executable name, version, licence, and repository
  URL. Add a Rust parser/helper module that reads or mirrors this manifest so
  both workflows and tests can rely on one contract. Date/Author: 2026-03-30 /
  plan author.

- Decision: create a dedicated installer-side domain module for dependency
  binaries instead of expanding `deps.rs` into a long mixed-responsibility
  file. Rationale: `deps.rs` should remain the orchestration surface, while
  archive naming, URL construction, local install paths, and manifest parsing
  belong in focused helpers. This also keeps files below the 400-line limit.
  Proposed module split: `installer/src/dependency_binaries/mod.rs`,
  `installer/src/dependency_binaries/manifest.rs`,
  `installer/src/dependency_binaries/install.rs`, and tests in sibling files.
  Date/Author: 2026-03-30 / plan author.

- Decision: publish dependency binaries to both the rolling release and tagged
  releases. Rationale: the task requires new builds when `main` changes the
  required versions and also requires the release page to expose downloadable
  binaries with licence information. Publishing to both workflows satisfies
  both the rolling mainline use case and the immutable tagged-release use case.
  Date/Author: 2026-03-30 / plan author.

- Decision: build new dependency binaries only when their committed manifest
  changes on `main`, but always allow manual rebuild via `workflow_dispatch`.
  Rationale: the requirement says version changes on `main` must trigger new
  publication, not that every push must rebuild unchanged upstream tools. This
  keeps the rolling workflow deterministic and cheaper while preserving a
  manual recovery path. Date/Author: 2026-03-30 / plan author.

- Decision: ship one shared release asset describing third-party provenance and
  licensing instead of trying to embed that information into every archive
  filename or workflow note. Rationale: the release page needs human-readable
  licence and repository references, and a generated Markdown or JSON sidecar
  is easy to test, upload, and keep consistent across both workflows. Proposed
  asset name: `dependency-binaries-licences.md`. Date/Author: 2026-03-30 / plan
  author.

## Context and orientation

The repository root is `/home/user/project`.

Relevant existing files:

- `installer/src/deps.rs`
  Current dependency-check/install orchestration. Today it checks whether
  `cargo dylint --version` and `dylint-link --version` succeed, then installs
  missing tools with `cargo binstall` or `cargo install`.
- `installer/src/cli.rs`
  Existing installer flags. No new user-facing flag is required by the task,
  but this file may need a small addition only if implementation decides to
  expose diagnostics or opt-out behaviour.
- `installer/src/error.rs`
  Existing semantic installer errors. This is the correct place for any new
  repository-download or binary-install error surface if callers need to
  distinguish failure reasons in tests.
- `installer/src/installer_packaging.rs` and
  `installer/src/bin/package_installer_bin.rs` Existing pattern for packaging a
  binary into a target-specific archive and for testing that packaging locally.
- `installer/src/prebuilt.rs`
  Existing download-first, fallback-safe installer logic for lint libraries.
  This is the behavioural precedent for the new dependency-binary path.
- `.github/workflows/release.yml`
  Tagged-release workflow that currently publishes `whitaker-installer`
  archives.
- `.github/workflows/rolling-release.yml`
  `main`-triggered workflow that currently publishes lint-library assets to the
  rolling release.
- `docs/whitaker-dylint-suite-design.md`
  Existing design source of truth for installer packaging, prebuilt artefacts,
  and release behaviour.
- `docs/roadmap.md`
  Must gain a new roadmap item before this feature can be marked done.

Recommended new files:

- `installer/dependency-binaries.toml`
  Repository-owned manifest for required dependency-tool versions and
  provenance.
- `installer/src/dependency_binaries/mod.rs`
  Public Rust entrypoint for dependency-binary metadata and installation
  helpers.
- `installer/src/dependency_binaries/tests.rs` or split sibling test files
  keeping each file under 400 lines.
- `installer/tests/behaviour_dependency_binaries.rs`
  Behaviour harness using `rstest-bdd`.
- `installer/tests/features/dependency_binaries.feature`
  Gherkin scenarios for happy and unhappy paths.

## Plan of work

### Stage A: Add the repository-owned dependency-binary manifest

Create `installer/dependency-binaries.toml` and commit it to the repository.
This file is the source of truth for both required versions and third-party
provenance. It should contain exactly two entries, one for `cargo-dylint` and
one for `dylint-link`.

Recommended fields per entry:

- `package` — Cargo package name (`cargo-dylint`, `dylint-link`).
- `binary` — executable filename without extension.
- `version` — required upstream version.
- `license` — Software Package Data Exchange (SPDX) identifier or the exact
  licence string supplied by the upstream crate metadata.
- `repository` — upstream repository URL.

Optional fields if they simplify implementation:

- `check_command` — how the installer verifies that the tool is installed.
- `windows_binary` — explicit `.exe` filename if the manifest is to be fully
  literal.

Acceptance for Stage A:

1. The repository contains one auditable manifest committed to `main`.
2. A unit test can parse the actual file and confirm both required tools are
   present.
3. The design doc and workflows can refer to this file as the version source of
   truth.

### Stage B: Introduce a Rust domain model for dependency binaries

Add a small Rust module subtree under `installer/src/dependency_binaries/`.
Keep `deps.rs` as the orchestration boundary, but move data modelling and
repository-install mechanics into this new subtree.

Recommended responsibilities:

- `manifest.rs`
  Parse the committed TOML into typed structs. Expose helpers such as
  `required_dependency_binaries()` and `find_dependency_binary(name)`.
- `install.rs`
  Build archive names and download URLs, install a downloaded archive into the
  Cargo bin directory, and verify post-install availability.
- `mod.rs`
  Re-export the public types and helpers used by `deps.rs`.

Expected helper behaviour:

1. Given a tool name and target triple, derive the repository asset filename.
   Recommended naming: `cargo-dylint-<target>-v<dependency-version>.tgz` and
   `dylint-link-<target>-v<dependency-version>.tgz` with `.zip` for Windows.
2. Generate a shared provenance asset path
   `dependency-binaries-licences.md`.
3. Resolve the local installation directory via the existing base-directory
   abstraction used elsewhere in the installer, creating the bin directory if
   needed.
4. Extract the downloaded binary archive into that directory and ensure the
   binary is executable on Unix.

Acceptance for Stage B:

1. There is one tested Rust API for manifest access and archive naming.
2. The API is reusable from both installer code and release-packaging tests.
3. No file crosses the 400-line limit.

### Stage C: Extend release packaging for dependency binaries

Reuse the `installer_packaging` pattern to build archives for `cargo-dylint`
and `dylint-link`. Do not leave archive creation as ad hoc shell inside
workflow YAML.

Recommended implementation:

1. Add a new packaging helper module, for example
   `installer/src/dependency_packaging.rs`, plus a thin CLI binary such as
   `installer/src/bin/package_dependency_binary.rs`.
2. The packager should accept:
   - package/tool name;
   - dependency version;
   - target triple;
   - path to the built executable;
   - output directory.
3. Package archives using the same format rule as the installer:
   `.tgz` for non-Windows, `.zip` for Windows.
4. Include a top-level directory inside each archive using the same versioned
   naming scheme as the asset filename so extraction is deterministic.
5. Generate or update a shared `dependency-binaries-licences.md` file from the
   committed manifest during the packaging/publish flow.

Acceptance for Stage C:

1. Unit tests validate archive naming, internal directory layout, Windows
   versus non-Windows format selection, and missing-binary failures.
2. One generated provenance/licence file contains both tools, both versions,
   both licence strings, and both repository URLs.

### Stage D: Extend `rolling-release.yml`

Update `.github/workflows/rolling-release.yml` so pushes to `main` publish new
dependency binaries when the committed dependency-binary manifest changes.

Recommended workflow shape:

1. Add a lightweight change-detection step early in the workflow that checks
   whether `installer/dependency-binaries.toml` changed in the pushed commit
   range. If it did not change, skip the dependency-binary build job. Keep
   `workflow_dispatch` able to force the job.
2. Add a new matrix job, or extend the existing matrix cleanly, to build the
   two upstream packages for each supported target.
3. Read versions from `installer/dependency-binaries.toml`, not from duplicated
   YAML literals.
4. Package the built executables with the new packaging CLI.
5. Upload the resulting archives plus the shared provenance/licence file as
   rolling-release assets.

Implementation note:

Because the rolling workflow already knows how to add targets and handle the
five-platform matrix, prefer reusing that matrix rather than inventing a second
target list.

Acceptance for Stage D:

1. A change to `installer/dependency-binaries.toml` on `main` causes fresh
   dependency-binary assets to be published to the rolling release.
2. A push to `main` that does not change the manifest does not rebuild these
   unchanged third-party binaries unless the workflow is manually dispatched.
3. The rolling release contains dependency archives and the provenance/licence
   sidecar.

### Stage E: Extend `release.yml`

Update `.github/workflows/release.yml` so tagged releases publish the same
dependency binaries and provenance/licence asset to the release page alongside
`whitaker-installer`.

Recommended workflow shape:

1. Reuse the same committed manifest and packaging CLI from Stages A-C.
2. Build both upstream tools for the same five supported targets.
3. Upload the archives and `dependency-binaries-licences.md` in the same
   release job that already creates or updates the GitHub Release.
4. Keep the existing installer archive publication unchanged.

Acceptance for Stage E:

1. Tagged releases expose `whitaker-installer` archives plus dependency-binary
   archives and the provenance/licence document on the GitHub Release page.
2. Asset names are deterministic and versioned by the dependency tool version,
   not the Whitaker crate version.

### Stage F: Teach the installer to prefer repository-hosted dependency binaries

Change `installer/src/deps.rs` so missing dependency tools are installed in
this order:

1. repository-hosted binary archive for the current target and required
   dependency version;
2. `cargo binstall`;
3. `cargo install`.

Recommended implementation details:

1. Keep `check_dylint_tools()` as the discovery step.
2. Replace the current `install_tool()` helper with a small orchestrator that:
   - looks up the tool in the committed manifest;
   - attempts repository download and installation;
   - verifies the installed binary by rerunning the same version command used by
     the existing detection path;
   - falls back to Cargo if any repository step fails.
3. Preserve the existing non-fatal behaviour: inability to use the repository
   path is not itself a hard installer error if Cargo fallback succeeds.
4. Emit concise stderr messages so behavioural tests can distinguish:
   - repository install success;
   - repository install failure with Cargo fallback;
   - total failure after all fallback paths are exhausted.

Acceptance for Stage F:

1. Supported platforms use repository-hosted dependency binaries first.
2. Missing or invalid repository assets trigger Cargo fallback without breaking
   the install flow.
3. Existing success cases that rely on `cargo binstall` or `cargo install`
   still work.

### Stage G: Add unit and behavioural tests

Add focused unit tests plus `rstest-bdd` scenarios. Follow the patterns in
`installer/tests/behaviour_installer_release.rs`,
`installer/tests/behaviour_binstall.rs`, and
`installer/tests/behaviour_prebuilt.rs`.

Required unit-test coverage:

1. Manifest parser accepts the two committed tools and rejects missing required
   fields.
2. Archive naming uses the committed dependency version and correct archive
   format per target.
3. Provenance/licence document generation includes package names, versions,
   licences, and repository URLs.
4. Repository install helper handles:
   - missing local bin directory;
   - Windows versus Unix executable names;
   - non-executable extracted files on Unix;
   - verification failure after extraction.
5. `deps.rs` fallback orchestration covers:
   - repository success, no Cargo call made;
   - repository miss, `cargo binstall` success;
   - repository miss and missing binstall, `cargo install` success;
   - repository miss, `cargo binstall` failure, `cargo install` success;
   - all paths fail, semantic error returned.

Required behavioural scenarios using `rstest-bdd` 0.5.0:

1. Missing `cargo-dylint` is installed from the repository asset successfully.
2. Missing `dylint-link` is installed from the repository asset successfully.
3. Repository asset is unavailable, so installer falls back to
   `cargo binstall`.
4. Repository asset and `cargo binstall` are unavailable, so installer falls
   back to `cargo install`.
5. Repository asset is unavailable, `cargo binstall` fails, so installer falls
   back to `cargo install`.
6. Repository asset exists but fails verification, so Cargo fallback is used.
7. Unsupported target or unsupported archive format takes the fallback path.
8. Release-packaging behaviour confirms the provenance/licence sidecar is
   emitted and references both upstream repositories.

Testing note:

Prefer dependency injection and stubs rather than live network calls. Reuse the
existing `StubExecutor` pattern and the `prebuilt` module's approach to
download/extract indirection if that keeps tests deterministic.

### Stage H: Update documentation and roadmap

Documentation work is part of the feature, not a follow-up.

1. Update `docs/whitaker-dylint-suite-design.md` with a new implementation
   decision section covering:
   - the repository-owned dependency-binary manifest;
   - rolling-release publication on version changes;
   - tagged-release publication on GitHub Releases;
   - installer repository-first fallback order;
   - the provenance/licence sidecar asset.
2. Add a new roadmap entry to `docs/roadmap.md`, recommended as:

   ```plaintext
   - [ ] 4.3.2. Publish repository-hosted dependency binaries for
     cargo-dylint and dylint-link, and teach the installer to prefer them
     before Cargo-based installation.
   ```

3. After all implementation and validation succeed, mark that new roadmap entry
   as `[x]`.
4. Keep this ExecPlan current if implementation details change.

Acceptance for Stage H:

1. The design document explains the final shipped behaviour.
2. The roadmap contains a dedicated item for this feature and marks it done
   only after the feature lands.

### Stage I: Run full quality gates

Use the repository-required commands with `set -o pipefail` and `tee` so logs
are preserved even if the output is truncated by the environment.

Required commands:

```bash
set -o pipefail; make check-fmt 2>&1 | tee /tmp/install-dependency-binaries-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/install-dependency-binaries-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/install-dependency-binaries-test.log
```

If documentation changed, also run:

```bash
set -o pipefail; make fmt 2>&1 | tee /tmp/install-dependency-binaries-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/install-dependency-binaries-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/install-dependency-binaries-nixie.log
```

Acceptance for Stage I:

1. All required gates exit successfully.
2. Logs are available under `/tmp/` for review if any failure occurs.

## Outcomes & Retrospective

This feature shipped as planned.

Outcomes:

1. Whitaker now publishes exact-version `cargo-dylint` and `dylint-link`
   archives from both rolling and tagged releases using the committed
   `installer/dependency-binaries.toml` manifest.
2. Release assets now include a generated provenance/licence Markdown sidecar
   describing package names, versions, licences, and upstream repositories.
3. The installer now attempts repository-hosted dependency binaries first,
   verifies the installed tool command, and falls back to `cargo binstall` or
   `cargo install` when the repository path is unavailable, verification fails,
   or when `cargo binstall` is absent or fails.
4. Coverage includes unit tests for manifest parsing, packaging, and install
   orchestration plus `rstest-bdd` behavioural scenarios for repository
   success, fallback, verification failure, unsupported targets, and provenance
   rendering.
5. Required gates passed:
   `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test`.

Retrospective:

1. The implementation stayed within the intended seam by keeping `deps.rs`
   focused on orchestration and moving dependency-binary rules into dedicated
   modules.
2. The only repair needed during validation was in the behaviour-test stub
   expectations: repository-failure scenarios should not expect a verification
   command that only occurs after a successful repository install.
3. Full-workspace `make test` remained dominated by existing slow UI suites;
   preserving logs and polling patiently was necessary to distinguish slow
   progress from a real hang.
