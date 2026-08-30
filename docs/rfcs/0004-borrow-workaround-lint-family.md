# RFC 0004: Borrow-workaround lint family

## Preamble

- **RFC number:** 0004
- **Status:** Proposed
- **Created:** 2026-08-30
- **Scope:** Candidate-generating lints for indirect ownership and borrowing
  shapes not covered by Whitaker's proposed Phase 9 ownership lints.
- **Depends on:** RFC 0003.
- **Related RFC:** RFC 0005.
- **Precedence documents:** `docs/ownership-shape-lints-design.md`,
  `docs/whitaker-dylint-suite-design.md`, and any later accepted ADRs.

## 1. Summary

This RFC proposes a `BOR` family of Whitaker rules for source shapes commonly
introduced after borrow-check failures:

- repeated collection lookups;
- remove, mutate, and reinsert cycles;
- `mem::take` or `mem::replace` followed by restoration;
- index round-trips used instead of direct mutable iteration;
- temporary key or identifier staging before mutation;
- duplicated search and mutation passes;
- owned snapshots spanning mutation;
- explicit lexical borrow fences; and
- single-use helper functions that split one borrow-sensitive operation.

These rules differ from ordinary style lints. Most candidate shapes can be
legitimate. Several are not made legal by Polonius because they represent real
aliasing, iterator invalidation, rollback, drop-order, or framework constraints.

The rules should therefore operate as **rewrite-backed lints**:

1. HIR and MIR analysis identify a mechanically bounded candidate.
2. The lint emits a versioned rewrite intent rather than asserting motive.
3. RFC 0005 materializes one or more direct alternatives.
4. RFC 0003 compiles those alternatives under NLL and Polonius.
5. Whitaker emits the final diagnostic only when at least one alternative is
   compiler-validated.

The result is a lint family that reports evidence, not folklore. It can say
that a temporary vector, clone, scope, or collection round-trip has a verified
simpler replacement and whether that replacement requires Polonius.

## 2. Problem

Whitaker's proposed Phase 9 ownership lints cover three important signals:

- a clone only used by immutable borrow;
- a by-value parameter that causes caller clones; and
- a non-escaping shared-ownership or interior-mutability wrapper.

Those rules will catch a substantial amount of agent-generated ownership
inflation. They do not cover workaround shapes that contain no obvious
unnecessary clone or wrapper.

Representative examples include:

```rust
let index = items.iter().position(|item| item.id == wanted)?;
let item = items.get_mut(index)?;
item.refresh();
```

```rust
let mut value = map.remove(&key)?;
value.refresh();
map.insert(key, value);
```

```rust
let ids: Vec<_> = graph.nodes().map(Node::id).collect();
for id in ids {
    graph.node_mut(id)?.refresh();
}
```

```rust
let snapshot = self.document.clone();
for shape in snapshot.shapes() {
    self.update_shape(shape);
}
```

Agents often learn these forms because they compile broadly on stable Rust.
Some are sound, clear, and necessary. Others are merely indirect versions of a
borrow that NLL rejects conservatively but Polonius accepts.

A broad syntactic lint would be noisy. A purely compiler-based search cannot
invent the missing direct rewrite. Whitaker needs rule-specific detectors and
materializers, with compiler validation as the emission gate.

## 3. Design principles

### 3.1. Report verified alternatives, not presumed motives

Diagnostics must describe observable structure:

> collection searched twice although a compiler-validated single-pass rewrite
> exists

They must not say:

> this code works around the borrow checker

The reason for the original shape is unknowable and irrelevant once a better
candidate has been verified.

### 3.2. Prefer stable formulations before Polonius-only formulations

Each detector may produce several alternatives. Whitaker should rank them in
this order:

1. an existing stable standard-library or crate API, such as `entry`;
2. a direct rewrite accepted by NLL;
3. a direct rewrite accepted only by Polonius;
4. no diagnostic when no candidate passes.

Polonius is an expanded language capability, not a reason to ignore a clearer
portable API.

### 3.3. Let the compiler reject speculative transformations

Candidate generation can be moderately broad when the final diagnostic is
gated by RFC 0003. Detection still needs a strict false-positive budget because
compiler acceptance does not prove semantic equivalence.

### 3.4. Separate portability from semantic confidence

A rewrite can be:

- NLL-valid but behaviourally risky;
- Polonius-only but locally mechanical; or
- compiler-valid under both while changing panic or drop behaviour.

The report must therefore carry two independent axes:

```rust,no_run
pub enum Portability {
    Nll,
    PoloniusNext,
}

pub enum SemanticConfidence {
    Mechanical,
    LocallyReasoned,
    ReviewRequired,
}
```

### 3.5. Preserve framework and API constraints

The rules must remain quiet when the shape is imposed by:

- an external trait or callback signature;
- async or thread boundaries;
- a framework handle type;
- collection invalidation rules;
- undo, rollback, or transaction semantics;
- mutation through a genuinely aliased handle; or
- a public API that Whitaker is not authorized to change.

## 4. Goals and non-goals

### 4.1. Goals

- Cover every workaround family listed in Section 2.
- Generate concrete rewrite intents with stable source anchors.
- Reuse Phase 9 HIR prefiltering and MIR confirmation infrastructure.
- Reuse RFC 0003 for NLL and Polonius validation.
- Emit useful diagnostics only after a candidate is validated.
- Prefer transformations that remove allocation, repeated lookup, or
  unnecessary indirection.
- Expose stable rule codes and JSON classifications for agents.
- Support collection and graph APIs through resolved paths and configurable
  effect summaries.
- Roll out high-confidence rules before research-grade rules.

### 4.2. Non-goals

- Ban `mem::take`, temporary vectors, indices, helper functions, snapshots, or
  remove-and-reinsert algorithms.
- Infer that every indirect ownership shape was caused by NLL.
- Prove panic, drop-order, transactional, or performance equivalence.
- Rewrite public APIs by default.
- Analyze arbitrary unsafe code.
- Understand undocumented effects of every third-party collection type.
- Emit unvalidated warnings by default.
- Make genuine simultaneous mutable aliases legal.

## 5. Shared analysis and intent emission

### 5.1. Two-stage detector

Each rule should use the Phase 9 pattern:

1. an HIR prefilter identifies syntactic candidates and editable source spans;
2. MIR confirmation classifies place identity, moves, uses, mutation,
   control-flow, and escape.

The detector should not attempt to run Polonius inside the lint process.

### 5.2. Stable source anchors

A `DefId` is useful within one compiler process but cannot identify source
across an overlay replay. Each intent should therefore record:

- workspace-relative file;
- whole-file SHA-256;
- editable byte range;
- source snippet SHA-256;
- enclosing item `DefPathHash` or normalized definition path;
- enclosing item signature fingerprint;
- package and target identity; and
- rule-specific payload.

### 5.3. Generic recipe envelope

The shared model should avoid adding every rule payload to one central enum.
A generic envelope permits independent lint and rewriter evolution.

```rust,no_run
pub struct RewriteIntent {
    pub schema_version: u16,
    pub id: RewriteId,
    pub rule: RuleId,
    pub anchor: SemanticAnchor,
    pub impact: ImpactScope,
    pub recipe: RecipeDescriptor,
    pub evidence: EvidenceSummary,
}

pub struct RecipeDescriptor {
    pub kind: RecipeKind,
    pub version: u16,
    pub payload: serde_json::Value,
}
```

The owning lint crate should define and test a typed payload. RFC 0005's
materializer registry should reject unsupported recipe versions.

### 5.4. Sidecar collection

Complex rewrite metadata does not fit cleanly into ordinary rustc suggestion
JSON. `whitaker check` should therefore set a session directory such as:

```plaintext
WHITAKER_REWRITE_SESSION_DIR=target/whitaker/rewrite-scan/<session>
```

Lint processes should atomically write one JSON file per crate process and
candidate. Files should be named from a crate disambiguator, process identifier,
and monotonic local counter. Aggregation occurs after Cargo finishes.

Ordinary `cargo dylint` use without a Whitaker session should behave as follows:

- emit an ordinary diagnostic when the lint has a self-contained,
  high-confidence NLL-valid suggestion;
- emit a help message inviting `whitaker rewrite check` for a strong but
  unvalidated candidate; or
- remain silent for experimental, validation-dependent rules.

### 5.5. Effect summaries

Several rules need to know whether a local operation changes collection
membership, escapes a value, or touches only a disjoint field.

```rust,no_run
pub struct EffectSummary {
    pub reads: PlaceSet,
    pub writes: PlaceSet,
    pub moves: PlaceSet,
    pub may_escape_arguments: bool,
    pub structurally_mutates_receiver: bool,
    pub may_reorder_receiver: bool,
    pub may_suspend: bool,
    pub analysis: EffectConfidence,
}

pub enum EffectConfidence {
    MirDerived,
    TrustedBuiltin,
    Configured,
    Unknown,
}
```

Whitaker may derive summaries for local, non-generic functions from MIR.
External methods require either:

- a built-in resolved-path summary;
- an explicit configuration entry; or
- `Unknown`, which prevents automatic rewriting.

The first release should include summaries for common standard-library,
`hashbrown`, `indexmap`, and `slotmap` operations only where their effects are
clear.

## 6. Proposed rule inventory

| Code | Canonical name | Primary workaround | Initial level | Initial rewrite policy |
| --- | --- | --- | --- | --- |
| `BOR001` | `lookup_then_relookup` | Repeated lookup around conditional mutation | Experimental warn after validation | Automatic for mechanical candidates |
| `BOR002` | `remove_then_reinsert` | Temporarily removing an entry to mutate it | Experimental allow | Review by default |
| `BOR003` | `take_then_restore` | `mem::take`, `replace`, or `Option::take` followed by restoration | Experimental allow | Review by default |
| `BOR004` | `index_round_trip_for_mutation` | `position` or index lookup followed by `get_mut` | Experimental warn after validation | Automatic for single-pass equivalents |
| `BOR005` | `staged_keys_before_mutation` | Collecting keys or IDs solely for a later mutation loop | Experimental allow | Review by default |
| `BOR006` | `split_search_and_mutate_pass` | Duplicated read and write traversals | Experimental warn after validation | Automatic for pure repeated predicates |
| `BOR007` | `snapshot_around_mutation` | Owned clone retained only as a read view during mutation | Experimental allow | Review by default |
| `BOR008` | `borrow_scope_fence` | Extra block, reference drop, or binding solely to end a borrow | Experimental allow | Automatic only for reference-only fences |
| `BOR009` | `single_use_borrow_split_helper` | Private helper introduced to split one borrow-sensitive operation | Research allow | Diff only |

_Table 1: Proposed borrow-workaround rules and first-release policy._

The `BOR` family should be separate from the broader `OWN` or ownership-shape
family. `OWN` rules diagnose ownership inflation. `BOR` rules diagnose
compiler-validated indirect control-flow or data-access shapes.

## 7. Rule contracts

### 7.1. `BOR001 lookup_then_relookup`

#### Intent

Detect repeated lookup of the same logical key and receiver when a direct
conditional borrow or a dedicated entry API removes the repetition.

#### Candidate shape

```rust
if map.contains_key(&key) {
    map.get_mut(&key)
} else {
    map.insert(key.clone(), make_value());
    map.get_mut(&key)
}
```

A second form searches mutably, returns or uses the successful borrow, then
performs a mutation and repeats the lookup on the failure path.

#### Candidate alternatives

The materializer should generate, in order:

1. a known entry-API rewrite;
2. a direct `match` or `if let` conditional mutable-borrow rewrite; and
3. a minimally changed form that removes only `contains_key`.

For a map with a compatible entry API, the preferred result may be:

```rust
map.entry(key).or_insert_with(make_value)
```

For an API without an entry abstraction, the direct form may be:

```rust
match map.get_mut(&key) {
    Some(value) => value,
    None => {
        map.insert(key, make_value());
        map.get_mut(&key).expect("inserted key must exist")
    }
}
```

The second form is a canonical Polonius Alpha use case.

#### Detection requirements

- Receiver places resolve to the same value.
- Key expressions have the same normalized expression fingerprint.
- Key evaluation is side-effect free or evaluated once in the candidate.
- Intervening mutation is understood.
- The repeated lookup result is not deliberately revalidating a changed
  invariant.
- Macro-only spans are excluded.

#### Suppressions

Suppress when:

- the mutation may remove or replace another entry that changes lookup
  semantics;
- a concurrent or interior-mutable container can change between lookups;
- a custom API has unknown lookup or insertion effects; or
- the repeated check produces a distinct user-facing error.

### 7.2. `BOR002 remove_then_reinsert`

#### Intent

Detect a value removed from a collection, used locally, and reinserted into the
same receiver under the same key when a direct mutable borrow is a viable
candidate.

#### Candidate shape

```rust
let mut value = map.remove(&key)?;
value.refresh();
map.insert(key, value);
```

#### Detection requirements

- `remove` and `insert` resolve to recognized methods.
- Receiver and key identity match.
- The removed value does not escape.
- Exactly one reinsertion postdominates the candidate region on normal
  completion.
- The absent-entry state is not queried or exposed.
- The key is not mutated.
- No second value is inserted under the key.

#### Semantic caveat

Remove-and-reinsert changes observable state during the middle of the region.
It may also change panic behaviour, drop order, map order, allocation, or
generation counters. Compiler acceptance cannot prove those properties equal.

The rule should therefore mark the candidate `ReviewRequired` unless the
intervening operations are a bounded sequence of field assignments or
recognized no-escape methods on the removed value.

#### Suppressions

Suppress when:

- map order or stable entry identity is observable;
- the collection uses generational IDs;
- the absence is intentional for recursive re-entry;
- early return, `?`, `break`, or `continue` can skip reinsertion;
- the removed value participates in rollback or transaction logic; or
- the direct candidate fails RFC 0003 validation.

### 7.3. `BOR003 take_then_restore`

#### Intent

Detect a field or local temporarily replaced by an empty/default value and
restored from the same local.

#### Candidate operations

- `std::mem::take`;
- `std::mem::replace`;
- `Option::take`;
- `Vec::split_off(0)` followed by restoration, where recognized; and
- configured equivalents.

#### Candidate shape

```rust
let mut cache = std::mem::take(&mut self.cache);
rebuild(&mut cache, &mut self.index);
self.cache = cache;
```

#### Candidate alternatives

The materializer may attempt:

- direct disjoint-field borrowing;
- destructuring `self` into disjoint mutable field borrows;
- changing a same-module helper from `&mut self` to the exact fields it uses; or
- inlining a single-use helper under the constraints of `BOR009`.

#### Restrictions

This rule is intentionally conservative. Taking a field can be a valid way to
model ownership transfer, panic recovery, re-entrancy, or a state-machine
transition. The first release should emit only after a candidate compiles and
should never apply it automatically by default.

### 7.4. `BOR004 index_round_trip_for_mutation`

#### Intent

Detect a collection searched for an index or position and then immediately
indexed mutably, when direct mutable iteration expresses the operation.

#### Candidate shape

```rust
let index = items.iter().position(|item| item.id == wanted)?;
let item = items.get_mut(index)?;
item.refresh();
```

#### Preferred rewrite

```rust
let item = items.iter_mut().find(|item| item.id == wanted)?;
item.refresh();
```

#### Detection requirements

- The index local is used only for the subsequent access.
- The collection is not structurally modified between search and access.
- The predicate is preserved exactly.
- Search order is unchanged.
- The access does not intentionally distinguish an impossible stale index.
- Indexing is not needed for later ordering, diagnostics, or another
  collection.

This is likely the highest-confidence new rule. Many candidates will compile
under NLL and should be preferred over a Polonius-only alternative.

### 7.5. `BOR005 staged_keys_before_mutation`

#### Intent

Detect a temporary collection of keys, IDs, or handles consumed exactly once by
a following mutation loop.

#### Candidate shape

```rust
let ids: Vec<_> = graph.nodes().map(Node::id).collect();
for id in ids {
    graph.node_mut(id)?.refresh();
}
```

#### Candidate alternatives

The materializer may attempt:

- `nodes_mut()` or another recognized mutable iterator;
- direct iteration over a disjoint index;
- iterator-owned keys when the API already returns them by value;
- a field-split helper; or
- a Polonius-only direct loop.

#### Detection requirements

- The staging collection is local, non-escaping, and single-use.
- Its item type is copied or cloned solely for staging.
- Loop order remains equal.
- The loop does not add, remove, or reorder elements in the iterated domain.
- The staged list is not a deliberate snapshot protecting against membership
  change.
- Any graph or arena API has a trusted effect summary.

Because iterator invalidation is often real, this rule should remain off by
default until corpus evidence demonstrates acceptable signal.

### 7.6. `BOR006 split_search_and_mutate_pass`

#### Intent

Detect two traversals over the same receiver where the first identifies a
candidate and the second repeats the same search to mutate it.

#### Candidate shape

```rust
let found = items.iter().any(|item| item.id == wanted);
if found {
    if let Some(item) = items.iter_mut().find(|item| item.id == wanted) {
        item.refresh();
    }
}
```

#### Preferred rewrite

```rust
if let Some(item) = items.iter_mut().find(|item| item.id == wanted) {
    item.refresh();
}
```

The rule also covers an immutable `find` followed by a mutable `find` when the
first result contributes only identity or existence.

#### Detection requirements

- Receiver and predicate fingerprints match.
- No mutation or externally visible call occurs between passes.
- Predicate evaluation is side-effect free.
- The first pass contributes no additional value.
- Short-circuit and iteration order remain equal.

This rule overlaps `BOR004`; `BOR004` should win for explicit index
round-trips. `BOR006` handles existence flags, copied IDs, and repeated
predicates without an index local.

### 7.7. `BOR007 snapshot_around_mutation`

#### Intent

Extend `clone_only_used_by_borrow` to clones whose only purpose is to provide a
read view while the original owner is mutated.

#### Candidate shape

```rust
let snapshot = self.document.clone();
for shape in snapshot.shapes() {
    self.update_shape(shape);
}
```

The existing clone lint is expected to suppress this shape when borrowing the
original appears to overlap mutation. `BOR007` should retain it as a
path-sensitive rewrite candidate.

#### Detection requirements

- Snapshot creation is clone-like.
- Snapshot data is only read.
- The snapshot does not escape.
- It is not used for rollback, undo, comparison, retry, diagnostics, or restore.
- No code observes snapshot identity.
- A candidate field split or direct borrow can be materialized.

#### Suppressions

Suppress when the clone is recorded in history, retained after the mutation,
compared with the result, used on an error path, or represents an immutable
transaction snapshot.

This rule should remain review-only in the first release.

### 7.8. `BOR008 borrow_scope_fence`

#### Intent

Detect syntax whose only observable purpose appears to be forcing a borrow
local out of scope before a later operation.

Candidate forms include:

- a nested block containing only reference-typed locals;
- an explicit `drop(reference_local)`;
- an intermediate binding used once before a mutation; and
- an immediately invoked closure used only as a scope.

#### Candidate rewrite

The materializer removes the fence, flattens the block, or substitutes the
single-use binding. RFC 0003 then determines whether NLL or Polonius accepts the
result.

#### Restrictions

Flattening can change local name resolution and the drop time of owned values.
The first release should require every affected local to be a reference, a
`Copy` scalar, or a compiler temporary without meaningful `Drop`.

Clippy already diagnoses some useless `drop` calls. Whitaker should avoid
duplicate diagnostics and focus on multi-statement scope fences with a verified
simplification.

### 7.9. `BOR009 single_use_borrow_split_helper`

#### Intent

Detect a small private helper called exactly once when inlining it permits a
clearer direct ownership formulation.

#### Candidate constraints

The helper must be:

- private and defined in the same module;
- called exactly once in the selected feature configuration;
- non-generic;
- non-async;
- non-unsafe;
- free of recursion;
- below a configurable MIR and source-size threshold;
- free of macro-only source; and
- removable without affecting documentation or tests that reference it.

The materializer should inline the body, substitute arguments, and optionally
remove the helper. The checker must compile all package targets.

This rule is research-grade. Its first release should produce a diff only and
must never auto-apply.

## 8. Candidate alternatives and ranking

A detector may emit several recipe alternatives. The rewriter should validate
all bounded alternatives and rank successful results by:

1. accepted project checker;
2. NLL portability;
3. semantic confidence;
4. existing idiomatic API use;
5. removed allocation or repeated lookup;
6. smaller edit count;
7. smaller changed source span; and
8. deterministic recipe preference.

```rust,no_run
pub struct CandidateScore {
    pub accepted_policy: bool,
    pub portability: Portability,
    pub semantic_confidence: SemanticConfidence,
    pub idiom_rank: u16,
    pub allocation_delta: AllocationDelta,
    pub edit_count: usize,
    pub changed_bytes: usize,
}
```

Performance claims must be descriptive unless benchmarked. Removing a
temporary vector or duplicate lookup is mechanically observable; claiming an
end-to-end speed-up is not.

## 9. Diagnostics and reporting

### 9.1. Final diagnostic ownership

The CLI, not the detector process, should own the final validated diagnostic.
This permits one message to include:

- the source workaround;
- the selected rewrite;
- NLL acceptance;
- Polonius acceptance;
- semantic-confidence level;
- affected targets;
- whether the project policy permits Polonius-only source; and
- a rewrite identifier consumable by RFC 0005.

A representative diagnostic is:

```text
warning[BOR005 staged_keys_before_mutation]:
temporary ID staging has a compiler-validated direct rewrite
  --> crates/graph/src/update.rs:44:5
   |
44 | let ids: Vec<_> = graph.nodes().map(Node::id).collect();
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = selected rewrite: iterate mutable nodes directly
   = compiler validation: Polonius Alpha accepted; NLL rejected with E0502
   = semantic confidence: locally reasoned
   = allocation removed: one temporary Vec
   = run `whitaker rewrite diff BOR005-7f3c2e` to inspect
```

### 9.2. Unvalidated candidates

The default policy should be silent for unvalidated candidates. An opt-in
research mode may display them as `advisory` findings:

```toml
[borrow_workarounds]
unvalidated = "advisory"
```

Advisory output must say that no valid rewrite has been established.

### 9.3. Rule precedence

When several rules identify the same region:

1. `BOR004` wins over `BOR006`;
2. existing `clone_only_used_by_borrow` wins over `BOR007` when no mutable
   overlap exists;
3. `BOR003` wins over `BOR002` for `Option::take` or `mem::take`;
4. a stable entry-API alternative suppresses a lower-ranked Polonius-only
   `BOR001` alternative; and
5. overlapping candidates are passed to RFC 0005's conflict planner.

## 10. Configuration

The proposed top-level configuration is:

```toml
[borrow_workarounds]
enabled = true
require_validation = true
unvalidated = "silent"
accepted_borrow_checker = "nll"
prefer_portable_rewrites = true
external_effect_summaries = []

[lookup_then_relookup]
enabled = true

[remove_then_reinsert]
enabled = false
allow_order_changing_maps = false

[take_then_restore]
enabled = false

[index_round_trip_for_mutation]
enabled = true

[staged_keys_before_mutation]
enabled = false
max_staged_items_hint = 1024

[split_search_and_mutate_pass]
enabled = true

[snapshot_around_mutation]
enabled = false

[borrow_scope_fence]
enabled = false

[single_use_borrow_split_helper]
enabled = false
max_helper_lines = 20
```

`accepted_borrow_checker = "polonius-next"` should be explicit. Discovering
that the compiler supports Polonius must not automatically authorize
Polonius-only rewrites.

Effect summaries for external APIs should use resolved definition paths and a
versioned schema. They should not match source text names alone.

## 11. Compatibility and migration

This RFC extends the proposed ownership phase without changing existing lint
behaviour.

On acceptance, the roadmap should add:

- shared rewrite-intent emission after Phase 9.1;
- `BOR001`, `BOR004`, and `BOR006` as the first validated tier;
- `BOR002`, `BOR003`, and `BOR007` as a review-only second tier;
- `BOR005` and `BOR008` after corpus tuning; and
- `BOR009` as a research rule outside the standard suite.

Existing lints should be amended so that a mutable-overlap result is not always
a terminal suppression. `clone_only_used_by_borrow` should classify original
place conflicts as:

```rust,no_run
pub enum OriginalPlaceConflict {
    None,
    Definite,
    PotentiallyPathSensitive,
}
```

`PotentiallyPathSensitive` should emit a rewrite intent for RFC 0003 rather than
discarding the candidate. This is the principal integration point between
Phase 9 and Polonius-aware validation.

## 12. Testing strategy

### 12.1. Pure unit tests

Shared tests should cover:

- expression and place fingerprints;
- receiver and key identity;
- postdominance of reinsertion;
- escape classification;
- effect-summary reduction;
- candidate ranking;
- rule precedence; and
- deterministic intent serialization.

### 12.2. Dylint UI tests

Each rule should include positive detector fixtures and negative fixtures for:

- macros;
- unknown external APIs;
- escaping locals;
- public and trait-constrained functions;
- async suspension;
- unsafe blocks;
- genuine structural mutation; and
- transactional or history use.

UI tests should verify candidate metadata as well as diagnostics.

### 12.3. Rewrite golden tests

Every materializer should have golden before-and-after fixtures. Fixtures should
preserve comments, attributes, indentation, and trailing commas.

### 12.4. Differential compiler tests

Each rule needs four fixtures where applicable:

- candidate accepted by NLL and Polonius;
- candidate accepted only by Polonius;
- candidate rejected by both;
- detector suppressed because semantics are not sufficiently constrained.

### 12.5. Behavioural equivalence tests

Reviewable and automatic recipes should run domain tests that detect:

- panic-path differences;
- map-order differences;
- drop-order differences;
- lost rollback data;
- iterator-membership changes; and
- changed early-return behaviour.

### 12.6. Corpus validation

The first corpus should include mutation-heavy code from Gauss, Weaver, ddlint,
Lille, Netsuke, and Stilyagi. mxd should serve as an async and framework
negative-control corpus.

Promotion requires a measured ratio of validated findings to scanned
candidates. A high candidate count with few materializable rewrites indicates
that a detector is too broad even if it emits no false final warning.

## 13. Rollout plan

### 13.1. Stage 1: Intent infrastructure

- Add the generic recipe envelope.
- Add atomic sidecar collection.
- Add place, expression, effect, and escape summaries.
- Amend `clone_only_used_by_borrow` with
  `PotentiallyPathSensitive`.

### 13.2. Stage 2: High-confidence rules

- Implement `BOR001`.
- Implement `BOR004`.
- Implement `BOR006`.
- Ship all three behind `--experimental`.
- Permit automatic application only after RFC 0003 validation.

### 13.3. Stage 3: Ownership-transfer rules

- Implement `BOR002`.
- Implement `BOR003`.
- Implement `BOR007`.
- Keep their rewrites review-only.

### 13.4. Stage 4: Structural rules

- Implement `BOR005`.
- Implement `BOR008`.
- Add effect-summary configuration and corpus tuning.

### 13.5. Stage 5: Research rule

- Prototype `BOR009`.
- Measure compile cost, materialization success, and reviewer acceptance.
- Do not include it in `ALL` until its signal is established.

## 14. Alternatives considered

### 14.1. One broad `possible_borrow_workaround` lint

A single lint would be simple to name but difficult to configure, test, explain,
or rewrite. The candidate families have materially different semantics and
risk. It is rejected.

### 14.2. Emit every syntactic candidate and let the checker filter

This would make final diagnostics accurate but could create excessive compiler
work. Strong detector contracts remain necessary.

### 14.3. Restrict the suite to known Polonius problem-case syntax

That would produce high precision but miss the transformed source actually
found in green repositories. The purpose of this RFC is to reverse common
workarounds, not merely recognize canonical examples.

### 14.4. Add only clone lints

Clone pressure is important but does not cover temporary removal, staging,
indices, duplicated passes, or helper splits. It is insufficient.

### 14.5. Treat compiler acceptance as sufficient for automatic fixes

Compilation does not prove equivalent panic, drop, ordering, rollback, or
performance behaviour. The semantic-confidence axis is required.

## 15. Open questions

1. Should `BOR001` recognize configured entry APIs for project-specific stores?
2. Which external collection crates belong in the trusted built-in summary set?
3. Can MIR-derived effect summaries for local methods remain stable enough
   across Whitaker's pinned rustc upgrades?
4. Should `BOR002` reject every candidate containing a potentially panicking
   call, or retain such candidates as review-only?
5. Should `BOR005` require a known mutable-iterator method before emitting an
   intent?
6. How should candidate counts and validation rates be exposed to maintainers?
7. Should `BOR008` be omitted if Clippy already emits an equivalent diagnostic
   for a particular source form?
8. Is `BOR009` better implemented as a standalone refactoring analysis rather
   than a Dylint rule?
9. Should Polonius-only candidates receive a separate `BORP` selector family?
10. Which rule codes should join `DEFAULT` after corpus validation?

## 16. Recommendation

Whitaker should add the nine-rule `BOR` family as rewrite-backed,
validation-gated analysis.

The first implementation should concentrate on repeated lookup, index
round-trips, and duplicated passes. These have bounded source shapes, useful
stable alternatives, and comparatively low semantic risk.

The more ambitious ownership-transfer, snapshot, staging, scope, and helper
rules should still be designed now so that shared intent, effect, and rewrite
infrastructure does not harden around clones alone. Their diagnostics should
remain silent or review-only until RFC 0003 verifies a candidate and corpus
evidence supports promotion.

## References

[^1]: Whitaker ownership-shape lints design:
    `../ownership-shape-lints-design.md`

[^2]: Whitaker Dylint suite design:
    `../whitaker-dylint-suite-design.md`

[^3]: RFC 0003, compiler-validated rewrite checking:
    `0003-compiler-validated-rewrite-checking.md`

[^4]: Rust Project Goals, "Stabilize and model Polonius Alpha":
    <https://rust-lang.github.io/rust-project-goals/2026/polonius.html>
