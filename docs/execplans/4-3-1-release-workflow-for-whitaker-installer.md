# Implement installer release workflow (roadmap 4.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 4.3.1 enables distribution of the `whitaker-installer` binary as
pre-compiled archives on GitHub Releases, so that users can install via
`cargo binstall whitaker-installer` without compiling from source. After this
change:

1. Tagging a commit with `v<version>` (e.g. `v0.2.1`) triggers a GitHub
   Actions workflow that cross-compiles the `whitaker-installer` binary for all
   five supported target triples.
2. Each build is packaged into a binstall-compatible archive:
   `whitaker-installer-<target>-v<version>.tgz` (`.zip` for Windows).
3. Each archive contains a top-level directory
   `whitaker-installer-<target>-v<version>/` with the binary inside
   (`whitaker-installer` or `whitaker-installer.exe`).
4. All archives are uploaded as assets on a GitHub Release tagged `v<version>`.
5. A new `installer_packaging` Rust module provides the archive creation logic,
   validated by unit tests (`rstest`) and behaviour-driven development (BDD)
   scenarios (`rstest-bdd` v0.5.0).
6. The roadmap marks 4.3.1 as done.
7. The design document records the implementation decision.
8. `make check-fmt`, `make lint`, and `make test` all pass.

Observable outcome: running `make test` shows new unit tests and BDD scenarios
passing. The `.github/workflows/release.yml` file is syntactically valid and
follows the conventions established by `rolling-release.yml`. Pushing a
`v0.2.1` tag would trigger the workflow, produce five archive assets on the
GitHub Release whose filenames match the binstall `pkg-url` template, and
running `cargo binstall whitaker-installer` on a supported platform would
download the correct archive.

## Constraints

- Archive naming must exactly match the binstall templates already in
  `installer/Cargo.toml` lines 79-84:
  `whitaker-installer-<target>-v<version>.tgz` (`.zip` for Windows), inner
  directory `whitaker-installer-<target>-v<version>/`, binary
  `whitaker-installer` (`whitaker-installer.exe` on Windows).
- The five supported target triples are those defined in
  `installer/src/artefact/target.rs:SUPPORTED_TARGETS`.
- Every file must stay under 400 lines (per `AGENTS.md`).
- Every module must begin with a `//!` doc comment.
- Workspace Clippy `too_many_arguments` limit is 4 (per `clippy.toml`). BDD
  step functions must have at most 4 parameters (world + 3 parsed values).
- The `rstest-bdd` world fixture parameter must be named `world` (the macro
  matches parameter names literally).
- Use caret version requirements for all dependencies.
- Comments and documentation use en-GB-oxendict spelling.
- Markdown wrapped at 80 columns; code blocks at 120.
- `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"` apply in
  non-test code. New public functions must not call `.expect()` or `.unwrap()`.
- No `unsafe` code.
- Existing public API surface of `binstall_metadata` and `artefact` modules
  must not break.
- The `v<version>` in release tags and archive names must match
  `package.version` in `installer/Cargo.toml` exactly.
- On completion, update `docs/roadmap.md` entry 4.3.1 to `[x]`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 15 files or more than
  800 net new lines of code, stop and escalate.
- Interface: if any existing public type's API must change in a
  backward-incompatible way (removing fields, renaming methods), stop and
  escalate.
- Dependencies: if more than 3 new external crate dependencies are needed,
  stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts per gate, stop and escalate with logs.
- Ambiguity: if the crates.io publishing step requires secrets or permissions
  not yet configured in the repository, document the gap and proceed with the
  GitHub Release workflow only; flag the crates.io step as manual/deferred.

## Risks

- Risk: The `flate2` crate (for gzip compression) is a transitive dependency
  already in `Cargo.lock` but not a direct workspace dependency. Adding it may
  cause a version conflict. Severity: low. Likelihood: low. Mitigation: use the
  same version already resolved in `Cargo.lock` with a caret requirement.

- Risk: The `zip` crate is not yet in the dependency tree. Adding it
  introduces a new dependency with its own transitive closure. Severity: low.
  Likelihood: certain (required for Windows `.zip` format). Mitigation: use a
  well-maintained version (e.g. `zip = "2"`). The Windows `.zip` format is a
  hard requirement from the design document.

- Risk: Cross-compiling the `whitaker-installer` binary for
  `aarch64-unknown-linux-gnu` may require linking against C libraries not
  available on the ubuntu-latest runner. Severity: medium. Likelihood: low.
  Mitigation: install `gcc-aarch64-linux-gnu` and set
  `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`, the
  same pattern used in the existing `rolling-release.yml`. The installer binary
  is simpler than the lint cdylibs (no `rustc_private` linkage).

- Risk: The crates.io publish step requires a `CARGO_REGISTRY_TOKEN` secret
  that may not be configured in the repository. Severity: medium. Likelihood:
  high. Mitigation: include the publish step in the workflow with a conditional
  gate on the secret. The release workflow succeeds (GitHub Release created)
  even if crates.io publishing is skipped. Document the secret requirement.

- Risk: BDD step functions with archive path parameters may exceed the
  4-argument Clippy limit. Severity: low. Likelihood: medium. Mitigation: split
  Gherkin steps so each step function parses at most 2–3 values from the
  feature text. Use the world struct to carry state between steps.

## Progress

- [x] (2026-03-01) Stage A: Add workspace dependencies and create
  `installer_packaging` module with naming/format functions.
- [x] (2026-03-01) Stage B: Implement archive creation and unit tests (23
  unit tests, 4 doc tests pass).
- [x] (2026-03-01) Stage C: Create Gherkin feature file and BDD behaviour
  tests (6 scenarios pass).
- [x] (2026-03-01) Stage D: Create GitHub Actions release workflow and
  packaging binary.
- [x] (2026-03-01) Stage E: Create the packaging binary
  (`whitaker-package-installer` CLI). Used `--crate-version` instead of
  `--version` to avoid clap conflict.
- [x] (2026-03-01) Stage F: Update documentation (roadmap, design doc,
  execplan), including clarity edits per review feedback.
- [x] (2026-03-01) Stage G: Quality gates. `make check-fmt`, `make lint`,
  and `make test` all exit 0. 914 tests passed, 2 skipped, 0 failed.

## Surprises & discoveries

- Observation: `flate2` and `zip` crates integrated without version conflicts.
  `flate2` was indeed already a transitive dependency. `zip` v2 added cleanly.
  Impact: none; the dependency risk was low and materialized as expected.

- Observation: The `zip` crate's `ZipWriter::start_file` API requires
  `SimpleFileOptions` (not `FileOptions` as in older versions). Using the v2
  API worked directly. Impact: none; documented for future reference.

- Observation: clap's `#[command(version)]` auto-generates a `--version` flag
  that conflicts with a `--version` CLI argument for the crate version. The
  field was renamed to `--crate-version` to avoid the conflict. Evidence: 3
  tests in `package_installer_bin.rs` failed with "Argument names must be
  unique". Impact: renamed CLI argument and updated workflow YAML. Resolved on
  first attempt.

## Decision log

- Decision: Implement archive packaging in Rust (a new module
  `installer/src/installer_packaging.rs`) rather than in shell within the
  workflow YAML.
  Rationale: Implementing in Rust enables unit testing and BDD testing of the
  archive structure, naming conventions, and format selection logic. Shell-based packaging in YAML is untestable locally and fragile. The
  existing `artefact::packaging` module and `whitaker-package-lints` binary
  establish a clear precedent for Rust-based packaging in this project.
  Date/Author: 2026-03-01 / plan author.

- Decision: Place the new module at `installer/src/installer_packaging.rs`
  (top-level peer to `binstall_metadata.rs`) rather than inside the `artefact/`
  subtree. Rationale: The `artefact/` module covers the prebuilt lint library
  artefact system (ADR-001). Installer binary packaging is a separate concern:
  it packages the installer itself for end-user distribution via
  cargo-binstall. This parallels the rationale from the 4.2.1 execplan, where
  `binstall_metadata` was placed at top level. Date/Author: 2026-03-01 / plan
  author.

- Decision: Create a new `whitaker-package-installer` binary target as a thin
  CLI wrapper around `package_installer()`, following the
  `whitaker-package-lints` pattern. Rationale: The existing
  `whitaker-package-lints` binary (`installer/src/bin/package_lints.rs`)
  establishes a clear precedent. A Rust binary enables in-process testing of
  the actual archive contents and keeps archive creation logic authoritative in
  one place, eliminating drift between shell and Rust implementations.
  Date/Author: 2026-03-01 / plan author.

- Decision: Trigger the release workflow on `v*` tag push and
  `workflow_dispatch` (manual). Rationale: Tag-push trigger is the standard
  GitHub pattern for versioned releases and integrates naturally with
  `git tag v0.2.1 && git push --tags`. Manual dispatch provides a fallback for
  re-running failed releases. Date/Author: 2026-03-01 / plan author.

- Decision: Add `flate2` (gzip) and `zip` as workspace dependencies for
  `.tgz` and `.zip` archive creation, respectively.
  Rationale: `flate2` is already a transitive dependency. `zip` is needed for
  the Windows target. Both are well-maintained, widely used crates. The `tar` crate is already a direct
  dependency. No alternative avoids both crates while satisfying the
  requirement for both `.tgz` and `.zip` formats. Date/Author: 2026-03-01 /
  plan author.

- Decision: Include a crates.io publish step in the release workflow, gated by
  the presence of a `CARGO_REGISTRY_TOKEN` secret. Rationale: The design
  document (§ Installer release artefacts) requires crates.io publishing so
  cargo-binstall can resolve the latest version from the registry. The
  conditional gate means the workflow succeeds even without the secret
  configured, and the secret can be added later. Date/Author: 2026-03-01 / plan
  author.

## Outcomes & retrospective

All acceptance criteria met:

- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0: 914 tests passed (23 new unit tests in
  `installer_packaging_tests.rs`, 6 new BDD scenarios in
  `behaviour_installer_release.rs`, 4 new CLI tests in
  `package_installer_bin.rs`). No regressions.
- Roadmap item 4.3.1 marked as `[x]`.
- Design document updated with implementation status note for 4.3.1.
- `.github/workflows/release.yml` created, syntactically valid, follows
  conventions established by `rolling-release.yml`.
- Archive naming cross-validated against `binstall_metadata::expand_pkg_url`.

Lessons learned:

- clap's `#[command(version)]` auto-generates a `--version` flag that
  conflicts with a `--version` CLI argument. Use a different name (e.g.
  `--crate-version`) for version-like parameters in clap binaries.
- The `zip` crate v2 uses `SimpleFileOptions` (not `FileOptions`). The
  `ZipWriter::start_file` API accepts `&str` for the path, not `Path`.

## Context and orientation

The Whitaker project is a Rust Cargo workspace at `/home/user/project/`
containing Dylint lint crates, a `common` library, and a `whitaker-installer`
CLI tool. The installer lives in `installer/` and is published as the
`whitaker-installer` crate (currently v0.2.1).

Key files and modules relevant to this task:

- `installer/Cargo.toml` (85 lines): the installer's package manifest. Lines
  78-84 contain the `[package.metadata.binstall]` section with `pkg-url`,
  `bin-dir`, and `pkg-fmt` templates that define the exact archive naming
  convention this release workflow must satisfy.
- `installer/src/binstall_metadata.rs` (129 lines): constants and template
  expansion helpers (`expand_pkg_url`, `expand_bin_dir`,
  `WINDOWS_OVERRIDE_TARGET`, `DEFAULT_PKG_FMT`, `WINDOWS_PKG_FMT`). These are
  the single source of truth for archive naming and will be reused by the new
  packaging module.
- `installer/src/artefact/target.rs` (241 lines): `TargetTriple` newtype with
  the list of 5 supported targets (`SUPPORTED_TARGETS`) and helper methods
  `is_windows()` (private), `library_extension()`, `library_prefix()`.
- `installer/src/artefact/packaging.rs` (228 lines): existing packaging module
  for lint library `.tar.zst` archives. Provides the pattern for the new
  installer packaging module (uses `tar` + `zstd` crates, `PackageParams`
  struct pattern, `compute_sha256`, `create_archive`).
- `installer/src/bin/package_lints.rs` (402 lines): thin CLI binary for lint
  packaging. Pattern for the new `package_installer_bin.rs` binary (uses
  `clap::Parser`, validates inputs, delegates to library functions).
- `installer/src/lib.rs` (65 lines): library root. Module declarations here.
- `.github/workflows/rolling-release.yml` (201 lines): existing workflow that
  builds lint libraries for 5 targets and publishes to a `rolling` release.
  Pattern for the release workflow (same matrix, runners, cross-compilation
  setup, `leynos/shared-actions/.github/actions/setup-rust` action).
- `.github/workflows/ci.yml` (89 lines): PR CI workflow.
- `installer/tests/behaviour_binstall.rs` (~237 lines): BDD test pattern for
  binstall metadata.
- `installer/tests/features/binstall_metadata.feature` (~42 lines): Gherkin
  feature file pattern.
- `Cargo.toml` (workspace root): `[workspace.dependencies]` section.
- `clippy.toml`: `too-many-arguments-threshold = 4`.
- `rust-toolchain.toml`: pinned to `nightly-2025-09-18`.
- `docs/whitaker-dylint-suite-design.md` lines 1356-1420: design specification
  for installer release artefacts, including naming conventions, archive
  structure, and binstall metadata templates.
- `docs/roadmap.md` line 123: the `[ ]` checkbox for 4.3.1.

Cargo-binstall template placeholders (from the design document):

- `{name}` -- crate name (`whitaker-installer`)
- `{version}` -- crate version (e.g. `0.2.1`)
- `{target}` -- Rust target triple (e.g. `x86_64-unknown-linux-gnu`)
- `{archive-format}` -- archive extension derived from `pkg-fmt` (`tgz`/`zip`)
- `{bin}` -- binary name (`whitaker-installer` or `whitaker-installer.exe`)

Supported target triples (from `installer/src/artefact/target.rs`):
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

## Plan of work

### Stage A: Workspace dependencies and module scaffolding

Add `flate2` and `zip` to `[workspace.dependencies]` in the root `Cargo.toml`
with caret requirements. Add both as `[dependencies]` in
`installer/Cargo.toml`. Add a `[[bin]]` entry for `whitaker-package-installer`
pointing to `src/bin/package_installer_bin.rs`.

Create `installer/src/installer_packaging.rs` (~180 lines) with:

- A `//!` module doc comment explaining the module's purpose.
- An `ArchiveFormat` enum: `Tgz` and `Zip` variants.
- Pure naming functions (all `#[must_use]`):
  - `archive_filename(version, target) -> String` -- produces
    `whitaker-installer-<target>-v<version>.tgz` (or `.zip` for Windows).
  - `inner_dir_name(version, target) -> String` -- produces
    `whitaker-installer-<target>-v<version>`.
  - `binary_filename(target) -> String` -- produces `whitaker-installer` or
    `whitaker-installer.exe` for Windows.
  - `archive_format(target) -> ArchiveFormat` -- returns `Zip` for
    `x86_64-pc-windows-msvc`, `Tgz` for all others.
- Use `binstall_metadata::WINDOWS_OVERRIDE_TARGET` for the Windows check to
  stay consistent with the existing module.
- An `InstallerPackageParams` struct grouping: `version: Version`,
  `target: TargetTriple`, `binary_path: PathBuf`, `output_dir: PathBuf`.
- An `InstallerPackageOutput` struct: `archive_path: PathBuf`,
  `archive_name: String`.
- An `InstallerPackagingError` enum (derive `thiserror::Error`):
  `Io(#[from] std::io::Error)`, `BinaryNotFound(PathBuf)`,
  `Zip(#[from] zip::result::ZipError)`.
- `package_installer(params) -> Result<InstallerPackageOutput,
  InstallerPackagingError>` that:
  1. Validates the binary file exists.
  2. Computes archive name and inner directory name.
  3. Delegates to `create_tgz_archive()` or `create_zip_archive()` based on
     `archive_format(target)`.
  4. Returns the output.
- `create_tgz_archive(output_path, inner_dir, binary_path, binary_name)` --
  uses the `tar` crate with a `flate2::write::GzEncoder` wrapper (similar to
  the pattern in `artefact/packaging.rs:create_archive` but substituting gzip
  for zstd, and wrapping the binary in a directory entry).
- `create_zip_archive(output_path, inner_dir, binary_path, binary_name)` --
  uses the `zip` crate's `ZipWriter` to create a `.zip` with the binary inside
  the inner directory.
- A `#[cfg(test)] #[path = "installer_packaging_tests.rs"] mod tests;` anchor.

Register the module in `installer/src/lib.rs`:

- Add `//! - [\`installer_packaging\`] - Installer binary archive packaging`
  to the doc comment (in alphabetical order, between `install_metrics` and
  `list`).
- Add `pub mod installer_packaging;` in the corresponding position.

Acceptance: `cargo check -p whitaker-installer` compiles without errors.

### Stage B: Unit tests

Create `installer/src/installer_packaging_tests.rs` (~200 lines) with `rstest`
unit tests:

1. `archive_filename_tgz_for_linux` -- parameterized over the four non-Windows
   targets, asserts filename ends with `.tgz` and contains the target and
   version.
2. `archive_filename_zip_for_windows` -- asserts the Windows target produces
   a `.zip` filename.
3. `inner_dir_name_matches_expected` -- asserts the inner directory name
   matches the pattern for each target.
4. `binary_filename_unix` -- asserts `whitaker-installer` for non-Windows.
5. `binary_filename_windows` -- asserts `whitaker-installer.exe` for Windows.
6. `archive_format_tgz_for_non_windows` -- parameterized over non-Windows
   targets.
7. `archive_format_zip_for_windows` -- asserts `Zip` for Windows.
8. `package_installer_creates_archive` -- parameterized over Linux `.tgz`
   and Windows `.zip` targets; creates a temp file as a fake binary, calls
   `package_installer`, reads back the archive entries and verifies the inner
   path matches `whitaker-installer-<target>-v<version>/whitaker-installer`
   (Unix) or `whitaker-installer-<target>-v<version>/whitaker-installer.exe`
   (Windows).
9. `package_installer_rejects_missing_binary` -- passes a non-existent path,
    asserts `BinaryNotFound` error.
10. `archive_name_matches_binstall_template` -- cross-validates by calling
    `binstall_metadata::expand_pkg_url` and asserting the URL ends with the
    archive filename from `archive_filename()`.

Use parameterized `#[rstest]` `#[case]` where appropriate, keeping parameter
count within the Clippy limit (use tuple cases if needed to keep under 4
params).

Acceptance: `cargo test -p whitaker-installer -- installer_packaging` passes
all tests.

### Stage C: BDD feature file and behaviour tests

Create `installer/tests/features/installer_release.feature` (~50 lines) with
Gherkin scenarios:

1. "Archive filename uses tgz for Linux target"
2. "Archive filename uses zip for Windows target"
3. "Archive contains correct directory structure for Unix"
4. "Windows archive contains exe binary"
5. "Archive filename matches binstall pkg-url template"
6. "Packaging rejects missing binary"

Create `installer/tests/behaviour_installer_release.rs` (~250 lines) with:

- `//!` doc comment explaining the file's purpose.
- Imports from `rstest::fixture`, `rstest_bdd_macros`, `tempfile`, and
  `whitaker_installer::installer_packaging`.
- An `InstallerReleaseWorld` struct (derive `Default`) with fields:
  `version: String`, `target: String`, `computed_filename: String`,
  `temp_dir: Option<tempfile::TempDir>`, `binary_path: Option<PathBuf>`,
  `package_output: Option<InstallerPackageOutput>`,
  `packaging_error: Option<InstallerPackagingError>` (`None` when packaging
  succeeds, `Some(err)` when it fails).
- A `#[fixture] fn world()` returning `InstallerReleaseWorld::default()`.
- Step definitions (`#[given]`, `#[when]`, `#[then]`) matching the Gherkin
  steps. Each step function has at most 4 parameters.
- Scenario bindings (`#[scenario]`) linking each Gherkin scenario to a binding
  function.

Acceptance:
`cargo test -p whitaker-installer --test behaviour_installer_release` passes
all scenarios.

### Stage D: GitHub Actions release workflow

Create `.github/workflows/release.yml` (~130 lines) following the structure of
the existing `rolling-release.yml`:

```yaml
name: Release

on:
  push:
    tags: ["v*"]
  workflow_dispatch:
    inputs:
      tag:
        description: "Version tag (e.g. v0.2.1) — must match an existing tag"
        required: true
        type: string

permissions:
  contents: read        # least privilege; publish job overrides to write

env:
  CARGO_TERM_COLOR: always
  RELEASE_TAG: ${{ github.event.inputs.tag || github.ref_name }}

jobs:
  build-installer:
    runs-on: ${{ matrix.os }}
    defaults:
      run:
        shell: bash
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            cross: true
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    steps:
      - Checkout (actions/checkout@v5, ref: RELEASE_TAG)
      - Setup Rust (leynos/shared-actions/.../setup-rust@...)
      - Install cross-compilation tools (if matrix.cross)
      - Add target (rustup target add)
      - Read version from installer/Cargo.toml (cargo metadata + jq,
        fail if empty)
      - Build installer binary (cargo build -p whitaker-installer
        --release --target)
      - Build packaging tool (cargo build --release -p whitaker-installer
        --bin whitaker-package-installer)
      - Package archive (invoke whitaker-package-installer with
        --crate-version, --target, --binary-path, --output-dir)
      - Upload artefact (actions/upload-artifact@v4,
        name: installer-${{ matrix.target }})

  publish:
    if: ref_type == 'tag' || (workflow_dispatch && inputs.tag != '')
    needs: build-installer
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - Checkout (ref: RELEASE_TAG)
      - Verify v-prefix and tag matches Cargo.toml version
      - Download all artefacts (actions/download-artifact@v4,
        pattern: installer-*, merge-multiple: true)
      - List artefacts
      - Create GitHub Release (idempotent: upload assets if release
        exists, otherwise gh release create --verify-tag)
      - Publish to crates.io (conditional on CARGO_REGISTRY_TOKEN secret)
```

The workflow reuses the same matrix of 5 targets and OS runners, the same
setup-rust action, and the same cross-compilation pattern as
`rolling-release.yml`.

### Stage E: Create the packaging binary

Create `installer/src/bin/package_installer_bin.rs` (~100 lines) as a thin CLI
wrapper around `package_installer()`, following the `package_lints.rs` pattern:

- `clap::Parser` struct with `--crate-version`, `--target`, `--binary-path`,
  `--output-dir` arguments.
- Validates inputs and delegates to `package_installer()`.
- Prints the archive path on success.
- Returns exit code 1 on error.

### Stage F: Documentation updates

1. In `docs/roadmap.md` line 123, change `- [ ]` to `- [x]`.
2. In `docs/whitaker-dylint-suite-design.md`, after the implementation status
   note for binstall metadata (around line 1417), add:

   ```plaintext
   **Implementation status (4.3.1):** The release workflow at
   `.github/workflows/release.yml` builds and packages the installer
   binary for all five supported targets and publishes archives to GitHub
   Releases. The `installer_packaging` module in
   `installer/src/installer_packaging.rs` provides archive creation
   logic, validated by unit tests and BDD scenarios.
   ```

3. Write this execplan to
   `docs/execplans/4-3-1-release-workflow-for-whitaker-installer.md`.

### Stage G: Quality gates

Run from the workspace root:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/4-3-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/4-3-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/4-3-1-test.log
```

All three must exit 0. Review the test log to confirm the new tests appear.

## Concrete steps

All commands run from `/home/user/project/`.

1. Add `flate2` and `zip` to `[workspace.dependencies]` in `Cargo.toml`.
2. Add `flate2`, `zip` to `[dependencies]` in `installer/Cargo.toml`.
3. Add `[[bin]]` entry for `whitaker-package-installer` in
   `installer/Cargo.toml`.
4. Create `installer/src/installer_packaging.rs` -- naming functions, format
   selection, archive creation, `package_installer()`.
5. Create `installer/src/installer_packaging_tests.rs` -- unit tests.
6. Edit `installer/src/lib.rs` -- add doc-comment entry and
   `pub mod installer_packaging;`.
7. Create `installer/src/bin/package_installer_bin.rs` -- thin CLI binary.
8. Create `installer/tests/features/installer_release.feature` -- Gherkin
   scenarios.
9. Create `installer/tests/behaviour_installer_release.rs` -- BDD step
   definitions and scenario bindings.
10. Create `.github/workflows/release.yml` -- release workflow.
11. Edit `docs/roadmap.md` -- mark 4.3.1 as `[x]`.
12. Edit `docs/whitaker-dylint-suite-design.md` -- add implementation note.
13. Create `docs/execplans/4-3-1-release-workflow-for-whitaker-installer.md`.
14. `set -o pipefail && make check-fmt 2>&1 | tee /tmp/4-3-1-check-fmt.log`
15. `set -o pipefail && make lint 2>&1 | tee /tmp/4-3-1-lint.log`
16. `set -o pipefail && make test 2>&1 | tee /tmp/4-3-1-test.log`
17. Commit.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. New unit tests in
  `installer_packaging_tests.rs` and BDD scenarios in
  `behaviour_installer_release.rs` all pass. The tests fail if the packaging
  module is removed or the archive structure is altered.
- Lint/typecheck: `make check-fmt` and `make lint` exit 0.
- No regressions: all existing tests continue to pass.
- Documentation: roadmap item 4.3.1 is checked. Design document has
  implementation status note for 4.3.1.
- Workflow: `.github/workflows/release.yml` is syntactically valid YAML
  following the conventions established by `rolling-release.yml`.
- Archive structure: unit tests verify that a `.tgz` archive contains the
  required binary entry path
  `whitaker-installer-<target>-v<version>/whitaker-installer`, and a `.zip`
  archive contains
  `whitaker-installer-<target>-v<version>/whitaker-installer.exe`.
- Binstall compatibility: a unit test cross-validates archive filenames against
  `binstall_metadata::expand_pkg_url` confirming the URL would resolve
  correctly.

Quality method (verification procedure):

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/4-3-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/4-3-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/4-3-1-test.log
```

All three must exit 0.

## Idempotence and recovery

All steps are idempotent. Re-running file creation overwrites the same content.
Re-running quality gates produces the same result.

To revert: remove the new files (`installer_packaging.rs`,
`installer_packaging_tests.rs`, `behaviour_installer_release.rs`,
`installer_release.feature`, `package_installer_bin.rs`, `release.yml`, the
execplan), remove the `pub mod installer_packaging;` line and the `[[bin]]`
entry, remove `flate2` and `zip` from both `Cargo.toml` files, and revert the
roadmap/design-doc edits.

## Artefacts and notes

Table: Summary of new files — list of added files and purpose.

| File                                                              | Purpose                                  | Est. lines |
| ----------------------------------------------------------------- | ---------------------------------------- | ---------- |
| `installer/src/installer_packaging.rs`                            | Archive naming and creation              | ~180       |
| `installer/src/installer_packaging_tests.rs`                      | Unit tests                               | ~200       |
| `installer/src/bin/package_installer_bin.rs`                      | Thin CLI for Continuous Integration (CI) | ~100       |
| `installer/tests/features/installer_release.feature`              | Gherkin scenarios                        | ~50        |
| `installer/tests/behaviour_installer_release.rs`                  | BDD step defs                            | ~250       |
| `.github/workflows/release.yml`                                   | GitHub Actions workflow                  | ~130       |
| `docs/execplans/4-3-1-release-workflow-for-whitaker-installer.md` | ExecPlan                                 | ~400       |

Table: Summary of modified files — list of changed files and purpose.

| File                                   | Change                                               |
| -------------------------------------- | ---------------------------------------------------- |
| `Cargo.toml`                           | Add `flate2` and `zip` to `[workspace.dependencies]` |
| `installer/Cargo.toml`                 | Add deps and `[[bin]]` entry                         |
| `installer/src/lib.rs`                 | Add doc-comment entry + module declaration           |
| `docs/roadmap.md`                      | `[ ]` to `[x]` on 4.3.1                              |
| `docs/whitaker-dylint-suite-design.md` | Add implementation status note                       |

Total: 7 new files + 5 modified files = 12 files (within 15-file tolerance).

## Interfaces and dependencies

### New workspace dependencies

- `flate2 = "1"` -- gzip compression for `.tgz` archives. Already a
  transitive dependency. Pure Rust backend.
- `zip = "2"` -- ZIP archive creation for Windows target.

### New public API surface

In `installer/src/installer_packaging.rs`:

```rust
/// Supported archive formats for installer packaging.
pub enum ArchiveFormat {
    /// Gzip-compressed tar archive (.tgz).
    Tgz,
    /// ZIP archive (.zip).
    Zip,
}

/// Parameters for packaging the installer binary.
pub struct InstallerPackageParams {
    pub version: Version,
    pub target: TargetTriple,
    pub binary_path: std::path::PathBuf,
    pub output_dir: std::path::PathBuf,
}

/// Result of packaging the installer binary.
pub struct InstallerPackageOutput {
    pub archive_path: std::path::PathBuf,
    pub archive_name: String,
}

/// Errors during installer packaging.
#[derive(Debug, thiserror::Error)]
pub enum InstallerPackagingError {
    #[error("I/O error during packaging: {0}")]
    Io(#[from] std::io::Error),
    #[error("binary file not found: {0}")]
    BinaryNotFound(std::path::PathBuf),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Compute the archive filename for a given version and target.
#[must_use]
pub fn archive_filename(version: &Version, target: &TargetTriple) -> String;

/// Compute the inner directory name for a given version and target.
#[must_use]
pub fn inner_dir_name(version: &Version, target: &TargetTriple) -> String;

/// Compute the binary filename for a given target.
#[must_use]
pub fn binary_filename(target: &TargetTriple) -> String;

/// Determine the archive format for a given target.
#[must_use]
pub fn archive_format(target: &TargetTriple) -> ArchiveFormat;

/// Package the installer binary into the appropriate archive format.
pub fn package_installer(
    params: InstallerPackageParams,
) -> Result<InstallerPackageOutput, InstallerPackagingError>;
```

### New binary

In `installer/src/bin/package_installer_bin.rs`:

```rust
/// Package the whitaker-installer binary into a release archive.
#[derive(clap::Parser)]
#[command(name = "whitaker-package-installer", version)]
struct Cli {
    /// Crate version (e.g. "0.2.1").
    #[arg(long = "crate-version")]
    crate_version: String,
    /// Target triple (e.g. "x86_64-unknown-linux-gnu").
    #[arg(long)]
    target: String,
    /// Path to the compiled binary.
    #[arg(long)]
    binary_path: std::path::PathBuf,
    /// Output directory for the archive.
    #[arg(long)]
    output_dir: std::path::PathBuf,
}
```
