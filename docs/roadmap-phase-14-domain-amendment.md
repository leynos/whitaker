# Phase 14 DOMAIN roadmap amendment

This supplement normatively supersedes conflicting Phase 14 task wording in
`roadmap.md` and implements the expanded
[RFC 0004 amendment](rfcs/0004-state-machine-adt-amendment.md). Non-conflicting
roadmap work remains applicable.

## 14. Validate not, only parse domain-modelling lints

### 14.1. Family registration and shared evidence model

- [ ] 14.1.1. Reserve the `DOMAIN` rule family: `DOMAIN001` to `DOMAIN099`
  for boundary refinement, `DOMAIN101` to `DOMAIN149` for type state spaces,
  `DOMAIN150` to `DOMAIN179` for local protocol surfaces, and `DOMAIN900` to
  `DOMAIN999` for grouped reports. Integrate the family with experimental
  selector semantics. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Rule-code allocation.
  Requires 3.6.1.
- [ ] 14.1.2. Add `common::domain_model` with compiler-independent boundary,
  witness-disposition, source-use, finite-dimension, relation, partition,
  projection, construction-surface, protocol, and evidence-strength models.
  See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Shared analysis architecture
  and [RFC 0004](rfcs/0004-fragile-types.md) §Shared state-space analysis.
  Requires 1.1.1.
- [ ] 14.1.3. Add resolved definition-path classifiers for `FromStr`,
  `str::parse`, `TryFrom`, `TryInto`, `NonZero*::new`, supported sentinel
  predicates, and configured custom refiners or wrappers. Include strict path
  validation and bounded warnings for malformed exclusions. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Initial refiner vocabulary
  and [RFC 0004](rfcs/0004-fragile-types.md) §Supported sentinel vocabulary.
  Requires 14.1.2.
- [ ] 14.1.4. Add local HIR use, producer, consumer, transition, and projection
  summaries plus narrow MIR confirmation for success-edge reachability, source
  mutation, alias escape, closed producer sets, receiver identity, dominance,
  and retained witnesses. Reuse ownership-shape MIR contracts where they
  provide the required place-equivalence facts. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §HIR prefilter and §MIR
  confirmation and [RFC 0004](rfcs/0004-fragile-types.md) §HIR and MIR
  responsibilities. Requires 9.1.4 and 14.1.2.
- [ ] 14.1.5. Define a deterministic finding-record schema for crate-post and
  workspace aggregation, including rule code, item identity, source and target
  types, finite dimensions, normalized relation or refinement fingerprint,
  evidence strength, protocol method identities, and source spans. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Crate-post aggregation and
  [RFC 0004](rfcs/0004-fragile-types.md) §Diagnostic precedence and
  deduplication. Requires 14.1.2.

### 14.2. Weak-representation boundary rules

- [ ] 14.2.1. Create the `discarded_parsed_value` lint crate and implement the
  direct discarded-witness forms for `?` and explicit matches, including
  resolved source and witness types plus inward-use labels. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Proposed rule:
  `discarded_parsed_value`. Requires 2.1.1, 14.1.2, and 14.1.3.
- [ ] 14.2.2. Add mutation, alias, parser-probe, source-retention, raw-boundary,
  test, and generated-code controls to `discarded_parsed_value`. Emit
  machine-applicable suggestions only for proven local binding rewrites. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Use classification,
  §Suggestions, and §Exemptions and false-positive controls. Requires 14.1.4
  and 14.2.1.
- [ ] 14.2.3. Add pure, property, behaviour-driven, UI, and localized smoke
  coverage for `discarded_parsed_value`, including raw and parsed values carried
  together. See [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Testing
  requirements. Requires 1.2.1, 2.3.4, and 14.2.2.
- [ ] 14.2.4. Create `validation_without_refinement` at `allow`, summarize only
  private same-crate intrinsic validators with no unknown effects, and require
  success-path use of the unchanged weak value. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Proposed rule:
  `validation_without_refinement`. Requires 14.1.4 and 14.2.3.
- [ ] 14.2.5. Implement `repeated_boundary_refinement` as a grouped
  `whitaker check` advisory report using normalized parser and predicate
  fingerprints. See [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Deferred
  report: `repeated_boundary_refinement`. Requires 3.5.2, 14.1.5, and 14.2.4.

### 14.3. Fragile-type and state-space rules

- [ ] 14.3.1. Implement field-role, finite-dimension, allowed-row, semantic
  partition, domain-projection, construction-surface, and evidence-strength
  collection in `common::domain_model::state_space`. Add bounded exhaustive
  tests for presence, Boolean, and sentinel dimensions up to four fields. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Shared state-space analysis and
  §Testing requirements. Requires 14.1.2.
- [ ] 14.3.2. Create the `manual_tagged_union` lint crate and detect exhaustive
  or closed-producer selector-to-payload mappings, including one-payload sums,
  unit variants, lockstep transitions, and common metadata outside the
  candidate payload group. Use mdtablefix #443 as a release-blocking fixture.
  See [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `manual_tagged_union`. Requires 2.1.1, 14.1.4, and 14.3.1.
- [ ] 14.3.3. Create `correlated_optional_fields` at `allow`; recognize
  option-only and mixed option/Boolean or option/tag exactly-one, at-most-one,
  all-or-none, and implication relationships. Require one exhaustive truth
  table or two independent evidence sites, and retain mdtablefix #448 as a
  positive fixture. See [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `correlated_optional_fields`. Requires 14.3.1 and 14.3.2.
- [ ] 14.3.4. Trial Clippy's `struct_excessive_bools` against the target corpus,
  then implement `constrained_boolean_state_space` for residual cases whose
  produced or accepted Boolean rows form a proven proper subset. Support
  arbitrary truth tables rather than only one-hot or mutual-exclusion sets,
  with mdtablefix #446 as a release-blocking fixture. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `constrained_boolean_state_space`. Requires 14.1.4 and 14.3.1.
- [ ] 14.3.5. Create `sentinel_state_encoding` at `allow`; recognize supported
  empty/non-empty, zero/non-zero, option-presence, and configured closed
  sentinel predicates only when coordinated producers or rejecting consumers
  establish a type-level state relation. Retain mdtablefix #445 as a positive
  fixture. See [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `sentinel_state_encoding`. Requires 14.1.3, 14.1.4, and 14.3.1.
- [ ] 14.3.6. Create `redundant_state_dimension` at `allow`; detect closed
  semantic partitions where one field dimension is masked under part of the
  state space and recommend variant-specific data. Retain mdtablefix #449 as a
  positive fixture and suppress unknown-effect or one-off branch masking. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `redundant_state_dimension`. Requires 14.1.4 and 14.3.1.
- [ ] 14.3.7. Create `primitive_reencoded_as_domain_type` at experimental
  `warn`; detect total, pure projections from persistent primitives into an
  already-existing local enum or newtype when behavioural consumers use only
  the projected form. Retain mdtablefix #447 as a release-blocking fixture and
  suppress raw or externally imposed representations. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `primitive_reencoded_as_domain_type`. Requires 14.1.4 and 14.3.1.
- [ ] 14.3.8. Create `bypassable_type_invariant` at `allow`, combining
  intrinsic invariant summaries with effectively visible public fields,
  unchecked constructors, independent setters, mutable inner access, and
  deserialization paths. Add builder, raw-model, FFI, runtime-resource, and
  external-contract suppressions. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed rule:
  `bypassable_type_invariant`. Requires 14.1.4 and 14.3.1.
- [ ] 14.3.9. Implement `invalid_default` as a report-only symbolic evaluator
  for direct constants and known standard defaults against supported intrinsic
  constraints. Unknown facts must suppress rather than approximate. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed report:
  `invalid_default`. Requires 14.3.1 and 14.3.8.

### 14.4. Local protocol-shape rules

- [ ] 14.4.1. Add `common::domain_model::protocol` summaries for terminal and
  predecessor methods, receiver-place identity, call-site dominance, state
  transfer, unknown external calls, and protocol evidence strength. Keep this
  model separate from finite state-space evaluation. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Shared state-space analysis.
  Requires 14.1.4.
- [ ] 14.4.2. Create `unguarded_terminal_transition` as a report-only or
  `allow` rule. Require a consuming or state-exposing terminal method, a
  predecessor that dominates every visible call on the same receiver, and a
  direct flow from predecessor mutations into terminal output. Suppress public
  methods with unknown external callers and optional preparation calls. See
  [RFC 0004](rfcs/0004-fragile-types.md) §Proposed protocol-shape rule:
  `unguarded_terminal_transition`. Requires 14.4.1.
- [ ] 14.4.3. Add MIR, UI, integration, and mutation coverage for branch
  dominance, receiver replacement, aliases, fallible predecessors, RAII
  cleanup, optional preparation, and the mdtablefix #444 flush-before-consume
  fixture. Requires 1.2.1 and 14.4.2.

### 14.5. Integration, documentation, and promotion

- [ ] 14.5.1. Add implemented `DOMAIN` rules to the experimental suite with
  independent feature gates, stable rule metadata, and diagnostic precedence
  that emits one primary finding per correlated field group or protocol pair.
  See [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Category, not monolith
  and [RFC 0004](rfcs/0004-fragile-types.md) §Diagnostic precedence and
  deduplication. Requires 14.2.3 and the corresponding completed rule tasks.
- [ ] 14.5.2. Add English, Welsh, and Gaelic Fluent entries and structured
  diagnostic arguments for every implemented rule. See
  [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Diagnostics and localization
  and [RFC 0004](rfcs/0004-fragile-types.md) §Diagnostics and localization.
  Requires 2.3.4 and 14.5.1.
- [ ] 14.5.3. Update the users' and developers' guides with family selection,
  intrinsic-versus-extrinsic validity, source-preserving parsed values,
  finite-state and sentinel evidence, builder/raw-type exemptions, local
  protocol guidance, suppression policy, and staged migration examples.
  Requires 14.5.1.
- [ ] 14.5.4. Run the family over representative df12 and third-party corpora,
  including source-faithful fixtures for mdtablefix #443 to #449. Classify
  findings by the RFC exception models, publish the false-positive budget, and
  promote each rule independently. `DOMAIN001`, `DOMAIN101`, and `DOMAIN106`
  should receive the first promotion decisions; `DOMAIN107`, `DOMAIN108`, and
  `DOMAIN151` remain experimental or report-only until their proof boundaries
  are stable. See [RFC 0003](rfcs/0003-weak-domain-boundaries.md) §Downstream
  corpus and [RFC 0004](rfcs/0004-fragile-types.md) §Downstream corpus.
  Requires 14.5.2 and 14.5.3.
