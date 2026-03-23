# Implement MinHash + LSH candidate generation (roadmap 7.2.2)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 7.2.2 extends the completed token pipeline from roadmap item
7.2.1. After this change, `crates/whitaker_clones_core/` will be able to take
the retained token fingerprints produced by `normalize`, `hash_shingles`, and
`winnow`, compute deterministic 128-value MinHash sketches, bucket those
sketches with locality-sensitive hashing (LSH), and return stable candidate
fragment pairs for later token-level Jaccard scoring in roadmap item 7.2.3.

This stage must stop at candidate generation. It must not accept or reject
clone pairs by Jaccard threshold, emit Static Analysis Results Interchange
Format (SARIF), group clone classes, traverse the filesystem, or touch the
abstract syntax tree (AST) refinement pipeline.

Observable outcome:

1. `whitaker_clones_core` exports a documented `index` module for MinHash and
   LSH candidate generation.
2. Unit tests cover happy paths, unhappy paths, and edge cases for sketch
   generation, LSH configuration validation, bucket collisions, pair
   deduplication, and deterministic ordering.
3. Behaviour-driven development (BDD) tests using workspace-pinned
   `rstest-bdd` v0.5.0 exercise end-to-end candidate generation from retained
   fingerprints to candidate pairs.
4. `docs/whitaker-clone-detector-design.md` records the final 7.2.2
   implementation decisions in a new `## Implementation decisions (7.2.2)`
   section.
5. `docs/roadmap.md` marks 7.2.2 done only after implementation and all
   quality gates succeed.
6. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass at the end of the implementation turn.

## Constraints

- Scope only roadmap item 7.2.2. Do not implement Jaccard acceptance,
  clone-class grouping, SARIF run generation, CLI wiring, filesystem discovery,
  or AST refinement in this change.
- Build on the existing pure-library crate `crates/whitaker_clones_core/`.
  Do not introduce `rustc_private` or turn the library into a CLI- or
  filesystem-aware component.
- Keep the MinHash sketch length fixed at 128 for this roadmap item, matching
  `docs/whitaker-clone-detector-design.md`. Bands and rows are configurable,
  but their validated product must equal 128.
- Keep every Rust source file below 400 lines. Add sibling modules under
  `crates/whitaker_clones_core/src/index/` rather than growing one large file.
- Every new module must begin with a `//!` module-level comment.
- Every public API added for 7.2.2 must carry Rustdoc comments with examples
  that compile under `cargo test --doc` through `make test`.
- Use only existing workspace dependencies unless explicit approval is given
  for more. The plan assumes no new third-party crates are needed.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for unit and behavioural coverage.
- Behaviour tests must respect the workspace Clippy threshold of 4 arguments.
  A BDD step can parse at most 3 values in addition to the world fixture.
- Integration tests under `tests/` must avoid `unwrap()` and `expect()`
  because the workspace denies them there.
- Preserve deterministic behaviour. The same fingerprints, fragment IDs, and
  LSH settings must always yield the same sketches, buckets, and candidate
  pairs.
- Record all final 7.2.2 design choices in
  `docs/whitaker-clone-detector-design.md`.
- Do not mark roadmap item 7.2.2 done until the implementation, tests, and
  full quality gates succeed.

## Tolerances

- Scope tolerance: if implementation starts needing Jaccard scoring,
  clone-class aggregation, or SARIF result modelling to make the code useful,
  stop and escalate because that crosses into roadmap item 7.2.3.
- API tolerance: if candidate generation cannot stay a pure library and starts
  needing file paths, workspace traversal, or CLI parsing, stop and escalate.
- Dependency tolerance: if implementation appears to require a new hashing,
  graph, or LSH crate, stop and escalate with the concrete reason before adding
  it.
- Size tolerance: if the change exceeds 16 touched files or 1400 net new lines
  of code, stop and escalate before proceeding further.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test`
  still fail after 3 targeted fix iterations, stop and escalate with the saved
  log paths.
- Design tolerance: if keeping a fixed 128-hash sketch while allowing only
  configurable bands and rows proves incompatible with the current design doc
  or testability requirements, stop and ask whether roadmap scope should be
  widened to expose `hashes` as configuration now.

## Risks

- Hash-family risk: MinHash needs 128 deterministic hash functions without
  introducing randomness or external crates. Mitigation: derive a stable seed
  set from a fixed constant using a simple deterministic generator, then test
  exact sketch stability.
- Set-semantics risk: MinHash operates on sets, but the current token pass
  produces `Vec<Fingerprint>`, possibly with duplicate hash values from
  repeated minima. Mitigation: deduplicate hash values before sketching and add
  unit tests proving duplicate entries do not change the sketch.
- Empty-input risk: short fragments from roadmap item 7.2.1 can yield zero
  retained fingerprints. Mitigation: define explicit unhappy-path behaviour for
  empty fingerprint sets and cover it in unit and behaviour tests.
- Collision-order risk: the same pair may collide in several bands, and
  insertion order can otherwise leak into output ordering. Mitigation: emit
  canonical `(left, right)` pairs with sorted fragment IDs and deduplicate via
  an ordered set.
- False-positive risk: LSH is only a candidate filter, so broad band settings
  can generate noisy pairs. Mitigation: keep candidate generation separate from
  Jaccard acceptance, document the trade-off in the design doc, and test both
  collision and non-collision scenarios.
- Test-ergonomics risk: BDD worlds and parsed parameters can easily violate
  Clippy's argument-count limit. Mitigation: mirror the existing
  `TokenPassWorld` pattern and keep each step to `world` plus at most 3 parsed
  values.

## Progress

- [x] Stage A: Gather repository context and draft this ExecPlan
  (2026-03-21).
- [x] Stage B: Add failing unit tests for MinHash configuration, empty input,
  deterministic sketches, and LSH pair generation (2026-03-22).
- [x] Stage C: Add failing `rstest-bdd` feature scenarios for happy,
  unhappy, and edge-case candidate generation (2026-03-22).
- [x] Stage D: Implement the new `index` module and export its public API from
  `crates/whitaker_clones_core/src/lib.rs` (2026-03-22).
- [x] Stage E: Make the targeted tests green and refactor for readability
  while keeping files below 400 lines (2026-03-22).
- [x] Stage F: Update `docs/whitaker-clone-detector-design.md` with
  `## Implementation decisions (7.2.2)` (2026-03-22).
- [x] Stage G: Mark roadmap item 7.2.2 done in `docs/roadmap.md`
  (2026-03-22).
- [x] Stage H: Run documentation and code quality gates successfully
  (2026-03-22).
- [x] Stage I: Finalize the living sections in this ExecPlan after
  implementation (2026-03-22).

## Surprises & Discoveries

- Roadmap item 7.2.1 already shipped as a standalone crate,
  `crates/whitaker_clones_core/`, but that crate currently exports only the
  `token` module. There is no `index` module yet.
- The current token API already gives 7.2.2 the exact upstream input it needs:
  `Fingerprint { hash, range }` values after deterministic winnowing.
- `docs/whitaker-clone-detector-design.md` explicitly assigns MinHash and LSH
  to an `index` responsibility inside `whitaker_clones_core`, so extending the
  existing crate is consistent with the published architecture.
- The design document's configuration example includes `hashes = 128`, but the
  roadmap item called out only configurable bands and rows. This plan keeps 128
  fixed and treats the product of `bands * rows` as a validated invariant for
  7.2.2.
- Existing behaviour tests in this crate already use a fixture-backed world and
  `tests/features/` layout, so 7.2.2 can follow the same pattern instead of
  inventing a second BDD style.
- The repository guidance and prior project notes emphasize that doc changes
  require `make fmt`, `make markdownlint`, and `make nixie` in addition to the
  requested Rust gates.
- `rstest` tuple cases in `src/index/tests.rs` needed to be wrapped as a
  single `#[case(((...), ...))]` argument tuple to match the helper signature.
  The first attempt compiled the implementation cleanly but failed test macro
  expansion.

## Decision Log

- Decision: implement roadmap item 7.2.2 in a new `index/` module subtree
  inside `crates/whitaker_clones_core/src/`, not by expanding the existing
  `token/` module. Rationale: the published design already separates token and
  index responsibilities, and this keeps file size and cohesion healthy.
  Date/Author: 2026-03-21 / Codex.
- Decision: keep MinHash signatures fixed at 128 values for 7.2.2 and validate
  `bands * rows == 128`. Rationale: this matches the design document and keeps
  the roadmap item focused on the explicitly requested configurables.
  Date/Author: 2026-03-21 / Codex.
- Decision: use an opaque, string-backed `FragmentId` newtype in the public
  candidate-generation API for 7.2.2. Rationale: this keeps the index layer
  decoupled from later file-path and span hashing work in roadmap item 7.2.3,
  while still giving tests and callers a stable identifier surface.
  Date/Author: 2026-03-21 / Codex.
- Decision: stop 7.2.2 at deduplicated candidate pairs and leave Jaccard
  scoring plus acceptance thresholds for 7.2.3. Rationale: the roadmap and
  design doc split candidate generation from token-level scoring. Date/Author:
  2026-03-21 / Codex.
- Decision: derive the 128 MinHash seeds from a fixed SplitMix64 stream and
  mix each retained fingerprint hash with a deterministic avalanche function.
  Rationale: this keeps the sketch stable without adding a dependency or
  relying on `DefaultHasher` randomness. Date/Author: 2026-03-22 / Codex.

## Context and orientation

### Repository state

The repository root is `/home/user/project`. Roadmap item 7.2.1 already added
`crates/whitaker_clones_core/` with these files:

- `crates/whitaker_clones_core/src/lib.rs`
- `crates/whitaker_clones_core/src/token/mod.rs`
- `crates/whitaker_clones_core/src/token/types.rs`
- `crates/whitaker_clones_core/src/token/normalize.rs`
- `crates/whitaker_clones_core/src/token/fingerprint.rs`
- `crates/whitaker_clones_core/src/token/error.rs`
- `crates/whitaker_clones_core/tests/token_pass_behaviour.rs`
- `crates/whitaker_clones_core/tests/features/token_pass.feature`

That crate currently exports only token-pass building blocks. There is no
MinHash, LSH, or candidate-generation API yet.

### Design requirements from `docs/whitaker-clone-detector-design.md`

The clone-detector design currently states all of the following:

1. The token pass should compute a 128-dimensional MinHash sketch for each
   fragment's retained fingerprint set.
2. Locality-sensitive hashing should split the sketch into configurable bands
   and rows, and candidate fragments are those that collide in at least one
   band.
3. Pair scoring by Jaccard similarity comes later, after candidate generation.
4. The `whitaker_clones_core` crate should own this index logic.

The design document also includes a minimal skeleton with `MinHasher` and
`LshIndex`, which makes those names the lowest-friction starting point.

### Testing and documentation references

The implementation should follow these local guides while keeping the plan
self-contained:

- `docs/rstest-bdd-users-guide.md` for fixture-backed scenario tests and the
  workspace's `rstest-bdd` v0.5.0 conventions.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` fixture composition
  and test-data reuse.
- `docs/rust-doctest-dry-guide.md` for public Rustdoc example style.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for keeping the
  algorithm split into small, comprehensible helpers.

## Proposed implementation shape

Add a new module tree under `crates/whitaker_clones_core/src/index/` and keep
each file focused:

- `mod.rs`: public re-exports and module wiring.
- `types.rs`: `MINHASH_SIZE`, `FragmentId`, `MinHashSignature`,
  `CandidatePair`, and validated LSH configuration types.
- `error.rs`: typed errors such as zero bands, zero rows, invalid
  `bands * rows`, and empty fingerprint input.
- `minhash.rs`: deterministic seed generation and `MinHasher::sketch`.
- `lsh.rs`: band bucketing, collision tracking, pair deduplication, and
  public candidate-generation entry points.
- `tests.rs`: focused unit tests local to the module.

Update `crates/whitaker_clones_core/src/lib.rs` so the crate publicly exports
the new index-layer types and functions alongside the existing token module.

The initial API should stay small and explicit. A concrete shape to implement
is:

```rust
pub const MINHASH_SIZE: usize = 128;

pub struct FragmentId(String);

pub struct LshConfig {
    /* validated bands and rows */
}

pub struct MinHashSignature([u64; MINHASH_SIZE]);

pub struct CandidatePair {
    pub left: FragmentId,
    pub right: FragmentId,
}

pub struct MinHasher {
    /* deterministic seeds */
}

impl MinHasher {
    pub fn new() -> Self;
    pub fn sketch(&self, fingerprints: &[Fingerprint]) -> Result<MinHashSignature>;
}

pub struct LshIndex {
    /* bucketed by band */
}

impl LshIndex {
    pub fn new(config: LshConfig) -> Self;
    pub fn insert(&mut self, id: &FragmentId, signature: &MinHashSignature);
    pub fn candidate_pairs(&self) -> Vec<CandidatePair>;
}
```

Implementation details to keep stable and testable:

1. Treat MinHash input as a set of fingerprint hash values. Deduplicate
   `Fingerprint.hash` values before sketching so repeated minima from winnowing
   do not distort the signature.
2. Derive 128 stable seeds from a fixed constant using a tiny deterministic
   generator implemented in-house. Avoid `DefaultHasher` or any random seeding.
3. For each seed, combine the seed with each fingerprint hash via a stable
   mixing function and keep the minimum mixed value.
4. Split the 128-value signature into validated bands and rows. For each band,
   create a deterministic bucket key from `(band_index, band_values...)`.
5. Track bucket members in deterministic containers and emit canonical
   candidate pairs where `left < right`.
6. Deduplicate pairs across bands and return them in a stable sorted order.

## Test plan

Start with failing tests, then implement to green.

Unit coverage belongs under `crates/whitaker_clones_core/src/index/tests.rs`.
At minimum, add tests for:

1. `LshConfig` rejects zero bands.
2. `LshConfig` rejects zero rows.
3. `LshConfig` rejects products that do not equal 128.
4. `MinHasher::sketch` rejects an empty fingerprint list.
5. Duplicate fingerprint hashes do not change the sketch.
6. Identical fingerprint sets yield identical signatures.
7. Insertion order does not change candidate-pair output.
8. A pair that collides in more than one band is emitted exactly once.
9. Self-pairs are never emitted.

Behaviour coverage belongs in:

- `crates/whitaker_clones_core/tests/min_hash_lsh_behaviour.rs`
- `crates/whitaker_clones_core/tests/features/min_hash_lsh.feature`

The BDD scenarios should cover:

1. Two fragments with identical retained fingerprints become a candidate pair.
2. Two clearly different fragments do not become a candidate pair.
3. Multiple colliding bands still produce one canonical pair.
4. Invalid band or row settings surface a typed error.
5. An empty retained-fingerprint list is rejected explicitly.

Reuse the current `TokenPassWorld` style: a small fixture-backed world with
`RefCell` state, helper accessors, and step functions that parse no more than 3
values.

## Documentation updates

During implementation, append a new `## Implementation decisions (7.2.2)`
section to `docs/whitaker-clone-detector-design.md`. Record the final answers
to these design questions:

1. How the 128 deterministic MinHash seeds are generated.
2. Whether duplicate fingerprint hashes are collapsed before sketching.
3. The exact validation rule for configurable bands and rows.
4. How empty fingerprint sets are handled.
5. How candidate pairs are ordered and deduplicated across multiple bands.

After the code and tests are green, mark roadmap item 7.2.2 as done in
`docs/roadmap.md`.

## Validation

Use `tee` and `set -o pipefail` for every quality gate so failures are captured
completely. The final implementation turn should run these commands from the
repository root:

```sh
set -o pipefail; make fmt | tee /tmp/7-2-2-fmt.log
set -o pipefail; make markdownlint | tee /tmp/7-2-2-markdownlint.log
set -o pipefail; make nixie | tee /tmp/7-2-2-nixie.log
set -o pipefail; make check-fmt | tee /tmp/7-2-2-check-fmt.log
set -o pipefail; make lint | tee /tmp/7-2-2-lint.log
set -o pipefail; make test | tee /tmp/7-2-2-test.log
```

Implementation is complete only when all six commands succeed, the new unit
tests and BDD scenarios pass, `docs/whitaker-clone-detector-design.md` records
the 7.2.2 decisions, and `docs/roadmap.md` marks 7.2.2 done.

## Outcomes & Retrospective

Implemented a new `crates/whitaker_clones_core/src/index/` module subtree with
typed configuration and error handling, deterministic 128-slot MinHash
sketching, ordered-band LSH bucketing, and canonical deduplicated
`CandidatePair` output. The crate root now re-exports the index API alongside
the token API.

Added unit coverage in `src/index/tests.rs` plus BDD coverage in
`tests/min_hash_lsh_behaviour.rs` and `tests/features/min_hash_lsh.feature`.
The implementation stayed within the planned scope: no Jaccard acceptance,
clone grouping, SARIF emission, CLI wiring, or AST work was added.

Documentation changes shipped as planned:

- `docs/whitaker-clone-detector-design.md` now records the 7.2.2
  implementation decisions.
- `docs/roadmap.md` marks 7.2.2 done.

Validation commands run successfully, with logs captured at:

- `/tmp/7-2-2-fmt.log`
- `/tmp/7-2-2-markdownlint.log`
- `/tmp/7-2-2-nixie.log`
- `/tmp/7-2-2-check-fmt.log`
- `/tmp/7-2-2-lint.log`
- `/tmp/7-2-2-test.log`

Additional targeted crate logs used during implementation:

- `/tmp/7-2-2-crate-test.log`
- `/tmp/7-2-2-crate-clippy.log`

Final verification outcome:

- `make fmt` passed.
- `make markdownlint` passed.
- `make nixie` passed.
- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed with `1147` tests run, `1147` passed, and `2` skipped.
