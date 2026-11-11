# Add the async-trait hygiene modules and the `AsyncTraitFamilyIndex` data model (10.1.1)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be kept up to date as work
proceeds.

Status: DRAFT

This plan must be approved before implementation begins. Approval also accepts
the design deviations D-1 to D-4 in the `Decision log`. D-2 is the deviation
that most needs explicit approval, because it moves sibling lookup and adapter
discovery forward from roadmap item 10.1.2 into this item.

## Purpose / big picture

Roadmap item 10.1.1 opens the async-trait architecture hygiene work (roadmap
§10). Eight planned lints will reason about "async interface families". A
family is a dyn-facing trait that returns boxed futures, plus an optional
`Native*` sibling trait that returns `impl Future + Send`, plus a blanket
adapter that bridges the two, following Axinite ADR 0006. All eight lints are
to query one shared, crate-local analysis called `AsyncTraitFamilyIndex`, so
that the error-prone reasoning is done once, deterministically, and verified
once.

This item delivers that shared data model, its derivations, and the modules
that hold it. It does not deliver any lint. It also does not deliver any
High-level Intermediate Representation (HIR) extraction or any configuration
loading; those belong to roadmap items 10.1.2, 10.1.3, and 10.2 onwards.

After this change a maintainer can:

1. Build an `AsyncTraitFamilyIndex` from plain facts in an ordinary unit
   test with no compiler involved. A fact is a statement such as "trait 3 is
   named `Tool` and lives in module 1", "trait 3 is used as `dyn` at site A",
   or "impl 9 implements trait 3 for a type parameter bounded by trait 4". The
   maintainer can then ask which traits are *dyn-required* (used as `dyn`
   directly, or reachable from such a trait through supertraits), which trait
   is the `Native*` sibling of which, which impls are blanket adapters, and
   which naming ambiguities exist.
2. See malformed fact sets rejected with a typed error, while
   well-formed but badly designed code is always representable. A missing
   sibling, a misplaced adapter, or an ambiguous `Native*` name is exactly what
   the future lints report, so it must never make the build fail.
3. Run `make verus-async-trait` and see Verus prove two things for all
   graphs. First, the closure algorithm returns exactly the traits reachable
   from the directly dyn-used traits. Second, discarding non-local traits
   before running it loses nothing.
4. Name the compiler-bound instantiation of the index
   (`HirAsyncTraitFamilyIndex`, keyed by a local `DefIndex` wrapper) from any
   lint crate that enables `whitaker_lint_core/dylint-driver`. That is where
   roadmap item 10.1.2 will populate it from HIR.

Success is observable as follows: the new unit, property, and behavioural tests
pass under `make test`; the new doctests pass under `make test-doc`;
`make verus-async-trait` reports every proof verified; `make check-fmt` and
`make lint` pass; and the documentation describes the model, its invariants,
and its re-use policy.

## Constraints

- Do not add a `dylint-driver` feature, `rustc_private` code, or a
  `rustc_*` dependency to the root `whitaker` package. ADR 0012
  (`docs/adrs/adr-0012-whitaker-cli-boundary.md`) made the root package a
  command-line interface only. Compiler-private driver code lives in
  `crates/whitaker_lint_core` behind its `dylint-driver` feature.
- Keep `whitaker-common` (`common/`) free of `rustc_*` dependencies. It is
  the compiler-independent crate, and it is published.
- `build` may fail only for *caller defects*: duplicate keys, a key
  registered under two item kinds, and references to unrecorded keys. Every
  state that compiling Rust code can produce must be representable and
  queryable, however poorly designed it is.
- Every collection in the index must iterate in an order that depends only on
  the facts, never on insertion order or hashing. No `HashMap` or `HashSet` may
  appear in stored state or in any iteration that affects output or error
  identity.
- `DefId` and `LocalDefId` implement neither `Ord` nor `PartialOrd`. The
  pinned toolchain `nightly-2026-05-28` declares this at
  `compiler/rustc_span/src/def_id.rs:255-257` and `:375-377`, citing
  rust-lang/rust#90317. Do not place them in `BTreeMap` or `BTreeSet` keys, and
  do not work around this with `unsafe` code or transmutes.
- No source file, proof file included, may exceed 400 lines (`AGENTS.md`).
- Every module begins with a `//!` comment. Every function, public or
  private, carries a `///` comment with an imperative summary. Public items
  carry Rustdoc examples that compile as doctests.
- The workspace denies, among others, `expect_used`, `unwrap_used`,
  `indexing_slicing`, `panic_in_result_fn`, `shadow_reuse`,
  `missing_const_for_fn`, `must_use_candidate`, `needless_pass_by_value`,
  `implicit_hasher`, `cognitive_complexity`, `missing_docs`, and
  `allow_attributes_without_reason` (root `Cargo.toml` `[workspace.lints]`).
- The new code in `whitaker-common` also faces the Whitaker Dylint suite
  through `make lint-whitaker`, which runs over `whitaker-common` with
  `--all-targets` (`Makefile:259`). That includes test support helpers.
- Verus proofs must be substantive. Neither proof file may use `assume`,
  `admit`, `assume_specification`, `uninterp`, any `#[verifier::external*]`
  attribute, `exec_allows_no_decreases_clause`, or `#[cfg]` gating of proof
  items. Neither may restate its conclusion as a premise.
- Do not change the behaviour of any existing lint, helper, or gate.
- Do not create an isolated Cargo cache, bypass `RUSTC_WRAPPER`, or run gates
  in parallel for this worktree.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation needs more than 40 non-ExecPlan
  files changed, or more than 4,500 net lines of code and other documentation.
  This ExecPlan file is excluded from both counts. `Cargo.lock` is permitted to
  change and counts as one file.
- Interface: stop and escalate if any *existing* public signature in
  `whitaker-common` or `whitaker_lint_core` must change.
- Dependencies: the only permitted manifest change is adding the existing
  workspace dependencies `googletest` and `pretty_assertions` as
  `[dev-dependencies]` of `whitaker-common`, with the resulting `Cargo.lock`
  update. Any other dependency change triggers escalation.
- Design: stop and escalate if the generic `K`/`S` model cannot express a
  field from design §5.1 without a `rustc` type. Do the same if the
  `LocalItemKey(DefIndex)` adapter cannot be built on the pinned toolchain.
- Proof: stop and escalate if either Verus file is not discharged after
  three focused attempts (about four hours of proof work), or if discharging it
  appears to need a forbidden construct.
- Iterations: stop and escalate if a gate still fails after three fix
  attempts for the same root cause.
- Ambiguity: stop and present options if the 10.1.1 and 10.1.2 boundary in
  D-2 proves wrong in a way that changes the public model.

## Risks

- Risk: Verus has no in-repository precedent for graph reachability or
  worklist loop invariants, so the closure proof is new ground. Severity:
  medium. Likelihood: medium. Mitigation: milestone EP-M1 writes the
  specification and lemma statements before any production code. The model uses
  dense indices over the exec view (`Vec<Vec<usize>>` adjacency, `Vec<bool>`
  visited) rather than `Set`/`Map` algebra. A ghost `expanded` sequence
  replaces worklist-membership quantifiers. The proof tolerance escalates
  rather than trusting an assumption.
- Risk: the Verus sidecar is a handwritten model of the production kernel,
  so the two could drift. The `indexing_slicing` lint forces `.get()` in
  production, so the two can never be textually identical. Severity: medium.
  Likelihood: medium. Mitigation, in three layers. First, a tripwire script,
  `scripts/check-verus-async-trait-mirror.sh`, hashes the production kernel
  between marker comments and compares the result with a hash recorded in the
  Verus file. Any kernel edit therefore fails CI until someone revisits the
  model and updates the recorded hash. Second, an exhaustive small-scope test
  checks every graph with at most three nodes against an independent oracle.
  Third, a proptest property checks larger graphs.
- Risk: ordering by `DefIndex` reintroduces the incremental-compilation
  hazard that made `DefId` unordered upstream. Severity: low. Likelihood: low.
  Mitigation: the index is built and consumed inside one late lint pass, and is
  never a query result or persisted. `DefIndex` is `#[orderable]` upstream. ADR
  0014 records the axiom (A-DEFINDEX) and the alternatives, `DefPathHash` and
  `(CrateNum, DefIndex)`.
- Risk: unit tests inside `dylint-driver`-gated modules of
  `whitaker_lint_core` do not run in `make test`, which runs that crate only
  with `--no-default-features` (`Makefile:37-46`). `cargo doc` does not build
  the gated module either. Severity: medium. Likelihood: high. Mitigation: keep
  all logic in the pure `whitaker-common` layer. The gated adapter holds only a
  newtype, conversions, and type aliases, which `make lint` compiles through
  Clippy with `--all-features`. The leftover runtime obligation is written into
  the roadmap entries for 10.1.2 and 10.2.1.
- Risk: every lint `cdylib` builds its own index, so the suite builds it
  eight times per crate. Severity: low. Likelihood: high. Mitigation: the build
  is linear in facts plus one graph traversal. Record the cost as an open
  question for suite wiring (10.4.1), which can share a cache if measurements
  justify it.
- Risk: the public API of the published `whitaker-common` grows.
  Severity: low. Likelihood: high. Mitigation: re-export only
  `AsyncTraitFamilyFacts`, `AsyncTraitFamilyIndex`, `FamilyIndexError`, and
  `FamilyNamingPolicy` at the crate root. Every other type stays under
  `whitaker_common::async_trait_hygiene::`. Structs have private fields with
  constructors and accessors. Errors and anomalies are `#[non_exhaustive]`.
- Risk: an ADR number collides again. ADR 0013 already exists on an
  unmerged remote branch as `adr-0013-brain-trust-lint-driver-interfaces.md`.
  Severity: low. Likelihood: low. Mitigation: re-fetch and re-scan all remote
  branches immediately before committing ADR 0014.

## Progress

- [x] (2026-11-11 17:20Z) Renamed the branch to
  `10-1-1-async-trait-hygiene-modules`. No remote branch of that name existed
  yet.
- [x] (2026-11-11 17:35Z) Reconnaissance: root crate, `whitaker_lint_core`,
  verification tooling, and documentation conventions.
- [x] (2026-11-11 17:45Z) Confirmed from the pinned compiler sources that
  `DefId` and `LocalDefId` are `!Ord`, and that `DefIndex` and `Span` are `Ord`.
- [x] (2026-11-11 17:50Z) Fetched remote branches; ADR 0013 is taken, so the
  next free ADR is 0014.
- [x] (2026-11-11 18:10Z) Drafted this ExecPlan (commit `c5f366fc`).
- [x] (2026-11-11 18:40Z) Expert review by three independent reviewers
  (structure and contracts; verification rigour; alternatives, conventions, and
  pre-mortem). Revised the plan; see the revision note.
- [ ] User approval of this ExecPlan, including deviations D-1 to D-4.
- [ ] EP-M0: baseline gates on the unmodified branch.
- [ ] EP-M1: Verus specification, lemmas, proofs, and mirror tripwire.
- [ ] EP-M2: fact types, builder, caller-defect validation, and fact
  queries.
- [ ] EP-M3: derivations (closure, siblings, adapters, counts, anomalies).
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
- Observation: `DefId` and `LocalDefId` are deliberately unordered, while
  `DefIndex` (`#[orderable]`) and `Span` (`impl Ord for Span`,
  `rustc_span/src/lib.rs:743`) are ordered. Impact: every `BTreeMap<DefId, …>`
  field in design §5.1 is unbuildable as written. Use sites can be stored as
  `BTreeSet<S>`. See D-3.
- Observation: `dylint-driver`-gated unit tests in `whitaker_lint_core` are
  not exercised by `make test`. Evidence: `Makefile:37-46`.
  `crates/whitaker_lint_core/src/hir/tests.rs` is an existing instance. Impact:
  logic must live in `whitaker-common`. See D-1 and D-2.
- Observation: doctests are not part of `make test`. `make test-doc`
  (`Makefile:433`) is the only lane that runs them, and CI runs it as a
  separate step. `AGENTS.md` still describes `make test` as running the
  doctests. Impact: this plan adds `make test-doc` to every milestone gate from
  EP-M2. The `AGENTS.md` drift is recorded here, not fixed, because it is
  outside this item's scope.
- Observation: `scripts/run-verus.sh` validates group names in two places:
  the `case` in `proof_files_for_group` and an allowlist at line 62. Impact:
  EP-M1 edits both. Otherwise `async-trait` would be treated as a file path.
- Observation: `docs/developers-guide.md:3730-3743` ("Adapter layer —
  `whitaker` (feature `dylint-driver`)") is stale since ADR 0012. Impact: EP-M5
  corrects it in its own documentation commit, because the new async-trait
  section sits beside it and would otherwise contradict it.

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
  `whitaker_common::rstest` and an adapter in `whitaker_lint_core::hir`. The
  `proofs.yml` path filter already covers `common/**`. Affected identifiers:
  ATH-PLACE (design §6.1) and RM-10.1.1 (roadmap wording
  `whitaker::async_trait_hygiene`). Options rejected: (a) everything in
  `whitaker_lint_core` behind the feature, which leaves the logic untested by
  the gate; (b) everything ungated in `whitaker_lint_core`, which is viable but
  breaks the "pure models live in `common`" convention and falls outside the
  `proofs.yml` path filter; (c) a new unpublished pure crate such as
  `crates/whitaker_async_trait_core`, following `whitaker_string_literals`.
  That avoids growing a published API, but it adds a crate, a path-filter
  entry, and a Dylint package-list entry for one module. Narrow re-exports and
  private fields contain the API growth in `common` well enough. Approving
  authority: the user, at plan approval. Date/Author: 2026-11-11, planning
  agent, revised after expert review.
- Decision D-2 (scope deviation, needs approval): 10.1.1 builds a *fully
  derived* index. Callers supply only syntactic facts. `build_with(policy)`
  derives `dyn_required`, `native_siblings`, `adapter_impls`, `impl_counts`,
  and a list of `FamilyAnomaly` values. Roadmap 10.1.2 keeps everything that
  reads the compiler or classifies syntax: the `hir::TyKind::TraitObject`
  visitor, supertrait extraction from predicates (both `trait A: B` and
  `where Self: B`), boxed-future alias shape matching, method return-shape
  classification, and filtering to local items. Roadmap 10.1.3 keeps
  configuration loading and maps the suite-level settings onto
  `FamilyNamingPolicy`. Rationale: two of three reviewers found that the first
  draft's boundary contradicted D-1. Sibling lookup needs only trait names and
  parents. Adapter discovery needs only impl self-type bounds. Neither reads
  the compiler, so placing them in 10.1.2 would put pure logic where the gate
  never runs it. The first draft also took siblings and adapters as supplied
  facts with strict coherence checks. That made states such as "blanket adapter
  but no sibling" unrepresentable, and those are exactly the states
  `async_dyn_family_requires_native_sibling` and
  `async_dyn_family_requires_blanket_adapter` must report. Deriving them makes
  RI-2 to RI-5 hold by construction and removes the coherence obligation
  entirely. Upstream change: roadmap 10.1.2 is reworded at completion, and
  10.1.3 is annotated (see EP-M5). Option kept as fallback if the user rejects
  D-2: siblings and adapters remain supplied facts, validated only for
  referential integrity, with no coherence errors. 10.1.2 then implements their
  derivation as pure functions in `whitaker_common::async_trait_hygiene`, not
  in the gated adapter. This shrinks EP-M3 by about a third. Date/Author:
  2026-11-11, planning agent, revised after expert review.
- Decision D-3 (representation deviation, needs approval):
  - The generic model is keyed by `K` and records use sites as `S`, with
    `K: Ord + Copy` and `S: Ord + Clone` stated on `impl` blocks, not on the
    struct definitions.
  - The driver instantiation uses `LocalItemKey`, a `#[derive(Ord)]`
    newtype over `rustc_span::def_id::DefIndex` for items of the local
    crate, and `rustc_span::Span` for sites.
  - Method shapes are keyed by `MethodKey<K> { trait_key, method_key }`
    rather than `(DefId, Symbol)`.
  - Supertraits are stored as `BTreeSet<K>` rather than `Vec<DefId>`, and
    dyn-use sites as `BTreeSet<S>` rather than `Vec<Span>`.
  - `impls` is a `BTreeMap<K, ImplInfo<K>>` keyed by the impl item.
  - `diagnostic_paths` is folded into the `def_path` of `TraitInfo`,
    `ImplInfo`, and `BoxedFutureAliasInfo` rather than kept as a separate
    map.
  - Boxed-future alias references use
    `AliasRef<K>::{Local(K), Foreign { def_path: String }}`, so a foreign
    alias such as `futures::future::BoxFuture` is representable.
  Rationale:
  - `DefId` is `!Ord`.
  - `Symbol`'s `Ord` compares interner indices, not text.
  - Sets make duplicate edges and sites idempotent and order-free, which
    makes RI-7 hold strictly.
  - A separate path map could disagree with its keys.
  - Restricting keys to local items is justified for traits by the
    local-restriction lemma (V-RESTRICT) and axiom A-CRATE-DAG. Foreign
    aliases are the one foreign reference the lints genuinely need (design
    §4.1 "Allow futures' `BoxFuture` by config" and §7.3), so they get an
    explicit variant.
  Alternative considered: `ItemKey { krate: CrateNum, index: DefIndex }` for
  every key. It is more general, but it makes "dangling" ambiguous between
  "unrecorded" and "foreign" and weakens V-RESTRICT's role. It is recorded in
  ADR 0014. Date/Author: 2026-11-11, planning agent, revised after expert
  review.
- Decision D-4 (failure semantics): `FamilyIndexError` reports caller
  defects only, and for a given fact set it is deterministic. Checks run in a
  fixed order over sorted structures, and each reports the least offending key.
  Lint consumers (10.2 onwards) must treat `Err` as a Whitaker defect: log it
  with `log::debug!` (the logging facade `whitaker-common` already uses) and
  emit no diagnostics, and never panic. This rule goes into the developers'
  guide re-use policy. Date/Author: 2026-11-11, planning agent.
- Decision D-5: do not add Kani harnesses in this item.
  Rationale: Verus establishes the unbounded closure property over the model.
  An exhaustive small-scope test (every graph with `n ≤ 3`) and a proptest
  oracle check the production kernel. The repository documents CBMC state
  explosion on `Vec` and iterator code (`scripts/run-kani.sh:112-158`), so a
  bounded Kani check would duplicate the exhaustive test at higher cost.
  Date/Author: 2026-11-11, planning agent.
- Decision D-6: do not add `insta` snapshots in this item.
  Rationale: the model renders no output format. Diagnostics, the first
  multivariant output, arrive in 10.2.1, which is where snapshots belong.
  P-ORDER and P-FAITHFUL check determinism and fidelity more strongly than a
  snapshot would. Date/Author: 2026-11-11, planning agent.
- Decision D-7: make no `docs/users-guide.md` change in this item.
  Rationale: no lint, configuration key, command, or user-visible behaviour
  changes. The consumers of these types are Whitaker's own lint crates, which
  are documented in the developers' guide. The users' guide changes in 10.4.3,
  when lints ship. Date/Author: 2026-11-11, planning agent.
- Decision D-8: `ReturnShape`, `SendBound`, and `LifetimeBound` stay
  exhaustive. `FamilyIndexError`, `FactKind`, and `FamilyAnomaly` are
  `#[non_exhaustive]`. Rationale: every consumer of the shape enums is a lint
  crate in this workspace. Exhaustive matching forces each lint to decide what
  a new shape means instead of silently ignoring it. Errors and anomalies are
  informational, and new facts or rules will add variants. Date/Author:
  2026-11-11, planning agent.
- Decision D-9: impl-method shapes are out of scope.
  Rationale: `async_trait_signature_markers_present` needs only the trait-side
  `'async_trait` markers for local traits. Impls of foreign `#[async_trait]`
  traits are not fixable by the user. An additive `impl_method_shapes` field
  can follow if 10.2.1 shows a need. Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. This section is completed at each milestone boundary and at
completion. It records the Verus summary counts, the mutation and
counterexample runs, and the kernel and model side-by-side comparison.

## Context and orientation

Whitaker is a suite of Dylint lints. Dylint loads lint libraries into a `rustc`
driver. Lint crates live under `crates/`, and each is a `cdylib` that links
compiler-private (`rustc_private`) crates only when its `dylint-driver` feature
is enabled. This keeps ordinary unit tests free of compiler linkage.

Three crates matter here.

- `common/` is the published package `whitaker-common`, imported as
  `whitaker_common`. It holds compiler-independent helpers and pure models, for
  example `common/src/lcom4/`, `common/src/rstest/`, and
  `common/src/decomposition_advice/`. Its behavioural tests live in
  `common/tests/*_behaviour.rs` with Gherkin files in `common/tests/features/`.
  Its property tests use `proptest`. Its builder precedent is
  `TraitMetricsBuilder` (`common/src/brain_trait_metrics/metrics.rs`). Its
  precedent for value types with private fields is `lcom4::MethodInfo`
  (`common/src/lcom4/mod.rs:44-79`).
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
`scripts/run-verus.sh <group>` runs them, and `make verus` runs every group. The
`.github/workflows/proofs.yml` workflow runs them nightly and on pull requests
that touch `verus/**`, `common/**`, or `Makefile`.
`scripts/check-verus-fragment-id-bridge.sh`, run from `ci.yml:412`, is the
precedent for a cheap CI check that keeps a proof tied to its runtime code.

Terms used in this plan:

- *Dyn-facing trait*: a trait used behind `dyn`, whose async methods return
  boxed futures such as `Pin<Box<dyn Future<Output = T> + Send + 'a>>`.
- *Native sibling*: a trait `NativeX` beside dyn-facing trait `X`, whose
  methods return `impl Future<Output = T> + Send`. The name prefix (`Native`)
  and whether it must share `X`'s parent module are set by `FamilyNamingPolicy`.
- *Blanket adapter*: an impl such as `impl<T: NativeX + Send + Sync> X for T`,
  which boxes each native future.
- *Dyn-used trait*: a local trait named in at least one `dyn Trait` type in
  the crate.
- *Dyn-required trait*: a dyn-used trait, or any trait reachable from one
  through supertrait edges. A `dyn A` object can call the methods of every
  supertrait of `A`, so every such supertrait must stay dyn-compatible.
- *Supertrait edge*: `A → B` when `A` declares `B` as a supertrait, either
  as `trait A: B` or as `where Self: B`. The Rust Reference states that the two
  forms are equivalent.
- *Fact*: a syntactic observation supplied to the builder.
- *Derived field*: a field `build_with` computes from facts.
- *Caller defect*: a fact set that no correct extractor could produce, such
  as a dangling key. It is the only kind of error.
- *Anomaly*: a well-formed but questionable family state, such as two
  candidate native siblings. It is recorded and queryable, never an error.
- *Representation invariant*: a property that every constructed index value
  satisfies and that accessors may rely on.
- *Closure kernel*: the function that computes dyn-required traits.

### Relevant skills and documents

Skills: `execplans` (this format), `rust-router` (entry), `arch-crate-design`
(crate placement), `rust-types-and-apis` (generic key design, newtypes, and
`#[non_exhaustive]`), `rust-errors` (the caller-defect error enum),
`rust-unit-testing` (rstest, googletest, pretty_assertions), `proptest`,
`verus`, `rust-verification` (method selection), `arch-decision-records` (ADR
0014), `nextest` (focused runs), and `en-gb-oxendict-style` (documentation
prose).

Documents:

- `docs/async-trait-architecture-hygiene-dylint-suite-design-for-whitaker.md`
  (§4.1 lint table, §5.1 fields, §5.2 algorithms, §6.1 placement, §6.4
  configuration).
- `docs/whitaker-dylint-suite-design.md` (suite architecture and the
  `dylint-driver` gating convention).
- `docs/whitaker-clone-detector-design.md` (precedent for pure models with
  Verus and Kani sidecars).
- `docs/rust-testing-with-rstest-fixtures.md` (fixtures and parameterized
  cases).
- `docs/rstest-bdd-users-guide.md` (behavioural tests and step rules).
- `docs/rust-doctest-dry-guide.md` (doctest style for the new public items).
- `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep
  validation and derivation free of bumpy-road conditionals).
- `docs/developers-guide.md` ("Verus scope and trust boundary", "Shared span
  recovery helpers", and "Shared fingerprint helpers").
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
  (external, accepted 2026-03-21). It is the source of the family shape: the
  `BoxFuture<'a, T>` alias, `Native*` naming, and the blanket adapter.
- `AGENTS.md` and the root `Cargo.toml` lint policy (governing standards).
- No Terms of Reference exist for this suite. None is invented here.

Traced design elements:

- ATH-FIELDS: design §5.1, the eleven index fields.
- ATH-DET: design §5.1 determinism note.
- ATH-CLOSURE: design §5.2.2,
  `dyn_required = dyn_used ∪ supertraits_transitive(dyn_used)`.
- ATH-SIBLING: design §5.2.3, name convention plus same-parent rule.
- ATH-ADAPTER: design §5.2.4, a blanket impl of the dyn-facing trait whose
  self type is a type parameter bounded by the native trait.
- ATH-PLACE: design §6.1, shared helper placement.
- ATH-LOCAL: design §1 and §3.3, single-crate scope.
- ATH-LINTS: design §4.1. The model must carry what each of the eight lints
  needs.

Trace links:

```plaintext
RM-10.1.1 -> ATH-PLACE   -> D-1 -> EP-M2, EP-M4 -> common/src/async_trait_hygiene/, whitaker_lint_core::async_trait_hygiene
RM-10.1.1 -> ATH-FIELDS  -> D-3 -> EP-M2, EP-M3 -> P-FAITHFUL, P-WELLFORMED, T-QUERIES
RM-10.1.1 -> ATH-DET     -> D-3, D-4 -> EP-M2, EP-M3 -> P-ORDER
RM-10.1.1 -> ATH-CLOSURE -> D-2 -> EP-M1, EP-M3 -> V-CLOSURE, T-KERNEL-EXHAUSTIVE, P-KERNEL-ORACLE, BDD
RM-10.1.2 -> ATH-SIBLING -> D-2 -> EP-M3 -> T-SIBLINGS, P-SIBLING-ORACLE, BDD
RM-10.1.2 -> ATH-ADAPTER -> D-2 -> EP-M3 -> T-ADAPTERS, BDD
RM-10.1.1 -> ATH-LOCAL   -> D-3 -> EP-M1, EP-M4 -> V-RESTRICT, LocalItemKey
RM-10.1.1 -> ATH-LINTS   -> D-3, D-8, D-9 -> EP-M2 -> per-lint coverage table below
```

Field mapping from design §5.1 to the model:

| Design field           | Model field and type                                          | Origin                          |
| ---------------------- | ------------------------------------------------------------- | ------------------------------- |
| `traits`               | `BTreeMap<K, TraitInfo<K>>`                                   | fact                            |
| `dyn_uses`             | `BTreeMap<K, BTreeSet<S>>` (each set non-empty)               | fact                            |
| `supertraits`          | `BTreeMap<K, BTreeSet<K>>`                                    | fact                            |
| `dyn_required`         | `BTreeSet<K>`                                                 | derived (closure kernel)        |
| `boxed_future_aliases` | `BTreeMap<K, BoxedFutureAliasInfo>`                           | fact                            |
| `trait_method_shapes`  | `BTreeMap<MethodKey<K>, AsyncMethodShape<K, S>>`              | fact                            |
| `impls`                | `BTreeMap<K, ImplInfo<K>>`                                    | fact                            |
| `adapter_impls`        | `BTreeMap<K, BTreeSet<K>>` (trait to adapter impls)           | derived                         |
| `native_siblings`      | `BTreeMap<K, SiblingMatch<K>>` plus reverse `native_of`       | derived under the naming policy |
| `impl_counts`          | `BTreeMap<K, ImplCounts>` (traits with at least one impl)     | derived                         |
| `diagnostic_paths`     | `def_path` in `TraitInfo`, `ImplInfo`, `BoxedFutureAliasInfo` | fact                            |
| (new) `anomalies`      | `BTreeSet<FamilyAnomaly<K>>`                                  | derived                         |

Table: Design §5.1 fields mapped to the 10.1.1 model.

Per-lint coverage, the check for ATH-LINTS:

| Lint (design §4.1)                                  | Model data it queries                                                          |
| --------------------------------------------------- | ------------------------------------------------------------------------------ |
| `async_trait_concrete_only`                         | `dyn_required`, method `ReturnShape`, `TraitInfo::def_path` for allowlists     |
| `async_dyn_family_requires_native_sibling`          | `dyn_required`, `native_sibling_of`, sibling anomalies                         |
| `async_dyn_family_requires_blanket_adapter`         | `native_sibling_of`, `adapters_of`, `impls_of` type-parameter impls            |
| `async_dyn_direct_impl_prefers_native`              | `adapters_of`, `impls_of` with `SelfTyKind::Concrete { path }`                 |
| `async_dyn_boxed_future_alias_required`             | `ReturnShape::{BoxedFutureAlias(AliasRef), RawBoxedFuture}`, `return_site`     |
| `native_async_future_must_be_send`                  | `native_of` (reverse lookup), `ImplFuture { send }`, `return_site`             |
| `native_async_multi_borrow_requires_named_lifetime` | `ImplFuture { lifetime: Elided }`, `borrowed_input_count`, `return_site`       |
| `async_trait_signature_markers_present`             | `has_async_trait_lifetime` plus a boxed return shape (trait methods only, D-9) |

Table: What each planned lint will query.

Item spans come from `tcx.def_span(key)` in the adapter layer. `S` is reserved
for non-item sites: dyn-use sites and method return-type sites.

Not discharged by this item: HIR extraction and syntactic classification
(RM-10.1.2), configuration loading (RM-10.1.3), and any lint behaviour (RM-10.2
and later).

## Abstract model and proof sketch

The model is specified independently of rustc first, so that the API can be
pressure-tested against it.

### Abstract state

Let `T` be the finite set of recorded trait keys. Let `E ⊆ T × T` be the
supertrait relation, where `(a, b) ∈ E` means `a` declares supertrait `b`. Let
`U ⊆ T` be the dyn-used traits.

```plaintext
reach_in(U, E, y, 0)    = y ∈ U
reach_in(U, E, y, k+1)  = reach_in(U, E, y, k) ∨ ∃ x. reach_in(U, E, x, k) ∧ (x, y) ∈ E
reachable(U, E, y)      = ∃ k. reach_in(U, E, y, k)
dyn_required            = { y ∈ T | reachable(U, E, y) }
```

Paths grow backward from their end node, which makes V-RESTRICT a one-step
induction. Equivalently, `dyn_required` is the least set `R` with `U ⊆ R` that
is closed under `E`, meaning `x ∈ R ∧ (x, y) ∈ E ⇒ y ∈ R`.

For siblings, let `name(t)` and `parent(t)` be the recorded name and parent
module of trait `t`, and let the policy be `(prefix, cross)`. Then:

```plaintext
candidates(t) = { n ∈ T | name(n) = prefix ++ name(t) ∧ (cross ∨ parent(n) = parent(t)) ∧ n ≠ t }
sibling(t)    = Unique(n)       if candidates(t) = {n}
              = Ambiguous(C)    if |candidates(t)| ≥ 2, with C = candidates(t)
              = absent          otherwise
native_of(n)  = { t | n ∈ candidates(t) }
adapters(t)   = { i ∈ impls | trait(i) = t ∧ self_ty(i) = TypeParam(B) ∧ sibling(t) = Unique(n) ∧ n ∈ B }
```

### Representation invariants of `AsyncTraitFamilyIndex<K, S>`

- RI-0 (disjoint key spaces): trait, impl, alias, and method-item keys are
  pairwise disjoint.
- RI-1 (referential integrity):
  - every key of `dyn_uses` and both ends of every `supertraits` edge are
    trait keys;
  - every `MethodKey::trait_key` and every `ImplInfo::trait_key` is a trait
    key;
  - every `SelfTyKind::TypeParam` bound and every `TraitInfo::parent`
    reference resolves as documented, with bounds being trait keys and
    `parent` being an opaque module key that is not validated;
  - every `AliasRef::Local(k)` names a key of `boxed_future_aliases`.
  The 10.1.2 extractor filters foreign traits out of bounds and supertraits,
  which V-RESTRICT shows is lossless for traits.
- RI-2 (closure):
  `dyn_required = { y ∈ T | reachable(dom(dyn_uses), supertraits, y) }`.
- RI-3 (counts): `impl_counts` has an entry exactly for each trait with at
  least one impl. Each entry counts that trait's impls by `SelfTyKind` variant,
  including impls that come from macro expansion. The `impl_counts(t)` accessor
  returns `ImplCounts::default()` for a trait with no entry.
- RI-4 (siblings): `native_siblings` and `native_of` equal the
  specification above under the policy passed to `build_with`.
- RI-5 (adapters): `adapter_impls` equals `adapters(t)` above for every `t`.
  Traits with no adapters have no entry.
- RI-6 (non-empty sites): every `dyn_uses` set is non-empty.
- RI-7 (determinism): `build_with(policy, F)` and `build_with(policy, G)`
  are equal, errors included, whenever `G` is any permutation of the fact
  sequence `F`.

### Lemmas (hypotheses about the eventual proof structure)

- L-SOUND (worklist soundness): throughout the worklist loop, every visited
  node is reachable. Witnesses are supplied with `k + 1`.
- L-CLOSED (closed-superset containment): if `U ⊆ S` and `S` is closed under
  `E`, then every reachable `y` is in `S`. Proved by induction on `k`.
- L-EXIT (exit closure): a ghost sequence `expanded` is maintained with three
  invariants:
  - `expanded[v] ⇒ visited[v] ∧ every successor of v is visited`;
  - `visited[v] ∧ ¬expanded[v] ⇒ v is on the worklist`;
  - every worklist element is visited and not expanded.
  At exit the worklist is empty, so every visited node is expanded. The visited
  set is therefore closed and contains `U`.
- L-COUNT: `¬s[i] ⇒ count_false(s.update(i, true)) = count_false(s) − 1`,
  proved by induction on the sequence length.
- L-TERM (termination): a node is pushed only when it becomes newly visited,
  duplicate roots included. The measure `count_false(visited) + worklist.len()`
  therefore falls by exactly one per outer iteration (by L-COUNT). The inner
  successor loop decreases on `adj[x].len() − j`.
- V-CLOSURE (main theorem): the kernel returns `visited` with
  `visited.len() = n` and `∀ y < n. visited[y] ⇔ reachable(roots, adj, y)`.
  This follows from L-SOUND, L-EXIT, and L-CLOSED.
- V-RESTRICT (local restriction), stated as relations rather than built by
  filtering:

  ```plaintext
  requires ∀ x, y. edge(adj_L, x, y) ⇔ local[x] ∧ local[y] ∧ edge(adj, x, y)
           ∀ y.    roots_L ∋ y       ⇔ local[y] ∧ roots ∋ y
           ∀ x, y. edge(adj, x, y) ∧ local[y] ⇒ local[x]        (no foreign-to-local edge)
  ensures  ∀ y. local[y] ⇒ (reachable(roots, adj, y) ⇔ reachable(roots_L, adj_L, y))
  ```

  The forward direction inducts on `k`. If `y` is local and `x → y`, then `x`
  is local, and the induction hypothesis covers `x`. The backward direction
  holds because restricted edges are a subset of all edges. This lemma
  justifies a design decision, namely the 10.1.2 extractor's filtering of
  foreign traits. It does not verify that filtering code.

### API pressure test

The specification leads directly to these shape decisions:

1. Derived fields have no setter. They are computed inside `build_with`, so
   RI-2 to RI-5 hold by construction.
2. All index fields are private and exposed through query methods.
   Consumers cannot break RI-0 to RI-7 after construction.
3. The closure kernel works on dense indices (`&[Vec<usize>]` adjacency and
   `&[usize]` roots, returning `Vec<bool>`), which is the representation Verus
   proves. The generic layer maps `K` to dense indices by rank in the sorted
   `traits` map, which also discharges RI-7 for `dyn_required`.
4. Validation runs once, in `build_with`, after all facts are known, so facts
   may arrive in any order. The 10.1.2 HIR visitor may meet an impl before its
   trait.
5. Sibling and adapter relationships are derived, never supplied, so the
   first draft's coherence errors cannot exist. Questionable states become
   `FamilyAnomaly` values.
6. Sets for sites and edges make repeated identical facts idempotent and
   make RI-7 hold strictly. Keyed facts (traits, impls, aliases, methods)
   reject duplicates, even identical ones, because a duplicate key signals an
   extractor bug.

## Verification plan

The relationship between proof and tests is as follows. Verus establishes the
unbounded semantic property of a handwritten model of the closure kernel, plus
the lemma that justifies local-only keys. An exhaustive small-scope test and a
proptest oracle connect the real production kernel to that property. A CI
tripwire forces the model to be revisited whenever the kernel changes. rstest
and rstest-bdd cover the executable API, the caller-defect errors, sibling and
adapter derivation, and realistic family shapes. None of these replaces the
others. The first end-to-end evidence through `rustc` arrives in 10.2.1, when a
lint populates the index.

### Axioms

- A-CRATE-DAG: a trait defined in crate `C` can name as supertraits only
  traits from `C` or from crates `C` depends on. Crate dependencies are
  acyclic, so no foreign trait has a local supertrait. Source: Rust's crate and
  name-resolution model. Used by V-RESTRICT's precondition, "no
  foreign-to-local edge".
- A-DYN-SUPER: a `dyn A` trait object can call the methods of `A`'s
  supertraits, and every supertrait of a dyn-compatible trait must itself be
  dyn-compatible. Source: the Rust Reference, "Traits" (dyn compatibility).
  Used to justify ATH-CLOSURE.
- A-SUPER-FORMS: `trait A: B` and `where Self: B` are equivalent supertrait
  declarations (Rust Reference). 10.1.2's extraction relies on this, and it is
  recorded here so that the edge relation is not specified as "syntactic bounds
  only".
- A-DEFINDEX: within one compilation session, `DefIndex` values for local
  items are unique and are assigned deterministically for identical input, and
  `DefIndex` implements `Ord` (`#[orderable]`). Used by `LocalItemKey` and RI-7.
- A-SPAN-ORD: `rustc_span::Span` implements `Ord`. Used for `S` in the
  adapter.
- A-BTREE: `BTreeMap` and `BTreeSet` iterate in ascending key order (standard
  library contract).

### Obligations

- Obligation V-CLOSURE: stated under `Lemmas`.
  Method: Verus deductive proof of an exec-mode model with loop invariants and
  `decreases` clauses. Use `visited.set(i, true)` and initialize with an
  explicit loop, not `vec![false; n]`. Rationale: the property must hold for
  all graph sizes and shapes, including cycles and self-loops. Rust's E0391
  forbids supertrait cycles in source, but the pure model must still handle
  them. This is the contractual core that every one of the eight lints
  inherits. Artefact: `verus/async_trait_family_closure.rs`, at about 300
  lines. Evidence: `make verus-async-trait` ends with
  `verification results:: N verified, 0 errors`. `N` is recorded in `Outcomes`
  when EP-M1 completes, and later runs must match it, so a silently skipped
  function is noticed. Before the proof bodies exist, the same command fails
  with unproved postconditions or loop invariants. That is the red step.
  Non-vacuity:
  - A proof function exhibits a witness graph (chain `0 → 1 → 2`, isolated
    node `3`, roots `[0]`) and proves `reachable(…, 2)` and
    `!reachable(…, 3)`. The negative half uses L-CLOSED with `S = {0, 1, 2}`.
  - An exec witness calls the model's `reachable_from` on the same graph and
    asserts on `v@[2]` and `v@[3]`. This shows the postcondition fully
    determines the output.
  - Two seeded mutations are each paired with a concrete counterexample
    graph, proved in a scratch copy, so the failure is refutation rather
    than proof brittleness. Mutation (a) omits the push of a newly visited
    successor; counterexample: chain `0 → 1 → 2` with roots `[0]` leaves
    `2` unvisited. Mutation (b) seeds `visited` with every node;
    counterexample: the isolated node `3` is visited. Both runs are
    recorded in `Outcomes`.
  - Every `requires` clause of a proof function is shown satisfiable by one
    of the witnesses.
  - The forbidden-construct scan in EP-M1 acceptance prints nothing. If the
    pinned Verus supports `--no-cheating`, it is passed too; EP-M1 checks
    this and records the answer.
- Obligation V-RESTRICT: stated under `Lemmas`.
  Method: Verus lemma over the same `reach_in` and `reachable` specification.
  Rationale: an introduced lemma that justifies a design decision (D-3), so it
  is cheap to prove and expensive to get wrong. Artefact:
  `verus/async_trait_family_restrict.rs`, at about 150 lines. It is kept
  separate to stay under 400 lines per file, and it duplicates the small spec
  block with a cross-reference comment. Non-vacuity: a witness has a local edge
  `a → b`, a local-to-foreign edge `a → f`, and roots `[a]`. This satisfies all
  three preconditions with a non-empty foreign set and shows a foreign node can
  be reachable while the lemma still holds. A counterexample adds the
  foreign-to-local edge `f → b` and uses roots `[f]`. The file then *proves*
  `reachable(full, b) ∧ ¬reachable(restricted, b)`, which shows that the third
  precondition is necessary.
- Obligation M-MIRROR: `scripts/check-verus-async-trait-mirror.sh` exits
  zero exactly when the SHA-256 of the text between `// verus-mirror: begin` and
  `// verus-mirror: end` in `common/src/async_trait_hygiene/closure.rs` equals
  the `// production-kernel-sha256: <hex>` line in
  `verus/async_trait_family_closure.rs`. Method: a shell check, modelled on
  `scripts/check-verus-fragment-id-bridge.sh`. `make verus-async-trait` runs it
  first, and a `ci.yml` step beside line 412 runs it on every pull request.
  Non-vacuity: EP-M3 edits one comment character inside the markers, observes
  the script fail with a message naming both files, and reverts the edit.
- Obligation T-KERNEL-EXHAUSTIVE: for every `n ≤ 3`, every adjacency
  relation (up to 2^9 = 512 edge sets), and every root set (up to 8), the
  production `reachable_from` equals an independent oracle. The oracle is a
  `BTreeSet<usize>` fixpoint that repeatedly adds successors until nothing
  changes. Method: a single deterministic test that enumerates all cases, at
  most about 4,700. Rationale: Kani-style bounded completeness without CBMC
  (D-5). Artefact: `common/src/async_trait_hygiene/tests/closure_cases.rs`.
- Obligation P-KERNEL-ORACLE: the same agreement for `n ∈ 4..=12`, with
  per-node successor lists drawn from `0..n` (including self-loops and cycles)
  and root lists drawn from `0..n` (including empty lists and duplicates).
  Method: proptest, with no `prop_assume` filtering. Artefact:
  `common/src/async_trait_hygiene/tests/closure_props.rs`. The same file also
  checks metamorphic relations: the result contains the roots, is closed under
  edges, and is monotone when roots or edges are added. Non-vacuity: named
  rstest cases pin each class: empty graph, empty roots, self-loop, two-cycle,
  diamond, disconnected component, every node a root, and an out-of-range
  successor (ignored by `.get()`; documented). A seeded "roots only" kernel
  must fail both the exhaustive test and the property, and that run is recorded.
- Obligation P-FAITHFUL: on `Ok`, every supplied fact can be read back
  through the accessors, and no other fact is present. This rules out a vacuous
  `build` that returns an empty index. Method: proptest over valid fact sets.
  Artefact: `common/src/async_trait_hygiene/tests/build_props.rs`.
- Obligation P-WELLFORMED: the generator first builds a valid fact set `F`
  and asserts `Ok`. It then injects exactly one defect `d` (a dangling
  supertrait, a dangling impl trait, a dangling local alias, a duplicate trait
  key, or a key reused across item kinds) and asserts the `Err` variant and key
  for `d`. Every `Ok` index satisfies RI-0 to RI-6 as checked by
  `assert_well_formed`. That function is a test-only checker written
  independently of `build_with`. Its RI-2 check runs its own `BTreeSet<K>`
  fixpoint directly over `K`, not the kernel or the dense mapping. Method:
  proptest plus an rstest table of named defect cases. Artefacts:
  `build_props.rs`, `build_cases.rs`, and `tests/support.rs`. Non-vacuity:
  defect injection guarantees every error variant is generated, with no
  filtering. Seeded fault: removing the alias-reference check from validation
  must make the property fail.
- Obligation P-ORDER (RI-7): building from a shuffled permutation of a fact
  sequence equals building from the original order. The sequences include
  conflicting duplicates and injected defects, so the error path is exercised
  too. Method: proptest with `prop_shuffle`. Artefact: `build_props.rs`.
  Non-vacuity: sequences have at least two facts. Seeded fault: a builder that
  lets a duplicate trait key silently overwrite the earlier one makes the
  result order-dependent, and the property must fail.
- Obligation T-SIBLINGS and P-SIBLING-ORACLE: sibling derivation matches the
  `candidates` specification. The named rstest cases are: same parent matches;
  different parent does not match by default; different parent matches when
  `allow_cross_module_siblings` is set; two candidates are `Ambiguous` and
  recorded as an anomaly; a trait named exactly the prefix plus its own name is
  not its own sibling; one native trait serving two dyn traits is recorded as
  an anomaly with `native_of` returning both; and a custom prefix works. A
  proptest compares derivation against a direct set-comprehension oracle over
  random names drawn from a small alphabet, which forces collisions. Artefact:
  `common/src/async_trait_hygiene/tests/derive_cases.rs` and `derive_props.rs`.
- Obligation T-ADAPTERS: adapter derivation matches `adapters(t)`. The named
  cases are: a type-parameter impl bounded by the unique sibling is an adapter;
  a type-parameter impl bounded by some other trait is not; a concrete impl is
  not; and with an ambiguous or absent sibling there is no adapter, while the
  impl is still counted. Artefact: `derive_cases.rs`.
- Obligation T-QUERIES: each accessor returns what the facts say. That
  includes the default `ImplCounts` for a trait without impls, `None` for an
  unknown key, supertraits and sites in ascending order, and `native_of` as the
  reverse of `native_sibling_of`. Method: rstest named cases with `googletest`
  matchers and `pretty_assertions` equality. Artefact:
  `common/src/async_trait_hygiene/tests/index_queries.rs`.
- Obligation B-FAMILIES: realistic ADR 0006 family shapes behave as a
  maintainer expects. See the feature file under `Plan of work`. Method:
  rstest-bdd. Artefacts:
  `common/tests/features/async_trait_family_index.feature` and
  `common/tests/async_trait_family_index_behaviour.rs`.
- Obligation C-ADAPTER: `LocalItemKey` round-trips `LocalDefId` and orders
  as `DefIndex` does. The aliases instantiate the model with `Span`. Method:
  compile-time evidence. `make lint` builds and lints `whitaker_lint_core` with
  `--all-features`. Residual gap: no runtime test runs in the gate until
  10.2.1. EP-M4 records, as supplementary evidence only, whether
  `cargo test -p whitaker_lint_core --features dylint-driver --lib` runs under
  the `make test` `RUSTFLAGS`. EP-M5 writes the obligation into the roadmap
  entries for 10.1.2 and 10.2.1. 10.1.2 owes a property that facts extracted
  from any compiling crate never make `build_with` fail. 10.2.1 owes a UI
  fixture covering diagnostic order across two traits and a foreign
  boxed-future alias.

Verus is used only for V-CLOSURE and V-RESTRICT. Builder validation, sibling
and adapter derivation, accessors, error construction, and the adapter newtype
are straightforward data plumbing or set comprehensions with direct oracles.
Proof obligations for them would be artificial, and tests give proportionate
assurance.

## Plan of work

### Stage A: pre-flight (EP-M0, no code changes)

Run the baseline gates. Re-fetch remote branches and confirm ADR 0014 is still
free.

### Stage B: proof first (EP-M1)

Create `verus/async_trait_family_closure.rs` containing:

- spec functions over the exec view: `edge(adj, x, y)`,
  `reach_in(adj, roots, y, k)` (recursive on `k`, growing paths backward),
  `reachable(adj, roots, y)` (existential with `#[trigger] reach_in`),
  `closed(adj, s)` (with `#[trigger]` on `adj@[x]@[j]`), and `count_false(s)`;
- the exec model
  `reachable_from(adj: &Vec<Vec<usize>>, roots: &Vec<usize>) -> (visited: Vec<bool>)`,
  with `requires` (every successor and root `< adj.len()`) and `ensures` as in
  V-CLOSURE, ghost `expanded`, invariants per L-EXIT, and `decreases` per
  L-TERM;
- the lemmas L-SOUND, L-CLOSED, and L-COUNT as `proof fn`s;
- the witnesses and the recorded `// production-kernel-sha256:` line, filled
  in at EP-M3 when the kernel exists. Until then it holds the literal
  placeholder `pending-EP-M3`, which the tripwire script rejects with a clear
  message. The script is not wired into CI until EP-M3.

Create `verus/async_trait_family_restrict.rs` with V-RESTRICT, its witness, and
its counterexample proof.

Write the specification, signatures, and `ensures` clauses first, then run
`make verus-async-trait` to see it fail. Then add invariants and lemma bodies
until it passes.

Wire the proofs in:

- in `scripts/run-verus.sh`, add an `async-trait` arm to
  `proof_files_for_group`, add both files to the `all` arm, and add
  `async-trait` to the allowlist at line 62;
- add a `verus-async-trait` target to `Makefile` and to `.PHONY`;
- `.github/workflows/proofs.yml` already triggers on `verus/**`, so it needs
  no change.

### Stage C: pure model (EP-M2, EP-M3)

The file layout below is sized to stay under 400 lines per file.

- `common/src/async_trait_hygiene/mod.rs` (about 90 lines): module
  documentation covering scope, re-use policy, invariants, the D-4 error rule,
  and re-exports.
- `common/src/async_trait_hygiene/facts/` (about 350 lines across
  `mod.rs` and `shape.rs`): fact value types, all with private fields,
  `const fn` constructors where possible, and accessors.
  - `TraitInfo<K>::new(name, def_path, parent: K, is_from_expansion)`.
  - `MethodKey<K> { trait_key, method_key }`, deriving `Ord`.
  - `AsyncMethodShape<K, S>::new(name, return_shape, return_site: S,
    is_dispatchable, borrowed_input_count: usize,
    has_async_trait_lifetime)`. `is_dispatchable` is false when the method
    has `where Self: Sized`. `borrowed_input_count` counts inputs whose type
    carries an elided or anonymous lifetime, including `&self`, `Foo<'_>`,
    and `Cow<'_, _>`.
  - `ReturnShape<K>`: `BoxedFutureAlias { alias: AliasRef<K>, send:
    SendBound, lifetime: LifetimeBound }`, `RawBoxedFuture { send,
    lifetime }`, `ImplFuture { send, lifetime }`, `AsyncFn`, and
    `NotAsync`.
  - `AliasRef<K>`: `Local(K)` or `Foreign { def_path: String }`.
  - `SendBound`: `Send` or `NotSend`.
  - `LifetimeBound`: `Absent`, `Elided` (meaning `'_`), `Named(String)`, or
    `Static`.
  - `BoxedFutureAliasInfo::new(name, def_path, send, has_lifetime_param)`.
  - `ImplInfo<K>::new(trait_key, self_ty: SelfTyKind<K>, def_path,
    is_from_expansion)`.
  - `SelfTyKind<K>`: `TypeParam { bounds: BTreeSet<K> }`,
    `Concrete { path: String }`, or `Other`.

  These are data only. The classifiers that produce them belong to 10.1.2.
- `common/src/async_trait_hygiene/policy.rs` (about 70 lines):
  `FamilyNamingPolicy::new(native_prefix, allow_cross_module_siblings)`, with
  `Default` giving `("Native", false)`, the design §5.2.3 defaults. The rustdoc
  records the constraint that index-shaping settings are suite-level only.
  Per-lint configuration tables (design §6.4) may filter results but must not
  change these settings, or the shared index would disagree between lints.
- `common/src/async_trait_hygiene/error.rs` (about 90 lines):
  `#[non_exhaustive] FamilyIndexError<K>`, derived with `thiserror`. The
  variants are `DuplicateKey { key, first: FactKind, second: FactKind }`
  (covering RI-0 and duplicate keyed facts) and
  `UnknownKey { key, expected: FactKind, referenced_by: FactKind }`. `FactKind`
  is a small `Copy` `#[non_exhaustive]` enum: `Trait`, `Impl`, `Alias`,
  `Method`, `DynUse`, `Supertrait`, `ImplBound`, `ReturnAlias`.
- `common/src/async_trait_hygiene/builder.rs` (about 300 lines):
  `AsyncTraitFamilyFacts<K, S>`, with `const fn new` and `Default`. It holds
  facts in insertion order. Its command methods return `()`: `add_trait`,
  `add_dyn_use`, `add_supertrait`, `add_boxed_future_alias`,
  `add_method_shape`, and `add_impl`. Its consuming queries are
  `build_with(self, policy: &FamilyNamingPolicy) -> Result<…>` and
  `build(self)`, which uses the default policy. Validation runs as a sequence
  of small functions, each returning `Result`, in a fixed order: duplicates and
  key-kind collisions first, then references. Each walks sorted structures and
  reports the least offending key (D-4).
- `common/src/async_trait_hygiene/closure.rs` (about 100 lines):
  `pub(crate) fn reachable_from(adjacency: &[Vec<usize>], roots: &[usize])
  -> Vec<bool>`,
  between the `verus-mirror` markers, plus
  `pub(crate) fn dyn_required_closure`, which maps keys to dense indices and
  back.
- `common/src/async_trait_hygiene/derive.rs` (about 200 lines): sibling
  derivation (`SiblingMatch<K>::{Unique(K), Ambiguous(BTreeSet<K>)}` and the
  reverse `native_of`), adapter derivation, impl counts, and anomalies.
  `#[non_exhaustive] FamilyAnomaly<K>` has two variants:
  `AmbiguousNativeSibling { dyn_trait, candidates }` and
  `NativeSiblingSharedByMany { native_trait, dyn_traits }`.
- `common/src/async_trait_hygiene/index.rs` (about 250 lines): the sealed
  `AsyncTraitFamilyIndex<K, S>`, deriving `Debug`, `Clone`, `PartialEq`, and
  `Eq`. Its query methods are `traits`, `trait_info`, `dyn_use_sites`,
  `supertraits_of`, `is_dyn_required`, `dyn_required`, `boxed_future_alias`,
  `method_shapes_of` (a range query on `MethodKey`), `impls`, `impl_info`,
  `impls_of`, `native_sibling_of`, `native_of`, `adapters_of`, `impl_counts`,
  `anomalies`, and `policy`. Each returns borrowed data or an iterator in key
  order. If the file exceeds 400 lines, split it into `index/facts.rs` and
  `index/derived.rs`.
- Tests in `common/src/async_trait_hygiene/tests/`, each file under 400
  lines: `mod.rs`, `support.rs` (fixtures, fact generators, the oracles, and
  `assert_well_formed`), `closure_cases.rs`, `closure_props.rs`,
  `build_cases.rs`, `build_props.rs`, `derive_cases.rs`, `derive_props.rs`, and
  `index_queries.rs`. `support.rs` uses `rstest` fixtures rather than plain
  helper functions, so it satisfies `rstest_helper_should_be_fixture` under
  `make lint-whitaker`.
- `common/src/lib.rs`: add `pub mod async_trait_hygiene;` and re-export only
  `AsyncTraitFamilyFacts`, `AsyncTraitFamilyIndex`, `FamilyIndexError`, and
  `FamilyNamingPolicy`.
- `common/Cargo.toml`: add `googletest = { workspace = true }` and
  `pretty_assertions = { workspace = true }` under `[dev-dependencies]`, and
  refresh `Cargo.lock` with `cargo update -p whitaker-common --offline` or,
  failing that, `cargo metadata` online.

Behavioural specification, in
`common/tests/features/async_trait_family_index.feature`. Scenarios 0 to 2 land
in EP-M2, and scenarios 3 to 9 are appended in EP-M3, so earlier
`#[scenario(index = N)]` values never shift.

```gherkin
Feature: Async-trait family index
  The family index records dyn-backed async interface families for one crate,
  derives which traits must stay dyn-compatible, and pairs dyn-facing traits
  with their native siblings and blanket adapters.

  Scenario: A supertrait that is not a recorded trait is rejected
    Given a trait named Tool in module app
    And Tool declares supertrait Missing
    When the family index is built
    Then the build is rejected for the unknown key Missing

  Scenario: A key recorded as both a trait and an impl is rejected
    Given a trait named Tool in module app
    And the key of Tool is also recorded as an impl of Tool
    When the family index is built
    Then the build is rejected for a duplicate key

  Scenario: Recorded dyn uses are read back in order
    Given a trait named Database in module db
    And Database is used as dyn at site 7
    And Database is used as dyn at site 3
    When the family index is built
    Then Database has dyn use sites 3 and 7

  Scenario: A dyn-used trait makes its supertraits dyn-required
    Given a trait named Database in module db
    And a trait named ConversationStore in module db
    And Database declares supertrait ConversationStore
    And Database is used as dyn at site 1
    When the family index is built
    Then Database is dyn-required
    And ConversationStore is dyn-required

  Scenario: Supertrait inheritance is followed transitively
    Given a trait named Tool in module tools
    And a trait named Describe in module tools
    And a trait named Named in module tools
    And Tool declares supertrait Describe
    And Describe declares supertrait Named
    And Tool is used as dyn at site 1
    When the family index is built
    Then Named is dyn-required

  Scenario: A trait with no dyn use is not dyn-required
    Given a trait named Database in module db
    And a trait named SuccessEvaluator in module eval
    And Database is used as dyn at site 1
    When the family index is built
    Then SuccessEvaluator is not dyn-required

  Scenario: A complete ADR 0006 family is recognized
    Given a trait named McpTransport in module mcp
    And a trait named NativeMcpTransport in module mcp
    And a blanket impl of McpTransport bounded by NativeMcpTransport
    And a concrete impl of McpTransport for StdioTransport
    When the family index is built
    Then the native sibling of McpTransport is NativeMcpTransport
    And McpTransport has 1 blanket adapter
    And McpTransport has 1 type-parameter impl and 1 concrete impl

  Scenario: A native trait in another module is not a sibling by default
    Given a trait named Tool in module tools
    And a trait named NativeTool in module native
    And a blanket impl of Tool bounded by NativeTool
    When the family index is built
    Then Tool has no native sibling
    And Tool has 0 blanket adapters

  Scenario: Cross-module matching records an ambiguous sibling
    Given cross-module siblings are allowed
    And a trait named Tool in module tools
    And a trait named NativeTool in module native
    And a trait named NativeTool in module legacy
    When the family index is built
    Then the native sibling of Tool is ambiguous
    And an ambiguous-sibling anomaly is recorded for Tool

  Scenario: An empty index builds with nothing dyn-required
    Given no facts
    When the family index is built
    Then no trait is dyn-required
    And no anomaly is recorded
```

Two traits share the name `NativeTool` in the ambiguity scenario, so the world
maps each `(name, module)` pair to a distinct `u32` key. Step text refers to a
trait by name only where the name is unique. Where it is not, the step names
the module as well, which the step definitions handle.

The step definitions in `common/tests/async_trait_family_index_behaviour.rs`
follow `common/tests/decomposition_adjacency_behaviour.rs`:

- the world is a `RefCell` fixture named `world`, not `_world`, annotated
  with `#[allow_fixture_expansion_lints]` and `#[fixture]`;
- `given` and `when` steps only mutate the world;
- `then` steps return `Result<(), String>` and report failure with `Err`,
  never with `assert!`;
- each step parses at most three values;
- the scenario functions carry `#[scenario(index = N)]` in declaration order.

### Stage D: adapter (EP-M4)

Create `crates/whitaker_lint_core/src/async_trait_hygiene.rs` (about 120
lines). It holds the newtype `LocalItemKey(DefIndex)`, deriving `Clone`, `Copy`,
`Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash`, with these
methods:

- `const fn from_local_def_id(id: LocalDefId) -> Self`;
- `const fn local_def_id(self) -> LocalDefId`;
- `fn def_id(self) -> DefId`.

It also defines these type aliases:

```rust
pub type HirAsyncTraitFamilyFacts = AsyncTraitFamilyFacts<LocalItemKey, Span>;
pub type HirAsyncTraitFamilyIndex = AsyncTraitFamilyIndex<LocalItemKey, Span>;
```

Register the module in `crates/whitaker_lint_core/src/lib.rs` under
`#[cfg(feature = "dylint-driver")] pub mod async_trait_hygiene;`. The module
documentation states that item spans come from `tcx.def_span`. There is no HIR
traversal here; that is 10.1.2.

### Stage E: documentation (EP-M5)

- Write ADR 0014,
  `docs/adrs/adr-0014-async-trait-family-index-representation.md`, using the
  ADR template in `docs/documentation-style-guide.md`. It records D-1, D-2,
  D-3, D-4, and A-DEFINDEX. The rejected options it lists are:
  - `DefPathHash` keys;
  - `(CrateNum, DefIndex)` keys;
  - `whitaker_lint_core`-only placement;
  - a separate unpublished crate;
  - supplied sibling and adapter facts with coherence errors.
- In the design document:
  - amend §5.1 to match the field-mapping table;
  - amend §5.2.3 and §5.2.4 to say where siblings and adapters are now
    derived;
  - amend §6.1 for the new placement;
  - add §6.1.1 "Implementation decisions (10.1.1)". It links ADR 0014 and
    records RI-0 to RI-7, the error rule (D-4), and the suite-level
    naming-policy constraint.
- In `docs/developers-guide.md`:
  - add a "## Shared async-trait hygiene helpers" section, with a model-layer
    table for `whitaker_common::async_trait_hygiene` and an adapter-layer
    table for `whitaker_lint_core::async_trait_hygiene`;
  - include the re-use policy in that section. The index is built at most
    once per crate per lint pass. Consumers query it and never mutate it.
    New derived fields go inside `build_with`. New facts get validation, a
    P-WELLFORMED defect case, and a P-FAITHFUL read-back. Consumers handle
    `Err` per D-4;
  - add a bullet under "Verus scope and trust boundary" describing the new
    proofs and the mirror tripwire;
  - in a separate commit, correct the stale "Adapter layer — `whitaker`"
    heading and text to `whitaker_lint_core`.
- In `docs/repository-layout.md:52-56`, add "async-trait family analysis"
  to the list of topics in `common/`.
- Add ADR 0014 to the ADR list in `docs/contents.md`.
- In `docs/roadmap.md`:
  - mark 10.1.1 `[x]` and correct its module path to
    `whitaker_common::async_trait_hygiene` and
    `whitaker_lint_core::async_trait_hygiene`;
  - reword 10.1.2 to "HIR fact extraction (dyn uses, supertrait predicates,
    impls, method return shapes, boxed-future alias shapes) and filtering to
    local items". Note that the closure, sibling, and adapter derivations
    exist, and add the never-fail-on-compiling-code property obligation;
  - annotate 10.1.3 to say that it maps the suite-level settings onto
    `FamilyNamingPolicy`;
  - add the UI-fixture obligation to 10.2.1.
- Make no `docs/users-guide.md` change (D-7).

## Milestones and plateaus

Each milestone ends with `make check-fmt`, `make lint`, `make test`, and, from
EP-M2 onwards, `make test-doc`, run sequentially, preferably by the
`scrutineer` agent, with output captured through `tee`. Documentation changes
also run `make fmt` first, then `make markdownlint`, `make nixie`, and
`make spelling`. Each milestone ends with at least one commit. Red tests are
observed locally before the green change, and are never committed failing.

- EP-M0: baseline. Outcome: the gates pass on the unmodified branch, with
  logs recorded, which establishes that later failures are ours. Recovery: if a
  baseline gate fails, record it under `Surprises` and escalate before
  starting. Compatibility decision: none.
- EP-M1: proof. Outcome: the two Verus files, the `run-verus.sh` group, and
  the `make verus-async-trait` target. Discharges V-CLOSURE and V-RESTRICT
  (ATH-CLOSURE, ATH-LOCAL). Acceptance:
  - `make verus-async-trait` verifies with zero errors, and `N` is recorded;
  - both seeded mutations and the V-RESTRICT counterexample are observed and
    recorded;
  - the forbidden-construct scan below prints nothing:

    ```bash
    forbidden='\b(assume|admit)\b|assume_specification|uninterp|#\[cfg'
    forbidden="${forbidden}|verifier::(external[a-z_]*|exec_allows_no_decreases_clause)"
    grep -nE "${forbidden}" \
      verus/async_trait_family_closure.rs verus/async_trait_family_restrict.rs
    ```

  - the standard gates pass.
  Conformance check: the specification matches design §5.2.2, and no production
  code changes. Recovery: the proof files are self-contained, so revert the
  commit. Remaining gaps: the production kernel and the mirror hash (EP-M3).
  Compatibility decision: none.
- EP-M2: facts and validation. Outcome: `facts/`, `policy.rs`, `error.rs`,
  `builder.rs`, and `index.rs` with fact accessors only. The red tests are
  `build_cases.rs`, `build_props.rs` (P-FAITHFUL, P-WELLFORMED, P-ORDER), the
  fact half of `index_queries.rs`, and BDD scenarios 0 to 2. Derived accessors
  are not exposed until EP-M3 adds the derivations, so no interim placeholder
  exists. Acceptance: the focused commands under `Concrete steps` and the
  standard gates pass. Recovery: revert to EP-M1. Compatibility decision: none
  (new API ahead of any release that contains it).
- EP-M3: derivations. Outcome: `closure.rs`, `derive.rs`, the derived
  accessors, the recorded mirror hash, and the tripwire wired into
  `make verus-async-trait` and `ci.yml`. The red tests are `closure_cases.rs`
  (T-KERNEL-EXHAUSTIVE), `closure_props.rs`, `derive_cases.rs`,
  `derive_props.rs`, the RI-2 to RI-5 checks in `assert_well_formed`, the
  derived half of `index_queries.rs`, and BDD scenarios 3 to 9. Acceptance: the
  seeded "roots only" kernel and the M-MIRROR negative control are observed
  failing and recorded; `make verus-async-trait` passes, tripwire included; the
  standard gates pass. Conformance check: compare kernel and model side by side
  and record the result in `Outcomes`. Recovery: revert to EP-M2.
- EP-M4: adapter. Outcome: the gated
  `whitaker_lint_core::async_trait_hygiene` module. Acceptance: `make lint`
  passes, which compiles the module with `--all-features` under Clippy.
  `make typecheck` also passes, along with the standard gates. The
  supplementary gated-test result is recorded. Conformance check: the root
  `whitaker` package and `common/Cargo.toml` gain no `rustc_*` dependency.
  Recovery: revert the module.
- EP-M5: documentation and completion. Outcome: the ADR, design, developers'
  guide, repository layout, contents, and roadmap updates. Acceptance: all
  documentation gates and standard gates pass, and the roadmap shows 10.1.1 as
  done. Conformance check: reconcile every `Surprises` entry with the design
  document and ADR 0014 before setting the status to `COMPLETE`.

## Concrete steps

Run every command from the worktree root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/debddaa4-b6ea-4cf8-95da-b4fae7591574`.
Log names follow
`/tmp/$ACTION-whitaker-10-1-1-async-trait-hygiene-modules.out`.

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-10-1-1-async-trait-hygiene-modules.out
make lint      2>&1 | tee /tmp/lint-whitaker-10-1-1-async-trait-hygiene-modules.out
make test      2>&1 | tee /tmp/test-whitaker-10-1-1-async-trait-hygiene-modules.out
make test-doc  2>&1 | tee /tmp/test-doc-whitaker-10-1-1-async-trait-hygiene-modules.out
make verus-async-trait 2>&1 | tee /tmp/verus-whitaker-10-1-1-async-trait-hygiene-modules.out
```

Once EP-M1 is green, the Verus run ends with:

```plaintext
verification results:: N verified, 0 errors
```

`N` is the number of proved functions and lemmas, recorded in `Outcomes`.

During EP-M2 and EP-M3, run the focused loop with the same `RUSTFLAGS` that
`make test` uses, so focused runs and the gate share build caches and deny
warnings alike. Read the flags from `make -n test`, then run:

```bash
RUSTFLAGS="<flags printed by make -n test for the main nextest pass>" \
  cargo nextest run --locked -p whitaker-common \
  -E 'test(async_trait_hygiene) | binary(async_trait_family_index_behaviour)' 2>&1 \
  | tee /tmp/test-focused-whitaker-10-1-1-async-trait-hygiene-modules.out
```

The gate is authoritative if the two ever disagree.

Before committing ADR 0014, re-scan for collisions:

```bash
git fetch git@github.com:leynos/whitaker.git '+refs/heads/*:refs/remotes/origin/*'
for b in $(git for-each-ref --format='%(refname)' refs/remotes/origin); do
  git ls-tree --name-only "$b" docs/adrs/; done | sort -u | grep adr-0014
```

The scan must print nothing.

## Validation and acceptance

The plan is complete when all of the following hold:

1. `make verus-async-trait` runs the mirror tripwire and both proof files,
   reports zero errors with the recorded `N`, and the forbidden-construct scan
   is empty.
2. `make test` passes and includes the new tests: the unit modules under
   `whitaker_common::async_trait_hygiene::tests`, the property tests, and the
   ten scenarios of `async_trait_family_index_behaviour`. Each new test was
   observed failing before its production code existed.
3. `make test-doc` passes. It includes a Rustdoc example on
   `AsyncTraitFamilyFacts` that builds a two-trait family with `u32` keys and
   `u32` sites and asserts that the supertrait is dyn-required. It also
   includes an example on `FamilyNamingPolicy` showing a cross-module sibling
   match.
4. `make check-fmt`, `make lint`, `make markdownlint`, `make nixie`, and
   `make spelling` pass.
5. The developers' guide documents both layers, the re-use policy, and the
   error rule. ADR 0014 exists and is linked from the design document and
   `docs/contents.md`. Roadmap 10.1.1 is checked, and 10.1.2, 10.1.3, and
   10.2.1 carry the follow-up wording and obligations.

## Idempotence and recovery

Every step adds files or appends to lists, so re-running a step after a partial
failure is safe once the partial edit is reverted with
`git checkout -- <path>`. Each milestone is one or more commits, so
`git revert` of a milestone's commits restores the previous plateau. No step
touches persisted data or deployed state.

## Interfaces and dependencies

New public items, all in `whitaker_common::async_trait_hygiene`:

- the four types also re-exported at the crate root:
  `AsyncTraitFamilyFacts<K, S>`, `AsyncTraitFamilyIndex<K, S>`,
  `FamilyIndexError<K>`, and `FamilyNamingPolicy`;
- the types reachable only through the module path: `FactKind`,
  `FamilyAnomaly<K>`, `SiblingMatch<K>`, `TraitInfo<K>`, `MethodKey<K>`,
  `AsyncMethodShape<K, S>`, `ReturnShape<K>`, `AliasRef<K>`, `SendBound`,
  `LifetimeBound`, `BoxedFutureAliasInfo`, `ImplInfo<K>`, `SelfTyKind<K>`, and
  `ImplCounts`.

The bounds are `K: Ord + Copy` and `S: Ord + Clone` on the `impl` blocks, with
`K: fmt::Debug` only where errors are formatted.

New items in `whitaker_lint_core::async_trait_hygiene`, behind `dylint-driver`:
`LocalItemKey`, `HirAsyncTraitFamilyFacts`, and `HirAsyncTraitFamilyIndex`.

New dev-dependencies of `whitaker-common`: `googletest` and
`pretty_assertions`, both existing workspace dependencies.

New scripts and targets: `scripts/check-verus-async-trait-mirror.sh` and
`make verus-async-trait`.

## Revision note

- 2026-11-11, initial draft (`c5f366fc`).
- 2026-11-11, revision 1, after review by three independent experts
  (structure and contracts; verification rigour; alternatives, conventions, and
  pre-mortem). Each change is listed with why it changed and how it affects the
  remaining work:
  - Validation is now restricted to caller defects, and anomalies are
    recorded rather than rejected. The first draft would have rejected the
    states the lints exist to report, silencing every lint on a crate.
  - Sibling lookup and adapter discovery are derived in 10.1.1 under a
    `FamilyNamingPolicy` (D-2 revised), because they are pure logic and the
    gate never runs gated code.
  - `AliasRef` is added, because foreign boxed-future aliases had to be
    representable.
  - `return_site` is added, because diagnostics and suggestions need the
    return-type span.
  - Sites are stored as sets, `diagnostic_paths` is folded into `def_path`,
    and RI-0 is added, which makes RI-7 hold strictly.
  - The Verus specification is reformulated with backward path growth, a
    ghost `expanded` sequence, a single termination measure, and L-COUNT.
  - V-RESTRICT moves to its own file and is stated relationally.
  - Non-vacuity is strengthened: counterexamples are proved for each seeded
    mutation, the forbidden-construct scan is wider, and the verified count
    is pinned.
  - New obligations close vacuity gaps: P-FAITHFUL, defect-injection
    generation, T-KERNEL-EXHAUSTIVE, metamorphic checks, the independent
    RI-2 oracle, and the M-MIRROR tripwire.
  - P-ORDER is restated, because its earlier seeded fault could not fail.
  - The BDD scenarios are reordered so scenario indices never shift.
  - Plumbing corrections: `make test-doc` is added to the gates, the second
    `run-verus.sh` allowlist is handled, `Cargo.lock` is permitted, the
    tolerances are raised to 40 files and 4,500 lines, the crate-root
    re-exports are narrowed, structs get private fields, and the
    `#[non_exhaustive]` policy is set (D-8).
  - Impl-method shapes are scoped out explicitly (D-9).
  - Roadmap follow-up obligations are added for 10.1.2, 10.1.3, and 10.2.1.

  Net effect on the remaining work: EP-M3 grows by the sibling and adapter
  derivations. 10.1.2 shrinks to HIR fact extraction. The plan still needs user
  approval, D-2 in particular.
