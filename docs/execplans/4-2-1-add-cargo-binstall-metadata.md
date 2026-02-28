# Add cargo-binstall metadata to installer/Cargo.toml (roadmap 4.2.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 4.2.1 enables users to install the `whitaker-installer` binary
without compiling from source by running `cargo binstall whitaker-installer`.
After this change:

1. `installer/Cargo.toml` contains a `[package.metadata.binstall]` section
   with `pkg-url`, `bin-dir`, and `pkg-fmt` templates matching the design
   document specification (§ Installer release artefacts, lines 1401-1409).
2. A Windows-specific override sets `pkg-fmt = "zip"` for the
   `x86_64-pc-windows-msvc` target.
3. A new `binstall_metadata` module in `installer/src/` exposes constants and
   template expansion helpers so the metadata values are testable.
4. Unit tests (`rstest`) parse the actual `installer/Cargo.toml` and verify
   every binstall field matches the design-document specification.
5. Behaviour-driven development (BDD) scenarios (`rstest-bdd` v0.5.0)
   validate template expansion for all supported targets and check for invalid
   placeholders.
6. The roadmap marks 4.2.1 as done.
7. The design document records implementation completion.
8. `make check-fmt`, `make lint`, and `make test` all pass.

Observable outcome: running `make test` shows new unit tests and BDD scenarios
passing. The `[package.metadata.binstall]` section in `installer/Cargo.toml`
contains the exact templates from the design document, and the new tests fail
if the metadata is removed or altered.

## Constraints

- The binstall metadata must exactly match the specification in
  `docs/whitaker-dylint-suite-design.md` lines 1401-1409. No deviation.
- Every file must stay under 400 lines (per `AGENTS.md`).
- Every module must begin with a `//!` doc comment.
- No new external dependencies. The `toml` crate is already in both
  `[dependencies]` and `[dev-dependencies]`.
- Workspace Clippy `too_many_arguments` limit is 4. BDD step functions
  must have at most 4 parameters (world + 3 parsed values).
- The `rstest-bdd` world fixture parameter must be named `world` (not
  `_world`) — the macro matches parameter names literally.
- Use caret version requirements for all dependencies.
- Comments and documentation use en-GB-oxendict spelling.
- Markdown wrapped at 80 columns; code blocks at 120.
- `clippy::expect_used = "deny"` and `clippy::unwrap_used = "deny"` apply in
  non-test code. The new module's public functions must not call `.expect()` or
  `.unwrap()` (they are infallible by construction, so this is not an issue).
- On completion, update `docs/roadmap.md` entry 4.2.1 to `[x]`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 10 files, stop and
  escalate.
- Interface: if a public application programming interface (API) signature in
  existing code must change, stop and escalate.
- Dependencies: if a new external dependency is required, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix attempts per gate, stop and escalate with logs.
- Ambiguity: if the cargo-binstall template placeholders do not match the
  documented behaviour in the design document, stop and present options.

## Risks

- Risk: The `toml` crate's table navigation for nested
  `package.metadata.binstall` may require careful chaining of `.get()` calls.
  Severity: low. Likelihood: low. Mitigation: the existing `behaviour_docs.rs`
  already demonstrates TOML table parsing; the same pattern applies here.
  Outcome: no issues encountered.

- Risk: BDD step functions with URL template parameters might exceed the
  4-argument Clippy limit. Severity: low. Likelihood: medium. Mitigation: split
  Gherkin steps so each step function parses at most 2–3 values from the
  feature text. Use the world struct to carry state between steps. Outcome: all
  step functions stayed within the 4-parameter limit.

## Progress

- [x] (2026-02-27) Stage A: Add binstall metadata to `installer/Cargo.toml`.
- [x] (2026-02-27) Stage B: Create `binstall_metadata` module and unit tests.
- [x] (2026-02-27) Stage C: Create Gherkin feature file and BDD behaviour
  tests.
- [x] (2026-02-27) Stage D: Update roadmap and design document.
- [x] (2026-02-27) Stage E: Run quality gates (`make check-fmt && make lint
  && make test`).
- [x] (2026-02-27) Stage F: Write execplan to `docs/execplans/` and commit.

## Surprises & discoveries

- Observation: `cargo fmt` reflows multi-line `.expect()` chains and import
  orderings differently from hand-written style. Evidence: first
  `make check-fmt` run showed diffs in `binstall_metadata_tests.rs` and
  `behaviour_binstall.rs`. Impact: resolved by running `make fmt` before
  re-checking. No functional change.

## Decision log

- Decision: Place the binstall metadata module at
  `installer/src/binstall_metadata.rs` (a top-level peer to `cli`, `pipeline`,
  etc.) rather than inside the `artefact/` subtree. Rationale: The `artefact/`
  module covers the prebuilt lint library artefact system (architecture
  decision record (ADR) 001). Binstall metadata is about the installer binary's
  own distribution packaging — a separate concern. A top-level module keeps
  concerns separated and avoids overloading the `artefact` namespace.
  Date/Author: 2026-02-27 / plan author

- Decision: Parse the actual `Cargo.toml` in tests via
  `env!("CARGO_MANIFEST_DIR")` rather than embedding expected values as string
  constants alone. Rationale: Parsing the real file detects drift. If someone
  changes the metadata without updating tests, assertions will catch the
  mismatch. The constants in the module serve as the single source of truth
  that both unit tests and BDD scenarios reference. Date/Author: 2026-02-27 /
  plan author

## Outcomes & retrospective

All eight deliverables listed in Purpose were achieved:

1. `installer/Cargo.toml` now contains the exact `[package.metadata.binstall]`
   section from the design document.
2. The Windows override for `x86_64-pc-windows-msvc` uses `pkg-fmt = "zip"`.
3. `installer/src/binstall_metadata.rs` exposes six constants and two template
   expansion helpers.
4. Unit tests in `binstall_metadata_tests.rs` parse the real `Cargo.toml` and
   validate all fields (11 tests).
5. Seven BDD scenarios in `behaviour_binstall.rs` cover happy paths (metadata
   presence, template expansion for Linux/Windows) and unhappy paths (invalid
   placeholder detection).
6. Roadmap item 4.2.1 is marked `[x]`.
7. Design document updated with implementation status note.
8. `make check-fmt`, `make lint`, and `make test` all pass (845 tests, 0
   failures).

No tolerances were breached. Total: 4 new files + 5 modified files = 9 files
(within the 10-file tolerance). No new dependencies added.

## Context and orientation

The Whitaker project is a Rust workspace at `/home/user/project/` containing
Dylint lint crates, a `common` library, and an installer command-line interface
(CLI). The installer lives in `installer/` and is published as the
`whitaker-installer` crate (v0.2.0).

Key files for this task:

- `installer/Cargo.toml` (84 lines) — the installer's package manifest, now
  including the `[package.metadata.binstall]` section.
- `installer/src/lib.rs` (65 lines) — the library root, now declaring
  `pub mod binstall_metadata;`.
- `installer/src/binstall_metadata.rs` (85 lines) — constants and expansion
  helpers.
- `installer/src/binstall_metadata_tests.rs` (146 lines) — unit tests.
- `installer/tests/features/binstall_metadata.feature` (42 lines) — Gherkin
  scenarios.
- `installer/tests/behaviour_binstall.rs` (239 lines) — BDD step definitions
  and scenario bindings.
- `docs/whitaker-dylint-suite-design.md` — design section updated with
  implementation status.
- `docs/roadmap.md` — 4.2.1 marked as done.

Cargo-binstall template placeholders:

- `{name}` — crate name (`whitaker-installer`)
- `{version}` — crate version (e.g. `0.2.0`)
- `{target}` — Rust target triple (e.g. `x86_64-unknown-linux-gnu`)
- `{archive-format}` — archive extension derived from `pkg-fmt` (`tgz`/`zip`)
- `{bin}` — binary name (`whitaker-installer` or `whitaker-installer.exe`)

Supported target triples (from the design document):
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`,
`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.

## Plan of work

### Stage A: Add binstall metadata to `installer/Cargo.toml`

Append the following TOML after the existing `[lints]` section (after line 76)
in `installer/Cargo.toml`:

```toml
[package.metadata.binstall]
pkg-url = "https://github.com/leynos/whitaker/releases/download/v{version}/{name}-{target}-v{version}.{archive-format}"
bin-dir = "{name}-{target}-v{version}/{bin}"
pkg-fmt = "tgz"

[package.metadata.binstall.overrides.x86_64-pc-windows-msvc]
pkg-fmt = "zip"
```

This is verbatim from the design document (lines 1401-1409). The file grows
from 77 to approximately 84 lines.

### Stage B: Create `binstall_metadata` module and unit tests

Create `installer/src/binstall_metadata.rs` (~80 lines) containing:

- Six public constants: `REPO_URL`, `PKG_URL_TEMPLATE`, `BIN_DIR_TEMPLATE`,
  `DEFAULT_PKG_FMT`, `WINDOWS_PKG_FMT`, `WINDOWS_OVERRIDE_TARGET`.
- Two public functions: `expand_pkg_url(version, target) -> String` and
  `expand_bin_dir(version, target) -> String`. Both are `#[must_use]`, pure,
  and infallible.
- A `#[cfg(test)] #[path = "binstall_metadata_tests.rs"] mod tests;` anchor.

Create `installer/src/binstall_metadata_tests.rs` (~140 lines) containing
`rstest`-based unit tests that:

- Parse the actual `installer/Cargo.toml` via `env!("CARGO_MANIFEST_DIR")`.
- Assert `pkg-url`, `bin-dir`, `pkg-fmt` match the module constants.
- Assert the Windows override sets `pkg-fmt = "zip"`.
- Assert exactly one override exists.
- Assert all essential fields are present.
- Parameterized tests expand templates for each non-Windows target (`.tgz`)
  and the Windows target (`.zip`).
- Verify `bin-dir` expands to `whitaker-installer` (Unix) and
  `whitaker-installer.exe` (Windows).
- Verify `pkg-url` starts with `REPO_URL`.

Register the module in `installer/src/lib.rs`:

- Add `//! - [\`binstall_metadata\`] - Cargo-binstall metadata constants and
  template expansion` to the doc comment (between `artefact` and `builder`).
- Add `pub mod binstall_metadata;` between `pub mod artefact;` and
  `pub mod builder;`.

### Stage C: Create Gherkin feature file and BDD behaviour tests

Create `installer/tests/features/binstall_metadata.feature` (~40 lines) with
seven scenarios:

1. Binstall metadata section exists in Cargo.toml — verifies `pkg-url`,
   `bin-dir`, and default `pkg-fmt` are present and correct.
2. Windows override uses zip format — verifies the override table.
3. URL template expands correctly for Linux — checks `.tgz` suffix and
   target presence.
4. URL template expands correctly for Windows — checks `.zip` suffix and
   target presence.
5. Binary directory expands correctly for Unix — checks path ends with
   `whitaker-installer`.
6. Binary directory expands correctly for Windows — checks path ends with
   `whitaker-installer.exe`.
7. No invalid placeholders in templates — checks that `{repo}` and `{crate}`
   are absent (they are common mistakes that cargo-binstall does not support).

Create `installer/tests/behaviour_binstall.rs` (~230 lines) following the
established BDD pattern:

- `//!` doc comment explaining the file's purpose.
- Imports from `rstest::fixture`, `rstest_bdd_macros`, `toml::Table`, and
  `whitaker_installer::binstall_metadata`.
- A `BinstallWorld` struct with `Option` fields for the parsed TOML table,
  binstall sub-table, overrides table, target, version, expanded URL, and
  expanded bin-dir.
- A `#[fixture] fn world()` returning `BinstallWorld::default()`.
- Step definitions (`#[given]`, `#[when]`, `#[then]`) matching the Gherkin
  steps. Each step function has at most 4 parameters.
- Scenario bindings (`#[scenario]`) linking each Gherkin scenario name to a
  binding function.

### Stage D: Update roadmap and design document

In `docs/roadmap.md` line 118, change `- [ ]` to `- [x]`.

In `docs/whitaker-dylint-suite-design.md`, after line 1412 (the sentence about
cargo-binstall placeholders), add:

```plaintext
**Implementation status:** The `[package.metadata.binstall]` entries above
have been added to `installer/Cargo.toml` and are validated by unit tests
in `installer/src/binstall_metadata_tests.rs` and BDD scenarios in
`installer/tests/features/binstall_metadata.feature`.
```

### Stage E: Quality gates

Run from the workspace root:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/binstall-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/binstall-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/binstall-test.log
```

All three must exit 0. Review the test log to confirm the new tests appear.

### Stage F: Write execplan and commit

Copy this plan to `docs/execplans/4-2-1-add-cargo-binstall-metadata.md`, update
Status to COMPLETE, and commit all changes.

## Concrete steps

All commands run from `/home/user/project/`.

1. Edit `installer/Cargo.toml` — append binstall metadata after `[lints]`.
2. Create `installer/src/binstall_metadata.rs` — constants and helpers.
3. Create `installer/src/binstall_metadata_tests.rs` — unit tests.
4. Edit `installer/src/lib.rs` — add doc-comment entry and
   `pub mod binstall_metadata;`.
5. Create `installer/tests/features/binstall_metadata.feature` — Gherkin.
6. Create `installer/tests/behaviour_binstall.rs` — BDD tests.
7. Edit `docs/roadmap.md` — mark 4.2.1 as `[x]`.
8. Edit `docs/whitaker-dylint-suite-design.md` — add implementation note.
9. `set -o pipefail && make check-fmt 2>&1 | tee /tmp/binstall-fmt.log`
10. `set -o pipefail && make lint 2>&1 | tee /tmp/binstall-lint.log`
11. `set -o pipefail && make test 2>&1 | tee /tmp/binstall-test.log`
12. Create `docs/execplans/4-2-1-add-cargo-binstall-metadata.md`.
13. Commit.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. New unit tests in `binstall_metadata_tests.rs`
  and BDD scenarios in `behaviour_binstall.rs` all pass. The tests fail if the
  binstall metadata is removed from `Cargo.toml`.
- Lint/typecheck: `make check-fmt` and `make lint` exit 0.
- No regressions: all existing tests continue to pass.
- Documentation: roadmap item 4.2.1 is checked. Design document notes
  implementation is complete.

Quality method (verification procedure):

```bash
make check-fmt && make lint && make test
```

All three must exit 0.

## Idempotence and recovery

All steps are idempotent. Re-running file creation overwrites the same content.
Re-running quality gates produces the same result.

To revert: remove the `[package.metadata.binstall]` and override sections from
`Cargo.toml`, delete `binstall_metadata.rs`, `binstall_metadata_tests.rs`,
`behaviour_binstall.rs`, and `binstall_metadata.feature`, remove the
`pub mod binstall_metadata;` line and doc-comment entry from `lib.rs`, and
revert the roadmap/design-doc edits.

## Artefacts and notes

Summary of new files:

| File                                                 | Purpose                           | Lines |
| ---------------------------------------------------- | --------------------------------- | ----- |
| `installer/src/binstall_metadata.rs`                 | Constants and expansion helpers   | 85    |
| `installer/src/binstall_metadata_tests.rs`           | Unit tests parsing Cargo.toml     | 146   |
| `installer/tests/features/binstall_metadata.feature` | Gherkin scenarios                 | 42    |
| `installer/tests/behaviour_binstall.rs`              | BDD step definitions and bindings | 239   |

Summary of modified files:

| File                                   | Change                                         |
| -------------------------------------- | ---------------------------------------------- |
| `installer/Cargo.toml`                 | Append 7 lines of binstall metadata            |
| `installer/src/lib.rs`                 | Add 2 doc-comment lines + 1 module declaration |
| `docs/roadmap.md`                      | `[ ]` to `[x]` on line 118                     |
| `docs/whitaker-dylint-suite-design.md` | Add 4-line implementation note                 |

Total: 4 new files + 5 modified files = 9 files (within 10-file tolerance).

## Interfaces and dependencies

No new external dependencies. All required crates are already workspace
dependencies: `toml`, `rstest`, `rstest-bdd`, `rstest-bdd-macros`.

New public API surface in `installer/src/binstall_metadata.rs`:

```rust
pub const REPO_URL: &str;
pub const PKG_URL_TEMPLATE: &str;
pub const BIN_DIR_TEMPLATE: &str;
pub const DEFAULT_PKG_FMT: &str;
pub const WINDOWS_PKG_FMT: &str;
pub const WINDOWS_OVERRIDE_TARGET: &str;

/// Expand the pkg-url template for a given version and target.
#[must_use]
pub fn expand_pkg_url(version: &str, target: &str) -> String;

/// Expand the bin-dir template for a given version and target.
#[must_use]
pub fn expand_bin_dir(version: &str, target: &str) -> String;
```
