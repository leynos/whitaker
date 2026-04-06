# Brain trust lints design for `brain_type` and `brain_trait`

## Purpose and scope

This design introduces two Whitaker lints that detect brain-like constructs in
Rust code:

- `brain_type` flags nominal types that have grown into overly complex,
  low-cohesion "brain" types.
- `brain_trait` flags traits that are too large or too behaviour-heavy to serve
  as a single, coherent interface.

The lints share a cohesion analysis implementation and provide decomposition
advice directly in diagnostic output. They are configurable and can emit
Structured Analysis Results Interchange Format (SARIF) output for tooling
integration.

Non-goals:

- Semantic clone or architectural analysis beyond local type and trait scopes.
- Whole-program data flow or code property graph analysis.
- Cross-crate or cross-language analysis.

## Background: the brain class smell in object-oriented code and Rust

Object-oriented literature (notably Lanza and Marinescu) describes a "brain
class" as a large, complex class that centralizes too much intelligence. Common
signals include:[^brain-class]

- Many methods and high overall complexity (Weighted Methods per Class (WMC)).
- At least one "brain method" that dominates the class's behaviour.
- Low cohesion (for example, high Lack of Cohesion in Methods (LCOM4) or low
  Tight Class Cohesion (TCC)).
- Significant access to foreign data (Access to Foreign Data (ATFD)).

Tools such as CodeScene combine these metrics to detect brain
classes.[^codescene] A brain class violates separation of concerns and becomes
difficult to maintain as responsibilities accumulate.

Rust does not have classes, but similar patterns emerge:

- Nominal types (structs and enums) with inherent and trait methods can grow
  into large, low-cohesion units.
- Traits can become "god interfaces" by accreting many responsibilities or
  by hosting complex default implementations.

## Lint overview

### `brain_type`

The unit of analysis is a nominal type plus all its methods defined in the
current crate:

- The type definition and all inherent `impl` blocks.
- All trait implementation methods for that type in the crate.

External crate methods are excluded. Each method contributes to complexity and
cohesion metrics.

### `brain_trait`

The unit of analysis is a single trait definition:

- Required methods and associated items form the interface size signal.
- Default method implementations contribute to complexity and cohesion signals.

An umbrella alias such as `brain_trust` was considered, but keeping distinct
lints keeps configuration clear while still allowing shared documentation.

## Metrics and heuristics

All metrics are configurable through `whitaker.toml`, and diagnostics report
measured values to justify findings.

### `brain_type` signals

- **Weighted Methods Count (WMC) using cognitive complexity**: sum the
  Cognitive Complexity (CC) of all methods in the type. We align the default
  brain method threshold with Clippy's CC threshold of 25.[^clippy-cognitive]
- **Brain method presence**: a method with CC >= 25 and at least 80 lines of
  code (LOC) flags a "brain method". Both thresholds are configurable.
- **Lack of cohesion (LCOM4)**: build a method graph and count connected
  components. LCOM4 >= 2 indicates low cohesion.
- **Foreign reach (ATFD analogue)**: count distinct external modules or types
  referenced by the type's methods. A high count indicates a wide domain reach.
- **Rust-specific boosters** (do not trigger alone):
  - Interior mutability fields (`Mutex`, `RwLock`, `RefCell`).
  - Heavy use of `async` methods coordinating input/output (I/O).
  - A very large public API surface (high `pub` method ratio).

### `brain_type` rule set (initial defaults)

- **Warn** when WMC is high (>= 60), at least one brain method is present, and
  LCOM4 >= 2 (or TCC <= 0.33).
- **Escalate** (configurable) when WMC >= 100, multiple brain methods exist, or
  cohesion is extremely low.

These defaults are intended as sensible starting points, not absolutes.

### `brain_trait` signals

Traits do not have fields, so cohesion and complexity focus on the interface
and default implementations:

- **Interface size**: total trait items, with a focus on required methods.
- **Default method complexity**: sum CC for default method bodies.
- **Implementor burden**: number of required methods indicates how much work
  each implementor must do.

### `brain_trait` rule set (initial defaults)

- **Warn** when a trait has at least 20 methods and default method complexity
  is at least 40.
- **Escalate** when a trait has 30 or more methods regardless of complexity.

## Cohesion analysis (LCOM4)

LCOM4 is computed by modelling each method as a node in a graph and adding
edges when methods are related. For `brain_type`, relationships include:

- Shared field access (two methods read or write the same field).
- Direct method calls between methods on the same type.

LCOM4 is the number of connected components in this graph. LCOM4 == 1 indicates
high cohesion, while LCOM4 >= 2 suggests multiple unrelated responsibilities.

We will provide a shared helper in the common crate, for example:

```rust
pub fn cohesion_components(methods: &[MethodInfo]) -> usize { /* ... */ }
```

This will allow reuse in future cohesion-aware lints.

### Implementation decisions (6.1.1)

- **Data representation**: `MethodInfo` carries the method name, a
  `BTreeSet<String>` of accessed field names, and a `BTreeSet<String>` of
  called method names. `BTreeSet` is chosen over `HashSet` for deterministic
  iteration and trivial `Eq`/`Ord` derivation.
- **Graph algorithm**: connected components are counted using an inline
  union-find with path compression and union-by-rank. This is O(n α(n))
  amortized and requires no external dependency.
- **Edge semantics**: two methods are connected when they share at least one
  field name in their `accessed_fields` sets, or when one method's
  `called_methods` set contains the other's name. Calls to methods not present
  in the input slice are silently ignored.
- **Empty input**: an empty method slice returns 0 components, which callers
  may treat as "not applicable" rather than "cohesive".
- **No validation errors**: `cohesion_components` is infallible. All string
  inputs are valid; the function cannot fail on well-typed data.

### Implementation decisions (6.1.2)

- **Extraction as pure builder, not HIR visitor**: the extraction module
  (`common/src/lcom4/extract.rs`) provides a `MethodInfoBuilder` that
  accumulates field accesses and method calls without depending on
  `rustc_private`. Lint drivers walk HIR bodies and feed data into the builder,
  following the same pattern as `complexity_signal` (pure library) and
  `bumpy_road_function` (HIR walker). This keeps the `common` crate free of
  compiler dependencies and fully testable without a compilation context.
- **Macro-span filtering via boolean parameter**: rather than importing
  `rustc_span::Span` and calling `from_expansion()` inside `common`, the
  builder's `record_field_access` and `record_method_call` methods accept an
  `is_from_expansion: bool` parameter. The caller (the HIR walker) passes the
  result of `expr.span.from_expansion()`. Entries where `is_from_expansion` is
  `true` are silently discarded. This mirrors the approach in
  `bumpy_road_function`'s `SegmentBuilder`, where `span.from_expansion()` is
  checked before creating a `LineSegment`.
- **Builder pattern over constructor**: `MethodInfoBuilder` uses a mutable
  builder rather than requiring the caller to pre-compute `BTreeSet` values.
  This is more ergonomic for HIR walkers that discover fields and calls
  incrementally during traversal, and avoids intermediate collection
  allocations in the caller.

### Implementation decisions (6.2.1)

- **Separate `brain_type_metrics` module**: metric collection for brain type
  detection lives in `common/src/brain_type_metrics/` rather than extending the
  `complexity_signal` module. The `complexity_signal` module provides per-line
  signal rasterization and smoothing for the bumpy road lint, whereas brain
  type metrics operate at the per-method aggregate level — a fundamentally
  different abstraction. Keeping them separate maintains single responsibility.
- **`MethodMetrics` stores pre-computed values**: `MethodMetrics` carries
  `cognitive_complexity: usize` and `lines_of_code: usize` as pre-computed
  values rather than computing cognitive complexity (CC) from source. The
  `common` crate has no `rustc_private` dependencies. The actual CC computation
  from HIR happens in the lint driver (6.2.2), which passes the pre-computed
  value into `MethodMetrics`. This follows the same pattern as
  `MethodInfoBuilder` (pure library stores and aggregates; HIR walker produces).
- **`TypeMetricsBuilder` for incremental construction**: follows the
  `MethodInfoBuilder` pattern from 6.1.2. The lint driver discovers methods
  incrementally during HIR traversal and calls `add_method()` for each. Brain
  method thresholds are provided at construction time so the builder identifies
  brain methods during `build()`. LCOM4 and foreign reach are set separately
  via `set_lcom4()` and `set_foreign_reach()`, defaulting to zero if not set.
- **`ForeignReferenceSet` with macro-span filtering**: `ForeignReferenceSet`
  accumulates distinct external module or type references using `BTreeSet` for
  deterministic iteration. Its `record_reference()` method accepts
  `is_from_expansion: bool`, mirroring the pattern in
  `MethodInfoBuilder::record_field_access()`. The HIR walker calls
  `record_reference(&path_string, span.from_expansion())`. Using plain `String`
  paths avoids coupling to `rustc_private` types.

### Implementation decisions (6.2.2)

- **Evaluation in `common`, not in the lint crate**: the threshold evaluation
  function `evaluate_brain_type()` and diagnostic formatting live in
  `common/src/brain_type_metrics/evaluation.rs`. This keeps the evaluation
  logic pure (no `rustc_private` dependency), independently testable, and
  reusable by future lints such as `brain_trait`.
- **`TypeMetrics` stores full brain method metrics**: changed from
  `Vec<String>` (names only) to `Vec<MethodMetrics>` so that diagnostic
  formatting can include per-method CC and LOC values (e.g.,
  `` `parse_all` (CC=31, LOC=140) ``). The `brain_method_names()` accessor is
  preserved with a `Vec<&str>` return type for backward compatibility.
- **`BrainTypeThresholds` uses a builder**: the struct has five threshold
  fields, exceeding the workspace Clippy `too_many_arguments` limit of four. A
  consuming-self builder (`BrainTypeThresholdsBuilder`) provides the
  construction path, following the `TypeMetricsBuilder` pattern from 6.2.1.
- **No serde in `common`**: the threshold struct does not derive `Deserialize`.
  The lint driver crate will deserialize from TOML configuration and convert
  into `BrainTypeThresholds`, following the `bumpy_road_function` Config →
  Settings conversion pattern.
- **Warn is AND-based, deny is OR-based**: the warn rule fires only when WMC,
  brain method presence, and LCOM4 all exceed their respective thresholds
  simultaneously. The deny rule fires when any single deny condition holds.
  Deny supersedes warn. This directly reflects the design document
  §`brain_type` rule set.
- **`BrainTypeDiagnostic` carries all measured values**: the diagnostic struct
  carries type name, disposition, WMC, LCOM4, foreign reach, and full brain
  method metrics. Formatting functions produce primary, note, and help strings
  matching the design document diagnostic format.

### Implementation decisions (6.2.3)

- **"Skip" strategy for macro-expanded nodes**: when `is_from_expansion`
  is `true`, the builder silently discards the increment. This follows the
  established pattern in `ForeignReferenceSet::record_reference()` and
  `MethodInfoBuilder::record_field_access()`, and aligns with Clippy issue
  #14417's guidance that macro invocations should be treated as atomic units
  akin to function calls (CC=0 contribution from expanded internals).
- **`CognitiveComplexityBuilder` in `common`**: the builder lives at
  `common/src/brain_type_metrics/cognitive_complexity.rs` with no
  `rustc_private` dependency. The HIR walker (in the future lint driver) maps
  each HIR expression to the appropriate builder call, passing
  `span.from_expansion()` as the `is_from_expansion` parameter.
- **Internal nesting depth tracking**: the builder maintains a stack
  recording whether each nesting level originates from a macro expansion.
  Effective depth (used for nesting increments) counts only non-expansion
  levels. This prevents macro-generated control flow from inflating the nesting
  penalty of subsequent real code.
- **API by increment category**: the builder exposes
  `record_structural_increment`, `record_nesting_increment`, and
  `record_fundamental_increment` rather than per-expression-type methods. This
  keeps the builder decoupled from Rust HIR node types.
- **Consuming `build()` with balance assertion**: `build()` panics if
  the nesting stack is not empty, catching mismatched `push_nesting` /
  `pop_nesting` calls at the point of use.

### Implementation decisions (6.3.1)

- **Separate `brain_trait_metrics` module**: trait metric collection lives in
  `common/src/brain_trait_metrics/mod.rs`, separate from `brain_type_metrics`.
  The two lints share conceptual ground, but their signals differ enough to
  keep modules focused and avoid overloading one API.
- **Explicit trait item taxonomy**: interface-size counting uses
  `TraitItemKind` with four variants: `RequiredMethod`, `DefaultMethod`,
  `AssociatedType`, and `AssociatedConst`. This makes counting rules explicit
  and testable.
- **Implementor burden definition**: `implementor_burden` is defined as
  exactly the required-method count. Default methods and associated items do
  not increase burden.
- **Macro filtering for default method CC**: `TraitMetricsBuilder` accepts
  `add_default_method(name, cc, is_from_expansion)`. When `is_from_expansion`
  is `true`, the entry is discarded and contributes to neither item counts nor
  complexity, matching existing macro-filtering semantics in `common`.
- **Single item struct with optional CC**: `TraitItemMetrics` stores
  `default_method_cc: Option<usize>` rather than splitting default and
  non-default items into separate structs. This keeps helper functions simple
  while preserving exact default-method complexity data.

### Implementation decisions (6.3.2)

- **"Methods" threshold counts methods only, not all items**: the "at least 20
  methods" threshold uses `required_method_count() + default_method_count()`
  and explicitly excludes associated types and associated consts. The
  `total_item_count()` accessor includes non-method items and is not used for
  threshold comparison. This aligns with the design document's use of "methods"
  rather than "items".
- **Evaluation in `common`, not in the lint crate**: the threshold evaluation
  function `evaluate_brain_trait()` and diagnostic formatting live in
  `common/src/brain_trait_metrics/evaluation.rs` and
  `common/src/brain_trait_metrics/diagnostic.rs`. This keeps the evaluation
  logic pure (no `rustc_private` dependency), independently testable, and
  reusable, following the pattern established by `brain_type` in 6.2.2.
- **Warn is AND-based, deny is OR-based**: the warn rule fires only when total
  method count >= `methods_warn` AND default method CC sum >= `default_cc_warn`
  simultaneously. The deny rule fires when total method count >=
  `methods_deny`, regardless of complexity. Deny supersedes warn.
- **`BrainTraitDiagnostic` carries all measured values**: the diagnostic struct
  carries trait name, disposition, required method count, default method count,
  default method CC sum, total item count, and implementor burden. Formatting
  functions produce primary, note, and help strings surfacing measured values.
- **`BrainTraitThresholds` uses a builder**: although only 3 fields (under the
  Clippy limit), a builder is used for consistency with `BrainTypeThresholds`
  and future extensibility.

### Implementation decisions (6.4.1)

- **Shared `decomposition_advice` module in `common`**: decomposition analysis
  lives in `common/src/decomposition_advice/` rather than under
  `brain_type_metrics` or `brain_trait_metrics`. The API is compiler-
  independent and reuses the same pure-library split established by roadmap
  6.2.1 and 6.3.1.
- **Integer-weighted sparse feature vectors**: `MethodProfile` records accessed
  fields, signature types, local types, and external domains as `BTreeSet`
  values. Feature vectors are sparse `BTreeMap<String, u64>` collections with
  category-prefixed keys and integer weights: field = 6, domain = 5, signature
  type = 4, local type = 3, keyword = 2. Integer weights were chosen to avoid
  workspace `float_arithmetic` lint friction while keeping deterministic
  scoring.
- **Cosine threshold by integer cross-multiplication**: the similarity graph
  uses cosine similarity with a threshold of 0.20, represented as `1 / 25`.
  Rather than computing floating-point square roots, the implementation
  compares squared dot products using integer cross-multiplication.
- **Deterministic weighted label propagation**: the clustering step uses a
  deterministic weighted label-propagation pass instead of Louvain or Leiden.
  Nodes are visited in lexical method-name order, neighbouring labels are
  scored by summed edge weight, and ties break lexically. This satisfies the
  community-detection requirement without adding a graph dependency.
- **Keyword extraction and stop-word policy**: method names are split across
  snake_case, camelCase, and punctuation boundaries, lowercased, and filtered
  through a fixed stop-word list: `build`, `create`, `do`, `get`, `handle`,
  `make`, `process`, `render`, `run`, `set`, and `update`. This keeps common
  orchestration verbs from dominating the communities.
- **Suggestion suppression for weak decompositions**: decomposition analysis
  returns no suggestions unless at least two non-singleton communities remain
  after clustering. Singleton noise methods are dropped, so diagnostics can
  later omit advice when no meaningful split exists.
- **Label and extraction-kind rules**: community labels prefer external domain
  features first, then fields, keywords, signature types, and local types.
  Suggested extraction kinds follow fixed rules: trait subjects produce
  `SubTrait`; type subjects with domain-led labels produce `Module`; all other
  type subjects produce `HelperStruct`.

## Implementation approach

### Metric collection

- **Cognitive complexity**: reuse an existing algorithm consistent with
  SonarSource rules, or integrate a suitable library. The `rust-code-analysis`
  crate provides CC metrics but may be heavy for lint
  execution.[^rust-code-analysis]
- **Line counts**: compute LOC from spans using `SourceMap`, similar to
  `module_max_lines`.[^module-max-lines]
- **Macro expansions**: avoid inflating CC with macro-generated HIR. When spans
  originate from macros, skip or cap complexity counts, following Clippy
  guidance.[^clippy-issue]

### Performance considerations

Deep analysis is only performed after lightweight thresholds are crossed (for
example, when a type exceeds a minimum method count). This avoids paying the
full analysis cost for trivial types.

### Graph construction

Whitaker does not maintain a global call graph for lints. We build a local
graph per type using `rustc_hir` and discard it after computing metrics, which
keeps the analysis scoped and performant. The approach aligns with existing
Whitaker lints that perform local HIR traversal.[^whitaker-design]

## Diagnostic output and developer guidance

Diagnostics include quantified metrics and clear explanations. For example:

```plaintext
brain_type: `Foo` has WMC=118, LCOM4=3, and a brain method `parse_all`
(CC=31, LOC=140).
```

Notes explain why the metrics matter and how they map to the brain class smell.
Messages are localized via Fluent entries, using the existing Whitaker tone.

### Decomposition advice

When a brain type or trait is detected, the lint produces decomposition advice
based on method clustering implemented in `common/src/decomposition_advice/`.
The analysis uses one integer-weighted sparse feature vector per method,
represented as `BTreeMap<String, u64>` with category-prefixed keys:

- `field:*` with weight `6`
- `domain:*` with weight `5`
- `sig:*` with weight `4`
- `local:*` with weight `3`
- `keyword:*` with weight `2`

The method metadata comes from accessed fields, signature types, local types,
external domains, and method-name keywords. Keyword extraction lowercases
tokens split from `snake_case`, `camelCase`, acronyms, and punctuation, then
removes the fixed stop-word list: `build`, `create`, `do`, `get`, `handle`,
`make`, `process`, `render`, `run`, `set`, and `update`.

The shipped clustering pipeline is:

- Build a similarity graph by comparing feature vectors with cosine similarity
  using integer cross-multiplication against threshold `1/25`. The
  implementation avoids floating-point square roots and floating-point vector
  weights entirely.
- Apply deterministic weighted label-propagation to group related methods.
  Nodes are visited in lexical method-name order, neighbouring labels are
  scored by summed edge weight, and ties break lexically.
- Drop singleton communities and suppress suggestions unless at least two
  non-singleton communities remain.
- Label each surviving community using the strongest shared feature with this
  precedence: external domain -> field -> keyword -> signature type -> local
  type.
- Map communities to extraction kinds with fixed rules: trait subjects produce
  `SubTrait`, domain-led type communities produce `Module`, and all other type
  communities produce `HelperStruct`.

Example note output:

```plaintext
Potential decomposition for `Foo`:
- [grammar] helper struct for `parse_nodes`, `parse_tokens`
- [serde::json] module for `decode_json`, `encode_json`
- [std::fs] module for `load_from_disk`, `save_to_disk`
```

Advice is concise and only emitted when clustering yields meaningful groups. If
the type is extremely large, the lint may cap advice length and report that
further decomposition analysis was omitted.

### Implementation decisions (6.4.2)

- **Dedicated note channel**: decomposition guidance is rendered as a separate
  diagnostic note instead of being folded into the existing metric note or help
  text. This keeps the previously shipped metric explanations stable while
  making cluster-based advice easy to spot.
- **Shared renderer in `common::decomposition_advice`**: both `brain_type` and
  `brain_trait` call the same `format_diagnostic_note()` helper, then expose
  thin per-lint wrappers. This avoids wording drift between the two lints and
  keeps the renderer free of `rustc_private`.
- **English-only multi-line template**: the current note format is
  `Potential decomposition for \`subject\`:` followed by one bullet per
  suggestion. Wording remains English-only until the later localization work
  moves it behind Fluent messages.
- **Hard caps for readability**: diagnostic notes show at most 3 suggestion
  areas and at most 3 method names per area. Hidden method names are reported
  inline as `+N more methods`, and hidden suggestion areas are reported on a
  trailing line as `N more areas omitted`.
- **Emit only for non-empty suggestions**: the renderer returns no note when
  clustering yields no surviving decomposition suggestions, so diagnostics stay
  quiet for weakly related subjects.

### Implementation decisions (6.4.3)

- **Verus proof workflow lives in a sidecar**: roadmap 6.4.3 adds
  `verus/decomposition_cosine_threshold.rs` plus shell wrappers at
  `scripts/install-verus.sh` and `scripts/run-verus.sh`. The `Makefile` exposes
  this as `make verus`, keeping proof tooling outside the Cargo workspace build
  path.
- **Pinned Verus release for reproducibility**: the install script currently
  pins Verus release `0.2026.03.17.a96bad0` and downloads the host-specific
  binary release into `${XDG_CACHE_HOME:-$HOME/.cache}/whitaker/verus`. On
  2026-03-22, that release required Rust toolchain
  `1.94.0-x86_64-unknown-linux-gnu` on Linux; the script lets Verus report the
  required toolchain, replays Verus's suggested `rustup install ...` command,
  falls back to the bare semantic version when the host-qualified name is not
  accepted by local `rustup`, and then reruns the proof.
- **Squared threshold constants made explicit in runtime code**: the
  decomposition runtime now shares `MIN_COSINE_THRESHOLD_NUMERATOR_SQUARED = 1`
  and `MIN_COSINE_THRESHOLD_DENOMINATOR_SQUARED = 25` from
  `common/src/decomposition_advice/vector.rs`. These names make the shipped
  mathematics harder to misread than the prior generic similarity names.
- **`1 / 25` means squared cosine, not raw cosine**: the runtime comparison
  `25 * dot^2 >= left_norm * right_norm` is the squared-denominator form of
  `cosine >= 0.20` for non-zero norms. The proof models the cosine predicate
  via positive real vector lengths whose squares are `left_norm` and
  `right_norm`, then uses Verus nonlinear reasoning to prove the equivalence.
- **Zero norms are handled by control flow, not denominator arithmetic**: the
  proof and runtime both model zero-norm safety as an early return to `false`
  before any denominator-bearing reasoning is needed. This keeps the executable
  code free of division and clarifies why divide-by-zero cannot occur.
- **Behaviour tests use a narrow test-support seam**: integration tests call
  `common::test_support::decomposition::methods_meet_cosine_threshold()` rather
  than exposing `MethodFeatureVector` or `cosine_threshold_met` publicly. The
  helper builds feature vectors internally and follows the same threshold path
  as production code.

### Implementation decisions (6.4.4)

- **Verus models sparse vectors as aligned non-negative sequences**: the new
  proof file `verus/decomposition_vector_algebra.rs` treats absent sparse-map
  entries as `0` in aligned `Seq<nat>` values. This keeps the proof close to
  the shipped `u64` runtime semantics while avoiding proof-only dependence on
  `BTreeMap<String, u64>`.
- **`make verus` now runs an explicit proof list**: `scripts/run-verus.sh`
  executes both `verus/decomposition_cosine_threshold.rs` and
  `verus/decomposition_vector_algebra.rs` in deterministic order. Once the
  repository had multiple proof files, a single-file default was no longer a
  trustworthy quality gate.
- **"No overlapping positive features" means positive-weight intersection is
  empty**: the zero-dot-product theorem is stated as "for every feature index,
  not both weights are positive". Shared zero-weight entries therefore remain
  valid inputs for the theorem, matching the roadmap wording more closely than
  a strict "no shared keys" rule.
- **Runtime behaviour stays private; tests observe a report seam**:
  behavioural coverage uses
  `common::test_support::decomposition::method_vector_algebra()` and its
  `MethodVectorAlgebraReport` instead of exposing `MethodFeatureVector`,
  `dot_product`, or `norm_squared` publicly. The helper computes the shipped
  runtime values and returns only the numeric observations needed by BDD
  assertions.
- **Unit coverage includes explicit zero-weight edge cases**: internal tests
  use `test_feature_vector(...)` to exercise shared-key inputs where one side
  carries weight `0`. Production builders currently emit only positive weights,
  but the roadmap theorem is about positive overlap, so the edge case is tested
  directly.

### Implementation decisions (6.4.5)

- **Kani sidecar workflow mirrors the existing Verus pattern**:
  `scripts/install-kani.sh` pins Kani 0.67.0 and downloads the pre-built
  tarball into `${XDG_CACHE_HOME:-$HOME/.cache}/whitaker/kani`.
  `scripts/run-kani.sh` invokes the pinned `cargo-kani` binary against the
  `common` crate, filtering to `verify_build_adjacency` harnesses. `make kani`
  exposes the workflow as a top-level quality gate.
- **Bounded symbolic model uses fixed-size edge arrays**: each Kani harness
  generates symbolic `SimilarityEdge` values from a fixed-size array of up to 3
  edges with an active-length field. The maximum node count is 3 (with
  `unwind(7)`), giving at most C(3,2) = 3 possible unique undirected edges.
  These bounds are kept deliberately small because Rust's standard `sort_by`
  generates deeply nested loops that cause CBMC state-space explosion at higher
  bounds. This keeps the search space tractable and avoids requiring
  `kani::Arbitrary` for `Vec`.
- **Harness inputs constrained to the production edge contract**:
  `build_similarity_edges` guarantees `left < right < node_count`,
  `weight > 0`, and no duplicate unordered pairs. The Kani harnesses enforce
  these same `kani::assume` preconditions so the proof covers exactly the
  inputs that production code can generate.
- **Five separate proof harnesses for failure localisation**: the five
  properties (correct length, edge preservation, in-bounds indices, symmetry,
  sorted neighbours) each have a dedicated harness. This simplifies root-cause
  analysis when a single property fails.
- **`build_adjacency` promoted to `pub(crate)`**: the function was private;
  promoting it to `pub(crate)` aligns with `build_similarity_edges` and allows
  unit tests and the `test_support::decomposition` adjacency report helper to
  call it directly without widening the public crate API.
- **Behavioural coverage observes adjacency through an `AdjacencyReport`
  seam**: integration tests use
  `common::test_support::decomposition::adjacency_report()` which validates
  declarative edge input, delegates to the shipped `build_adjacency`, and
  returns an `AdjacencyReport` with convenience predicates (`is_symmetric`,
  `all_indices_in_bounds`, `is_sorted`, `neighbours_of`). The report is the
  only public interface; raw adjacency vectors remain crate-internal.
- **One unhappy-path BDD scenario validates test-support input
  rejection**: a scenario with `left >= right` is rejected by the
  `adjacency_report` helper before reaching `build_adjacency`, covering the
  edge-contract validation without changing production semantics.
- **`cfg(kani)` registered in `common/Cargo.toml`**: adding
  `check-cfg = ['cfg(kani)']` under `[lints.rust]` silences the
  `unexpected_cfgs` warning without requiring a build script.
- **Stale `rstest-bdd` version comment corrected**: the `common/Cargo.toml`
  dev-dependency comment now reads `0.5.x` instead of the outdated `0.2.x`.

## SARIF output

The lints can optionally emit SARIF 2.1.0 (Static Analysis Results Interchange
Format) for IDE and continuous integration (CI) tooling. The output is opt-in
via configuration or environment variables.

Planned approach:

- Collect diagnostics in a shared module when SARIF output is enabled.
- Serialize results using `serde`, including rule metadata, locations, and
  messages.
- Keep messages in English for consistent tool ingestion.
- Avoid overhead when SARIF output is disabled.

## Configuration, localization, and testing

- **Localization**: add Fluent entries for `brain_type` and `brain_trait` in
  line with existing lint tone.
- **Configuration**: add `brain_type` and `brain_trait` sections to
  `whitaker.toml`.
- **Testing**: add UI tests under `crates/brain_type/ui/` and
  `crates/brain_trait/ui/` for positive and negative cases.
- **Documentation**: update `docs/users-guide.md` with lint descriptions,
  configuration keys, and SARIF usage.

Example configuration:

```toml
[brain_type]
wmc_warn = 60
brain_method_min_cc = 25
brain_method_min_lines = 80
lcom4_warn = 2
foreign_types_warn = 10

[brain_trait]
methods_warn = 20
methods_deny = 30
default_cc_warn = 40
```

## Conclusion

`brain_type` and `brain_trait` will help teams spot overgrown types and traits
before they become maintenance risks. The shared cohesion analysis provides a
foundation for future lints, and the decomposition advice aligns with
Whitaker's actionable guidance philosophy. Optional SARIF output extends the
lints to CI-integrated reporting without affecting default workflows.

## References

[^brain-class]: M. Lanza and R. Marinescu, *Object-Oriented Metrics in
                Practice*,
  chapter on brain classes.
[^codescene]: CodeScene documentation on code health and brain class metrics:
  <https://docs.codescene.io/>.
[^clippy-cognitive]: Rust Clippy lint list (cognitive complexity):
  <https://rust-lang.github.io/rust-clippy/master/>.
[^clippy-issue]: Rust Clippy issue on macro expansion and complexity:
  <https://github.com/rust-lang/rust-clippy/issues/14417>.
[^rust-code-analysis]: `rust-code-analysis` metrics documentation:
  <https://github.com/mozilla/rust-code-analysis>.
[^whitaker-design]: Whitaker Dylint suite design:
  [whitaker-dylint-suite-design.md](whitaker-dylint-suite-design.md).
[^module-max-lines]: `module_max_lines` implementation:
  [crates/module_max_lines/src/driver.rs](../crates/module_max_lines/src/driver.rs).
