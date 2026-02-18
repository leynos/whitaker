# Extract prebuilt libraries to XDG share path (roadmap 3.4.5)

This execution plan (ExecPlan) is a living document. The sections Constraints,
Tolerances, Risks, Progress, Surprises & Discoveries, Decision Log, and
Outcomes & Retrospective must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

The canonical plan file is
`docs/execplans/3-4-4-extract-libraries-to-xdg-share.md`.

## Purpose / big picture

Roadmap item 3.4.5 requires the prebuilt installer path from ADR-001: prebuilt
lint libraries must extract to
`~/.local/share/whitaker/lints/<toolchain>/<target>/lib` (platform-equivalent
path on macOS/Windows) and `DYLINT_LIBRARY_PATH` must point at that exact
directory.

Today, the prebuilt pipeline writes to `<target-dir>/<toolchain>/release`,
which does not include target segregation and does not match ADR-001.

After this change, a successful prebuilt install will:

1. Resolve a canonical Whitaker data directory from `BaseDirs`.
2. Extract to `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`.
3. Feed that path to wrapper/snippet generation so users get the correct
   `DYLINT_LIBRARY_PATH`.
4. Preserve existing fallback behaviour: any prebuilt failure still triggers
   local compilation.

Success is observable by:

1. Unit tests proving destination path construction, extraction destination, and
   fallback on destination errors.
2. Behaviour tests (`rstest-bdd` v0.5.0) covering happy and unhappy flows,
   including edge cases around destination layout.
3. `make check-fmt`, `make lint`, and `make test` all succeeding.
4. `docs/roadmap.md` item 3.4.5 marked done.
5. Design decision updates recorded in `docs/whitaker-dylint-suite-design.md`.

## Constraints

- Implement ADR-001 path contract exactly:
  `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`.
- Keep prebuilt failures non-fatal; fallback to local build must remain intact.
- Keep `rstest-bdd` at workspace version `0.5.0`; do not introduce alternate
  BDD frameworks.
- Add both unit and behavioural test coverage for happy and unhappy paths.
- Keep files below 400 lines; split tests/modules when needed.
- Preserve en-GB-oxendict spelling in docs/comments.
- Use existing Make targets for quality gates and capture logs with
  `set -o pipefail` and `tee`.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 12 files changed or 450 net LOC, stop and
  escalate.
- Interface: if fulfilling the path contract requires breaking public installer
  CLI flags or removing existing options, stop and escalate.
- Behaviour: if `--target-dir` semantics for local builds would need to change,
  stop and escalate.
- Iterations: if quality gates still fail after 5 fix cycles, stop and
  escalate with failure summary and options.

## Risks

- Risk: ambiguity around `--target-dir` for prebuilt installs.
  Mitigation: treat prebuilt destination as ADR-canonical path and keep
  `--target-dir` scope for local build staging in this task; record decision in
  design doc.

- Risk: list/scanner compatibility with the new directory shape.
  Mitigation: update scanner/list logic to recognise the prebuilt layout, with
  tests covering both legacy and new layouts.

- Risk: platform path handling differences (`Application Support`, LocalAppData)
  causing subtle path bugs. Mitigation: route destination derivation through
  `BaseDirs::whitaker_data_dir` and validate with targeted unit tests.

- Risk: regressions in wrapper/snippet path propagation.
  Mitigation: add tests asserting `DYLINT_LIBRARY_PATH` points to the final
  `<...>/lib` directory from successful prebuilt installs.

## Context and orientation

Current relevant code:

- `installer/src/main.rs` creates `PrebuiltConfig` using `target_dir` and exits
  early on prebuilt success via `finish_install(...)`.
- `installer/src/prebuilt.rs` currently builds destination as
  `staging_base/<toolchain>/release`.
- `installer/src/wrapper.rs` and `installer/src/output.rs` already emit
  `DYLINT_LIBRARY_PATH` for whichever staging path they receive.
- `installer/tests/behaviour_prebuilt.rs` and
  `installer/tests/features/prebuilt_download.feature` already cover checksum,
  network, not-found, toolchain mismatch, and build-only fallback.
- `installer/src/scanner.rs` currently assumes `<root>/<toolchain>/release`.

Reference docs to follow while implementing:

- `docs/adr-001-prebuilt-dylint-libraries.md`
- `docs/whitaker-dylint-suite-design.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`

## Plan of work

### Phase 1: Canonical prebuilt destination path

Create a dedicated path-construction helper that derives the prebuilt library
destination from `whitaker_data_dir`, toolchain, and target:

- `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`

Implementation notes:

- Place logic in a testable library module (not only in binary-local code).
- Validate/sanitise target and toolchain with existing domain newtypes where
  practical.
- Return semantic installer errors for missing directory roots.

Planned files:

- `installer/src/prebuilt_path.rs` (new)
- `installer/src/lib.rs` (module export)
- `installer/src/error.rs` (if a new error variant is needed)
- `installer/src/main.rs` (use helper while building `PrebuiltConfig`)

### Phase 2: Prebuilt extraction path contract

Refactor prebuilt orchestration to extract directly to the canonical
`<...>/lib` destination instead of `<toolchain>/release`.

Implementation notes:

- Update `PrebuiltConfig` to carry an explicit destination directory rather than
  a base path.
- Ensure extraction creates the destination directory before unpacking.
- Keep existing rename logic (`lib<crate>@<toolchain>.<ext>`) and fallback
  semantics unchanged.

Planned files:

- `installer/src/prebuilt.rs`
- `installer/src/prebuilt_tests.rs`

### Phase 3: DYLINT_LIBRARY_PATH propagation and UX

Verify that successful prebuilt installs pass the canonical destination to
`finish_install`, wrapper generation, and shell snippets.

Implementation notes:

- Keep wrapper generation behaviour unchanged; only path input changes.
- Assert that path points at the `lib` leaf, not parent directories.

Planned files:

- `installer/src/main.rs`
- `installer/src/output.rs` (tests only, if needed)
- `installer/src/wrapper.rs` (tests only, if needed)

### Phase 4: Listing compatibility for new layout

Ensure `whitaker-installer list` can discover libraries in the prebuilt layout
as well as the existing local-build layout.

Implementation notes:

- Accept both:
  `<root>/<toolchain>/release/...` and `<root>/<toolchain>/<target>/lib/...`.
- Keep output format stable.

Planned files:

- `installer/src/scanner.rs`
- `installer/src/list.rs`
- `installer/tests/behaviour_staging.rs` (if behaviour coverage is extended)

### Phase 5: Unit tests

Add/extend unit tests for:

- destination path builder happy path and invalid/missing base-dir handling;
- prebuilt extraction destination equals `<...>/lib`;
- fallback when destination creation/extraction fails;
- scanner support for both legacy and new directory layouts.

Primary files:

- `installer/src/prebuilt_path.rs` (new tests)
- `installer/src/prebuilt_tests.rs`
- `installer/src/scanner.rs`
- `installer/src/list.rs`

### Phase 6: Behaviour tests with `rstest-bdd` v0.5.0

Extend BDD coverage in `installer/tests/behaviour_prebuilt.rs` and
`installer/tests/features/prebuilt_download.feature`.

Required scenarios:

- happy path: successful prebuilt download returns a staging path ending in
  `<toolchain>/<target>/lib`;
- unhappy path: destination preparation failure causes fallback with actionable
  reason;
- edge case: build-only still skips prebuilt attempt;
- edge case: toolchain mismatch still falls back and does not write output into
  target path.

BDD implementation style must follow the mutable-world fixture guidance from
`docs/rstest-bdd-users-guide.md` and fixture hygiene guidance from
`docs/rust-testing-with-rstest-fixtures.md`.

### Phase 7: Documentation and decision recording

Update documentation to reflect implemented behaviour and decisions:

- Add a design decision entry to `docs/whitaker-dylint-suite-design.md` under
  the prebuilt distribution section, including rationale and scope boundaries.
- Update user/developer installer path examples where they describe prebuilt
  locations.
- Mark roadmap item 3.4.5 done in `docs/roadmap.md` only after tests and gates
  pass.

Planned files:

- `docs/whitaker-dylint-suite-design.md`
- `docs/developers-guide.md`
- `docs/users-guide.md` (if path examples mention prebuilt staging)
- `docs/roadmap.md`

### Phase 8: Quality gates and evidence capture

Run mandatory checks with log capture:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/whitaker-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/whitaker-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/whitaker-test.log

Review logs for:

- zero formatter/lint/test failures;
- no warnings promoted to errors;
- no unexpected regressions in installer behaviour suites.

## Validation checklist

- Unit tests pass for destination derivation, extraction destination, and
  scanner compatibility.
- BDD scenarios pass for happy path, unhappy path, and edge cases.
- Wrapper/snippet output references the canonical `<...>/lib` directory on
  successful prebuilt installs.
- `make check-fmt`, `make lint`, and `make test` pass.
- Design decision and roadmap updates are committed.

## Progress

- [x] 2026-02-18: Draft ExecPlan created for roadmap item 3.4.5.
- [x] Phase 1 complete: canonical destination helper implemented.
- [x] Phase 2 complete: prebuilt extraction path updated.
- [x] Phase 3 complete: path propagation to wrappers/snippets verified.
- [x] Phase 4 complete: list/scanner compatibility implemented.
- [x] Phase 5 complete: unit tests added and passing.
- [x] Phase 6 complete: BDD scenarios added and passing.
- [x] Phase 7 complete: docs and roadmap updated.
- [x] Phase 8 complete: `make check-fmt`, `make lint`, `make test` all pass.

## Surprises & Discoveries

- The documentation behaviour test for prebuilt path metadata still asserted
  the legacy `dylint/lib/<toolchain>/release` shape. The test had to be
  updated to assert the new `<whitaker>/lints/<toolchain>/<target>/lib`
  structure to keep docs and implementation aligned.

## Decision Log

- Decision: Prebuilt artefacts use a canonical destination rooted at
  `whitaker_data_dir`, independent of `--target-dir` local build staging.
  Rationale: this gives deterministic cache keys by toolchain and target while
  preserving existing local build behaviour.

- Decision: Behaviour coverage extends the existing prebuilt BDD feature file
  rather than creating a parallel feature file. Rationale: keeps all prebuilt
  success/fallback behaviour in one executable specification.

- Decision: `whitaker-installer list` scans both default local staging and the
  prebuilt `whitaker/lints` root when no `--target-dir` override is supplied.
  Rationale: without dual-root scanning, successful prebuilt installs would be
  invisible to the default listing command.

## Outcomes & Retrospective

Implementation complete.

Delivered outcomes:

- New `prebuilt_path` module derives
  `<whitaker_data_dir>/lints/<toolchain>/<target>/lib`.
- Prebuilt orchestration now extracts directly to an explicit destination
  directory (`PrebuiltConfig::destination_dir`).
- Installer prebuilt flow resolves destination via `BaseDirs` and still falls
  back to local compilation on all prebuilt failures.
- Scanner supports both legacy `<toolchain>/release` and new
  `<toolchain>/<target>/lib` layouts.
- List command merges discovered libraries from both default roots.
- Unit tests and BDD scenarios were expanded for happy paths, fallback paths,
  and edge cases.
- Design docs and roadmap were updated, including roadmap item 3.4.5 marked
  done.
- Quality gates passed:
  - `make check-fmt`
  - `make lint`
  - `make test` (617 passed, 2 skipped)
