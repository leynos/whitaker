# Architectural decision record (ADR) 003: formal proof strategy for the clone detector pipeline

## Status

Accepted (2026-03-29): Use Verus for local algebraic and canonicalization
invariants, Kani for bounded stateful algorithm checks, and ordinary unit and
behaviour tests for end-to-end clone-detector regression.

## Date

2026-03-29.

## Context and problem statement

Whitaker's clone detector pipeline mixes several different kinds of correctness
obligations. Some are small semantic invariants over arithmetic and ordering,
such as validating locality-sensitive hashing (LSH) configuration or
canonicalizing fragment pairs. Others are bounded behavioural properties over
stateful algorithms, such as ensuring MinHash sketching is deterministic for a
fixed input set, or ensuring repeated LSH bucket collisions still emit only one
candidate pair.

Roadmap item 6.4.3 already established a successful Verus sidecar workflow for
a narrow arithmetic predicate in the brain-trust decomposition logic. The
adjacent roadmap items 6.4.4 to 6.4.6 also established the intended split
between Verus for algebraic reasoning and Kani for bounded graph and adjacency
checks.

The problem is to choose a proof strategy for the clone detector pipeline that
raises assurance without forcing every algorithm into the wrong proof model,
duplicating runtime logic in proof-only code, or replacing ordinary tests with
formal methods where proofs are a poor fit.

## Decision drivers

- Match the proof tool to the shape of the property being checked.
- Keep proof scope proportional to the implementation risk and maintenance
  cost.
- Preserve deterministic runtime behaviour without adding proof-tool runtime
  dependencies to the Cargo workspace.
- Prefer proofs to narrow shipped seams to reduce drift between runtime code
  and proof artefacts.
- Keep normal Cargo development workflows independent of proof execution.
- Use bounded model checking where collection state, iteration order, and
  insertion order are part of the behaviour under test.

## Requirements

### Functional requirements

- Provide machine-checked assurance for high-value MinHash, LSH, and
  clone-detector invariants.
- Cover both local semantic invariants and bounded behavioural properties.
- Make the ownership of each proof obligation explicit in the roadmap.

### Technical requirements

- Proof tooling must live in sidecar scripts and files rather than normal
  library dependencies.
- Verus targets must stay small, explicit, and close to pure runtime seams.
- Kani targets must exercise real runtime code through bounded harnesses
  rather than clean-room reimplementations.
- Runtime APIs may gain narrow proof seams when needed, but the production API
  should not widen solely for proof convenience.
- Ordinary unit, behaviour, and regression tests remain required even when a
  formal proof exists.

## Options considered

### Option A: ordinary tests only

Rely exclusively on unit tests, behaviour tests, and integration tests for the
clone detector pipeline.

### Option B: Verus for all clone-detector proofs

Model every important clone-detector invariant in Verus, including
configuration checks, canonicalization, MinHash sketch construction, and full
LSH bucket enumeration.

### Option C: Kani for all clone-detector proofs

Use Kani harnesses for both local semantic invariants and bounded behavioural
checks across the clone detector pipeline.

### Option D: mixed Verus and Kani strategy

Use Verus for local algebraic and canonicalization properties, use Kani for
bounded stateful algorithm checks, and keep ordinary tests for exact
regression, fixture coverage, and full pipeline observability.

| Topic                             | Option A      | Option B               | Option C               | Option D |
| --------------------------------- | ------------- | ---------------------- | ---------------------- | -------- |
| Local arithmetic invariants       | Indirect only | Strong                 | Adequate               | Strong   |
| Stateful bounded behaviour        | Good          | Costly to model        | Strong                 | Strong   |
| Collection-heavy algorithms       | Test only     | Poor fit               | Good fit               | Good fit |
| Proof maintenance cost            | Low           | High                   | Moderate               | Moderate |
| Drift risk from proof-only models | None          | High                   | Medium                 | Medium   |
| Coverage of exact regressions     | Strong        | Weak unless duplicated | Weak unless duplicated | Strong   |

_Table 1: Trade-offs between proof strategies for the clone detector pipeline._

## Decision outcome / proposed direction

Adopt Option D.

Whitaker will use a mixed formal-method strategy for clone-detector algorithm
implementations and detector invariants:

- Use **Verus** for small semantic invariants whose runtime shape is already
  close to a proof-friendly predicate or constructor.
- Use **Kani** for bounded behavioural properties over the real MinHash and LSH
  implementations, especially where stateful collection updates or insertion
  order affect the observable result.
- Keep **ordinary tests** for exact regression vectors, fixture-driven
  scenarios, and end-to-end pipeline behaviour.

Initial target allocation:

- **Verus**
  - `LshConfig::new` and its `bands * rows == MINHASH_SIZE` invariant.
  - `CandidatePair::new` canonical lexical ordering and self-pair
    suppression.
  - Small pure helpers introduced to state set semantics explicitly, where the
    proof value justifies the seam.
- **Kani**
  - Bounded `MinHasher::sketch` checks for determinism, duplicate-hash
    insensitivity, and empty-input failure.
  - Bounded `LshIndex` checks for no self-pairs, canonical pair ordering,
    repeated-band deduplication, and insertion-order independence for small
    bounded inputs.
- **Ordinary tests**
  - Exact SplitMix64 regression vectors and seed-stream stability.
  - Token-pass, candidate-generation, and SARIF-emission behaviour suites.

This ADR does not claim that MinHash or LSH statistical quality can be proved
exhaustively with these tools. The decision covers implementation and detector
invariants, not probabilistic performance guarantees.

## Goals and non-goals

Goals:

- Use each proof tool where it is the best technical fit.
- Add formal assurance to the highest-value clone-detector invariants.
- Keep proof workflows reproducible and explicit in repository tooling.
- Preserve the existing expectation that tests remain the first regression
  safety net.

Non-goals:

- Prove the probabilistic collision quality of MinHash or LSH exhaustively.
- Replace unit, behaviour, or integration tests with proofs.
- Model the entirety of `BTreeMap` or `BTreeSet` semantics in Verus.
- Force every clone-detector stage into formal verification before the runtime
  pipeline exists.

## Migration plan

1. Record this decision in an ADR and add explicit roadmap tasks under `7.2`
   for the selected Verus and Kani work.
2. Add clone-detector proof workflow targets and wrapper scripts so Verus and
   Kani can run reproducibly outside normal Cargo builds.
3. Implement the first Verus proofs for `LshConfig` and `CandidatePair`.
4. Implement bounded Kani harnesses for `MinHasher::sketch` and `LshIndex`
   candidate-pair invariants.
5. Extend the same split to later clone-detector stages only where the runtime
   code exposes a stable, proof-worthy seam.

## Known risks and limitations

- Verus becomes expensive quickly once proofs depend on collection internals or
  bit-level mixing routines rather than small semantic seams.
- Kani guarantees are bounded by the harness size and search space; they do not
  replace unbounded mathematical proofs.
- Proof-only helper seams can drift from production code if they do not stay
  close to shipped constructors or predicates.
- Formal proofs may still leave algorithm-quality questions unresolved, such as
  whether a chosen banding configuration is operationally effective on real
  repositories.

## Architectural rationale

This decision aligns the clone detector with the proof strategy already used in
the brain-trust roadmap: Verus for local algebraic correctness, Kani for
bounded stateful behaviour. That keeps formal methods aligned with Whitaker's
larger architectural principle of choosing small, coherent modules and testing
the real implementation rather than elaborate stand-in models.
