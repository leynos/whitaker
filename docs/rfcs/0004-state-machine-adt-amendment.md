# RFC 0004 amendment: state-machine ADT and protocol coverage

## Preamble

- **Amends:** RFC 0004, *Fragile types and invalid-state encodings*
- **Status:** Proposed
- **Repository:** `leynos/whitaker`
- **Created:** 2026-08-30
- **Lint family:** Validate not, only parse
- **Family selector:** `DOMAIN`

## Normative status

This document normatively amends RFC 0004. Where the two documents conflict,
this amendment takes precedence. Unchanged RFC 0004 requirements remain in
force, including its intrinsic-versus-extrinsic distinction, evidence-first
diagnostic policy, construction-surface analysis, exemption model, and staged
experimental rollout.

The amendment follows a review against the state-machine algebraic-data-type
(ADT) issues recorded in `leynos/mdtablefix` issues 443 through 449. That corpus
showed that the original proposal described the right principle but narrowed
several rule contracts too far to diagnose representative Rust state-machine
smells.

## Decision

Broaden RFC 0004 from types that explicitly reject invalid field combinations
to types whose product-shaped storage poorly represents a proven finite
semantic state space. A finding may therefore rest on one of four defects:

1. the representation admits rows that producers and consumers reject;
2. several storage rows are observationally equivalent because one dimension
   becomes irrelevant;
3. a primitive is stored even though all behavioural uses project it into an
   existing domain type; or
4. an API exposes a terminal operation before a required predecessor
   transition.

The lints must continue to require semantic evidence. Field count, names, and
vague aesthetic preference remain insufficient.

## Revised rule-code allocation

RFC 0003's allocation becomes:

| Range | Purpose |
| --- | --- |
| `DOMAIN001` to `DOMAIN099` | Boundary refinement and proof retention |
| `DOMAIN101` to `DOMAIN149` | Type state spaces, semantic partitions, projections, and invariant surfaces |
| `DOMAIN150` to `DOMAIN179` | Local protocol and lifecycle surfaces |
| `DOMAIN180` to `DOMAIN199` | Reserved local domain-modelling rules |
| `DOMAIN900` to `DOMAIN999` | Grouped crate and workspace reports |

## Revised rule catalogue

| Rule code | Canonical name | Initial level | Primary evidence |
| --- | --- | --- | --- |
| `DOMAIN101` | `manual_tagged_union` | Experimental `warn` | A closed selector chooses unit or payload-bearing states |
| `DOMAIN102` | `correlated_optional_fields` | `allow`, then experimental `warn` | Option presence is constrained by options, booleans, or closed tags |
| `DOMAIN103` | `constrained_boolean_state_space` | `allow`, Clippy-first | Produced or accepted Boolean rows form a proven proper subset |
| `DOMAIN104` | `sentinel_state_encoding` | `allow` | Empty, zero, `None`, or another supported sentinel jointly encodes persistent state |
| `DOMAIN105` | `redundant_state_dimension` | `allow` | A field is irrelevant within part of a closed semantic partition |
| `DOMAIN106` | `primitive_reencoded_as_domain_type` | Experimental `warn` | A stored primitive is totally projected into an existing domain type before behavioural use |
| `DOMAIN107` | `bypassable_type_invariant` | `allow` | An intrinsic invariant coexists with an exposed bypass path |
| `DOMAIN108` | `invalid_default` | Report-only initially | The default is provably rejected by the same invariant |
| `DOMAIN151` | `unguarded_terminal_transition` | Report-only or `allow` | Every visible terminal use is dominated by a required predecessor call |

The original `mutually_exclusive_bool_fields` proposal is replaced by
`constrained_boolean_state_space`. The original `DOMAIN104` and `DOMAIN105`
allocations move to `DOMAIN107` and `DOMAIN108` respectively.

## Amendment to `manual_tagged_union`

`DOMAIN101` must no longer require two or more payload candidates. It may report
a unit-or-payload sum when all of the following hold:

1. a Boolean, enum, or other closed selector distinguishes semantic states;
2. one selector value requires or activates a payload field;
3. another selector value denotes a unit state without that payload;
4. producers update the selector and payload in lockstep, or consumers reject
   or repair mismatches; and
5. one exhaustive site or two independent sites establish the mapping.

For example:

```rust
struct ProcessBuffer {
    table_lines: Vec<String>,
    in_table: bool,
}
```

may encode:

```rust
enum TableRun {
    Streaming,
    Buffering(Vec<String>),
}
```

The lint should report the selector-to-payload relationship separately from a
stronger non-empty-payload invariant. It must not claim that
`Buffering(Vec::new())` is invalid unless the evidence proves non-emptiness.

## Amendment to `correlated_optional_fields`

`DOMAIN102` covers mixed finite presence dimensions, not only two or more
`Option` fields. Its candidate field group may contain:

- one or more `Option<T>` or configured nullable fields;
- Boolean selectors;
- closed enum variants; and
- transparent local wrappers around those dimensions.

The rule recognizes exactly-one, at-most-one, all-or-none, equivalence, and
implication relations. For example:

```rust
struct DefinitionScanState {
    numeric_list_range: Option<Range<usize>>,
    skip_numeric_conversion: bool,
}
```

may establish:

```text
skip_numeric_conversion => numeric_list_range.is_some()
```

An exhaustive two-dimensional truth table suffices. Otherwise, require two
independent evidence sites. This resolves the original RFC contradiction in
which its worked Boolean-plus-option example qualified conceptually while the
formal rule required two optional fields.

## Replacement for `mutually_exclusive_bool_fields`

`DOMAIN103 constrained_boolean_state_space` warns when persistent Boolean
fields encode a finite state space whose produced or accepted rows form a
proven proper subset of the Cartesian product.

Mutual exclusion and one-hot state are special cases, not the whole rule. The
detector may use:

- an exhaustive tuple match that accepts, rejects, or marks every row;
- a closed producer set whose constructors or transitions emit only selected
  rows;
- repeated assertions or guards that establish the same allowed rows; or
- one producer partition plus one independent consumer partition.

For three fields, the detector compares the observed semantic partition with
all eight Boolean tuples. It must not infer a state machine from names, field
count, or co-occurrence in one conditional.

Clippy remains first in line for count-only diagnostics. Whitaker adds value
only when it identifies the exact correlated subset and the evidence that gives
those rows meaning.

## New rule: `sentinel_state_encoding`

`DOMAIN104` reports a persistent state encoded indirectly by supported sentinel
predicates when coordinated producers or rejecting consumers establish a
closed relationship.

The initial sentinel vocabulary is deliberately small:

- `Option<T>::is_some()` and `is_none()`;
- collection or string empty versus non-empty;
- integer zero versus non-zero;
- `NonZero*` construction; and
- configured closed predicates with pure summaries.

For example:

```rust
struct HtmlTableState {
    buffer: Vec<String>,
    depth: usize,
}
```

may encode `Outside | Inside { buffer, depth: NonZeroUsize }` when code
maintains and consumes the equivalence between buffer emptiness and zero depth.
One coincidental emptiness check is insufficient. The relation must describe
persistent state and appear in a closed producer set, an exhaustive consumer,
or two independent enforcement sites.

The diagnostic should recommend an enum or constrained payload without
pretending that every empty collection or zero scalar is suspect.

## New rule: `redundant_state_dimension`

`DOMAIN105` reports a field whose value becomes observationally irrelevant
within part of a closed semantic partition.

For example:

```rust
if repeat_prefix {
    repeat_whole_prefix();
} else if let Some(outer) = outer_prefix {
    repeat_outer_prefix(outer);
} else {
    indent_only();
}
```

partitions `bool × Option<T>` into three semantic behaviours even though the
storage has four rows. When `repeat_prefix` is true, the `outer_prefix`
dimension is masked. An ADT can attach the outer payload only to the state that
uses it.

Require one exhaustive partition plus corroborating producer or consumer
evidence, or two independent partitions. Suppress one-off branch-local
optimizations and fields whose ignored value remains meaningful for equality,
serialization, diagnostics, or another operation.

## New rule: `primitive_reencoded_as_domain_type`

`DOMAIN106` reports a persistent primitive when local behavioural consumers use
it only through a total, pure projection into an existing local domain type.

For example:

```rust
fn opening_rewrite(conflict: bool) -> Strategy {
    if conflict {
        Strategy::Preserve
    } else {
        Strategy::Compress
    }
}
```

supports storing `Strategy` directly when the Boolean has no independent
behavioural meaning.

The initial rule requires:

1. a total projection over the primitive's complete finite domain;
2. a local nominal target type;
3. all non-diagnostic behavioural uses of the stored field to consume the
   projected value or repeat an equivalent projection; and
4. no raw, wire, FFI, serialization, bitset, or externally imposed
   representation boundary.

Logging the primitive does not count as independent domain behaviour. Public
compatibility, layout, and persistence constraints suppress the rule unless a
strong internal wrapper already exists.

## Renumbered invariant rules

The substance of `bypassable_type_invariant` and `invalid_default` remains as
specified by RFC 0004, but their identifiers become `DOMAIN107` and
`DOMAIN108`. Documentation, configuration keys, Fluent slugs, finding records,
and tests must use the new codes before either rule ships.

## New protocol rule: `unguarded_terminal_transition`

`DOMAIN151` concerns temporal API obligations rather than finite value truth
tables. Report a terminal method when all of the following hold:

1. it consumes `self` or exposes accumulated state in a way that can omit
   pending work;
2. every visible local call on the same receiver is dominated by a particular
   predecessor method;
3. the predecessor mutates, drains, commits, closes, synchronizes, or otherwise
   prepares state used by the terminal operation;
4. the owner could encapsulate the sequence in one consuming finalizer; and
5. no public, trait, generated, or unresolved external caller prevents the
   analysis from seeing legitimate unprepared uses.

The canonical shape is:

```rust
state.flush();
let output = state.into_output();
```

with remediation:

```rust
fn finish(mut self) -> Vec<String> {
    self.flush();
    self.output
}
```

Method names are not evidence. MIR dominance, receiver-place identity, and the
flow of predecessor mutations into terminal output are required. Optional
preparation, RAII cleanup, independent idempotent calls, fallible predecessors,
and public call graphs with unknown consumers must suppress or lower the
finding to report-only.

## Shared evidence-model amendments

`common::domain_model::state_space` gains compiler-independent models for:

- finite field dimensions such as presence, Boolean, empty/non-empty, and
  zero/non-zero;
- allowed storage rows;
- semantic partitions that map several rows to one behaviour;
- selector-to-payload mappings, including unit variants;
- domain projections from primitives into nominal types;
- producer, consumer, transition, and diagnostic-only uses; and
- evidence strength and contradiction handling.

A separate `common::domain_model::protocol` module should model terminal and
predecessor methods, receiver identity, call-site dominance, state transfer,
unknown external uses, and protocol evidence strength. Protocol analysis must
not be squeezed into the finite truth-table model.

The first release should bound exhaustive dimensions to four fields and use
deterministic bit-vector rows and `BTree*` collections. Unknown predicates and
contradictory evidence suppress findings rather than inviting approximation.

## Diagnostic precedence

For one correlated field group, emit the most specific finding in this order:

1. `manual_tagged_union`;
2. `correlated_optional_fields`;
3. `constrained_boolean_state_space`;
4. `sentinel_state_encoding`;
5. `redundant_state_dimension`;
6. `primitive_reencoded_as_domain_type`;
7. `bypassable_type_invariant`; and
8. `invalid_default` as a secondary advisory when it adds distinct evidence.

`unguarded_terminal_transition` owns a protocol pair rather than a field group
and therefore deduplicates separately.

## Normative mdtablefix acceptance corpus

Before promotion, source-faithful fixtures derived from these issues must
produce the expected candidate finding:

| mdtablefix issue | Required owner |
| --- | --- |
| #443, `bool + Vec<T>` table buffering | `DOMAIN101` |
| #444, `flush(); into_out()` terminal sequence | `DOMAIN151` |
| #445, empty buffer plus zero depth | `DOMAIN104` |
| #446, three-Boolean fence observation table | `DOMAIN103` |
| #447, conflict Boolean projected to `Strategy` | `DOMAIN106` |
| #448, `Option<Range> + bool` implication | `DOMAIN102` |
| #449, `bool × Option<T>` with masked data | `DOMAIN105` |

Passing means identifying the intended correlated fields or protocol methods,
not necessarily prescribing the exact refactoring proposed by each issue. The
same corpus must include refactored enum/finalizer forms that remain quiet.

An implementation that leaves all seven original shapes quiet has not met RFC
0004's coverage goal. Builders, raw deserialization models, FFI layouts,
independent options, ordinary counters, optional cleanup, and unrelated
booleans must remain negative fixtures.

## Rollout amendments

Implement in confidence order:

1. broaden `manual_tagged_union` and add the #443 fixture;
2. fix mixed presence support in `correlated_optional_fields` with #448;
3. implement `primitive_reencoded_as_domain_type` with #447;
4. trial generalized Boolean state spaces with #446;
5. add sentinel and redundant-dimension rules with #445 and #449;
6. retain `bypassable_type_invariant` and `invalid_default` at conservative
   levels under their new codes; and
7. develop `unguarded_terminal_transition` independently at report-only or
   `allow`, using #444 as its positive fixture.

`DOMAIN101` and `DOMAIN106` should receive the first promotion decisions.
`DOMAIN107`, `DOMAIN108`, and `DOMAIN151` remain experimental or report-only
until their proof boundaries stabilize on real corpora.

## Roadmap relationship

The accompanying
[Phase 14 DOMAIN roadmap amendment](../roadmap-phase-14-domain-amendment.md)
normatively supersedes conflicting Phase 14.1, 14.3, and 14.4 task wording in
`docs/roadmap.md`. Non-conflicting Phase 14 work remains applicable.

## Recommendation

Accept this amendment before implementing RFC 0004. The mdtablefix corpus
captures the distinction the original draft blurred: code may maintain an
invariant perfectly today and still force humans to maintain a relationship
that an ADT, newtype, domain enum, or consuming finalizer could make structural.
The lints should find that representational tax without degenerating into a
field-count style tribunal.

## References

- [RFC 0003: weak representations at domain
  boundaries](0003-weak-domain-boundaries.md)
- [RFC 0004: fragile types and invalid-state encodings](0004-fragile-types.md)
- [mdtablefix issue #443](https://github.com/leynos/mdtablefix/issues/443)
- [mdtablefix issue #444](https://github.com/leynos/mdtablefix/issues/444)
- [mdtablefix issue #445](https://github.com/leynos/mdtablefix/issues/445)
- [mdtablefix issue #446](https://github.com/leynos/mdtablefix/issues/446)
- [mdtablefix issue #447](https://github.com/leynos/mdtablefix/issues/447)
- [mdtablefix issue #448](https://github.com/leynos/mdtablefix/issues/448)
- [mdtablefix issue #449](https://github.com/leynos/mdtablefix/issues/449)
