# Prove `dot_product` and `norm_squared` algebraic properties with Verus (roadmap 6.4.4)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

Implementation must not begin until the user explicitly approves this plan.

## Purpose / big picture

Roadmap item 6.4.4 extends the existing decomposition-advice proof work from
the cosine-threshold predicate to the vector algebra underneath it. After this
change, Whitaker will not rely only on local reasoning and tests to justify the
sparse-vector helpers in `common/src/decomposition_advice/vector.rs`. It will
also ship a repeatable Verus proof for the algebraic properties that future
clustering and threshold proofs depend on: `dot_product` is commutative,
`norm_squared` is always non-negative, and `dot_product` becomes zero when the
two vectors have no overlapping positive features.

Observable success after implementation:

1. `make verus` runs both the existing 6.4.3 threshold proof and a new 6.4.4
   vector-algebra proof, and ends with zero verification errors.
2. Unit tests exercise the shipped Rust helpers directly, including happy
   paths, unhappy paths, and edge cases around empty vectors and disjoint
   features.
3. Behaviour tests using `rstest-bdd` `0.5.0` cover the same properties
   through a narrow public test-support seam rather than by widening the
   production API.
4. `docs/brain-trust-lints-design.md` records the final 6.4.4 proof-model
   decisions.
5. `docs/roadmap.md` marks 6.4.4 done only after `make check-fmt`,
   `make lint`, `make test`, and `make verus` all succeed.

## Constraints

- Scope only roadmap item 6.4.4. Do not absorb roadmap items 6.4.5 or 6.4.6
  into this change.
- Keep the shipped runtime behaviour of
  `common/src/decomposition_advice/vector.rs` unchanged unless a tiny refactor
  is required to make the proof target and runtime helper names line up more
  clearly. Any such refactor must preserve results exactly.
- Keep `common` free of a runtime dependency on Verus. Proof artefacts must
  stay in the top-level sidecar structure under `verus/` and `scripts/`.
- Treat the existing Rust helpers as the source of truth. The proof may use a
  proof-friendly mathematical model, but the model must be documented as an
  abstraction of the shipped helpers, not as a replacement design.
- Do not expose `MethodFeatureVector`, `dot_product`, or `norm_squared`
  publicly just to make integration tests easier. Use `common::test_support`
  for any new observable seam.
- Keep source files under 400 lines. If the new proof grows too large, split
  it into small sibling Verus files or local modules instead of growing one
  large file.
- Use the workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  `0.5.0` for new behavioural coverage.
- Behaviour tests must respect the workspace Clippy `too_many_arguments`
  threshold of 4. Each behaviour-driven development (BDD) step may parse at
  most 3 values from feature text in addition to the world fixture.
- Public helpers added to `common::test_support` require Rustdoc comments with
  examples that follow `docs/rust-doctest-dry-guide.md`.
- Update `docs/brain-trust-lints-design.md` with the implementation decisions
  taken during delivery.
- Mark roadmap item 6.4.4 done only after the proof command, unit tests,
  behavioural tests, and all required quality gates succeed.
- Run long validation commands through `tee` with `set -o pipefail`, because
  this environment truncates long command output.

## Tolerances (exception triggers)

- Scope: if the change grows beyond 10 touched files or 900 net lines, stop
  and escalate before continuing.
- Interface: if the only practical implementation path requires widening the
  public production API of `common::decomposition_advice`, stop and escalate.
- Proof modelling: if Verus cannot express a convincing abstraction of the
  sparse-vector helpers after 2 modelling iterations, stop and present options
  instead of forcing a weak or misleading theorem.
- Tooling: if `make verus` cannot be updated to run all required proof files
  with one runner change and one refinement iteration, stop and escalate with
  the exact runner failure.
- Semantics: if proving the requested properties appears to require changing
  the meaning of "no overlapping positive features", stop and document the
  competing interpretations before proceeding.
- Validation: if `make check-fmt`, `make lint`, `make test`, or `make verus`
  still fail after 3 targeted fix iterations, stop and escalate with captured
  logs.
- Dependencies: if a new external dependency is required, stop and escalate.

## Risks

- Proof-drift risk: the Rust code works over `BTreeMap<String, u64>`, but
  Verus proofs are easier over mathematical maps or aligned sequences.
  Severity: high. Likelihood: medium. Mitigation: write down the abstraction
  explicitly in the proof file and in the design doc, and keep theorem names
  tied to the shipped helper names.
- Feature-overlap interpretation risk: the roadmap says "no overlapping
  positive features", not merely "disjoint keys". Shared keys with zero weight
  matter to the formal statement even if production vectors usually contain
  only positive weights. Severity: medium. Likelihood: medium. Mitigation:
  define the property as "for every feature, not both weights are positive" and
  cover the runtime cases that are actually constructible today.
- Runner coverage risk: `scripts/run-verus.sh` currently points at only
  `verus/decomposition_cosine_threshold.rs`, so a new proof file would be easy
  to add but never execute by default. Severity: high. Likelihood: high.
  Mitigation: update the runner to execute an explicit deterministic list of
  proof files.
- Test-seam risk: current integration coverage only exposes
  `methods_meet_cosine_threshold()` through `common::test_support`. Severity:
  medium. Likelihood: high. Mitigation: add a narrow test-support report or
  helper functions for dot product and squared norm results.
- Clippy-and-behaviour-driven development (BDD) ergonomics risk: behavioural
  tests are subject to the same argument-count and `expect` restrictions as
  production code. Severity: medium. Likelihood: medium. Mitigation: use a
  small world fixture, return `Result<(), String>` from step functions, and
  split overly wide steps.

## Progress

- [x] 2026-03-26: Review roadmap item 6.4.4, the decomposition design
  document, the existing 6.4.3 proof workflow, and the current vector helper
  implementation.
- [x] 2026-03-26: Draft this ExecPlan with concrete file targets, proof scope,
  testing strategy, and validation commands.
- [x] Add a new Verus proof file for vector algebra and wire `make verus` to
  execute it deterministically.
- [x] Add unit tests for `dot_product` and `norm_squared` happy paths,
  unhappy paths, and edge cases.
- [x] Add `rstest-bdd` behavioural coverage using a narrow
  `common::test_support::decomposition` seam.
- [x] Record 6.4.4 implementation decisions in
  `docs/brain-trust-lints-design.md`.
- [x] Mark roadmap item 6.4.4 done in `docs/roadmap.md`.
- [x] Run `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint`, `make test`, and `make verus` successfully.
- [x] Finalize the living sections in this ExecPlan after implementation.

## Surprises & Discoveries

- `common/src/decomposition_advice/vector.rs` already contains the two runtime
  helpers in scope for this roadmap item: `MethodFeatureVector::norm_squared()`
  and the free function `dot_product(...)`.
- `dot_product(...)` is implemented with a size-based branch that iterates the
  smaller map for efficiency before looking up matching features in the larger
  map. The proof will need to address commutativity despite this asymmetric
  implementation shape.
- `norm_squared()` is currently a plain sum of `weight * weight` over
  `u64` values. That makes non-negativity obvious in Rust, but the proof still
  needs to state it explicitly as a machine-checked property.
- The existing test-only constructor `test_feature_vector(...)` can build
  vectors containing zero-weight entries even though production feature-vector
  construction only emits positive weights. This matters for edge-case test
  design and for the exact meaning of the zero-result theorem.
- `make verus` currently delegates to `scripts/run-verus.sh`, and that script
  runs only `verus/decomposition_cosine_threshold.rs`.
- The 6.4.3 implementation already established the sidecar pattern this item
  should reuse: top-level `verus/` files, shell wrappers under `scripts/`, and
  design-document decisions recorded under a dedicated subsection.
- Verus's required toolchain hint currently includes the host triple
  (`1.94.0-x86_64-unknown-linux-gnu` on Linux), but local `rustup` may reject
  that exact name. The installer therefore needs a fallback that strips the
  host suffix and installs the bare semantic version instead.

## Decision Log

- Decision: plan for a separate proof file,
  `verus/decomposition_vector_algebra.rs`, instead of folding 6.4.4 into
  `verus/decomposition_cosine_threshold.rs`. Rationale: 6.4.3 already owns the
  threshold proof, and keeping 6.4.4 separate preserves roadmap boundaries and
  makes `make verus` output easier to interpret. Date/Author: 2026-03-26 /
  Codex.
- Decision: prefer a deterministic multi-file proof runner over a single-file
  default. Rationale: once the repository has more than one proof file,
  `make verus` must exercise all of them or it stops being a trustworthy gate.
  Date/Author: 2026-03-26 / Codex.
- Decision: define the zero-result theorem in terms of "no overlapping
  positive features" rather than "disjoint maps". Rationale: this matches the
  roadmap wording and remains correct even if future refactors admit explicit
  zero weights in sparse vectors. Date/Author: 2026-03-26 / Codex.
- Decision: expose any new behavioural-test seam through
  `common::test_support::decomposition` rather than the production
  decomposition module. Rationale: tests need observable results, but the
  production API should stay narrow and compiler-independent. Date/Author:
  2026-03-26 / Codex.
- Decision: return a small numeric report struct from the behavioural-test seam
  instead of exposing multiple helper functions. Rationale: one report keeps
  BDD assertions explicit while avoiding extra public test-support surface.
  Date/Author: 2026-03-27 / Codex.
- Decision: keep the proof model in `Seq<nat>` rather than a Verus map model.
  Rationale: the roadmap properties are algebraic, absent sparse-map entries
  naturally map to zero-weight sequence positions, and recursive proofs over
  aligned sequences are smaller than map-domain proofs. Date/Author: 2026-03-27
  / Codex.

## Context and orientation

The implementation under proof lives in
`common/src/decomposition_advice/vector.rs`.

Today, the relevant runtime shape is:

1. `MethodFeatureVector::norm_squared()` sums `weight * weight` over the
   vector's sparse `BTreeMap<String, u64>`.
2. `dot_product(left, right)` picks the smaller map by length, iterates its
   entries, multiplies weights for matching feature keys, and sums the products.
3. `common/src/decomposition_advice/community.rs` uses those helpers to build
   similarity edges and to feed the already-proved threshold predicate.

For this roadmap item, "no overlapping positive features" should be treated as
the following mathematical condition:

```plaintext
For every feature key k, not (left[k] > 0 and right[k] > 0).
```

Absent keys count as weight `0`. This condition is stronger and clearer than
"the maps have no common keys", because it still allows shared keys whose
weights are zero without invalidating the theorem.

The repository already has one proof file,
`verus/decomposition_cosine_threshold.rs`, plus sidecar scripts
`scripts/install-verus.sh` and `scripts/run-verus.sh`. The user also supplied
Chutoro proof examples that follow the same pattern: small top-level Verus
files with explicit lemmas and shell wrappers instead of Cargo-integrated proof
dependencies. This item should follow that pattern.

## Proposed implementation shape

The safest implementation is to keep the runtime Rust code small and to put the
new proof effort into a sibling Verus file plus narrow tests.

### Milestone 1: establish the failing red phase

Start by making the missing work observable before writing the final proof.

1. Update `scripts/run-verus.sh` so it runs an explicit ordered list of proof
   files rather than a single default file. The list should include the
   existing `verus/decomposition_cosine_threshold.rs` and the planned new file
   `verus/decomposition_vector_algebra.rs`.
2. Add a skeletal `verus/decomposition_vector_algebra.rs` with theorem
   signatures for:
   - dot-product commutativity,
   - squared-norm non-negativity,
   - zero dot product under no overlapping positive features.
3. Add unit-test and BDD scaffolding that expresses the intended runtime
   properties.
4. Run the targeted checks and confirm they fail for the right reason before
   filling in the proof and runtime test helpers.

Recommended red-phase commands:

```sh
set -o pipefail && make verus | tee /tmp/6-4-4-red-verus.log
set -o pipefail && cargo test -p common decomposition_advice::tests:: -- --nocapture | tee /tmp/6-4-4-red-unit.log
set -o pipefail && cargo test -p common --test decomposition_vector_algebra_behaviour -- --nocapture | tee /tmp/6-4-4-red-bdd.log
```

The first command should fail because the new proof file is not yet proved. The
later commands should fail because the new tests or helpers are not yet
implemented.

### Milestone 2: add the Verus proof

Implement the new proof in `verus/decomposition_vector_algebra.rs`.

The proof model should stay close to the runtime helper names while still being
easy for Verus to reason about. The most practical shape is a mathematical
vector model with absent entries treated as zero, expressed either as a finite
map or as aligned sequences. The file should define proof-friendly spec
functions representing `dot_product` and `norm_squared`, then prove:

1. `dot_product(left, right) == dot_product(right, left)`.
2. `0 <= norm_squared(vector)`.
3. If no feature has positive weight in both vectors, then
   `dot_product(left, right) == 0`.

If the proof needs a helper lemma that
`norm_squared(vector) == dot_product(vector, vector)`, that is acceptable as an
internal proof aid even though it is not a separate roadmap requirement.

Keep the proof file under 400 lines. If the lemmas become too large, split
shared abstractions into a second proof-only sibling file and have
`scripts/run-verus.sh` execute both.

### Milestone 3: add runtime unit coverage

Add focused unit tests under `common/src/decomposition_advice/tests/`. Prefer a
new sibling file such as
`common/src/decomposition_advice/tests/vector_algebra.rs` and wire it from
`common/src/decomposition_advice/tests.rs`.

Required unit coverage:

1. `dot_product` returns the same result in both operand orders for vectors
   with overlapping features.
2. `dot_product` returns `0` for vectors with no overlapping positive
   features.
3. `norm_squared` returns `0` for an empty vector.
4. `norm_squared` returns a positive value when at least one weight is
   positive.
5. Edge cases involving explicit zero weights created through
   `test_feature_vector(...)`, so the runtime tests match the roadmap's
   "positive features" wording rather than relying only on production builder
   behaviour.

These tests should exercise the shipped Rust helpers directly rather than only
through higher-level decomposition logic.

### Milestone 4: add behavioural coverage with `rstest-bdd`

Add an integration test pair:

- `common/tests/decomposition_vector_algebra_behaviour.rs`
- `common/tests/features/decomposition_vector_algebra.feature`

Do not expose private vector types publicly. Instead, add one narrow helper to
`common/src/test_support/decomposition.rs`. The cleanest shape is a small
report-producing function, for example:

```plaintext
vector_algebra_report(left, right) -> { dot_product, left_norm_squared, right_norm_squared }
```

That helper should build the internal feature vectors and return only the
observable numeric results needed by BDD assertions.

Suggested scenarios:

1. Happy path: overlapping positive features yield the same dot product
   regardless of operand order, and both squared norms are positive.
2. Unhappy path: unrelated methods with no overlapping positive features yield
   `dot_product = 0`.
3. Edge case: a stop-word-only or otherwise empty feature vector yields
   `norm_squared = 0`.

Follow existing repository patterns:

- use a small world fixture,
- return `Result<(), String>` from step functions,
- keep step argument counts within the Clippy threshold,
- bind scenarios by index as in the existing decomposition BDD tests.

### Milestone 5: document the design decisions and close the roadmap item

Update `docs/brain-trust-lints-design.md` with a new
`### Implementation decisions (6.4.4)` section. Record at least:

1. the chosen proof abstraction for sparse vectors,
2. the exact meaning of "no overlapping positive features",
3. the reason `make verus` now runs multiple proof files,
4. the decision to keep the behavioural-test seam in
   `common::test_support::decomposition`.

After all validation succeeds, mark roadmap item 6.4.4 done in
`docs/roadmap.md`.

## Validation and evidence

After implementation, run the full required gates with log capture:

```sh
set -o pipefail && make fmt | tee /tmp/6-4-4-fmt.log
set -o pipefail && make markdownlint | tee /tmp/6-4-4-markdownlint.log
set -o pipefail && make nixie | tee /tmp/6-4-4-nixie.log
set -o pipefail && make check-fmt | tee /tmp/6-4-4-check-fmt.log
set -o pipefail && make lint | tee /tmp/6-4-4-lint.log
set -o pipefail && make test | tee /tmp/6-4-4-test.log
set -o pipefail && make verus | tee /tmp/6-4-4-verus.log
```

Expected success signals:

```plaintext
- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0 and reports the new unit and behavioural tests as passed.
- `make verus` exits 0 and reports zero verification errors across both proof files.
```

Before marking the roadmap item done, inspect the captured logs and make sure
the new proof file and the new BDD test binary both actually ran.

## Outcomes & Retrospective

Shipped:

- `verus/decomposition_vector_algebra.rs` proves dot-product commutativity,
  squared-norm non-negativity, and zero dot product under no overlapping
  positive features.
- `scripts/run-verus.sh` now executes an explicit proof-file list, and
  `scripts/install-verus.sh` handles Verus's host-qualified toolchain hint
  robustly.
- Runtime coverage now includes
  `common/src/decomposition_advice/tests/vector_algebra.rs`,
  `common/tests/decomposition_vector_algebra_behaviour.rs`, and
  `common/tests/features/decomposition_vector_algebra.feature`.
- Behaviour tests observe runtime results through
  `common::test_support::decomposition::method_vector_algebra()`.

Evidence gathered during implementation:

- `make verus` passed after the new proof and runner changes.
- `cargo test -p common decomposition_advice::tests::vector_algebra -- --nocapture`
  passed.
- `cargo test -p common --test decomposition_vector_algebra_behaviour -- --nocapture`
  passed.
- Full gates passed with captured logs:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  `make test`, and `make verus`.

Remaining roadmap work:

- 6.4.5 still needs Kani coverage for adjacency construction.
- 6.4.6 still needs Kani coverage for label propagation.
