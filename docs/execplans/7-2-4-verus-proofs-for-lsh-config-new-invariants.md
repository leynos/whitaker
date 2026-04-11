# Add clone-detector proof sidecars and prove `LshConfig::new` invariants (roadmap 7.2.4 and 7.2.5)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.2.4 adds the reusable proof plumbing for the clone-detector
token pipeline. Roadmap item 7.2.5 then uses that plumbing to prove the first
clone-detector constructor invariant: `LshConfig::new` must reject zero
`bands`, reject zero `rows`, and accept only configurations whose
`bands * rows` product equals the fixed `MINHASH_SIZE` of 128.

This work matters because roadmap item 7.2.2 already made `LshConfig` a hard
runtime boundary for MinHash and locality-sensitive hashing (LSH). If that
boundary drifts, every later proof and every later token-pass candidate check
inherits the bug. Architecture Decision Record (ADR) 002 explicitly assigns
this constructor to Verus and reserves Kani for bounded behavioural checks over
the same clone-detector crate, so 7.2.4 and 7.2.5 establish the proof split
that later items 7.2.6, 7.2.7, and 7.2.8 build on.

Observable outcome:

1. The repository gains reproducible, sidecar proof entry points for the
   clone detector: a Verus path and a Kani path, both driven by scripts and
   `Makefile` targets rather than normal Cargo builds.
2. `make verus-clone-detector` succeeds and runs a clone-detector Verus proof
   file dedicated to `LshConfig::new`.
3. `make kani-clone-detector` succeeds and runs a bounded clone-detector Kani
   smoke harness so the Kani workflow is real and observable before 7.2.7 and
   7.2.8 land.
4. Unit tests in `crates/whitaker_clones_core/src/index/tests.rs` cover happy
   paths, unhappy paths, and edge cases for `LshConfig::new`, including zero
   `bands`, zero `rows`, invalid products, and a large-value overflow edge.
5. Behaviour-driven development (BDD) coverage using workspace-pinned
   `rstest-bdd` v0.5.0 extends
   `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs` and
   `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature` with
   scenario coverage for valid and invalid LSH configuration inputs.
6. `docs/whitaker-clone-detector-design.md` records the new proof-workflow and
   `LshConfig` decisions, and `docs/roadmap.md` marks 7.2.4 and 7.2.5 done only
   after the implementation turn completes and all gates succeed.
7. The implementation turn finishes with `make fmt`, `make markdownlint`,
   `make nixie`, `make check-fmt`, `make lint`, and `make test` all passing,
   plus the new proof targets passing.

## Constraints

- Scope only roadmap items 7.2.4 and 7.2.5. Do not mark 7.2.6, 7.2.7, or
  7.2.8 done in this change, even if some scaffolding is intentionally shared.
- Keep proof tooling in sidecar scripts and proof files, consistent with ADR
  002. Normal Cargo builds and the default `make test` flow must remain
  independent of proof execution.
- Preserve the existing runtime API shape unless a small, crate-private proof
  seam is genuinely needed to reduce drift or remove duplication. Do not widen
  the public API solely for proof convenience.
- Build on the existing clone-detector crate
  `crates/whitaker_clones_core/`. Do not move `LshConfig` into a separate crate
  or introduce CLI, filesystem, or `rustc_private` concerns here.
- If Kani harnesses use `#[cfg(kani)]`, register that cfg in the relevant
  crate's `unexpected_cfgs` allowlist instead of accepting noisy warnings or
  adding broad lint suppressions.
- Keep each Rust source file below 400 lines. Add focused sibling modules if
  proof harness code or test helpers would otherwise bloat one file.
- Every new Rust module must begin with a `//!` module-level comment.
- Every new public API or newly public helper must carry Rustdoc with examples
  that compile under the normal doc-test flow.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for all unit and behavioural coverage.
- Integration tests under `tests/` must avoid `unwrap()` and `expect()`
  because the workspace denies them there.
- Behaviour tests must stay within the workspace Clippy argument-count limit:
  `world` plus at most three parsed values per step.
- Preserve the runtime contract already documented in
  `docs/whitaker-clone-detector-design.md`: `bands > 0`, `rows > 0`, and
  `bands * rows == MINHASH_SIZE`.
- Record all final design decisions in
  `docs/whitaker-clone-detector-design.md`.
- Do not mark roadmap items 7.2.4 and 7.2.5 done until the implementation,
  proof targets, tests, and full quality gates succeed.

## Tolerances (exception triggers)

- Toolchain tolerance: if no Kani release can be pinned reproducibly against
  the repository's current `nightly-2025-09-18` toolchain without changing
  `rust-toolchain.toml`, stop and escalate before changing the pinned Rust
  toolchain.
- Scope tolerance: if the Kani workflow cannot be made observable without also
  implementing substantive `MinHasher` or `LshIndex` proofs, stop and ask
  whether 7.2.7 or 7.2.8 should be pulled forward deliberately.
- API tolerance: if Verus proof drift cannot be managed without exporting a
  new public helper, stop and ask before widening the clone-detector public
  surface.
- File-size tolerance: if the implementation would push a Rust source file
  over 400 lines or touch more than 14 files, stop and refactor the plan into
  smaller modules or ask whether the scope should be split.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` still
  fail after three targeted fix iterations, stop and escalate with saved log
  paths.
- Proof-model tolerance: if the Verus model needs to reason about the internals
  of `BTreeSet`, `BTreeMap`, or MinHash mixing logic to prove 7.2.5, stop and
  narrow the proof back to constructor semantics only.
- Harness tolerance: if `cfg(kani)` introduces unexpected lint or build noise
  under normal Cargo workflows, move the Kani harness into a more isolated
  module arrangement before proceeding, but do not disable lints broadly.

## Risks

- Kani compatibility risk: Kani versioning may lag the repository's pinned
  nightly. Mitigation: treat compatible Kani pin selection as the first risky
  milestone and escalate if it requires a toolchain bump.
- Proof drift risk: a standalone Verus model can drift from the runtime
  constructor semantics over time. Mitigation: keep the proof focused on the
  exact constructor contract, mirror the same constant names, and add ordinary
  unit and BDD regression tests around the runtime constructor.
- Overscoping risk: sidecar plumbing can easily absorb later proof work.
  Mitigation: add only the minimal Kani smoke harness needed to make the Kani
  target real, and reserve `MinHasher`/`LshIndex` proof obligations for their
  own roadmap items.
- Regression-noise risk: proof scripts that modify global toolchains or caches
  can make local development brittle. Mitigation: install proof tools into
  repository-owned or XDG cache locations and keep sidecar scripts idempotent.
- Test-coverage risk: the current BDD suite already covers zero bands but not
  zero rows or a non-zero invalid product. Mitigation: extend the existing
  `min_hash_lsh` behaviour file instead of creating a disconnected second
  harness.
- Overflow edge risk: `checked_mul` protects the runtime from large inputs, but
  the proof must still preserve the semantic claim that only exact products of
  128 are accepted. Mitigation: add a targeted large-value unit test and keep
  the proof claim phrased in terms of acceptance semantics, not machine-width
  overflow internals.
- State-explosion risk: even tiny Kani harnesses can become impractical if they
  explore unconstrained symbolic inputs or use overly generous unwind bounds.
  Mitigation: follow Chutoro's pattern of a deterministic smoke harness, small
  explicit `#[kani::unwind(N)]` bounds, and `kani::assume` guards that mirror
  production preconditions.

## Progress

- [x] Stage A: Gather repository context and draft this ExecPlan
  (2026-04-08).
- [x] Stage B: Add clone-detector proof sidecar tooling and `Makefile`
  targets (2026-04-09).
- [x] Stage C: Add or extend failing unit tests for `LshConfig::new` happy,
  unhappy, and edge cases (2026-04-09).
- [x] Stage D: Extend `rstest-bdd` coverage for `LshConfig` configuration
  outcomes (2026-04-09).
- [x] Stage E: Add the Verus proof for `LshConfig::new` invariants
  (2026-04-09).
- [x] Stage F: Add the minimal Kani smoke harness needed to make the
  clone-detector Kani target observable (2026-04-09).
- [x] Stage G: Update the clone-detector design document and mark roadmap
  items 7.2.4 and 7.2.5 done (2026-04-09).
- [x] Stage H: Run documentation, proof, lint, and test gates successfully
  (2026-04-09).
- [x] Stage I: Finalize the living sections in this ExecPlan after the
  implementation turn (2026-04-09).

## Surprises & Discoveries

- The repository already has a pinned Verus sidecar workflow:
  `scripts/install-verus.sh`, `scripts/run-verus.sh`, the top-level `verus/`
  directory, and `make verus`. Clone-detector proof tooling should extend that
  pattern rather than inventing a second Verus installation model.
- The repository has no Kani tooling yet. There is no `make kani`, no Kani
  install wrapper, and no existing `cfg(kani)` or Kani harness module.
- `LshConfig::new` already enforces the exact 7.2.5 runtime contract in
  `crates/whitaker_clones_core/src/index/types.rs`, so the first proof can be
  narrowly scoped to constructor semantics instead of reshaping the runtime
  code.
- Existing unit coverage in
  `crates/whitaker_clones_core/src/index/tests.rs` already exercises zero bands
  and one invalid product case. Existing BDD coverage in
  `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature` already
  exercises zero bands. The implementation should extend these tests rather
  than replace them.
- `Makefile` currently exposes only a global `verus` target. If fast
  clone-detector-only iteration is desired without regressing the existing
  proof workflow, the cleanest approach is to add clone-detector-specific
  targets alongside the current umbrella target.
- The workspace root pins `rstest-bdd = "0.5.0"`, matching the user's
  requirement, so the plan must keep that exact version in view even though
  some repository comments elsewhere still mention older text.
- Chutoro's Kani setup is directly relevant here. The useful patterns are:
  keep harnesses adjacent to the internal modules they inspect, allow
  `cfg(kani)` explicitly in crate lint configuration, separate a practical
  local Kani target from heavier exhaustive runs, and use documented
  `kani::assume` guards plus explicit unwind limits to keep solver work
  tractable.
- `leta` was not stable in this workspace during this turn.
  `Connection reset by peer` / `EOF while parsing a value` appeared, so the
  implementation used direct file inspection instead of semantic navigation.
- `cargo search kani-verifier` resolved `kani-verifier = "0.67.0"` during the
  implementation turn, and the sidecar installer now pins that release unless
  the environment overrides it explicitly.
- The shared Verus installer needed one extra hardening step during validation:
  Verus printed an ANSI-coloured
  `rustup install 1.94.0-x86_64-unknown-linux-gnu` hint, so
  `scripts/install-verus.sh` had to strip ANSI escape codes before extracting
  the fallback toolchain suggestion.
- A later proof review found that the first Verus file was too tautological to
  count as implementation-shaped reasoning, and that the first Kani symbolic
  harness did not cover the `checked_mul(None)` overflow path. The follow-up
  change tightened the Verus model to mirror the constructor's real branch
  order and added a dedicated overflow Kani harness.
- In this repository shape, Verus cannot directly prove the compiled
  `crates/whitaker_clones_core` implementation without trusted assumptions for
  external code. That limitation needs to stay documented in the developer
  guide rather than implied.

## Decision Log

- Decision: plan 7.2.4 and 7.2.5 as one implementation turn. Rationale:
  7.2.5 directly depends on 7.2.4, and splitting the sidecar plumbing from the
  first proof would create extra doc churn without reducing risk. Date/Author:
  2026-04-08 / Codex.
- Decision: keep proof workflows opt-in through `Makefile` targets and wrapper
  scripts, not through normal `cargo test` or build hooks. Rationale: ADR 002
  explicitly requires sidecar proof tooling independent of ordinary Cargo
  development. Date/Author: 2026-04-08 / Codex.
- Decision: extend the existing `min_hash_lsh` unit and BDD coverage instead
  of creating a second test harness just for `LshConfig`. Rationale: the
  runtime contract already lives inside that feature area, and this keeps the
  behavioural story coherent for a novice reader. Date/Author: 2026-04-08 /
  Codex.
- Decision: add a minimal Kani smoke harness around `LshConfig::new` so the
  new Kani sidecar is demonstrably real, while still reserving substantive
  `MinHasher` and `LshIndex` bounded proofs for roadmap items 7.2.7 and 7.2.8.
  Rationale: 7.2.4 asks for clone-detector Kani checks, and a zero-harness
  workflow would be difficult to validate. Date/Author: 2026-04-08 / Codex.
- Decision: borrow Chutoro's two-tier Kani shape conceptually, but keep the
  Whitaker scope smaller for this roadmap slice. Rationale: the first Whitaker
  Kani target should be a practical local smoke gate with small bounds; any
  heavier `kani-full` style target should remain a future extension once 7.2.7
  and 7.2.8 add broader bounded checks. Date/Author: 2026-04-08 / Codex.
- Decision: fix the shared Verus installer while implementing the clone-detector
  proof workflow. Rationale: `make verus-clone-detector` and `make verus` both
  depend on the same parser, so the ANSI-coloured toolchain hint had to be
  normalized centrally rather than papered over in the new target. Date/Author:
  2026-04-09 / Codex.
- Decision: keep the Verus artefact as a faithful branch-by-branch model of
  `LshConfig::new` rather than adding trusted external-code assumptions.
  Rationale: `assume_specification`, `external_fn_specification`, or
  `external_body` would only document or trust the production function, not
  prove it. The follow-up proof therefore mirrors the real constructor logic,
  while Kani remains the implementation-executing proof. Date/Author:
  2026-04-11 / Codex.
- Decision: extend the Kani harness set with an overflow-specific proof for the
  `checked_mul(None)` path. Rationale: the bounded `[0, 128]²` harness is still
  useful for exhaustive local reasoning, but it cannot witness arithmetic
  overflow by construction. A separate symbolic harness closes that coverage
  gap without blowing up the state space of the bounded harness. Date/Author:
  2026-04-11 / Codex.

## Context and orientation

### Repository state

The repository root is `/home/user/project`.

Clone-detector runtime code already lives in
`crates/whitaker_clones_core/src/index/`:

- `mod.rs` wires the public index API.
- `types.rs` defines `MINHASH_SIZE`, `FragmentId`, `CandidatePair`,
  `MinHashSignature`, and `LshConfig`.
- `error.rs` defines the typed `IndexError` values `ZeroBands`, `ZeroRows`,
  `InvalidBandRowProduct`, and `EmptyFingerprintSet`.
- `minhash.rs` and `lsh.rs` implement candidate-generation behaviour from
  roadmap item 7.2.2.
- `tests.rs` contains unit coverage for this area.

Current behaviour coverage for this area already exists in:

- `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`
- `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature`

Current proof infrastructure already exists only for the decomposition-advice
area:

- `scripts/install-verus.sh`
- `scripts/run-verus.sh`
- `verus/decomposition_cosine_threshold.rs`
- `verus/decomposition_vector_algebra.rs`
- `Makefile` target `verus`

There is no existing Kani sidecar or Kani proof directory in the repository.

### Runtime contract to preserve

`LshConfig::new` currently does three things in order:

1. Converts `bands` into `NonZeroUsize`, returning `IndexError::ZeroBands`
   when the conversion fails.
2. Converts `rows` into `NonZeroUsize`, returning `IndexError::ZeroRows` when
   that conversion fails.
3. Calls `validate_product` and accepts only when
   `checked_mul(bands, rows) == Some(MINHASH_SIZE)`.

That contract is already documented in `docs/whitaker-clone-detector-design.md`
under `## Implementation decisions (7.2.2)`, which states that `bands` and
`rows` must both be greater than zero and must multiply to the fixed
`MINHASH_SIZE` of 128.

### Relationship to ADR 002

ADR 002 makes the proof split explicit:

1. Verus owns local semantic invariants such as `LshConfig::new`.
2. Kani owns bounded behavioural checks for real MinHash and LSH code.
3. Ordinary tests remain the first regression net and are not replaced by
   proofs.

This plan follows that split literally. Verus proves the constructor contract.
Kani gets only enough workflow and smoke coverage to make the sidecar real.
Ordinary unit and BDD tests continue to guard the runtime API.

### Local documentation and testing guides to follow

Use these repository guides while implementing, but keep the implementation
turn self-contained:

- `docs/rstest-bdd-users-guide.md` for fixture-backed BDD scenarios.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` fixture patterns.
- `docs/rust-doctest-dry-guide.md` for Rustdoc example style.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping
  helpers small and cohesive.
- `docs/whitaker-dylint-suite-design.md` for repository-wide lint and
  `unexpected_cfgs` considerations when introducing specialized build modes.

### Relevant learnings from `leynos/chutoro`

Chutoro uses Kani in a way that is directly applicable to this roadmap item.
The most relevant implementation lessons are:

1. Put Kani harnesses next to the internal modules they exercise so they can
   use `pub(crate)` seams instead of widening the public API.
2. Keep harnesses behind `#[cfg(kani)]`, and explicitly allow that cfg in the
   crate's `unexpected_cfgs` checklist.
3. Keep the everyday Kani target small and practical. Chutoro's `make kani`
   runs only smoke-sized harnesses, while heavier exhaustive runs are reserved
   for `make kani-full`.
4. Add module-level harness documentation with direct `cargo kani` and `make`
   commands so the proof entry points remain discoverable.
5. Use explicit `#[kani::unwind(N)]` bounds and `kani::assume` guards that
   mirror production preconditions. This is how Chutoro keeps symbolic state
   bounded without silently changing the runtime contract under test.

Whitaker should adopt lessons 1, 2, 4, and 5 directly in 7.2.4. Lesson 3 should
shape the interface design so the first target is practical locally, without
forcing 7.2.4 to absorb future heavyweight harnesses now.

## Proposed implementation shape

The change should stay narrowly focused and add exactly four kinds of artefact.

First, extend proof tooling at the repository root:

- Add `scripts/install-kani.sh` to install a pinned `kani-verifier` into a
  cache directory, run the one-time Kani setup step idempotently, and print the
  resolved tool path for callers.
- Add `scripts/run-kani.sh` to run explicit clone-detector harness names in
  `crates/whitaker_clones_core`, mirroring the existing `scripts/run-verus.sh`
  wrapper style.
- Extend `scripts/run-verus.sh` so it can run either the current decomposition
  proofs, the new clone-detector proofs, or a caller-specified file path, while
  preserving existing behaviour for `make verus`.
- Update `Makefile` to add clone-detector-specific proof targets and an
  umbrella Kani target. The intended shape is:

  1. `make verus` runs all Verus proof files, including the new clone-detector
     proof.
  2. `make verus-clone-detector` runs only the clone-detector Verus proof set.
  3. `make kani-clone-detector` runs only the practical clone-detector Kani
     smoke harness set with intentionally small bounds.
  4. `make kani` runs all practical Kani sidecar harness sets currently
     registered. In this roadmap slice, that will effectively mean the
     clone-detector set.
  5. The script and target layout must leave room for a future heavier
     `kani-full` style target once 7.2.7 and 7.2.8 introduce broader bounded
     checks.

Second, add the first clone-detector Verus proof file:

- `verus/clone_detector_lsh_config.rs`

That file should model the `LshConfig::new` contract directly in terms of
`bands`, `rows`, and `MINHASH_SIZE`. The proof goal is semantic, not
implementation-detail-heavy: acceptance if and only if `bands > 0`, `rows > 0`,
and `bands * rows == MINHASH_SIZE`, with explicit lemmas for zero bands, zero
rows, a valid exact product, and an invalid non-zero product.

Third, add the minimal Kani harness needed to make the new workflow concrete:

- Prefer `crates/whitaker_clones_core/src/index/kani.rs`, compiled only under
  Kani, with one proof harness that symbolically chooses bounded `bands` and
  `rows`, calls the real `LshConfig::new`, and asserts the returned
  `IndexError` or accepted config matches the documented runtime contract.
- Mirror Chutoro's local style by giving this module a `//!` comment that
  documents the direct `cargo kani` invocation and the `make` target that wraps
  it.

This harness is deliberately narrow. It is a tooling smoke check and a future
anchor point, not a substitute for roadmap items 7.2.7 and 7.2.8.

Fourth, extend the existing test and documentation surfaces:

- `crates/whitaker_clones_core/src/index/tests.rs`
- `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`
- `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature`
- `docs/whitaker-clone-detector-design.md`
- `docs/roadmap.md`

## Implementation details

### Stage B: add clone-detector proof sidecar tooling and `Makefile` targets

Start with the proof workflow plumbing because 7.2.5 depends on it.

1. Extend `scripts/run-verus.sh` so proof files are grouped by domain rather
   than hard-coded as one flat list. Preserve the current default behaviour for
   `make verus`, but make it possible to select a `clone-detector` group for
   fast local iteration.
2. Add `scripts/install-kani.sh` following the same operational style as the
   Verus installer: strict shell flags, explicit cache directory, idempotent
   installation, and a stable printed path. Use Kani's official installation
   flow as the basis for the script. If pinning a compatible Kani release is
   not possible against the current Rust toolchain, stop here and escalate.
3. Add `scripts/run-kani.sh` to call the installed Kani tool with an explicit
   manifest path, explicit harness list, and explicit unwind defaults for the
   practical clone-detector harnesses.
4. If the harness module uses `#[cfg(kani)]`, update
   `crates/whitaker_clones_core/Cargo.toml` with an `unexpected_cfgs` allowlist
   entry for `cfg(kani)`, following Chutoro's pattern.
5. Update `Makefile` with the new proof targets. Keep the target names obvious
   and parallel so a novice can discover them from `make help`.

### Stage C: add failing unit tests first

Extend `crates/whitaker_clones_core/src/index/tests.rs` before touching proof
files.

Keep the existing valid and invalid tests, then add focused cases for:

1. `rows == 0` as a dedicated runtime rejection case.
2. A non-zero invalid product such as `(16, 16)` or `(3, 42)`.
3. A large-value overflow edge, such as `(usize::MAX, 2)`, proving that the
   constructor rejects overflow rather than panicking.
4. Additional valid products such as `(2, 64)` and `(32, 4)` if not already
   covered by the final shape.

If a helper would exceed the Clippy argument-count threshold, replace the
helper signature with a small parameter object rather than silencing the lint.

### Stage D: extend the `rstest-bdd` coverage

Add or extend scenarios in the existing `min_hash_lsh` behaviour harness so a
reader can observe the `LshConfig` contract without reading the unit tests.

The behavioural suite should include at least:

1. A happy-path scenario using a valid `(bands, rows)` pair that still yields a
   candidate pair.
2. An unhappy-path scenario for zero bands. This already exists and should be
   kept.
3. A new unhappy-path scenario for zero rows.
4. A new unhappy-path scenario for a non-zero invalid product.

Do not create a second feature file unless the existing one becomes hard to
read. This change belongs in the current MinHash/LSH behaviour story.

### Stage E: add the Verus proof for `LshConfig::new`

Add `verus/clone_detector_lsh_config.rs`.

Keep the proof small and explicit. A good starting shape is:

```rust
spec fn lsh_config_accepts(bands: int, rows: int) -> bool { ... }

proof fn lemma_zero_bands_rejected(rows: int) { ... }
proof fn lemma_zero_rows_rejected(bands: int) { ... }
proof fn lemma_exact_product_is_accepted() { ... }
proof fn lemma_invalid_product_is_rejected(bands: int, rows: int) { ... }
```

Use the same constant name `MINHASH_SIZE` inside the proof file so drift is
harder to introduce. Prefer a direct semantic model of the constructor over a
complex model of `NonZeroUsize` or machine-width overflow.

If the proof becomes awkward because the runtime constructor duplicates logic
in several branches, extract one crate-private runtime helper such as
`validate_lsh_dimensions` and prove the semantics that helper expresses. Do not
make the helper public unless the user explicitly approves widening the API.

### Stage F: add the minimal Kani smoke harness

Create the narrowest useful Kani harness that makes `make kani-clone-detector`
meaningful.

Recommended shape:

1. Add `crates/whitaker_clones_core/src/index/kani.rs`.
2. Compile it only when Kani runs.
3. Give it a module-level `//!` comment with the direct `cargo kani` command
   and the corresponding `make` target, mirroring Chutoro's discoverability
   pattern.
4. Add one deterministic smoke harness first, then one symbolic harness only
   if the deterministic harness alone is too weak to prove the workflow is
   wired correctly.
5. Use an explicit small unwind bound, such as `#[kani::unwind(4)]`, unless a
   tighter value is enough.
6. Symbolically choose small `bands` and `rows` values and constrain them with
   `kani::assume` only where those assumptions match real production
   preconditions or bound the search space transparently.
7. Call the real `LshConfig::new` runtime constructor.
8. Assert that:
   - `Ok(config)` implies both values are non-zero and their product equals
     `MINHASH_SIZE`.
   - `Err(IndexError::ZeroBands)` implies `bands == 0`.
   - `Err(IndexError::ZeroRows)` implies `rows == 0` and `bands != 0`.
   - `Err(IndexError::InvalidBandRowProduct { .. })` implies both are non-zero
     and the exact product is not `MINHASH_SIZE`.

This harness is not the formal proof for 7.2.5. It is the minimal bounded
runtime check that proves the Kani sidecar works and that future clone-detector
Kani items have a stable place to live.

### Stage G: update design and roadmap documents

Update `docs/whitaker-clone-detector-design.md` with the final decisions from
the implementation turn. Add explicit prose covering:

1. The proof-workflow split: Verus for local constructor semantics, Kani for
   bounded runtime checks.
2. The chosen `Makefile` target names and sidecar script locations.
3. Any proof seam introduced in runtime code, if one was required.
4. Any Kani compatibility constraint discovered during the implementation.

Only after all implementation work and quality gates succeed, mark roadmap
items 7.2.4 and 7.2.5 done in `docs/roadmap.md`.

## Validation and evidence

During the implementation turn, run every relevant gate through `tee` with
`set -o pipefail`, as required by `AGENTS.md`, and retain the logs for review.

Use this exact command pattern:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/7-2-4-fmt.log
make markdownlint 2>&1 | tee /tmp/7-2-4-markdownlint.log
make nixie 2>&1 | tee /tmp/7-2-4-nixie.log
make verus-clone-detector 2>&1 | tee /tmp/7-2-4-verus-clone-detector.log
make kani-clone-detector 2>&1 | tee /tmp/7-2-4-kani-clone-detector.log
make check-fmt 2>&1 | tee /tmp/7-2-4-check-fmt.log
make lint 2>&1 | tee /tmp/7-2-4-lint.log
make test 2>&1 | tee /tmp/7-2-4-test.log
```

If a broader proof smoke pass is useful after the clone-detector targets are
green, also run:

```sh
set -o pipefail
make verus 2>&1 | tee /tmp/7-2-4-verus.log
make kani 2>&1 | tee /tmp/7-2-4-kani.log
```

The implementation turn is complete only when all of the following are true:

1. Unit tests and BDD tests for `LshConfig::new` pass.
2. The clone-detector Verus proof target passes.
3. The clone-detector Kani target passes.
4. `make check-fmt`, `make lint`, and `make test` pass.
5. Documentation format and lint gates pass for the modified Markdown files.
6. `docs/whitaker-clone-detector-design.md` and `docs/roadmap.md` reflect the
   final state accurately.

## Acceptance checklist for the implementation turn

The implementation turn should be considered acceptable only if a novice can do
all of the following from a fresh checkout after reading this ExecPlan:

1. Discover the clone-detector proof entry points from `make help`.
2. Run `make verus-clone-detector` and observe a successful proof of
   `LshConfig::new` invariants.
3. Run `make kani-clone-detector` and observe a successful bounded runtime
   harness for the clone-detector crate.
4. Read `crates/whitaker_clones_core/src/index/tests.rs` and the
   `min_hash_lsh` feature file to understand the happy and unhappy runtime
   cases without reverse-engineering the proof files.
5. Read `docs/whitaker-clone-detector-design.md` and see the final 7.2.4 and
   7.2.5 design choices recorded clearly.
6. Open `docs/roadmap.md` and see both roadmap items marked done only after the
   work is truly complete.

## Outcomes & Retrospective

Implemented the 7.2.4 and 7.2.5 scope as planned: the repository now has
clone-detector-specific Verus and Kani sidecar targets, a new Verus proof for
`LshConfig::new`, a bounded Kani harness module adjacent to the index code, and
expanded unit plus `rstest-bdd` coverage for zero rows, invalid non-zero
products, and overflow rejection. The only material change from the draft was a
small shared-tooling fix in `scripts/install-verus.sh` after validation exposed
ANSI escape codes in Verus's toolchain suggestion output.

No tolerance gates were hit. Validation succeeded with: `make fmt`,
`make markdownlint`, `make nixie`, `make verus-clone-detector`,
`make kani-clone-detector`, `make check-fmt`, `make lint`, `make test`,
`make verus`, and `make kani`. Later roadmap items 7.2.6, 7.2.7, and 7.2.8
remain intentionally untouched; the new proof sidecars and harness location are
the intended base for that future work.
