# Move library file discovery and remaining shell logic into Rust

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as
work proceeds.

Status: COMPLETE

## Context

The `whitaker-package-lints` binary was introduced to centralise tar/zstd
archive creation, manifest JSON generation, and SHA-256 computation in
Rust. However, both the Makefile `package-lints` target and the CI
workflow `rolling-release.yml` still contain shell loops that:

1. Map target triples to platform-specific library extensions
   (`LIB_EXT` case/matrix).
2. Construct expected library filenames (`lib${lint}.${EXT}`).
3. Iterate crate names to discover built library files in the release
   directory.

This duplicated shell logic is brittle -- it uses space-separated
`LIB_FILES` strings, has inconsistent Windows handling (CI checks for
`dll` prefix stripping; Makefile does not), and silently misses the suite
crate because the Makefile uses `-p suite` while the actual library is
`libwhitaker_suite.so`.

The tar/zstd and SHA-256 operations are already fully in Rust
(`create_archive()` and `compute_sha256()`) with good test coverage,
but the user has asked to confirm this and strengthen the tests.

## Purpose / big picture

After this change:

1. `TargetTriple` gains runtime `library_extension()` and
   `library_prefix()` methods suitable for cross-compilation.
2. The binary gains a `--release-dir` flag that auto-discovers library
   files using the canonical crate list from `resolution.rs`.
3. Both Makefile and CI pass `--release-dir` instead of shell loops.
4. New tests confirm tar/zstd creation, SHA-256 self-consistency, and
   multi-file packaging.

Success is observable by:

- `make package-lints` has no `LIB_EXT` or `LIB_FILES` shell logic.
- CI "Package artefact" step is a single binary invocation with
  `--release-dir`; the `lib_ext` matrix field is removed.
- `make check-fmt && make lint && make test` all green.
- Existing `library_files` positional invocation still works.

## Constraints

- 400-line file limit per AGENTS.md.
- Clippy max 4 params; group in structs.
- No `unsafe`. en-GB-oxendict spelling.
- No new external dependencies.
- Backwards compatibility: positional `library_files` must still work.
- Canonical crate lists: `LINT_CRATES` and `SUITE_CRATE` from
  `installer/src/resolution.rs`.

## Tolerances (exception triggers)

- Scope: > 10 files or > 200 net new lines -- escalate.
- Dependencies: > 0 new external crates -- escalate.
- Iterations: tests fail after 3 fix attempts -- escalate.

## Risks

- Risk: Makefile `LINT_CRATES` has `suite` but the library is
  `libwhitaker_suite.so`. The Rust discovery must use the correct
  package name `whitaker_suite` from `resolution::SUITE_CRATE`.
  Severity: medium. Mitigation: the `-p suite` build flag still works
  (Cargo resolves workspace members); only the discovery step uses
  the Rust constant.

- Risk: Changing `library_files` from `required = true` to
  `required_unless_present = "release_dir"` might affect clap parsing.
  Severity: low. Mitigation: test both invocation modes.

## Progress

- [x] Stage A: Add `library_extension()` and `library_prefix()` to
  `TargetTriple` with unit tests.
- [x] Stage B: Add `--release-dir` flag and discovery logic to the
  binary with unit tests.
- [x] Stage C: Simplify the Makefile `package-lints` target.
- [x] Stage D: Simplify the CI workflow "Package artefact" step.
- [x] Stage E: Add BDD scenarios for multi-library packaging and
  manifest `files` field.
- [x] Stage F: Add SHA-256 self-consistency test to packaging unit
  tests.
- [x] Stage G: Update ADR-001 implementation notes.
- [x] Stage H: Run quality gates.

## Surprises & discoveries

- The two-pass SHA-256 algorithm means the manifest digest records the
  hash of the pass-1 archive, not the final on-disk archive (which has
  a different hash because it embeds the pass-1 digest). The
  self-consistency test was changed to verify determinism (identical
  inputs produce identical digests) rather than self-reference.

- The `package_lints.rs` binary file approached the 400-line limit.
  Resolved by removing section divider comments and condensing doc
  comments on the new fields.

## Decision log

- Decision: Use `conflicts_with` and `required_unless_present` in clap
  to make `--release-dir` and positional `library_files` mutually
  exclusive. Rationale: simpler than subcommands; both invocation modes
  are tested.

- Decision: The `discover_library_files` function uses
  `LINT_CRATES` and `SUITE_CRATE` from `resolution.rs` as the canonical
  crate list. Rationale: keeps the crate list in a single authoritative
  location; the Makefile's `LINT_CRATES` variable is used only for the
  build step (which must use Cargo package names like `suite`).

## Outcomes & retrospective

All stages completed successfully. Quality gates green:

- `make check-fmt` -- clean.
- `make lint` -- zero warnings (cargo doc + clippy).
- `make test` -- 555 tests passed, 0 failed, 2 skipped.

Test count increased from 542 to 555 (+13 new tests across unit,
integration, and BDD layers).

### What went well

- The canonical crate list in `resolution.rs` (`LINT_CRATES` +
  `SUITE_CRATE`) gave a single source of truth; the discovery function
  was straightforward.
- `clap`'s `conflicts_with` / `required_unless_present` made the
  mutual exclusion between `--release-dir` and positional
  `library_files` clean.
- Both the Makefile and CI workflow became significantly simpler:
  Makefile shrank by 13 lines; CI workflow by 28 lines.

### What was tricky

- The two-pass SHA-256 algorithm meant a naive "archive hash equals
  manifest hash" assertion would always fail. Resolved by testing
  determinism instead.
- `package_lints.rs` hit the 400-line file limit (407 lines). Resolved
  by removing section dividers and condensing doc comments to fit
  within 395 lines.

### Scope check

- 9 files modified -- within the 10-file tolerance.
- 0 new external dependencies -- within tolerance.
- All tests passed on first gate run after final fixes -- within the
  3-iteration tolerance.
