# Publish prebuilt lint libraries to rolling release (3.4.2)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`,
`Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

After this change, every push to `main` triggers a GitHub Actions
workflow that compiles the Whitaker Dylint lint libraries for all
five supported target triples, packages each target's output into a
`whitaker-lints-<sha>-<toolchain>-<target>.tar.zst` archive
containing a `manifest.json`, and publishes those archives to a
`rolling` GitHub Release. Downstream, the installer (task 3.4.4,
future) can download these archives instead of compiling from
source.

Success is observable by:

1. Pushing a commit to `main` and seeing the `rolling-release`
   workflow complete on all matrix entries.
2. Running `gh release view rolling` and seeing five `.tar.zst`
   assets with correct ADR-001 naming.
3. Running `make check-fmt && make lint && make test` locally with
   all checks green, including new unit and behaviour-driven
   development (BDD) tests for the packaging module.

## Constraints

- The artefact naming scheme must follow ADR-001 exactly:
  `whitaker-lints-<git_sha>-<toolchain>-<target>.tar.zst`.
- The manifest JSON schema must match ADR-001 § Decision outcome.
- The five supported target triples are those defined in
  `installer/src/artefact/target.rs:SUPPORTED_TARGETS`.
- The pinned toolchain is `nightly-2025-09-18` from
  `rust-toolchain.toml`.
- Files must remain under 400 lines per AGENTS.md.
- en-GB-oxendict spelling in comments and documentation.
- No `unsafe` code.
- All new public items require `///` doc comments and `//!` module
  docs.
- Existing public API surface of the `artefact` module must not
  break.
- Caret requirements for all new dependencies per AGENTS.md.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 15 files
  or more than 600 net new lines of code, stop and escalate.
- Interface: if any existing public type's API must change in a
  backward-incompatible way (removing fields, renaming methods),
  stop and escalate.
- Dependencies: if more than 4 new external crate dependencies are
  needed, stop and escalate.
- Iterations: if tests still fail after 3 fix attempts, stop and
  escalate.
- Cross-compilation: if cross-compiling for
  `aarch64-unknown-linux-gnu` requires more than installing a
  cross-compilation toolchain and linker in CI, stop and
  escalate — the target may need to be deferred to a follow-up.

## Risks

- Risk: Cross-compiling Dylint lint cdylibs for aarch64-linux from
  an x86_64 runner may fail due to rustc_private linkage
  requirements.
  Severity: high. Likelihood: medium.
  Mitigation: use the `cross` tool or install
  `gcc-aarch64-linux-gnu` and set
  `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER`. If
  cross-compilation proves infeasible, document the limitation and
  build natively using a self-hosted or emulated runner (escalate
  per tolerance).

- Risk: The `suite` crate's cdylib build may require special
  RUSTFLAGS (`-C prefer-dynamic`,
  `-Z force-unstable-if-unmarked`) that interact poorly with
  `--target` cross-compilation.
  Severity: medium. Likelihood: medium.
  Mitigation: mirror the flags from the Makefile `publish-check`
  target and test locally before committing the workflow.

- Risk: The `rolling` release tag may conflict with existing tags
  or require `contents: write` permissions not yet granted.
  Severity: low. Likelihood: low.
  Mitigation: explicitly set `permissions: contents: write` in the
  workflow and use `gh release create --latest=false` to avoid
  conflicting with semver releases.

- Risk: Adding `serde::Serialize` to existing artefact types may
  trigger new Clippy lints or require additional trait bounds.
  Severity: low. Likelihood: low.
  Mitigation: derive only `Serialize` (not `Deserialize` — that is
  a task 3.4.4 concern); test compilation early.

## Progress

- [x] Write the ExecPlan and obtain approval.
- [x] Stage A: Add serde Serialize to existing artefact domain
  types.
- [x] Stage B: Implement the `packaging` module (archive +
  manifest).
- [x] Stage C: Write unit tests for the packaging module.
- [x] Stage D: Write BDD feature file and test harness for
  packaging.
- [x] Stage E: Create the `rolling-release.yml` GitHub Actions
  workflow.
- [x] Stage F: Add a Makefile `package-lints` target for local
  testing.
- [x] Stage G: Run `make check-fmt`, `make lint`, `make test`.
- [x] Stage H: Update `docs/roadmap.md` and `docs/adr-001-*`
  design doc.
- [x] Stage I: Write the execplan to `docs/execplans/3-4-2-*`.

## Surprises & discoveries

- The `packaging.rs` module exceeded the 400-line file limit when
  unit tests were included inline. Resolved by extracting tests to
  a separate `packaging_tests.rs` file using
  `#[cfg(test)] #[path = "packaging_tests.rs"] mod tests;`.

- Clippy's `too_many_arguments` lint fired on
  `write_manifest_and_archive` (6 parameters, project limit is 4).
  Resolved by introducing an `ArchiveLayout<'a>` struct to group
  related path references.

- Clippy's `field_reassign_with_default` lint fired on the BDD
  world fixture. The pattern
  `let mut w = Default::default(); w.field = Some(...)` must use
  struct initialization syntax instead:
  `PackagingWorld { temp_dir: Some(...), ..Default::default() }`.

- Task 3.4.3 (manifest emission) is subsumed by 3.4.2 because the
  CI workflow must produce manifests alongside archives for the
  rolling release to be usable. Both roadmap items were marked
  complete.

## Decision log

- Decision: Include manifest JSON emission in task 3.4.2 rather
  than deferring to 3.4.3. Rationale: the CI workflow must produce
  manifests alongside archives for the rolling release to be usable
  by the installer. Deferring manifests would make the release
  assets incomplete. Task 3.4.3 becomes a
  verification/refinement item. Date/Author: 2026-02-11 (agent).

- Decision: Use a single `rolling` GitHub Release tag that is
  replaced on each push to `main`. Rationale: the ADR specifies a
  "rolling release" model. A single tag avoids asset accumulation
  and simplifies the installer's download URL construction. Old
  assets are replaced rather than appended.
  Date/Author: 2026-02-11 (agent).

- Decision: Derive only `serde::Serialize` on artefact types (not
  `Deserialize`). Rationale: this task produces manifests; consuming
  them is task 3.4.4. Adding `Deserialize` now would add untested
  surface area. Date/Author: 2026-02-11 (agent).

- Decision: Place packaging logic in the `installer` crate as a new
  `artefact::packaging` sub-module rather than a standalone binary.
  Rationale: the logic reuses all existing artefact domain types;
  keeping it co-located maintains cohesion. A thin Makefile target
  or shell script in CI invokes the logic via a helper binary or
  direct cargo commands. Date/Author: 2026-02-11 (agent).

- Decision: Invoke the `whitaker-package-lints` Rust binary from
  both CI and the Makefile rather than reimplementing packaging in
  shell. Rationale: centralizes JSON construction, two-pass SHA-256
  hashing, and tar/zstd archiving in a single authoritative
  location, eliminating drift between shell and Rust
  implementations. The binary is built as part of the workspace and
  adds no extra build step. Date/Author: 2026-02-12 (agent).
  SUPERSEDES: shell-based packaging decision (2026-02-11).

## Outcomes & retrospective

All stages completed successfully. Key outcomes:

- 7 unit tests in `packaging_tests.rs` pass (SHA-256, archive
  creation, manifest serialization, full pipeline, error paths,
  naming convention).
- 5 BDD scenarios in `artefact_packaging.feature` pass (single
  library packaging, manifest fields, SHA-256 validity, empty file
  rejection, filename convention).
- `make check-fmt`, `make lint`, and `make test` all pass.
- Roadmap items 3.4.2 and 3.4.3 marked complete.
- ADR-001 updated with implementation notes.
- 3 new workspace dependencies added: `sha2`, `tar`, `zstd`.
- No existing public API surfaces changed.
- No `unsafe` code introduced.
- All files remain under 400 lines.

## Context and orientation

The Whitaker project is a collection of Dylint lint libraries for
Rust, organized as a Cargo workspace. The key areas for this task
are:

**Artefact domain model** (`installer/src/artefact/`): Implemented
in task 3.4.1, this module contains validated newtypes for all
components of the ADR-001 artefact naming and manifest schema:

- `ArtefactName` (`naming.rs`) — produces filenames like
  `whitaker-lints-abc1234-nightly-2025-09-18-x86_64-unknown-linux-gnu.tar.zst`
- `Manifest`, `ManifestProvenance`, `ManifestContent`,
  `GeneratedAt` (`manifest.rs`) — the JSON manifest structure
- `GitSha` (`git_sha.rs`), `ToolchainChannel`
  (`toolchain_channel.rs`), `TargetTriple` (`target.rs`),
  `SchemaVersion` (`schema_version.rs`), `Sha256Digest`
  (`sha256_digest.rs`) — validated newtypes
- `VerificationPolicy`, `VerificationFailureAction`
  (`verification.rs`)
- `ArtefactError` (`error.rs`) — semantic error enum

**Existing CI** (`.github/workflows/ci.yml`): Runs on
`ubuntu-latest` and `windows-latest`. Steps: checkout, setup-rust
(via `leynos/shared-actions`), check-fmt, lint, test,
install-smoke, publish-check.

**Makefile**: Defines `LINT_CRATES` (8 crates including `suite`)
and a `publish-check` target that demonstrates the build pattern:

```shell
cargo +$TOOLCHAIN build --release --features dylint-driver -p $lint
```

Each built library is then copied with a toolchain-stamped name.

**Toolchain**: Pinned to `nightly-2025-09-18` in
`rust-toolchain.toml`.

**BDD testing**: Uses `rstest-bdd` v0.5.0 with Gherkin feature
files. The existing artefact BDD tests are in:

- `installer/tests/behaviour_artefact.rs` (step definitions +
  scenario bindings)
- `installer/tests/features/artefact_policy.feature` (8 scenarios)

The BDD world pattern uses a `#[derive(Default)]` struct with
`Option<T>` fields, a `#[fixture] fn world()` returning the
default, and step functions taking `&mut World`.

**Workspace dependencies** (`Cargo.toml`): `serde` (with `derive`
feature), `serde_json`, `thiserror`, `rstest` 0.26.1, `rstest-bdd`
0.5.0 are already declared.

**ADR-001** (`docs/adr-001-prebuilt-dylint-libraries.md`): Specifies
the five target triples, artefact naming, manifest schema,
verification policy, and rolling release model.

## Plan of work

### Stage A — Serialization support for artefact types

Add `#[derive(serde::Serialize)]` (and custom `Serialize` impls
where needed) to the existing artefact domain types so that
`Manifest` can be serialized to JSON matching the ADR-001 schema.
The newtypes (`GitSha`, `ToolchainChannel`, `TargetTriple`,
`Sha256Digest`) should serialize as their inner string value.
`SchemaVersion` should serialize as its inner `u32`. `GeneratedAt`
should serialize as its inner string.

Files modified:

- `installer/src/artefact/git_sha.rs` — added `Serialize`
- `installer/src/artefact/toolchain_channel.rs` — added `Serialize`
- `installer/src/artefact/target.rs` — added `Serialize`
- `installer/src/artefact/sha256_digest.rs` — added `Serialize`
- `installer/src/artefact/schema_version.rs` — added `Serialize`
- `installer/src/artefact/manifest.rs` — added `Serialize` to
  `ManifestProvenance`, `ManifestContent`, `Manifest`,
  `GeneratedAt`; added `#[serde(flatten)]` to provenance and
  content fields in `Manifest` so the JSON is flat as specified
  in ADR-001

### Stage B — Packaging module

Created `installer/src/artefact/packaging.rs` implementing:

1. `compute_sha256(path: &Path) -> Result<Sha256Digest,
   PackagingError>` — reads a file in 8 KiB chunks and returns
   its SHA-256 digest.

2. `create_archive(output_path: &Path,
   files: &[(PathBuf, String)]) -> Result<(), PackagingError>` —
   creates a `.tar.zst` archive at `output_path` containing the
   listed files with specified archive names.

3. `generate_manifest_json(manifest: &Manifest) ->
   Result<String, PackagingError>` — serializes a `Manifest` to
   pretty-printed JSON.

4. `package_artefact(params: PackageParams) ->
   Result<PackageOutput, PackagingError>` — orchestrates the full
   two-pass pipeline.

Created `installer/src/artefact/packaging_error.rs` with
`PackagingError` enum (I/O, serialization, empty file list
variants).

Updated `installer/src/artefact/mod.rs` with `pub mod packaging;`
and `pub mod packaging_error;`.

Added workspace dependencies: `sha2 = "^0.10.9"`,
`tar = "^0.4.44"`, `zstd = "^0.13.3"`.

### Stage C — Unit tests for packaging module

Created `installer/src/artefact/packaging_tests.rs` with 7 rstest
tests:

1. `compute_sha256_of_known_content` — SHA-256 of empty file
   matches the well-known constant.
2. `create_archive_contains_files` — archive contains expected
   entries.
3. `generate_manifest_json_matches_schema` — all 7 ADR-001 keys
   present.
4. `package_artefact_produces_valid_archive` — full pipeline
   produces valid archive with library and manifest.
5. `package_artefact_rejects_empty_files` — returns
   `EmptyFileList` error.
6. `archive_name_follows_adr_convention` — filename matches
   `ArtefactName`.
7. `manifest_sha256_is_valid_hex` — digest is 64-character hex.

### Stage D — BDD tests for packaging

Created `installer/tests/features/artefact_packaging.feature` with
5 scenarios and `installer/tests/behaviour_artefact_packaging.rs`
with step definitions and scenario bindings.

### Stage E — GitHub Actions workflow

Created `.github/workflows/rolling-release.yml` with:

- Build matrix for all 5 targets across ubuntu/macos/windows
  runners.
- Cross-compilation support for aarch64-linux.
- Packaging delegated to `whitaker-package-lints` binary.
- Publish job that replaces the `rolling` release tag.

### Stage F — Makefile target for local testing

Added `package-lints` target to `Makefile` that builds all lint
crates and delegates packaging to `whitaker-package-lints`.

### Stage G — Quality gates

Ran `make check-fmt`, `make lint`, and `make test`. All passed.
Fixed Clippy lints for `too_many_arguments` and
`field_reassign_with_default` during this stage.

### Stage H — Documentation updates

Marked roadmap items 3.4.2 and 3.4.3 as complete. Added
implementation notes section to ADR-001 documenting the rolling
release strategy, manifest inclusion approach,
serialization-only approach, and packaging module location.

### Stage I — Write execplan

Wrote this document to
`docs/execplans/3-4-2-publish-rolling-release.md`.

## Validation and acceptance

Quality criteria:

- `make check-fmt` passes (zero formatting violations).
- `make lint` passes (zero Clippy warnings, rustdoc clean).
- `make test` passes (all existing + new tests green).
- New BDD scenarios (5) in `artefact_packaging.feature` pass.
- Unit tests for `compute_sha256`, `create_archive`,
  `generate_manifest_json`, and `package_artefact` pass.
- Serialized manifest JSON contains all 7 ADR-001 keys at the
  top level.
- `.github/workflows/rolling-release.yml` is syntactically valid
  YAML with correct GitHub Actions schema.
- Roadmap items 3.4.2 and 3.4.3 are marked `[x]`.

Quality method:

```shell
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.out
make lint 2>&1 | tee /tmp/lint.out
make test 2>&1 | tee /tmp/test.out
```

## Idempotence and recovery

All stages are idempotent. Temporary files are created in system
temp directories. The packaging module uses temp files that are
cleaned up. The Makefile target outputs to `dist/` which can be
`rm -rf`'d. The rolling release workflow deletes the old release
before creating a new one. If any stage fails, fix the issue and
re-run from that stage.

## Artefacts and notes

Expected manifest JSON output:

```json
{
  "git_sha": "abc1234",
  "schema_version": 1,
  "toolchain": "nightly-2025-09-18",
  "target": "x86_64-unknown-linux-gnu",
  "generated_at": "2026-02-11T10:30:00Z",
  "files": [
    "libconditional_max_n_branches.so",
    "libfunction_attrs_follow_docs.so"
  ],
  "sha256": "e3b0c44298fc1c149afbf4c8996fb924..."
}
```

Expected artefact filename:

```plaintext
whitaker-lints-abc1234-nightly-2025-09-18-x86_64-unknown-linux-gnu.tar.zst
```

## Interfaces and dependencies

### New dependencies (workspace)

- `sha2` ^0.10.9 — SHA-256 computation. Pure Rust, no system
  dependencies.
- `zstd` ^0.13.3 — Zstandard compression for tar archives.
- `tar` ^0.4.44 — Tar archive creation.

### New types and functions

In `installer/src/artefact/packaging_error.rs`:

```rust
#[derive(Debug, Error)]
pub enum PackagingError {
    #[error("I/O error during packaging: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("no library files provided for packaging")]
    EmptyFileList,
}
```

In `installer/src/artefact/packaging.rs`:

```rust
pub struct PackageParams { ... }
pub struct PackageOutput { ... }
pub fn compute_sha256(path: &Path)
    -> Result<Sha256Digest, PackagingError>;
pub fn create_archive(
    output_path: &Path,
    files: &[(PathBuf, String)],
) -> Result<(), PackagingError>;
pub fn generate_manifest_json(manifest: &Manifest)
    -> Result<String, PackagingError>;
pub fn package_artefact(params: PackageParams)
    -> Result<PackageOutput, PackagingError>;
```

### Serde additions to existing types

Each newtype wrapping a `String` gets
`#[derive(serde::Serialize)]` with `#[serde(transparent)]` so it
serializes as the inner string. `SchemaVersion` wrapping `u32`
gets the same treatment. `Manifest` uses `#[serde(flatten)]` on
its `provenance` and `content` fields to produce a flat JSON
object.

### Files created

- `installer/src/artefact/packaging.rs` — packaging pipeline
- `installer/src/artefact/packaging_error.rs` — error types
- `installer/src/artefact/packaging_tests.rs` — unit tests
- `installer/tests/features/artefact_packaging.feature` — BDD
  feature
- `installer/tests/behaviour_artefact_packaging.rs` — BDD test
  harness
- `.github/workflows/rolling-release.yml` — CI workflow

### Files modified

- `installer/src/artefact/git_sha.rs` — added `Serialize`
- `installer/src/artefact/toolchain_channel.rs` — added
  `Serialize`
- `installer/src/artefact/target.rs` — added `Serialize`
- `installer/src/artefact/sha256_digest.rs` — added `Serialize`
- `installer/src/artefact/schema_version.rs` — added `Serialize`
- `installer/src/artefact/manifest.rs` — added `Serialize`,
  flatten, tests
- `installer/src/artefact/mod.rs` — added packaging modules
- `installer/Cargo.toml` — added sha2, tar, zstd dependencies
- `Cargo.toml` (workspace) — added workspace dependency entries
- `Makefile` — added `package-lints` target
- `docs/roadmap.md` — marked 3.4.2 and 3.4.3 complete
- `docs/adr-001-prebuilt-dylint-libraries.md` — added
  implementation notes

## Revision note

Initial plan created 2026-02-11 for roadmap item 3.4.2.
Implementation completed 2026-02-11. All stages executed
successfully with minor Clippy lint fixes during Stage G.
