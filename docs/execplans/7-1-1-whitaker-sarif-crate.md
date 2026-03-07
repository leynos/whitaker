# Create the `whitaker_sarif` crate with SARIF 2.1.0 models, builders, and merge logic (roadmap 7.1.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.1.1 delivers the SARIF 2.1.0 foundation crate for the clone
detector pipeline (roadmap §7). After this change a developer can:

1. Construct well-formed SARIF 2.1.0 documents using fluent Rust builders.
2. Define Whitaker clone detection rules (WHK001, WHK002, WHK003).
3. Attach Whitaker-specific metadata (similarity profile, scores, group info)
   to results via a typed properties extension.
4. Merge runs from multiple detection passes and deduplicate results by
   `(fingerprint, file, region)`.
5. Resolve stable file paths for the token pass, AST pass, and refined SARIF
   outputs under `target/whitaker/`.

The crate is a pure library with no compiler dependencies, usable by downstream
crates `whitaker_clones_core`, `whitaker_clones_cli`, and the `clone_detected`
Dylint lint.

Observable outcome: `make check-fmt`, `make lint`, and `make test` pass. Unit
tests cover serialization round-trips, builder correctness, merge
deduplication, and path construction. Behaviour-driven development (BDD)
scenarios exercise end-to-end construction and merge workflows using
`rstest-bdd` v0.5.0.

## Constraints

- Scope only roadmap item 7.1.1. Do not implement any token or AST pipeline
  logic (those are 7.2.x and 7.3.x).
- Keep the crate free of `rustc_private` dependencies. Accept only plain Rust
  types (`String`, `usize`, enums, `Vec`, `HashMap`).
- Follow established module layout and line-count constraint (< 400 lines per
  source file). Split files proactively.
- Use builder patterns when constructor argument count would exceed the
  workspace Clippy `too_many_arguments` threshold (4).
- Use workspace-pinned `rstest = "0.26.1"`, `rstest-bdd = "0.5.0"`, and
  `rstest-bdd-macros = "0.5.0"`.
- Use workspace-pinned `serde = "1.0.228"`, `serde_json = "1.0.149"`,
  `thiserror = "2"`, `camino = "1.2.1"`.
- Preserve deterministic behaviour (stable ordering for serialized output).
- Use en-GB-oxendict spelling in all comments and documentation.
- All public APIs documented with `///` Rustdoc including examples.
- Every module begins with a `//!` module-level doc comment.
- Completion must include roadmap checkbox update for 7.1.1.
- Update `docs/whitaker-clone-detector-design.md` with implementation
  decisions.

## Tolerances (exception triggers)

- Scope tolerance: if implementation exceeds 20 touched files or 2000 net
  lines of code, stop and escalate.
- API tolerance: if implementing 7.1.1 requires changing any existing public
  APIs in `common` or other crates, stop and escalate.
- Dependency tolerance: if any new dependency beyond those already in
  `[workspace.dependencies]` is required, stop and escalate.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after 3 targeted fix iterations, stop and escalate with captured logs.

## Risks

- Clippy strictness risk: the workspace denies `float_arithmetic`,
  `unwrap_used`, `expect_used`, `indexing_slicing`, `string_slice`,
  `cognitive_complexity`, and enforces `missing_docs`. All code must comply.
  Severity: medium. Likelihood: high. Mitigation: use `get()` instead of
  indexing, `?` operator for fallible operations, and builders to avoid complex
  constructors. Store similarity scores as `f64` fields without performing
  arithmetic in this crate.

- File size risk: SARIF has many types. A single `model.rs` could exceed 400
  lines. Severity: medium. Likelihood: high. Mitigation: split model into a
  `model/` directory module with sibling files grouped by SARIF concept.

- BDD step argument limit risk: Clippy counts BDD step parameters toward
  `too_many_arguments`. Severity: low. Likelihood: medium. Mitigation: keep
  step functions to at most `world` + 3 parsed values. Split complex setup into
  multiple `And` steps.

- Serde attribute complexity risk: SARIF uses `camelCase` field naming plus a
  `$schema` field requiring `#[serde(rename = "$schema")]`. Severity: low.
  Likelihood: low. Mitigation: test serialization format against known SARIF
  examples.

- `Result` name collision risk: `SarifResult` must be used to avoid collision
  with `std::result::Result`. Severity: low. Likelihood: certain. Mitigation:
  use `SarifResult` consistently and document the choice.

## Progress

- [x] Stage A: Write this ExecPlan and gain approval.
- [x] Stage B: Create crate scaffolding (`Cargo.toml`, `src/lib.rs`, workspace
  wiring).
- [x] Stage C: Implement error types (`error.rs`).
- [x] Stage D: Implement SARIF model types (`model/` directory).
- [x] Stage E: Implement rule definitions (`rules.rs`).
- [x] Stage F: Implement Whitaker properties extension
  (`whitaker_properties.rs`).
- [x] Stage G: Implement builders (`builders/` directory).
- [x] Stage H: Implement path helpers (`paths.rs`).
- [x] Stage I: Implement merge logic (`merge.rs`).
- [x] Stage J: Add unit tests across all modules.
- [x] Stage K: Add BDD feature file and step definitions.
- [x] Stage L: Run quality gates (`make check-fmt`, `make lint`, `make test`).
- [x] Stage M: Record implementation decisions in design doc.
- [x] Stage N: Mark roadmap item 7.1.1 done.
- [x] Stage O: Finalize living sections of this ExecPlan.

## Surprises & Discoveries

- Clippy's `expect_used` and `unwrap_used` denials in `[lints.clippy]` apply
  to integration test files as well as library code. The canonical BDD test
  pattern in `brain_trait_metrics_behaviour.rs` avoids this by using
  `match`/`panic!` arms and `with_*()` helper functions instead of `.expect()`.
  The initial BDD test file had to be rewritten to follow this pattern.

- Rustdoc's `redundant_explicit_links` warning fires when module-level doc
  comments use `[`Type`](super::Type)` syntax and the type is already
  re-exported in the parent module. The fix was to use bare `[`Type`]` links
  instead, relying on intra-doc link resolution.

- The `unwrap_or_else(|_| Value::Null)` pattern triggers Clippy's
  `unnecessary_lazy_evaluations` lint because `Value::Null` is a simple
  constant. Changed to `unwrap_or(Value::Null)`.

- `Level` enum's manual `Default` impl triggers Clippy's `derivable_impls`
  lint. Using `#[derive(Default)]` with `#[default]` on the `Warning` variant
  is the idiomatic approach in Rust edition 2024.

## Decision Log

- Decision: use `SarifResult` rather than `Result` for the SARIF result type
  to avoid collision with `std::result::Result`. Rationale: idiomatic Rust
  re-exports `Result` from the error module; naming the SARIF type
  `SarifResult` keeps both usable without qualification. Date/Author:
  2026-03-04 / DevBoxer.

- Decision: split model types across a `model/` directory module rather than a
  single file. Rationale: the 400-line-per-file constraint would be violated by
  a monolithic `model.rs` containing all SARIF types. Splitting by concept
  (log, run, result, location, descriptor) keeps each file focused.
  Date/Author: 2026-03-04 / DevBoxer.

- Decision: split builders across a `builders/` directory module. Rationale:
  same file-size constraint; each builder is self-contained. Date/Author:
  2026-03-04 / DevBoxer.

- Decision: deduplication key uses `(whitakerFragment fingerprint, file URI,
  region)
  ` tuple. Results lacking any component are preserved unconditionally. Rationale: safe default that avoids discarding unkeyed results while efficiently deduplicating keyed ones via `
  HashSet<ResultKey>`. Date/Author: 2026-03-04 / DevBoxer.

- Decision: `ResultBuilder::build()` returns `Result<SarifResult>` validating
  required fields (`rule_id`, `message`). Other builders return values
  directly. Rationale: only `SarifResult` has fields that are truly required by
  the SARIF spec and could reasonably be missing at build time. Date/Author:
  2026-03-04 / DevBoxer.

- Decision: BDD step definitions use `match`/`panic!` and `with_*()` helpers
  instead of `.expect()` to satisfy the workspace-wide `expect_used = "deny"`
  lint. Follows the canonical `brain_trait_metrics_behaviour.rs` pattern.
  Date/Author: 2026-03-04 / DevBoxer.

- Decision: `WhitakerProperties` wraps under a `{"whitaker": {...}}` JSON
  envelope to namespace Whitaker-specific metadata within the SARIF property
  bag, avoiding conflicts with other tool-specific properties. Date/Author:
  2026-03-04 / DevBoxer.

## Outcomes & Retrospective

Implemented roadmap 7.1.1 end-to-end.

Delivered:

- Created `crates/whitaker_sarif/` with full SARIF 2.1.0 model, builder,
  merge, rule, properties, and path helper modules.
- 62 unit tests across all source modules covering serialization round-trips,
  builder correctness, merge deduplication, rule definitions, property
  conversion, and path construction.
- 8 BDD scenarios in `tests/features/sarif.feature` with step definitions in
  `tests/sarif_behaviour.rs` following the canonical rstest-bdd v0.5.0 pattern.
- Wired public exports through `src/lib.rs`.
- Recorded 8 implementation decisions in
  `docs/whitaker-clone-detector-design.md`
  (`## Implementation decisions (7.1.1)`).
- Marked roadmap item 7.1.1 done in `docs/roadmap.md`.

Validation:

- `make check-fmt` passed (`/tmp/7-1-1-check-fmt.log`).
- `make lint` passed (`/tmp/7-1-1-lint.log`).
- `make test` passed (`/tmp/7-1-1-test.log`) with summary:
  `972 tests run: 972 passed (2 slow), 2 skipped`.

Scope check:

- Touched ~20 files total (within tolerance).
- All new code files remain under 400 lines.
- No new dependencies added beyond those already in
  `[workspace.dependencies]`.
- No existing public APIs were changed.

## Context and orientation

The Whitaker project is a Cargo workspace providing Dylint lints for Rust code
quality. The workspace root is at `/home/user/project` and its members are
declared in `/home/user/project/Cargo.toml` as
`["common", "crates/*", "installer", "suite"]`. New library crates live under
`crates/`.

The clone detector pipeline (roadmap §7) introduces several new crates.
`whitaker_sarif` is the first; it provides SARIF 2.1.0 data models used by all
subsequent crates. The full design lives in
`docs/whitaker-clone-detector-design.md`. The SARIF crate responsibilities are
defined in §Crate responsibilities and §SARIF schema and mapping of that
document.

Key reference files:

- `Cargo.toml` — workspace root with dependency pins.
- `AGENTS.md` — commit gating and code style rules.
- `Makefile` — `check-fmt`, `lint`, `test` targets.
- `common/Cargo.toml` — example library crate Cargo.toml.
- `common/src/lib.rs` — example library module layout.
- `common/tests/brain_trait_metrics_behaviour.rs` — canonical BDD pattern.
- `common/tests/features/brain_trait_metrics.feature` — canonical feature file.
- `docs/whitaker-clone-detector-design.md` — SARIF schema and mapping spec.
- `docs/rstest-bdd-users-guide.md` — BDD testing patterns for rstest-bdd
  v0.5.0.
- `docs/roadmap.md` — roadmap item 7.1.1.

The workspace uses Rust edition 2024, nightly toolchain `nightly-2025-09-18`,
and enforces very strict Clippy lints (see root `Cargo.toml` `[lints.clippy]`).
All code files must remain under 400 lines.

## Plan of work

### Stage B: Create crate scaffolding

Create the directory `crates/whitaker_sarif/` with `Cargo.toml` and
`src/lib.rs`. The `Cargo.toml` follows the `common` crate pattern. Add
`whitaker_sarif` to `[workspace.dependencies]` in the root `Cargo.toml`.

### Stage C: Implement error types

File: `crates/whitaker_sarif/src/error.rs` — `SarifError` enum with variants
for serialization, I/O, invalid level, merge conflict, and missing builder
field. Convenience type alias `Result<T>`.

### Stage D: Implement SARIF model types

Split into `model/` directory: `mod.rs`, `log.rs` (`SarifLog`), `run.rs`
(`Run`, `Tool`, `ToolComponent`, `Invocation`, `Artifact`), `result.rs`
(`SarifResult`, `Level`, `Message`), `location.rs` (`Location`,
`PhysicalLocation`, `ArtifactLocation`, `Region`, `RelatedLocation`),
`descriptor.rs` (`ReportingDescriptor`, `MultiformatMessageString`). All types
derive `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` with
`#[serde(rename_all = "camelCase")]`.

### Stage E: Implement rule definitions

File: `crates/whitaker_sarif/src/rules.rs` — constants and constructor
functions for WHK001, WHK002, WHK003. Provides `all_rules()`.

### Stage F: Implement Whitaker properties extension

File: `crates/whitaker_sarif/src/whitaker_properties.rs` — `WhitakerProperties`
struct with `From<WhitakerProperties>` for `serde_json::Value` and
`TryFrom<&Value>`. Builder for fluent construction.

### Stage G: Implement builders

Split into `builders/` directory: `mod.rs`, `log_builder.rs`
(`SarifLogBuilder`), `run_builder.rs` (`RunBuilder`), `result_builder.rs`
(`ResultBuilder`), `location_builder.rs` (`LocationBuilder`, `RegionBuilder`).

### Stage H: Implement path helpers

File: `crates/whitaker_sarif/src/paths.rs` — constants and functions for
`target/whitaker/` file layout.

### Stage I: Implement merge logic

File: `crates/whitaker_sarif/src/merge.rs` — `deduplicate_results` and
`merge_runs` functions with `ResultKey`-based deduplication.

### Stage J–K: Add unit and BDD tests

Unit tests in `#[cfg(test)]` modules within each source file. BDD feature file
at `tests/features/sarif.feature` with 8 scenarios. Step definitions in
`tests/sarif_behaviour.rs` following the `brain_trait_metrics_behaviour.rs`
pattern.

### Stage L: Run quality gates

Run `make check-fmt`, `make lint`, `make test` with log capture.

### Stage M: Record implementation decisions

Update `docs/whitaker-clone-detector-design.md` with implementation decisions.

### Stage N: Mark roadmap complete

Update `docs/roadmap.md` to mark 7.1.1 done.

### Stage O: Finalize living sections

Set status to COMPLETE and populate remaining sections.

## Validation and acceptance

Quality criteria:

- Tests: `make test` passes with all new unit and BDD tests green.
- Lint: `make lint` passes with zero warnings.
- Format: `make check-fmt` passes.
- File size: every source file is under 400 lines.
- Roadmap: 7.1.1 is marked `[x]` in `docs/roadmap.md`.
- Design doc: implementation decisions recorded.

Quality method:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/7-1-1-check-fmt.log
make lint 2>&1 | tee /tmp/7-1-1-lint.log
make test 2>&1 | tee /tmp/7-1-1-test.log
```
