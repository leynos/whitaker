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
class" as a large, complex class that centralises too much intelligence. Common
signals include:[^brain-class]

- Many methods and high overall complexity (Weighted Methods per Class (WMC)).
- At least one "brain method" that dominates the class's behaviour.
- Low cohesion (for example, high LCOM4 or low Tight Class Cohesion (TCC)).
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

Deep analysis is only performed after cheap thresholds are crossed (for
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
Messages are localised via Fluent entries, using the existing Whitaker tone.

### Decomposition advice

When a brain type or trait is detected, the lint produces decomposition advice
based on method clustering. The analysis uses a feature vector per method built
from:

- Accessed fields.
- Types used in signatures or local variables.
- External domains (for example, `serde::de` or `tokio::fs`).
- Method name keywords (excluding common verbs like "get" or "set").

The clustering pipeline is:

- Build a similarity graph using cosine similarity across feature vectors.
- Apply community detection (for example, Louvain or Leiden) to group related
  methods. When method counts are large, consider approximate neighbour search
  such as Hierarchical Navigable Small Worlds (HNSW) to avoid O(n^2) cost.
- Label clusters using common field names, domains, and keywords.
- Generate suggestions that map clusters to likely extractions (new helper
  struct, module, or trait).

Example help output:

```plaintext
Note: `Foo` splits into three areas:
- [parse]: 11 methods using grammar and tokens (extract `FooParser`).
- [serde]: 6 methods for serialisation (move to `foo::serde_glue`).
- [fs_io]: 5 methods for file I/O (extract `FooStorage` trait).
```

Advice is concise and only emitted when clustering yields meaningful groups. If
the type is extremely large, the lint may cap advice length and report that
further decomposition analysis was omitted.

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

## Configuration, localisation, and testing

- **Localisation**: add Fluent entries for `brain_type` and `brain_trait` in
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
