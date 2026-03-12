# Build method community detection for decomposition advice (roadmap 6.4.1)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

This plan must also be written to
`docs/execplans/6-4-1-method-community-detection.md` as the first
implementation step.

## Purpose / big picture

Roadmap item 6.4.1 adds the analysis layer behind decomposition advice for
`brain_type` and `brain_trait`. After this change, Whitaker will be able to
take per-method metadata, turn each method into a feature vector, run a
deterministic community-detection pass, and return structured decomposition
suggestions such as "these parsing methods belong together" or "these
serialization methods should move into a dedicated module".

This is not the diagnostic-emission step. Roadmap item 6.4.2 will decide how to
render concise notes in lint output. The deliverable for 6.4.1 is the shared,
compiler-independent analysis engine and the structured suggestion objects that
6.4.2 can later format.

Observable outcome:

1. `common` exports a new shared decomposition-analysis module that accepts
   pre-extracted method profiles and returns stable decomposition suggestions.
2. Unit tests cover happy, unhappy, and edge cases for feature extraction,
   similarity scoring, community detection, and suggestion labelling.
3. Behaviour tests using `rstest-bdd` v0.5.0 validate end-to-end clustering on
   synthetic method sets for both type-like and trait-like inputs.
4. `docs/brain-trust-lints-design.md` records the final algorithm and data
   modelling decisions for 6.4.1.
5. `docs/roadmap.md` marks 6.4.1 done only after implementation and all
   quality gates pass.
6. `make check-fmt`, `make lint`, and `make test` pass at the end of the
   implementation turn.

## Constraints

- Scope only roadmap item 6.4.1. Do not implement 6.4.2 diagnostic note
  rendering, configuration loading, localisation wiring, or SARIF output in
  this change.
- Keep the analysis engine in `common` and keep `common` free of
  `rustc_private` dependencies. The compiler-facing lint drivers remain
  responsible for extracting raw method metadata from High-level Intermediate
  Representation (HIR).
- Design the API so both `brain_type` and `brain_trait` can reuse it. The
  shared types must not assume only stateful type methods or only trait methods
  with bodies.
- Keep source files under 400 lines. Split the module into sibling files early
  rather than letting one file absorb the whole algorithm.
- Use only existing workspace dependencies unless explicit approval is given to
  add a new one. Prefer a deterministic in-house implementation over bringing
  in a graph library just for this feature.
- Preserve deterministic behaviour. The same input method profiles must always
  yield the same communities, labels, and ordering.
- Use workspace-pinned `rstest`, `rstest-bdd`, and `rstest-bdd-macros`
  (`0.5.0`) for tests.
- Behaviour tests must respect the workspace Clippy threshold of 4 arguments.
  Each BDD step can parse at most 3 values from feature text in addition to the
  world fixture.
- Public APIs added in `common` require Rustdoc comments with examples that
  follow `docs/rust-doctest-dry-guide.md`.
- Update `docs/brain-trust-lints-design.md` with implementation decisions
  taken during delivery.
- Do not mark roadmap item 6.4.1 as done until the implementation, tests, and
  all requested quality gates succeed.

## Tolerances (exception triggers)

- Scope: if implementation grows beyond 12 touched files or 1400 net lines of
  code, stop and escalate.
- Interface: if implementing 6.4.1 requires changing existing public APIs in
  `common::brain_type_metrics` or `common::brain_trait_metrics`, stop and
  escalate.
- Dependency: if a new external dependency appears necessary for graph or
  clustering work, stop and escalate before adding it.
- Algorithm: if the chosen community-detection implementation cannot
  consistently separate the synthetic parser/serde/fs fixture into stable
  communities after one prototype and one refinement pass, stop and escalate
  with the observed failure mode.
- Validation: if `make check-fmt`, `make lint`, or `make test` still fail
  after 3 targeted fix iterations, stop and escalate with captured logs.
- Ambiguity: if the extraction target kind (`helper struct`, `module`, or
  `sub-trait`) cannot be inferred without inventing behaviour not supported by
  the design doc, stop and present the options before proceeding.

## Risks

- Algorithm-selection risk: the design doc names Louvain or Leiden as examples,
  but the workspace currently has no graph/community dependency. Severity:
  medium. Likelihood: medium. Mitigation: start with deterministic weighted
  label propagation over a cosine-similarity graph, which still qualifies as
  community detection and is implementable with `std`.
- Data-shape risk: required trait methods have no bodies, so they may lack
  field and local-type features. Severity: medium. Likelihood: high.
  Mitigation: make signature types, external domains, and method-name keywords
  first-class features, and cover trait-only scenarios in BDD tests.
- Noise risk: common verbs such as `get`, `set`, `parse`, or `run` may
  dominate keyword features and collapse unrelated methods together. Severity:
  medium. Likelihood: medium. Mitigation: add a stop-word list, split
  snake_case and camelCase names, and keep keywords lower-weight than fields
  and domains.
- Stability risk: many community-detection algorithms are order-sensitive.
  Severity: high. Likelihood: medium. Mitigation: sort all inputs, use
  deterministic tie-breakers, and assert exact cluster membership/order in unit
  tests.
- Overreach risk: 6.4.1 can drift into 6.4.2 if formatting logic is added too
  early. Severity: medium. Likelihood: medium. Mitigation: return structured
  suggestion data only; keep formatting changes out of scope.

## Progress

- [x] Stage A: Draft this ExecPlan and capture the current repository state.
- [x] Stage B: Add failing unit tests and BDD scenarios that define the
  decomposition-analysis contract.
- [x] Stage C: Implement a shared decomposition module in `common`.
- [x] Stage D: Export the new API from `common/src/lib.rs`.
- [x] Stage E: Make tests green and refactor for readability while keeping
  deterministic behaviour.
- [x] Stage F: Record implementation decisions for 6.4.1 in
  `docs/brain-trust-lints-design.md`.
- [x] Stage G: Mark roadmap item 6.4.1 done.
- [x] Stage H: Run `make check-fmt`, `make lint`, and `make test`
  successfully.
- [x] Stage I: Finalize the living sections in this document.

## Surprises & Discoveries

- The repository does not yet contain dedicated `brain_type` or `brain_trait`
  lint crates. The work delivered so far for roadmap items 6.2.x and 6.3.x
  lives in `common` as pure data structures, evaluation logic, and BDD-tested
  contracts. 6.4.1 should follow that precedent.
- Current decomposition advice in
  `common/src/brain_type_metrics/diagnostic.rs` and
  `common/src/brain_trait_metrics/diagnostic.rs` is static text only. There is
  no existing feature-vector or clustering implementation to extend.
- The workspace already standardizes on indexed `#[scenario(..., index = N)]`
  bindings and fixture-backed worlds in `common/tests/`. Reusing that pattern
  will keep the new behaviour tests consistent with 6.2.1, 6.3.1, and 6.3.2.
- Recent project history shows recurring Clippy friction around
  `too_many_arguments` and `expect_used` in integration tests. The new BDD
  harness should use helper structs, `Result`-returning steps where useful, and
  no `.expect()` in `tests/`.
- Workspace lint policy also denies `float_arithmetic`, which made the
  original floating-point vector plan a poor fit. The implementation uses
  integer weights and compares cosine thresholds via cross-multiplication
  instead.
- During targeted BDD reruns, changing only the `.feature` file was not always
  enough to rebuild the behaviour binary. Touching the `.rs` harness source was
  sufficient to force recompilation when iterating on scenarios.
- The final label precedence means strong keyword communities can legitimately
  outrank local-type names. In practice that produced stable labels such as
  `report` and `summary` instead of `ReportState` and `SummaryState`.

## Decision Log

- Decision: place the new logic in a shared module under `common/`, not inside
  `brain_type_metrics` or `brain_trait_metrics`. Rationale: the decomposition
  analysis is conceptually shared by both lints and follows the same
  compiler-independent split used by roadmap 6.2.1 and 6.3.1. Date/Author:
  2026-03-06 / Codex.
- Decision: treat 6.4.1 as "produce structured suggestions" rather than
  "render diagnostic notes". Rationale: roadmap 6.4.2 already owns the
  diagnostic-emission work, so 6.4.1 should stop at reusable analysis output.
  Date/Author: 2026-03-06 / Codex.
- Decision: plan around deterministic weighted label propagation as the first
  implementation choice. Rationale: it is a community-detection algorithm,
  requires no new dependency, and is far easier to make deterministic and
  testable than a full Louvain/Leiden implementation. Final algorithm choice
  must still be recorded in the design doc during implementation. Date/Author:
  2026-03-06 / Codex.
- Decision: implement feature vectors with integer weights and an integer-only
  cosine-threshold comparison. Rationale: this satisfies the intended scoring
  model while conforming to the workspace `float_arithmetic` lint and keeping
  deterministic ordering simple. Date/Author: 2026-03-07 / Codex.
- Decision: suppress advice unless clustering yields at least two non-singleton
  communities. Rationale: a single cohesive cluster or one cluster plus
  singleton noise does not justify decomposition advice and would create noisy
  diagnostics for roadmap 6.4.2. Date/Author: 2026-03-07 / Codex.
- Decision: final label precedence is domain -> field -> keyword -> signature
  type -> local type. Rationale: external domains and shared state are stronger
  responsibility signals than incidental local helper types, while keywords
  remain useful when only name-level intent is shared. Date/Author: 2026-03-07
  / Codex.

## Context and orientation

### Repository state

The project is a Rust workspace. Shared, compiler-independent logic lives in
`common/`. The existing brain-trust roadmap work is split as follows:

- `common/src/brain_type_metrics/` contains type metric collection,
  threshold evaluation, and diagnostic formatting for roadmap 6.2.1 and 6.2.2.
- `common/src/brain_trait_metrics/` contains trait metric collection,
  threshold evaluation, and diagnostic formatting for roadmap 6.3.1 and 6.3.2.
- `common/tests/` contains the corresponding BDD harnesses and feature files.

The current diagnostic help functions only return generic decomposition text:

- `common/src/brain_type_metrics/diagnostic.rs`
- `common/src/brain_trait_metrics/diagnostic.rs`

No reusable method-profile or clustering module exists yet.

### Design requirements from `docs/brain-trust-lints-design.md`

The "Decomposition advice" section requires the following pipeline:

1. Build a feature vector per method from accessed fields, types used in
   signatures or locals, external domains, and method-name keywords.
2. Build a similarity graph using cosine similarity between method vectors.
3. Apply community detection to group related methods.
4. Label each cluster using common fields, domains, and keywords.
5. Generate decomposition suggestions from those labelled clusters.

The same design section says advice should be concise and meaningful, and
should be omitted when clustering does not produce useful groups. That is
important for 6.4.1 because the analysis result must be able to distinguish
"real community found" from "do not emit advice".

### Relationship to prior roadmap items

- Roadmap 6.2.1 established the pattern of pure metric containers and builders
  in `common`, populated later by compiler-aware callers.
- Roadmap 6.3.1 did the same for trait metrics, including macro-filtering and
  BDD coverage.
- Roadmap 6.4.1 should mirror that split: `common` owns the feature-vector,
  graph, and clustering logic; future lint-driver or diagnostic code will own
  HIR extraction and message rendering.

## Proposed implementation shape

Create a new shared module directory:

- `common/src/decomposition_advice/mod.rs`
- `common/src/decomposition_advice/profile.rs`
- `common/src/decomposition_advice/vector.rs`
- `common/src/decomposition_advice/community.rs`
- `common/src/decomposition_advice/suggestion.rs`
- `common/src/decomposition_advice/tests.rs`

The exact split may vary to stay under the 400-line limit, but the public API
should look roughly like this:

```rust
pub enum SubjectKind {
    Type,
    Trait,
}

pub enum SuggestedExtractionKind {
    HelperStruct,
    Module,
    SubTrait,
}

pub struct DecompositionContext {
    subject_name: String,
    subject_kind: SubjectKind,
}

pub struct MethodProfile {
    name: String,
    accessed_fields: BTreeSet<String>,
    signature_types: BTreeSet<String>,
    local_types: BTreeSet<String>,
    external_domains: BTreeSet<String>,
}

pub struct DecompositionSuggestion {
    label: String,
    extraction_kind: SuggestedExtractionKind,
    methods: Vec<String>,
    rationale: Vec<String>,
}

pub fn suggest_decomposition(
    context: &DecompositionContext,
    methods: &[MethodProfile],
) -> Vec<DecompositionSuggestion>;
```

Implementation notes:

- `MethodProfile` should stay compiler-independent and accept pre-normalized
  strings from callers.
- Method-name keywords should be derived inside the module rather than passed
  in, so all callers get the same tokenization and stop-word behaviour.
- `suggest_decomposition()` should return an empty vector when there are too
  few methods, no meaningful graph edges, or only singleton communities.
- The module should own ordering rules so the caller does not have to sort
  suggestions after the fact.

## Plan of work

### Stage B: Write failing tests first (red)

Add tests that define the contract before implementing the algorithm.

Files to add:

- `common/src/decomposition_advice/tests.rs`
- `common/tests/features/decomposition_advice.feature`
- `common/tests/decomposition_advice_behaviour.rs`

Unit-test coverage matrix:

1. Keyword extraction splits `snake_case` and `camelCase`, lowercases tokens,
   and removes stop words.
2. Feature vectors prefix feature categories so `field:id` cannot collide with
   `keyword:id`.
3. Cosine similarity is zero for disjoint profiles and positive for related
   profiles.
4. Graph construction drops self-edges and prunes edges below the similarity
   threshold.
5. Community detection yields deterministic clusters regardless of input order.
6. Singleton-only or disconnected graphs produce no suggestions.
7. Cluster labels prefer external domains first, then fields, then keywords,
   then type names.
8. Suggested extraction kind follows deterministic rules:
   `Type` defaults to `HelperStruct`, domain-heavy groups can become `Module`,
   and `Trait` defaults to `SubTrait`.

Behaviour-test scenarios (`rstest-bdd` v0.5.0):

1. Happy path: parser, serde, and filesystem methods form three separate
   communities with stable labels.
2. Happy path: trait methods with shared signature/domain features form a
   sub-trait suggestion even without field-access features.
3. Unhappy path: a small or weakly-related method set produces no
   decomposition suggestions.
4. Edge path: noisy verbs and input order changes do not alter community
   membership.
5. Edge path: singleton noise methods are excluded from the returned
   suggestions.

The BDD world should store synthetic `MethodProfile` instances and assert the
resulting `DecompositionSuggestion` objects. Keep step functions within the
workspace argument limit by using helper structs rather than many scalar
parameters.

### Stage C: Implement the shared decomposition module

Implement the module in `common/src/decomposition_advice/`.

#### Stage C1: Method profiles and feature vectors

Create `MethodProfile` and `DecompositionContext`, plus helper functions for:

- splitting method names into keywords,
- dropping stop words such as `get`, `set`, `run`, `handle`, `process`,
  `update`, and `build`,
- normalizing features into prefixed sparse keys such as `field:tokens`,
  `domain:serde::de`, `sig:AstNode`, `local:PathBuf`, and `keyword:parse`,
- weighting features so structural/domain signals dominate keywords.

Use a sparse vector representation such as `BTreeMap<String, u64>` for
deterministic iteration and integer-only cosine-similarity testing.

Initial weighting heuristic:

- `field:*` -> `6`
- `domain:*` -> `5`
- `sig:*` -> `4`
- `local:*` -> `3`
- `keyword:*` -> `2`

These weights may be tuned during implementation, but the final values must be
recorded in the design doc and reflected in tests.

#### Stage C2: Similarity graph

Build a weighted undirected graph in plain Rust collections.

Algorithm:

```plaintext
for each method pair (i, j) where i < j:
  score = cosine_similarity(vector[i], vector[j])
  if score >= MIN_EDGE_WEIGHT:
    add symmetric edge with weight = score
```

Use a deterministic threshold such as `MIN_EDGE_WEIGHT = 0.20` for the initial
implementation. If the prototype shows that threshold failing the synthetic
fixtures, refine it once and record the final value in the design doc.

#### Stage C3: Community detection

Use deterministic weighted label propagation as the first implementation.

Suggested procedure:

```plaintext
assign each node its own label
repeat until stable or max_iterations reached:
  visit nodes in lexical method-name order
  for each node:
    score neighbouring labels by summed edge weight
    choose the label with the highest score
    break ties by lexical label order
group nodes by final label
drop singleton groups
```

This is sufficient for 6.4.1 because the requirement is community detection,
not a specific named library or modularity optimizer. If this approach cannot
produce stable, meaningful communities for the synthetic fixtures, escalate
before reaching for a new dependency.

#### Stage C4: Cluster labelling and suggestions

For each non-singleton community:

1. Collect recurring features from member vectors.
2. Choose a label from the strongest recurring feature with this precedence:
   external domain -> field -> keyword -> signature type -> local type.
3. Infer `SuggestedExtractionKind`:
   - `Trait` subject -> `SubTrait` by default.
   - `Type` subject with a dominant external domain -> `Module`.
   - Other `Type` subjects -> `HelperStruct`.
4. Build a deterministic `DecompositionSuggestion` with:
   - stable label,
   - sorted method names,
   - short rationale list of dominant features.

Return suggestions sorted by descending community size, then by label.

### Stage D: Wire exports

Update `common/src/lib.rs` to:

- add `pub mod decomposition_advice;`
- re-export the new public types and `suggest_decomposition()`

Do not modify the existing `brain_type_metrics` or `brain_trait_metrics`
formatters in this roadmap item. 6.4.2 will consume the shared suggestions and
decide how to surface them in diagnostics.

### Stage E: Green and refactor

Make the red tests pass, then refactor without changing behaviour.

Refactoring guardrails:

- keep pure helpers small and well named,
- prefer value objects over long parameter lists,
- keep clustering logic free of hidden global state,
- split files before they approach 400 lines,
- add succinct comments only where the algorithm would otherwise be opaque.

### Stage F: Record design decisions

Update `docs/brain-trust-lints-design.md` with a new subsection:

- `### Implementation decisions (6.4.1)`

Record at least:

1. the final public data model for method profiles and suggestions,
2. the chosen community-detection algorithm and why it was selected,
3. the final feature weights and edge threshold,
4. the stop-word and tokenization policy,
5. the rules for labelling communities and choosing extraction kinds,
6. when the analysis returns no suggestion at all.

### Stage G: Mark roadmap complete

After implementation and successful validation, update `docs/roadmap.md`:

```markdown
- [x] 6.4.1. Build feature vectors for methods and cluster with community
  detection to form decomposition suggestions. See
  [brain trust lints design](brain-trust-lints-design.md) §Decomposition
  advice. Requires 6.2.1.
```

Do not update this checkbox during the draft-planning turn.

### Stage H: Validate

Because the implementation will change Rust code and documentation, run all of
the following with `set -o pipefail` and `tee` to capture full logs:

```sh
make fmt 2>&1 | tee /tmp/6-4-1-fmt.log
make markdownlint 2>&1 | tee /tmp/6-4-1-markdownlint.log
make nixie 2>&1 | tee /tmp/6-4-1-nixie.log
make check-fmt 2>&1 | tee /tmp/6-4-1-check-fmt.log
make lint 2>&1 | tee /tmp/6-4-1-lint.log
make test 2>&1 | tee /tmp/6-4-1-test.log
```

Expected implementation-time outcomes:

- the new unit tests fail before implementation and pass afterwards,
- the new BDD scenarios fail before implementation and pass afterwards
- `make check-fmt`, `make lint`, and `make test` all finish successfully
- no existing brain-metric tests regress.

## Outcomes & Retrospective

Roadmap item 6.4.1 is now implemented. `common` exports a new
`decomposition_advice` module with:

- `MethodProfile` and `MethodProfileBuilder` for compiler-independent
  per-method metadata,
- sparse feature-vector construction over fields, domains, signature types,
  local types, and derived keywords,
- deterministic similarity-graph construction and weighted label-propagation
  community detection,
- `DecompositionSuggestion` values with stable labels, extraction kinds, and
  rationales.

Validation now covers both unit and behavioural contracts:

- unit tests exercise tokenization, vector construction, similarity edges,
  order-invariant community detection, suppression of weak decompositions, and
  extraction-kind selection,
- `rstest-bdd` scenarios cover happy paths for type and trait decomposition,
  unhappy paths with no meaningful split, singleton-noise suppression, and
  stable clustering under reordered input.

The roadmap entry is marked done, and the design document records the final
algorithm and heuristics actually shipped. This keeps roadmap 6.4.2 focused on
diagnostic presentation only; it can consume the structured suggestions without
needing to revisit clustering behaviour.
