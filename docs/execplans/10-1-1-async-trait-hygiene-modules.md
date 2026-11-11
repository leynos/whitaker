# Add the async-trait hygiene modules and the `AsyncTraitFamilyIndex` data model (10.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

This plan must be approved before implementation begins. Approval of this plan
also accepts the three design deviations recorded as D-1, D-2, and D-3 in the
`Decision log`.

## Purpose / big picture

Roadmap item 10.1.1 opens the async-trait architecture hygiene work (roadmap
§10). Eight planned lints will reason about "async interface families": a
dyn-facing trait that returns boxed futures, an optional `Native*` sibling
trait that returns `impl Future + Send`, and a blanket adapter that bridges
them, following Axinite ADR 006. All eight lints are to query one shared,
crate-local analysis called `AsyncTraitFamilyIndex`, so that the expensive and
error-prone reasoning is done once, deterministically, and verified once.

This item delivers that shared data model and the modules that hold it. It does
not deliver any lint, any High-level Intermediate Representation (HIR)
extraction, or any configuration; those are roadmap items 10.1.2, 10.1.3, and
10.2 onwards.

After this change a maintainer can:

1. Build an `AsyncTraitFamilyIndex` from plain facts, such as "trait 3 is
   used as `dyn` at site A" and "trait 3 has supertrait 5", in an ordinary unit
   test with no compiler involved. They can query which traits are
   *dyn-required*, meaning used as `dyn` directly or reachable from such a
   trait through supertraits, and see invalid facts rejected with a typed error.
2. Run `make verus-async-trait` and see Verus prove, for all graphs, that
   the dyn-required closure algorithm returns exactly the set of traits
   reachable from the directly dyn-used traits. Verus also proves that
   discarding non-local traits loses nothing.
3. Name the compiler-bound instantiation of the index
   (`HirAsyncTraitFamilyIndex`, keyed by a local `DefIndex` wrapper) from any
   lint crate that enables `whitaker_lint_core/dylint-driver`, ready for
   roadmap item 10.1.2 to populate it from HIR.

Success is observable as: the new unit, property, and behavioural tests pass
under `make test`; `make verus-async-trait` reports all proofs verified;
`make check-fmt` and `make lint` pass; and the documentation describes the
model, its invariants, and its re-use policy.

## Constraints

- Do not add a `dylint-driver` feature, `rustc_private` code, or a
  `rustc_*` dependency to the root `whitaker` package. ADR 0012
  (`docs/adrs/adr-0012-whitaker-cli-boundary.md`) made the root package a
  command-line interface only. Compiler-private driver code lives in
  `crates/whitaker_lint_core` behind its `dylint-driver` feature.
- Keep `whitaker-common` (`common/`) free of `rustc_*` dependencies. It is
  the compiler-independent crate.
- Every collection in the index must iterate in an order that depends only on
  the facts, never on insertion order or hashing. No `HashMap` or `HashSet` may
  appear in the model's stored state or in any iteration that affects output.
- `DefId` and `LocalDefId` implement neither `Ord` nor `PartialOrd`. The
  pinned toolchain `nightly-2026-05-28` declares this at
  `compiler/rustc_span/src/def_id.rs:255-257` and `:375-377`, citing
  rust-lang/rust#90317. The model must not try to place them in `BTreeMap` or
  `BTreeSet` keys, or work around this with `unsafe` code or transmutes.
- No source file may exceed 400 lines (`AGENTS.md`). Plan the module and test
  split up front.
- Every module begins with a `//!` comment. Every function, public or
  private, carries a `///` comment with an imperative summary. Public items
  carry Rustdoc examples that compile as doctests. `whitaker-common` doctests
  run in the workspace doctest lane.
- The workspace denies, among others, `expect_used`, `unwrap_used`,
  `indexing_slicing`, `panic_in_result_fn`, `shadow_reuse`,
  `missing_const_for_fn`, `must_use_candidate`, `needless_pass_by_value`,
  `implicit_hasher`, `cognitive_complexity`, `missing_docs`, and
  `allow_attributes_without_reason` (root `Cargo.toml` `[workspace.lints]`).
  Production code returns `Result` rather than panicking.
- Verus proofs must be substantive. Do not use `assume`, `admit`,
  `#[verifier::external_body]`, `assume_specification`, or any other trusted
  shortcut in the new proof file. The proof must not merely restate its
  conclusion.
- Do not change the behaviour of any existing lint, helper, or gate.
- Do not create an isolated Cargo cache, bypass `RUSTC_WRAPPER`, or run gates
  in parallel for this worktree.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 30 non-ExecPlan
  files changed, or more than 3,000 net lines of code and other documentation.
  This ExecPlan file is excluded from both counts.
- Interface: stop and escalate if any existing public signature in
  `whitaker-common` or `whitaker_lint_core` must change.
- Dependencies: the only permitted manifest change is adding the existing
  workspace dependencies `googletest` and `pretty_assertions` as
  `[dev-dependencies]` of `whitaker-common`. Any other new dependency, or any
  new workspace dependency, triggers escalation.
- Design: stop and escalate if the generic `K`/`S` model cannot express a
  field from design §5.1 without a `rustc` type. Do the same if the
  `LocalItemKey(DefIndex)` adapter cannot be built on the pinned toolchain.
- Proof: stop and escalate if the Verus closure proof is not discharged
  after three focused attempts (about four hours of proof work), or if
  discharging it appears to need a trusted assumption.
- Iterations: stop and escalate if a gate still fails after three fix
  attempts for the same root cause.
- Ambiguity: stop and present options if the boundary between 10.1.1 and
  10.1.2 proves different from the one recorded in D-2 in a way that affects
  the public model.

## Risks

- Risk: Verus has no in-repository precedent for graph reachability, `Set`
  reasoning, or worklist loop invariants, so the closure proof is new ground.
  Severity: medium. Likelihood: medium. Mitigation: milestone EP-M1 writes the
  specification and lemma statements before any production code. The model is
  dense-index based (`Seq<Seq<nat>>` adjacency, `Vec<bool>` visited set) to
  avoid `Set`/`Map` algebra. The proof tolerance escalates rather than trusting
  an assumption.
- Risk: the Verus sidecar is a hand-written mirror of the production kernel,
  so the two could drift. Severity: medium. Likelihood: low. Mitigation: the
  production kernel `reachable_from` stays under about 40 lines with the same
  control flow. Both files carry cross-reference comments. Proptest checks the
  production kernel against an independent fixpoint oracle (obligation
  P-KERNEL-ORACLE). The developers' guide records the trust boundary.
- Risk: ordering by `DefIndex` reintroduces the incremental-compilation
  hazard that made `DefId` unordered upstream. Severity: low. Likelihood: low.
  Mitigation: the index is built and consumed inside one late lint pass, is
  never a query result, and is never persisted. `DefIndex` is `#[orderable]`
  upstream. Record the axiom (A-DEFINDEX) and the rationale in ADR 0014. If
  reviewers prefer `DefPathHash`, the generic model is unaffected; only the
  adapter key changes.
- Risk: unit tests inside `dylint-driver`-gated modules of
  `whitaker_lint_core` do not run in `make test`. The `Makefile` runs that
  crate only with `--no-default-features`. Severity: medium. Likelihood: high.
  Mitigation: keep all logic in the pure `whitaker-common` layer. The gated
  adapter holds only a newtype, conversions, and type aliases; it is compiled
  and linted by `make lint`, which uses Clippy with `--all-features`.
  Behavioural coverage of the adapter arrives with the first lint crate
  (10.2.1). This is a recorded gap, not a hidden one.
- Risk: the 400-line limit forces an early split of the model and its tests.
  Severity: low. Likelihood: high. Mitigation: the file layout in
  `Plan of work` is sized up front, with a fallback split named for each file.
- Risk: an ADR number may collide again. ADR 0013 already exists on an
  unmerged remote branch as `adr-0013-brain-trust-lint-driver-interfaces.md`.
  Severity: low. Likelihood: low. Mitigation: re-fetch and re-scan all remote
  branches immediately before committing ADR 0014.

## Progress

- [x] (2026-11-11 17:20Z) Renamed the branch to
  `10-1-1-async-trait-hygiene-modules`. No remote branch of that name existed
  yet.
- [x] (2026-11-11 17:35Z) Reconnaissance: root crate, `whitaker_lint_core`,
  verification tooling, and documentation conventions.
- [x] (2026-11-11 17:45Z) Confirmed from pinned compiler sources that `DefId`
  and `LocalDefId` are `!Ord`, and that `DefIndex` is orderable.
- [x] (2026-11-11 17:50Z) Fetched remote branches; ADR 0013 is taken, so the
  next free ADR is 0014.
- [x] (2026-11-11 18:10Z) Drafted this ExecPlan.
- [ ] Expert review and revision of this ExecPlan.
- [ ] User approval of this ExecPlan, including deviations D-1 to D-3.
- [ ] EP-M0: baseline gates on the unmodified branch.
- [ ] EP-M1: Verus specification, lemmas, and proof for the closure kernel.
- [ ] EP-M2: pure value types, facts builder, and validation.
- [ ] EP-M3: closure kernel, derived fields, and sealed index queries.
- [ ] EP-M4: `dylint-driver` adapter in `whitaker_lint_core`.
- [ ] EP-M5: documentation, ADR 0014, roadmap update, and final gates.

## Surprises & discoveries

- Observation: the root `whitaker` package no longer has a `dylint-driver`
  feature or any HIR helper. Evidence: root `Cargo.toml:88-131` has no
  `[features]` table. `src/lib.rs` declares only `adapters`, `cli`,
  `diagnostics`, and `ports`. ADR 0012 and
  `docs/execplans/3-5-1-root-whitaker-binary.md` record the move of the driver
  library to `crates/whitaker_lint_core`. Impact:
  `whitaker::async_trait_hygiene`, as written in roadmap 10.1.1 and design
  §6.1, cannot be built. See D-1.
- Observation: `DefId` and `LocalDefId` are deliberately unordered.
  Evidence: `impl !Ord for DefId {}` and `impl !Ord for LocalDefId {}` in the
  pinned compiler source. Impact: every `BTreeMap<DefId, …>` field in design
  §5.1 is unbuildable as written. See D-3.
- Observation: `dylint-driver`-gated unit tests in `whitaker_lint_core` are
  not exercised by `make test`. Evidence: `Makefile:37-46` excludes
  `whitaker_lint_core` from the all-features pass and re-runs it with
  `--no-default-features`. Impact: logic must live in `whitaker-common`. See
  D-1 and the adapter risk.
- Observation: `docs/developers-guide.md:3730-3743` ("Adapter layer —
  `whitaker` (feature `dylint-driver`)") is stale since ADR 0012. Evidence: no
  `whitaker::hir` module exists. The helpers are `whitaker_lint_core::hir`.
  Impact: EP-M5 corrects it in its own documentation commit, because the new
  async-trait section sits beside it and would otherwise contradict it.

## Decision log

- Decision D-1 (architecture deviation, needs approval): place the
  compiler-independent model in `whitaker_common::async_trait_hygiene`
  (`common/src/async_trait_hygiene/`). Place the compiler-bound adapter in
  `whitaker_lint_core::async_trait_hygiene`
  (`crates/whitaker_lint_core/src/async_trait_hygiene.rs`) behind
  `dylint-driver`. Rationale: the design's target crate no longer hosts
  compiler code (ADR 0012). The design's own reason for avoiding `common` was
  to keep compiler dependencies out of it, and that is preserved because the
  model is generic over its key and site types. Gated unit tests do not run in
  the gate, so testable logic must not live behind the feature. This split
  mirrors the existing span-recovery helpers: a pure policy in
  `whitaker_common::rstest` and an adapter in `whitaker_lint_core::hir`.
  Affected identifiers: ATH-PLACE (design §6.1) and RM-10.1.1 (roadmap wording
  `whitaker::async_trait_hygiene`). Upstream changes: design §6.1 amended; ADR
  0014; roadmap 10.1.1 wording corrected when it is marked done. Options
  rejected: (a) everything in `whitaker_lint_core` behind the feature, which
  leaves the logic untested by the gate; (b) everything ungated in
  `whitaker_lint_core`, which is viable but splits the repository's "pure
  models live in `common`" convention, and the `proofs.yml` path filter covers
  `common/**` but not `crates/whitaker_lint_core/**`. Approving authority: the
  user, at plan approval. Date/Author: 2026-11-11, planning agent.
- Decision D-2 (scope interpretation, needs approval): 10.1.1 owns the
  *pure* derivations that the model's representation invariants require. That
  means the dyn-required closure kernel (`dyn_required`) and the impl-count
  aggregation (`impl_counts`). Roadmap 10.1.2 keeps everything that reads the
  compiler or classifies syntax: the `hir::TyKind::TraitObject` visitor,
  supertrait-edge extraction from predicates, boxed-future alias shape
  matching, native sibling lookup by naming convention, and blanket adapter
  discovery. Rationale: the user requires proof-sketch-first planning, and the
  model's central invariant is "`dyn_required` is exactly the supertrait
  closure of the directly dyn-used traits". A model that stores `dyn_required`
  without deriving it would accept inconsistent states. A model that leaves it
  empty until 10.1.2 is not a data model. Validating a supplied `dyn_required`
  for leastness costs as much as computing it. Sibling and adapter
  relationships remain *supplied facts* in 10.1.1, validated for referential
  consistency. Their classification depends on configuration (10.1.3) and
  syntax (10.1.2). Upstream change: roadmap 10.1.2 wording is annotated at
  completion to say that the closure kernel landed in 10.1.1 and 10.1.2 feeds
  it. Date/Author: 2026-11-11, planning agent.
- Decision D-3 (representation deviation, needs approval): key the generic
  model by `K: Ord + Copy + Debug` and record use sites with a generic
  `S: Clone`. The driver instantiation uses `LocalItemKey`, a `#[derive(Ord)]`
  newtype over `rustc_span::def_id::DefIndex` for items of the local crate, and
  `rustc_span::Span` for sites. Method shapes are keyed by
  `MethodKey<K> { trait_key, method_key }` rather than `(DefId, Symbol)`.
  Supertraits are stored as `BTreeSet<K>` rather than `Vec<DefId>`. `impls` is a
  `BTreeMap<K, ImplInfo<K>>` keyed by the impl item rather than a `Vec`.
  Rationale: `DefId` is `!Ord` (see `Surprises`). `Symbol`'s `Ord` compares
  interner indices, not text, so it is deterministic but meaningless as a key,
  while the method's own item key is unique. Sets and maps make duplicate edges
  or impls unrepresentable rather than merely discouraged. Restricting keys to
  local items is justified by the local-restriction lemma (V-RESTRICT) and
  axiom A-CRATE-DAG. Only local traits can be family members, and no foreign
  trait can have a local supertrait. Date/Author: 2026-11-11, planning agent.
- Decision D-4: do not add Kani harnesses in this item.
  Rationale: Verus establishes the unbounded closure property. Proptest checks
  the production kernel against an independent oracle. The repository documents
  CBMC state explosion on `Vec` and iterator code
  (`scripts/run-kani.sh:112-158`), so a bounded check of the same property
  would add cost without adding a distinct guarantee. Date/Author: 2026-11-11,
  planning agent.
- Decision D-5: do not add `insta` snapshots in this item.
  Rationale: the model renders no output format. Diagnostics, the first
  multivariant output, arrive in 10.2.1, which is where snapshots belong.
  Determinism, the property a snapshot would guard, is checked more strongly by
  the insertion-order property test (P-ORDER). Date/Author: 2026-11-11,
  planning agent.
- Decision D-6: make no `docs/users-guide.md` change in this item.
  Rationale: no lint, configuration key, command, or user-visible behaviour
  changes. `whitaker-common` is described as "Internal common utilities for
  Whitaker lints", and its consumers are Whitaker's own lint crates, which are
  documented in the developers' guide. The users' guide changes in 10.4.3 when
  lints ship. Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. This section is completed at each milestone boundary and at
completion.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint libraries into a `rustc`
driver. Lint crates live under `crates/`, and each is a `cdylib` that links
compiler-private (`rustc_private`) crates only when its `dylint-driver` feature
is enabled. This keeps ordinary unit tests free of compiler linkage.

Three crates matter here.

- `common/` is the package `whitaker-common`, imported as
  `whitaker_common`. It holds compiler-independent helpers and pure models, for
  example `common/src/lcom4/`, `common/src/rstest/`, and
  `common/src/decomposition_advice/`. Its behavioural tests live in
  `common/tests/*_behaviour.rs` with Gherkin files in `common/tests/features/`.
  Its property tests use `proptest`.
- `crates/whitaker_lint_core/` is the compiler-private driver library, with
  `publish = false`. Its `dylint-driver` feature
  (`crates/whitaker_lint_core/Cargo.toml:9-15`) enables `rustc_ast`,
  `rustc_hir`, `rustc_lint`, `rustc_span`, and `dylint_linting`.
  `crates/whitaker_lint_core/src/lib.rs` gates `pub mod hir;` on that feature.
  `make test` runs this crate only without features.
- The root package `whitaker` is the command-line interface (ADR 0012) and
  is not touched.

Verus proofs live in `verus/*.rs` as self-contained, implementation-shaped
models (`docs/developers-guide.md`, "Verus scope and trust boundary").
`scripts/run-verus.sh <group>` runs them; `make verus` runs every group. The
`.github/workflows/proofs.yml` workflow runs them nightly and on pull requests
that touch `verus/**` or `common/**`.

Terms used in this plan:

- *Dyn-facing trait*: a trait used behind `dyn`, whose async methods return
  boxed futures such as `Pin<Box<dyn Future<Output = T> + Send + 'a>>`.
- *Native sibling*: a trait `NativeX` beside dyn-facing trait `X`, whose
  methods return `impl Future<Output = T> + Send`.
- *Blanket adapter*: `impl<T: NativeX + Send + Sync> X for T`, which boxes
  each native future.
- *Dyn-used trait*: a local trait named in at least one `dyn Trait` type in
  the crate.
- *Dyn-required trait*: a dyn-used trait, or any trait reachable from one
  through supertrait edges. A `dyn A` object can call the methods of every
  supertrait of `A`, so every such supertrait must stay dyn-compatible.
- *Supertrait edge*: `A → B` when `A` declares `B` as a supertrait, either
  as `trait A: B` or as `where Self: B`. The Rust Reference states that these
  two forms are equivalent.
- *Representation invariant*: a property every constructed index value
  satisfies, which accessors may rely on.
- *Closure kernel*: the function that computes dyn-required traits from
  dyn-used traits and supertrait edges.

### Relevant skills and documents

Skills: `execplans` (this format), `rust-router` (entry), `arch-crate-design`
(crate placement), `rust-types-and-apis` (generic key design and newtypes),
`rust-unit-testing` (rstest, googletest, pretty_assertions), `proptest`,
`verus`, `rust-verification` (method selection), `arch-decision-records` (ADR
0014), `nextest` (running focused tests), and `en-gb-oxendict-style`
(documentation prose).

Documents:

- `docs/async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md`
  (§5.1 fields, §5.2 algorithms, §6.1 placement).
- `docs/whitaker-dylint-suite-design.md` (suite architecture and the
  `dylint-driver` gating convention).
- `docs/whitaker-clone-detector-design.md` (precedent for pure models with
  Verus and Kani sidecars).
- `docs/rust-testing-with-rstest-fixtures.md` (fixtures and parameterized
  cases).
- `docs/rstest-bdd-users-guide.md` (behavioural tests, step rules).
- `docs/rust-doctest-dry-guide.md` (doctest style for the new public items).
- `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep the
  builder validation free of bumpy-road conditionals).
- `docs/developers-guide.md` ("Verus scope and trust boundary" and "Shared
  span recovery helpers").
- `docs/adrs/adr-0012-whitaker-cli-boundary.md`.
- `docs/documentation-style-guide.md` (ADR template and Markdown rules).

## Conformance basis

Upstream artefacts, pinned at branch base commit `d9eb8a09` (2026-11-11):

- Roadmap `docs/roadmap.md` §10.1, item 10.1.1 (RM-10.1.1), which requires
  1.1.1 (done). Items 10.1.2 (RM-10.1.2) and 10.1.3 (RM-10.1.3) bound the scope.
- Design
  `docs/async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md`
  (status "Proposed"), last changed in `bd4c93a8`.
- ADR 0012 `docs/adrs/adr-0012-whitaker-cli-boundary.md` (crate
  boundary).
- Axinite ADR 0006, "Dual-trait pattern for dyn-backed async interfaces"
  (external, accepted 2026-03-21), the source of the family shape.
- `AGENTS.md` and the root `Cargo.toml` lint policy (governing standards).
- No Terms of Reference exist for this suite. None is invented here.

Traced design elements:

- ATH-FIELDS: design §5.1, the eleven index fields.
- ATH-DET: design §5.1 determinism note (`BTreeMap`/`BTreeSet`).
- ATH-CLOSURE: design §5.2.2,
  `dyn_required = dyn_used ∪ supertraits_transitive(dyn_used)`.
- ATH-PLACE: design §6.1, shared helper placement.
- ATH-LOCAL: design §1 and §3.3, single-crate scope.

Trace links:

```plaintext
RM-10.1.1 -> ATH-PLACE  -> D-1 -> EP-M2, EP-M4 -> common/src/async_trait_hygiene/, whitaker_lint_core::async_trait_hygiene
RM-10.1.1 -> ATH-FIELDS -> D-3 -> EP-M2, EP-M3 -> index accessor tests; P-WELLFORMED
RM-10.1.1 -> ATH-DET    -> EP-M3 -> P-ORDER
RM-10.1.1 -> ATH-CLOSURE -> D-2 -> EP-M1, EP-M3 -> V-CLOSURE (Verus); P-KERNEL-ORACLE; BDD scenarios 1-3
RM-10.1.1 -> ATH-LOCAL  -> D-3 -> EP-M1, EP-M4 -> V-RESTRICT (Verus); LocalItemKey
```

Field mapping from design §5.1 to the model:

| Design field           | Model field and type                          | Source in 10.1.1                         |
| ---------------------- | --------------------------------------------- | ---------------------------------------- |
| `traits`               | `BTreeMap<K, TraitInfo<K>>`                   | supplied fact                            |
| `dyn_uses`             | `BTreeMap<K, Vec<S>>` (each vector non-empty) | supplied fact                            |
| `supertraits`          | `BTreeMap<K, BTreeSet<K>>`                    | supplied fact                            |
| `dyn_required`         | `BTreeSet<K>`                                 | derived by the closure kernel            |
| `boxed_future_aliases` | `BTreeMap<K, BoxedFutureAliasInfo>`           | supplied fact                            |
| `trait_method_shapes`  | `BTreeMap<MethodKey<K>, AsyncMethodShape<K>>` | supplied fact                            |
| `impls`                | `BTreeMap<K, ImplInfo<K>>`                    | supplied fact                            |
| `adapter_impls`        | `BTreeMap<K, AdapterInfo<K>>`                 | supplied fact, validated against `impls` |
| `native_siblings`      | `BTreeMap<K, NativeSiblingInfo<K>>`           | supplied fact, validated                 |
| `impl_counts`          | `BTreeMap<K, ImplCounts>`                     | derived from `impls`                     |
| `diagnostic_paths`     | `BTreeMap<K, String>`                         | supplied fact                            |

Table: Design §5.1 fields mapped to the 10.1.1 model.

Not discharged by this item: HIR extraction and syntactic classification
(RM-10.1.2), configuration (RM-10.1.3), and any lint behaviour (RM-10.2 and
later).

## Abstract model and proof sketch

The model is specified independently of rustc first, so that the API can be
pressure-tested against it.

### Abstract state

Let `T` be the finite set of local trait keys. Let `E ⊆ T × T` be the
supertrait relation, where `(a, b) ∈ E` means `a` declares supertrait `b`. Let
`U ⊆ T` be the dyn-used traits. Then:

```plaintext
Reach(U, E) = { y ∈ T | ∃ u ∈ U, ∃ path u = x0 → x1 → … → xk = y with (xi, xi+1) ∈ E }
dyn_required = Reach(U, E)          (k = 0 admits U ⊆ dyn_required)
```

`Reach(U, E)` is equivalently the least set `R` with `U ⊆ R` that is closed
under `E`, meaning `x ∈ R ∧ (x, y) ∈ E ⇒ y ∈ R`.

### Representation invariants of `AsyncTraitFamilyIndex<K, S>`

- RI-1 (referential integrity): every key referenced by `dyn_uses`,
  `supertraits` (both ends), `trait_method_shapes` (trait part),
  `impls[i].trait_key`, `adapter_impls` (both ends), `native_siblings` (both
  ends), and `diagnostic_paths` is a key of `traits`. Exception:
  `diagnostic_paths` may also name alias or impl keys, so its rule is "names a
  known trait, alias, method, or impl key".
- RI-2 (closure): `dyn_required = Reach(dom(dyn_uses), supertraits)`.
- RI-3 (counts): for every trait `t`, `impl_counts(t)` equals the
  per-`SelfTyKind` count of `impls` whose `trait_key` is `t`. Traits with no
  impls report `ImplCounts::default()`.
- RI-4 (adapter coherence): if `adapter_impls[t] = a`, then
  `a.impl_key ∈ impls`, `impls[a.impl_key].trait_key = t`,
  `impls[a.impl_key].self_ty` is a type parameter whose bounds contain
  `a.native_trait`, and `native_siblings[t].native_trait = a.native_trait`.
- RI-5 (sibling sanity): no trait is its own sibling, and the sibling
  relation is injective, so no native trait serves two dyn-facing traits.
- RI-6 (non-empty sites): every `dyn_uses` vector is non-empty.
- RI-7 (determinism): the built index is a function of the *set* of facts.
  Supplying the same facts in any order yields an equal index.

### Lemmas (hypotheses about the eventual proof structure)

- L-SOUND (worklist soundness): throughout the worklist algorithm, every
  visited node is in `Reach(U, E)`.
- L-CLOSED (closed-superset containment): if `U ⊆ S` and `S` is closed under
  `E`, then `Reach(U, E) ⊆ S`. Proved by induction on path length.
- L-EXIT (exit closure): when the worklist is empty, the visited set
  contains `U` and is closed under `E`. This is maintained by the invariant
  "every visited node not on the worklist has all its successors visited".
- L-TERM (termination): the pair `(unvisited count, worklist length)`
  decreases lexicographically on every iteration.
- V-CLOSURE (main theorem): the kernel returns exactly `Reach(U, E)`. This
  follows from L-SOUND, L-EXIT, and L-CLOSED.
- V-RESTRICT (local restriction): partition `V = L ⊎ F` (local and
  foreign). If no edge goes from `F` to `L`, then
  `Reach(U, E) ∩ L = Reach(U ∩ L, E|L×L)`. The proof inducts on path length,
  showing that a path ending in `L` cannot contain an `F` node. Once a path
  leaves `L` it cannot return, so a path ending in `L` lies wholly in `L`. This
  lemma justifies D-3's decision to drop foreign traits and foreign supertrait
  edges before running the closure kernel.

### API pressure test

The specification leads directly to these shape decisions:

1. `dyn_required` and `impl_counts` have no setter. They are derived inside
   `AsyncTraitFamilyFacts::build`, so RI-2 and RI-3 hold by construction.
2. All index fields are private and exposed through query methods.
   Consumers cannot break RI-1 to RI-7 after construction.
3. The kernel works on dense indices (`&[Vec<usize>]` adjacency and
   `&[usize]` roots, returning `Vec<bool>`). That is the representation Verus
   proves. The generic layer maps `K` to dense indices through the sorted order
   of `traits`, which also discharges RI-7 for `dyn_required`.
4. Validation happens once, in `build`, after all facts are known, so facts
   may arrive in any order. This is required because the HIR visitor in 10.1.2
   may meet an impl before its trait.
5. RI-1 rejects dangling keys rather than silently dropping them. Filtering
   to local items is the adapter's job in 10.1.2, and V-RESTRICT shows it is
   lossless. The pure model therefore treats a dangling key as a caller defect.

## Verification plan

The relationship between proof and tests is as follows. Verus establishes the
unbounded semantic property of a hand-mirrored model of the closure kernel.
Proptest connects the real production kernel to that property by comparing it
with an independent oracle. rstest and rstest-bdd cover the executable API, the
validation errors, and realistic family shapes. None of these replaces the
others. The first end-to-end evidence through `rustc` arrives in 10.2.1, when a
lint populates the index.

### Axioms

- A-CRATE-DAG: a trait defined in crate `C` can name as supertraits only
  traits from `C` or from crates `C` depends on. Crate dependencies are
  acyclic, so no foreign trait has a local supertrait. Source: Rust crate and
  name-resolution model. Used by V-RESTRICT's precondition, "no edge `F → L`".
- A-DYN-SUPER: a `dyn A` trait object can call the methods of `A`'s
  supertraits, and every supertrait of a dyn-compatible trait must itself be
  dyn-compatible. Source: the Rust Reference, "Traits" (dyn compatibility).
  Used to justify ATH-CLOSURE.
- A-SUPER-FORMS: `trait A: B` and `where Self: B` are equivalent supertrait
  declarations (Rust Reference). This is relied on by 10.1.2's extraction, and
  is recorded here so that the model's edge relation is not specified as
  "syntactic bounds only".
- A-DEFINDEX: within one compilation session, `DefIndex` values for local
  items are unique and are assigned deterministically for identical input, and
  `DefIndex` implements `Ord` (`#[orderable]`). Used by `LocalItemKey` and RI-7.
- A-BTREE: `BTreeMap` and `BTreeSet` iterate in ascending key order (standard
  library contract).

### Obligations

- Obligation V-CLOSURE: for all well-formed adjacency `adj` (every successor
  index `< n`) and roots `r` (every root `< n`), the kernel model returns
  `visited` with `visited.len() == n` and
  `∀ y < n. visited[y] ⇔ reachable(adj, r, y)`. Method: Verus deductive proof
  of an exec-mode model with loop invariants. Rationale: the property must hold
  for all graph sizes and shapes, including cycles, which Rust's E0391 forbids
  in source but which the pure model must still handle. This is the contractual
  core that every one of the eight lints inherits. Domain: all `n`, all
  adjacency lists, all root lists. Artefact:
  `verus/async_trait_family_closure.rs`. Evidence: `make verus-async-trait`
  prints a verification summary with zero errors. Before the proof bodies
  exist, the same command fails with unproved postconditions. That is the red
  step. Non-vacuity: proof functions exhibit a witness graph (chain
  `0 → 1 → 2`, isolated node `3`, root `0`) and prove `reachable(…, 2)` and
  `!reachable(…, 3)`, so the precondition is inhabited and the postcondition
  distinguishes both outcomes. Two seeded mutations must make verification
  fail, and both runs are recorded in `Outcomes & retrospective`: (a) omitting
  the push of a newly visited successor onto the worklist, which breaks
  completeness; and (b) seeding `visited` with every node, which breaks
  soundness. The proof file must contain no `assume`, `admit`, or
  `external_body`, which a `grep` in EP-M1 acceptance confirms.
- Obligation V-RESTRICT: as stated under `Lemmas`.
  Method: Verus lemma over the same `reachable` specification. Rationale: this
  is an introduced lemma that justifies a design decision (D-3), so it is cheap
  to prove and expensive to get wrong. Artefact: the same proof file. Evidence:
  as for V-CLOSURE. Non-vacuity: a witness with one foreign node `f`, a local
  edge `a → b`, and an edge `a → f` (local to foreign, which is allowed) shows
  the hypothesis is satisfiable with a non-empty `F`. A seeded counterexample
  that adds an edge `f → b` and roots at `f` makes `b` reachable in the full
  graph but not in the restricted one. Proving the lemma's conclusion without
  the hypothesis must fail, which demonstrates that the hypothesis is used.
- Obligation P-KERNEL-ORACLE: the production `reachable_from` agrees with an
  independent oracle, a naive fixpoint that repeatedly adds successors until
  nothing changes, written separately in the test module. Method: proptest.
  Rationale: this bridges the hand-mirrored Verus model to the compiled code.
  Domain: `n ∈ 0..=12`, per-node successor lists drawn from `0..n` (including
  self-loops and cycles), and root lists drawn from `0..n` (including empty
  lists and duplicates). Out-of-range indices are tested separately by a named
  case, because the production kernel ignores them via `.get()`. Artefact:
  `common/src/async_trait_hygiene/tests/closure_props.rs`. Evidence:
  `cargo nextest run -p whitaker-common async_trait_hygiene` passes with the
  default 256 cases. Non-vacuity: no `prop_assume` filtering. Named rstest
  cases in `closure_cases.rs` pin each class: empty graph, empty roots,
  self-loop, two-cycle, diamond, disconnected component, and every node a root.
  A seeded mutation that replaces the kernel body with "roots only" must fail
  the property, and that run is recorded.
- Obligation P-WELLFORMED: for generated fact sets, which include deliberately
  dangling references, `build()` returns `Ok` exactly when the facts satisfy
  RI-1, RI-4, RI-5, and RI-6. Every `Ok` index satisfies RI-1 to RI-6, as
  checked by `assert_well_formed`, a test-only checker written independently of
  `build`. Method: proptest plus rstest tables of named error cases. Artefact:
  `common/src/async_trait_hygiene/tests/build_props.rs` and `build_cases.rs`.
  Non-vacuity: the generator mixes valid and dangling keys with a tunable
  probability. Assert that both outcomes occur (`Ok` and each `Err` variant)
  across a fixed-seed run of the named-case table, rather than by filtering.
  Seeded fault: removing the adapter-coherence check from `build` must make the
  property fail.
- Obligation P-ORDER (RI-7): building from a shuffled permutation of the
  same facts yields an index equal (`==`) to building from the original order.
  Method: proptest with `prop_shuffle`. Artefact: `build_props.rs`.
  Non-vacuity: fact vectors have length at least 2. Seeded fault: storing
  `dyn_uses` sites in a `Vec` without the documented site ordering must make
  the property fail. The plan fixes site order as "the order supplied, grouped
  by trait", so this property generates permutations that preserve each trait's
  site order and permute everything else. The documentation states that site
  order is caller-defined.
- Obligation T-QUERIES: each accessor returns what the facts say: the
  default `ImplCounts` for a trait without impls, `None` for an unknown key,
  and supertraits in ascending key order. Method: rstest named cases with
  `googletest` matchers and `pretty_assertions` equality. Artefact:
  `common/src/async_trait_hygiene/tests/index_queries.rs`.
- Obligation B-FAMILIES: realistic ADR 006 family shapes behave as a
  maintainer expects. See the feature file under `Plan of work`. Method:
  rstest-bdd. Artefact:
  `common/tests/features/async_trait_family_index.feature` and
  `common/tests/async_trait_family_index_behaviour.rs`.
- Obligation C-ADAPTER: `LocalItemKey` round-trips `LocalDefId` and orders
  as `DefIndex` does. The aliases instantiate the model with `Span`. Method:
  compile-time evidence only. `make lint` builds and lints `whitaker_lint_core`
  with `--all-features`, and the doctest is marked `ignore` with a stated
  reason because doctests cannot link `rustc_private`. Residual gap: no runtime
  test in the gate until 10.2.1. EP-M4 records, as supplementary evidence only,
  whether `cargo test -p whitaker_lint_core --features dylint-driver --lib`
  runs under the `make test` `RUSTFLAGS`.

Verus is used only for V-CLOSURE and V-RESTRICT. Builder validation, accessors,
error construction, and the adapter newtype are straightforward control flow
and data plumbing. Giving them proof obligations would be artificial, so tests
give them proportionate assurance.

## Plan of work

### Stage A: pre-flight (no code changes)

Run the baseline gates (EP-M0). Re-fetch remote branches and confirm ADR 0014
is still free.

### Stage B: proof first (EP-M1)

Create `verus/async_trait_family_closure.rs` containing the following:

- the spec functions `edge(adj, x, y)`, `reachable_in(adj, roots, y, k)`
  (recursive on `k`), and `reachable(adj, roots, y)` (existential over `k`);
- the exec function
  `reachable_from(adj: &Vec<Vec<usize>>, roots: &Vec<usize>) -> (visited: Vec<bool>)`,
  which mirrors the production kernel line for line, with `requires` and
  `ensures` as in V-CLOSURE;
- the lemmas L-SOUND, L-CLOSED, L-EXIT, and V-RESTRICT as `proof fn`s; and
- the non-vacuity witnesses.

Write the specification, signatures, and `ensures` clauses first, then run
`make verus-async-trait` to see it fail. Then add loop invariants and lemma
bodies until it passes.

Wire the proof in:

- add an `async-trait` group to `scripts/run-verus.sh`, and the new file to
  the `all` group;
- add a `verus-async-trait` target to `Makefile` and to `.PHONY`; and
- check that `.github/workflows/proofs.yml` already triggers on `verus/**`.
  It does, so no workflow change is needed.

### Stage C: pure model (EP-M2, EP-M3)

The file layout below has each file sized to stay under 400 lines.

- `common/src/async_trait_hygiene/mod.rs` (about 80 lines): module
  documentation stating scope, re-use policy, and invariants, plus re-exports.
- `common/src/async_trait_hygiene/model.rs` (about 250 lines): value types.
  - `TraitInfo<K> { name: String, parent: K, is_from_expansion: bool }`.
  - `MethodKey<K> { trait_key: K, method_key: K }`, deriving `Ord`.
  - `AsyncMethodShape<K> { name: String, return_shape: ReturnShape<K>,
    is_dispatchable: bool, borrowed_input_count: u8,
    has_async_trait_lifetime: bool }`. `is_dispatchable` is false when the
    method has `where Self: Sized`. `borrowed_input_count` counts reference
    inputs, including the receiver.
  - `ReturnShape<K>`: `BoxedFutureAlias { alias: K, send: SendBound,
    lifetime: LifetimeBound }`, `RawBoxedFuture { send, lifetime }`,
    `ImplFuture { send, lifetime }`, `AsyncFn`, and `NotAsync`.
  - `SendBound`: `Send` or `NotSend`.
  - `LifetimeBound`: `Absent`, `Elided` (meaning `'_`), `Named(String)`, or
    `Static`.
  - `BoxedFutureAliasInfo { name: String, send: SendBound,
    has_lifetime_param: bool }`.
  - `ImplInfo<K> { trait_key: K, self_ty: SelfTyKind<K>,
    is_from_expansion: bool }`.
  - `SelfTyKind<K>`: `TypeParam { bounds: BTreeSet<K> }`, `Concrete`, and
    `Other`.
  - `AdapterInfo<K> { impl_key: K, native_trait: K }`.
  - `NativeSiblingInfo<K> { native_trait: K }`.
  - `ImplCounts { type_param: usize, concrete: usize, other: usize }`, with
    `Default`.

  These shape types are data only. The classifiers that produce them belong to
  10.1.2 (`shape.rs`, per design §6.1). If the file exceeds 400 lines, split it
  into `model/family.rs` and `model/shape.rs`.
- `common/src/async_trait_hygiene/error.rs` (about 90 lines):
  `FamilyIndexError<K>`, derived with `thiserror`. Its variants are
  `DuplicateTrait { key }`, `DuplicateImpl { key }`, `DuplicateMethod { key }`,
  `UnknownTrait { key, referenced_by: FactKind }`, `UnknownImpl { key }`,
  `SelfSibling { key }`, `SiblingNotInjective { native_trait, first, second }`,
  and `AdapterIncoherent { dyn_trait, reason: AdapterDefect }`. `FactKind` and
  `AdapterDefect` are small `Copy` enums, so every error is inspectable without
  string matching.
- `common/src/async_trait_hygiene/facts.rs` (about 250 lines):
  `AsyncTraitFamilyFacts<K, S>` holds facts in insertion order. Its command
  methods return `()`: `add_trait`, `add_dyn_use`, `add_supertrait`,
  `add_boxed_future_alias`, `add_method_shape`, `add_impl`,
  `add_native_sibling`, `add_adapter`, and `add_diagnostic_path`. Its consuming
  query
  `build(self) -> Result<AsyncTraitFamilyIndex<K, S>, FamilyIndexError<K>>`
  validates in a fixed order: duplicates, references, siblings, then adapters.
  Validation is a sequence of small single-responsibility functions, each
  returning `Result`, so no function grows a bumpy-road conditional ladder.
- `common/src/async_trait_hygiene/closure.rs` (about 90 lines):
  `pub(crate) fn reachable_from(adjacency: &[Vec<usize>], roots: &[usize])
  -> Vec<bool>`,
  which mirrors the Verus model, plus
  `pub(crate) fn dyn_required_closure<K>(…) -> BTreeSet<K>`. That function maps
  keys to dense indices by their rank in `traits`, calls the kernel, and maps
  back.
- `common/src/async_trait_hygiene/index.rs` (about 220 lines): the sealed
  `AsyncTraitFamilyIndex<K, S>`, deriving `Debug`, `Clone`, `PartialEq`, and
  `Eq` where `S` allows. Its query methods are `traits`, `trait_info`,
  `dyn_use_sites`, `supertraits_of`, `is_dyn_required`, `dyn_required`,
  `boxed_future_alias`, `boxed_future_aliases`, `method_shapes_of`, `impls`,
  `impl_info`, `adapter_for`, `native_sibling_of`, `impl_counts`, and
  `diagnostic_path`. Each returns borrowed data or an iterator in key order.
- Tests in `common/src/async_trait_hygiene/tests/` with a `mod.rs`:
  `support.rs` (fact-building fixtures and `assert_well_formed`),
  `closure_cases.rs`, `closure_props.rs`, `build_cases.rs`, `build_props.rs`,
  and `index_queries.rs`. Each file stays under 400 lines.
- `common/src/lib.rs`: add `pub mod async_trait_hygiene;` and re-export the
  public types, following the existing `pub use` blocks.
- `common/Cargo.toml`: add `googletest = { workspace = true }` and
  `pretty_assertions = { workspace = true }` under `[dev-dependencies]`.

Behavioural specification, in
`common/tests/features/async_trait_family_index.feature`:

```gherkin
Feature: Async-trait family index
  The family index records dyn-backed async interface families for one crate
  and derives which traits must stay dyn-compatible.

  Scenario: A dyn-used trait makes its supertraits dyn-required
    Given a trait named Database
    And a trait named ConversationStore
    And Database declares supertrait ConversationStore
    And Database is used as dyn
    When the family index is built
    Then Database is dyn-required
    And ConversationStore is dyn-required

  Scenario: Supertrait inheritance is followed transitively
    Given a trait named Tool
    And a trait named Describe
    And a trait named Named
    And Tool declares supertrait Describe
    And Describe declares supertrait Named
    And Tool is used as dyn
    When the family index is built
    Then Named is dyn-required

  Scenario: A trait with no dyn use is not dyn-required
    Given a trait named Database
    And a trait named SuccessEvaluator
    And Database is used as dyn
    When the family index is built
    Then SuccessEvaluator is not dyn-required

  Scenario: A complete ADR 006 family is recorded
    Given a trait named McpTransport
    And a trait named NativeMcpTransport
    And NativeMcpTransport is the native sibling of McpTransport
    And a blanket adapter implements McpTransport for NativeMcpTransport
    When the family index is built
    Then McpTransport has a blanket adapter
    And McpTransport has 1 type-parameter impl

  Scenario: An adapter without a native sibling is rejected
    Given a trait named Tool
    And a trait named NativeTool
    And a blanket adapter implements Tool for NativeTool
    When the family index is built
    Then the build is rejected as an incoherent adapter

  Scenario: A supertrait that is not a recorded trait is rejected
    Given a trait named Tool
    And Tool declares supertrait Missing
    When the family index is built
    Then the build is rejected as an unknown trait
```

The step definitions in `common/tests/async_trait_family_index_behaviour.rs`
follow `common/tests/decomposition_adjacency_behaviour.rs`. The world is a
`RefCell` fixture named `world`, not `_world`, annotated with
`#[allow_fixture_expansion_lints]` and `#[fixture]`. `given` and `when` steps
only mutate the world. `then` steps return `Result<(), String>` and report
failure with `Err`, never with `assert!`. Trait names map to keys through a
name-to-`u32` table in the world. `#[scenario(index = N)]` attributes follow
declaration order. Each step parses at most three values.

### Stage D: adapter (EP-M4)

Create `crates/whitaker_lint_core/src/async_trait_hygiene.rs` (about 120
lines). It holds the newtype `LocalItemKey(DefIndex)`, deriving `Clone`, `Copy`,
`Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, with these
methods:

- `const fn from_local_def_id(id: LocalDefId) -> Self`;
- `const fn local_def_id(self) -> LocalDefId`;
- `fn def_id(self) -> DefId`.

It also defines
`pub type HirAsyncTraitFamilyFacts = AsyncTraitFamilyFacts<LocalItemKey, Span>;`
and
`pub type HirAsyncTraitFamilyIndex = AsyncTraitFamilyIndex<LocalItemKey, Span>;`.
Register the module in `crates/whitaker_lint_core/src/lib.rs` under
`#[cfg(feature = "dylint-driver")] pub mod async_trait_hygiene;`.

There is no HIR traversal here; that is 10.1.2. Before using any rustc paths,
confirm them in the pinned source:
`rustc_span::def_id::{DefIndex, LocalDefId, DefId}`, the public field in
`LocalDefId { local_def_index }`, and `LocalDefId::to_def_id`.

### Stage E: documentation (EP-M5)

- Write ADR 0014,
  `docs/adrs/adr-0014-async-trait-family-index-representation.md`. It records
  D-1, D-3, A-DEFINDEX, and the rejected options (`DefPathHash` keys,
  `whitaker_lint_core`-only placement, and storing `dyn_required` as a supplied
  fact), using the ADR template in `docs/documentation-style-guide.md`.
- In the design document, amend §5.1 (types per the field-mapping table) and
  §6.1 (placement), and add "§6.1.1 Implementation decisions (10.1.1)". That
  section links ADR 0014 and records D-2 and the representation invariants RI-1
  to RI-7.
- In `docs/developers-guide.md`:
  - add "## Shared async-trait hygiene helpers", with a policy-layer table
    for `whitaker_common::async_trait_hygiene` and an adapter-layer table for
    `whitaker_lint_core::async_trait_hygiene`;
  - include the re-use policy. The index is built at most once per crate per
    lint pass, consumers query it and never mutate it, new derived fields are
    added inside `build`, and new facts get validation and a P-WELLFORMED
    case;
  - add a bullet for the new proof under "Verus scope and trust boundary";
  - in a separate commit, correct the stale "Adapter layer — `whitaker`"
    heading and text to `whitaker_lint_core`.
- Add ADR 0014 to the ADR list in `docs/contents.md`.
- In `docs/repository-layout.md`, mention the async-trait hygiene model under
  the `common/` description only if that section enumerates modules. Leave it
  untouched if it does not.
- In `docs/roadmap.md`, mark 10.1.1 `[x]` and correct its module path to
  `whitaker_common::async_trait_hygiene` and
  `whitaker_lint_core::async_trait_hygiene`. Annotate 10.1.2 to say that the
  closure kernel exists and 10.1.2 supplies its inputs.
- Make no `docs/users-guide.md` change (D-6).

## Milestones and plateaus

Each milestone ends with `make check-fmt`, `make lint`, and `make test`
passing, run sequentially, preferably by the `scrutineer` agent, with output
captured through `tee`. Each milestone ends with at least one commit. Commit
messages follow `AGENTS.md`. Red tests are observed locally before the green
change, but are never committed failing.

- EP-M0: baseline. Outcome: the three gates pass on the unmodified branch,
  with logs recorded. This plateau establishes that later failures are ours.
  Recovery: if a baseline gate fails, record it under `Surprises` and escalate
  before starting. Compatibility decision: none.
- EP-M1: proof. Outcome: `verus/async_trait_family_closure.rs`, the
  `run-verus.sh` group, and the `make verus-async-trait` target. Discharges
  V-CLOSURE and V-RESTRICT (ATH-CLOSURE, ATH-LOCAL). Acceptance:
  `make verus-async-trait` verifies with zero errors. Both seeded mutations are
  observed failing and recorded.
  `grep -nE
  'assume|admit|external_body|external_fn' verus/async_trait_family_closure.rs`
  prints nothing, and the three standard gates pass. Conformance check: the
  specification matches design §5.2.2, and no production code changes.
  Recovery: the proof file is self-contained, so revert the commit. Remaining
  gaps: the production kernel (EP-M3). Compatibility decision: none.
- EP-M2: model types and validation. Outcome: `model.rs`, `error.rs`,
  `facts.rs` (with a `build` that validates but derives only placeholder-free
  fields), and `index.rs` with the supplied-fact accessors. The red tests are
  `build_cases.rs`, `build_props.rs` (P-WELLFORMED), and the BDD scenarios 5
  and 6 (rejections), observed failing first. This milestone delivers the
  supplied-fact half of RI-1 and RI-4 to RI-6. To keep the plateau coherent,
  `dyn_required` and `impl_counts` are not exposed until EP-M3 adds them. Their
  accessors land together with their derivation, so no interim placeholder
  exists. Acceptance:
  `cargo nextest run -p whitaker-common async_trait_hygiene` and
  `cargo nextest run -p whitaker-common --test async_trait_family_index_behaviour`
  pass, along with the three standard gates. Recovery: revert to EP-M1.
  Compatibility decision: none (new pre-1.0 API).
- EP-M3: derivations. Outcome: `closure.rs`, derived `dyn_required` and
  `impl_counts`, and their accessors. The red tests are `closure_cases.rs`,
  `closure_props.rs` (P-KERNEL-ORACLE), P-ORDER, the RI-2 and RI-3 checks in
  `assert_well_formed`, `index_queries.rs`, and BDD scenarios 1 to 4. The
  seeded "roots only" mutation is observed failing and recorded. Acceptance is
  as for EP-M2, plus `make verus-async-trait` still passing. Conformance check:
  the kernel mirrors the Verus model, so compare the two side by side and
  record the result in `Outcomes`. Recovery: revert to EP-M2. Compatibility
  decision: none.
- EP-M4: adapter. Outcome: the gated `whitaker_lint_core::async_trait_hygiene`
  module. Acceptance: `make lint` passes, which compiles the module with
  `--all-features` under Clippy. A
  `cargo check -p whitaker_lint_core --features dylint-driver`, run with the
  `make typecheck` `RUSTFLAGS`, also passes, along with the three standard
  gates. Record the supplementary gated-test run result. Conformance check: the
  root `whitaker` package and `common/Cargo.toml` gain no `rustc_*` dependency.
  Recovery: revert the module.
- EP-M5: documentation and completion. Outcome: the ADR, design, developers'
  guide, contents, and roadmap updates. Acceptance: `make fmt`, then
  `make check-fmt`, `make markdownlint`, `make nixie`, `make spelling`,
  `make lint`, and `make test` pass. The roadmap shows 10.1.1 as done.
  Conformance check: reconcile every `Surprises` entry with the design document
  and ADR 0014 before setting the status to `COMPLETE`.

## Concrete steps

Run every command from the worktree root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/debddaa4-b6ea-4cf8-95da-b4fae7591574`.
Log names follow
`/tmp/$ACTION-whitaker-10-1-1-async-trait-hygiene-modules.out`.

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-10-1-1-async-trait-hygiene-modules.out
make lint      2>&1 | tee /tmp/lint-whitaker-10-1-1-async-trait-hygiene-modules.out
make test      2>&1 | tee /tmp/test-whitaker-10-1-1-async-trait-hygiene-modules.out
make verus-async-trait 2>&1 | tee /tmp/verus-whitaker-10-1-1-async-trait-hygiene-modules.out
```

Expected Verus tail once EP-M1 is green:

```plaintext
verification results:: N verified, 0 errors
```

Here `N` is the number of proved functions and lemmas.

Focused test loop during EP-M2 and EP-M3:

```bash
cargo nextest run -p whitaker-common async_trait_hygiene 2>&1 \
  | tee /tmp/test-focused-whitaker-10-1-1-async-trait-hygiene-modules.out
cargo nextest run -p whitaker-common --test async_trait_family_index_behaviour 2>&1 \
  | tee /tmp/test-bdd-whitaker-10-1-1-async-trait-hygiene-modules.out
```

The focused runs omit the `make test` `RUSTFLAGS`. If they disagree with the
gate, the gate is authoritative.

Before committing ADR 0014, re-scan for collisions:

```bash
git fetch git@github.com:leynos/whitaker.git '+refs/heads/*:refs/remotes/origin/*'
for b in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
  git ls-tree --name-only "$b" docs/adrs/; done | sort -u | grep adr-0014
```

The scan must print nothing.

## Validation and acceptance

The plan is complete when all of the following hold:

1. `make verus-async-trait` reports zero errors, and the proof file contains
   no trusted escape hatches.
2. `make test` passes and includes the new tests. These are the unit
   modules under `whitaker_common::async_trait_hygiene::tests`, the two
   proptest properties, and the six scenarios of
   `async_trait_family_index_behaviour`. Each new test was observed failing
   before its production code existed.
3. `make check-fmt`, `make lint`, `make markdownlint`, `make nixie`, and
   `make spelling` pass.
4. A Rustdoc example on `AsyncTraitFamilyFacts` builds a two-trait family
   with `u32` keys and `&'static str` sites, and asserts that the supertrait is
   dyn-required. It runs in `make test-doc`.
5. The developers' guide documents both layers and the re-use policy. ADR
   0014 exists and is linked from the design document and `docs/contents.md`.
   Roadmap 10.1.1 is checked.

## Idempotence and recovery

Every step adds files or appends to lists, so re-running a step after a partial
failure is safe once the partial edit is reverted with
`git checkout -- <path>`. Each milestone is one or more commits, so
`git revert` of a milestone's commits restores the previous plateau. No step
touches persisted data or deployed state.

## Interfaces and dependencies

New public items in `whitaker_common::async_trait_hygiene`:
`AsyncTraitFamilyFacts<K, S>`, `AsyncTraitFamilyIndex<K, S>`,
`FamilyIndexError<K>`, `FactKind`, `AdapterDefect`, `TraitInfo<K>`,
`MethodKey<K>`, `AsyncMethodShape<K>`, `ReturnShape<K>`, `SendBound`,
`LifetimeBound`, `BoxedFutureAliasInfo`, `ImplInfo<K>`, `SelfTyKind<K>`,
`AdapterInfo<K>`, `NativeSiblingInfo<K>`, and `ImplCounts`. The bounds are
`K: Ord + Copy + fmt::Debug` and `S: Clone`.

New items in `whitaker_lint_core::async_trait_hygiene`, behind `dylint-driver`:
`LocalItemKey`, `HirAsyncTraitFamilyFacts`, and `HirAsyncTraitFamilyIndex`.

New dev-dependencies of `whitaker-common`: `googletest` and
`pretty_assertions`, both existing workspace dependencies.

## Revision note

- 2026-11-11: initial draft, pending expert review.
