# Map candidate spans to `ra_ap_syntax` nodes and extract AST feature vectors (7.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

## Purpose / big picture

Whitaker's clone detector runs in two passes. Pass A (already shipped, roadmap
items 7.2.x) tokenises Rust source, winnows fingerprints, and uses MinHash plus
locality-sensitive hashing (LSH) to emit candidate clone pairs into a Static
Analysis Results Interchange Format (SARIF) report. Pass A finds Type-1
(whitespace/comment differences) and Type-2 (identifier/literal renaming)
clones, but it is blind to *near-miss* (Type-3) clones because tokens alone
cannot see structure.

Pass B (the AST engine) refines those candidates using a real Rust parse tree.
This ExecPlan delivers the **first half** of Pass B, roadmap item 7.3.1: given a
candidate's byte span, map it to the smallest covering syntax node from the
`ra_ap_syntax` parser, and extract a deterministic **AST feature vector** from
that subtree. The feature vector has three components described in the design
document: a depth-weighted **node-kind histogram**, a **production multiset**
of parent→child (bigram) and parent→child→grandchild (trigram) edges, and a
**canonical Merkle-style subtree hash** with normalised leaves.

Scoring those features into a Type-3 similarity, and writing SARIF Run 1, is the
**next** roadmap item (7.3.2) and is explicitly out of scope here. This item
produces the pure, well-tested building blocks 7.3.2 will consume.

What a reader can observe after this change:

- The workspace builds on `nightly-2026-05-28`: `rust-toolchain.toml` names the
  new channel, and the whole Dylint suite (lint crates, `clippy_utils`, vendored
  shims, installer, UI tests) builds and passes its gates on it — an overdue
  maintenance bump that also unblocks a clean `ra_ap_syntax` pin for Pass B.
- A new `whitaker_clones_core::ast` module exists with a single adapter entry
  point, `lower_span(file_text, span) -> Result<NormalisedTree, AstError>`, and
  three pure feature functions, `kind_histogram`, `production_multiset`, and
  `canonical_hash`.
- `cargo test -p whitaker_clones_core` passes, including new `rstest` unit
  tests, an `rstest-bdd` behavioural feature, an `insta` snapshot of a feature
  vector, and `proptest` invariants.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- `make kani-clone-detector` verifies the bounded smallest-covering-node
  selection and structural-bound harnesses; `make verus-clone-detector`
  discharges a histogram-accumulation order-independence lemma.

This plan does **not** change any user-facing behaviour or command-line surface;
no `docs/users-guide.md` change is required for 7.3.1 (recorded in the Decision
Log). It does add internal interface documentation to the clone-detector design
document and the developers' guide.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- **Scope boundary.** Do not implement Type-3 scoring (cosine/SimHash/edit
  distance), SARIF Run 1 emission, or any `clones.refined.sarif` writing. Those
  are item 7.3.2. The public surface added here must be *forward-compatible*
  with 7.3.2 but must not anticipate it with dead code.
- **Crate location.** All new code lands in `crates/whitaker_clones_core`
  (a pure library crate, `publish = false`, edition 2024). Do not add
  `ra_ap_syntax` to the root `whitaker` crate, the `installer`, the `suite`, or
  any Dylint driver crate.
- **Hexagonal dependency rule (machine-enforced).** Exactly one file under
  `crates/whitaker_clones_core/src/ast/` may import `ra_ap_syntax` (or its
  transitive parser crates `rowan` / `ra_ap_parser`): the adapter
  (`ast/lowering.rs`). Every other `ast/` file (the domain) must not name those
  crates, and no domain file may `use` the adapter (`ast::lowering`) — the
  dependency points adapter→domain only. This is enforced by a guard test
  delivered in Stage A (`tests/ast_boundary.rs`), not merely by review: it
  asserts no domain source line matches `^\s*use\s+(ra_ap_\w+|rowan)\b` and no
  bare `ra_ap_syntax::`/`rowan::` path appears outside comments, with the
  forbidden-crate list as a `const`.
- **No persisted `KindId` from 7.3.1.** Only `AstHash` (which is seeded with
  `PARSER_SCHEMA_VERSION`) is hashable/serialisable in this item. `KindId` is an
  in-memory opaque token and must not be persisted, so a future cache (7.6.x)
  cannot accidentally compare raw discriminants across parser pins.
- **Bounded per-candidate cost.** Lowering touches one candidate subtree; the
  upstream `min_nodes`/node-count bound from the Pass A config governs subtree
  size. 7.3.1 does not lower whole files; the smallest-covering selection climbs
  only to the tightest covering ancestor.
- **Toolchain bump (Stage 0).** This item bumps `rust-toolchain.toml` from
  `nightly-2025-09-18` (rustc 1.92.0-nightly) to **`nightly-2026-05-28`**
  (rustc ≈ 1.9x-nightly, comfortably ≥ 1.95) as a prerequisite Stage 0, landing
  as its own atomic commit before the AST work. The bump is suite-wide: after
  it, the *entire* workspace — every Dylint lint crate, `clippy_utils`, the
  vendored `rustc_*` shims, the installer, and all UI `.stderr` fixtures — must
  build and pass `make check-fmt`/`make lint`/`make test` on the new channel.
  The selected `ra_ap_syntax` version is then a *contemporaneous* snapshot
  (matching the new nightly), not a backwards-bisected one, and must compile
  cleanly under `-D warnings`. Do not regress any other crate's behaviour while
  bumping; UI-diagnostic drift is re-baselined, not suppressed.
- **No silenced lints.** `cargo clippy --workspace --all-targets --all-features
  -- -D warnings` must pass with no new `#[allow(...)]` except as a tightly
  scoped last resort with a written reason.
- **No `expect()`/`unwrap()` outside tests.** The design document's Pass B
  sketch uses `SourceFile::parse(...).ok().expect("parse")`; this is forbidden
  by `AGENTS.md` outside `#[cfg(test)]`. Replace it with typed `thiserror`
  errors.
- **File size.** No source file exceeds 400 lines (`AGENTS.md`). Split feature
  math across files as needed.
- **Determinism.** All feature outputs must be byte-identical across runs and
  platforms for identical input; this is required for SARIF golden tests in
  7.3.2 and for the `insta` snapshot here. No `HashMap` iteration order, no
  `f64` non-determinism, no raw pointer or address dependence.
- **Spelling and docs.** en-GB-oxendict spelling in comments and prose;
  module-level `//!` docs on every new module; rustdoc `///` with runnable
  examples on every public item.

## Tolerances (exception triggers)

Thresholds that trigger escalation rather than autonomous continuation.

- **Stage 0 toolchain bump.** If, after the bump to `nightly-2026-05-28`, the
  Dylint suite cannot be made to build and pass `make lint`/`make test` because
  (a) `dylint` v5 cannot drive the new nightly and no compatible
  `dylint_linting`/`dylint_testing` release exists, or (b) lint-crate/
  `clippy_utils` `rustc_private` breakage requires rustc-internal API archaeology
  beyond a focused effort, **stop and escalate** with the build evidence rather
  than suppressing errors or partially reverting. Re-baselining `.stderr`
  fixtures is expected and is *not* an escalation; masking a genuine behaviour
  change behind a re-bless is forbidden.
- **Dependency.** Introducing `ra_ap_syntax` is a new external dependency,
  *mandated by the design document* and therefore pre-approved. Post-bump it is
  a contemporaneous snapshot; if no snapshot near `nightly-2026-05-28` compiles
  under `-D warnings`, **stop and escalate** with the build evidence.
- **Transitive surface.** If pinning `ra_ap_syntax` requires pinning more than
  three additional transitive crates with `--precise` to build, stop and
  escalate (it suggests a deeper toolchain mismatch).
- **Verus.** If the histogram-accumulation lemma (Stage F) cannot be made
  substantive and well-founded within two focused attempts, stop and escalate;
  do **not** commit a vacuous proof or a restatement of the assumption. The
  fallback (Kani + proptest only, citing ADR-003) requires a Decision Log entry
  and is an escalation, not a silent downgrade.
- **Kani.** If a bounded harness still times out after trying a tightened unwind
  bound and a `kissat`/`cadical` solver swap, stop and escalate.
- **Scope (Stage 0 excepted).** Stage 0's toolchain bump is *expected* to be a
  large, mostly-mechanical diff (re-baselined `.stderr` fixtures, ~105 string
  updates, any lint-crate `rustc_private` fixes); it is a single atomic commit
  and is exempt from the per-feature file-count budget. For the AST work
  (Stages A–G), if implementation appears to require changes to more than ~16
  files (net) or touches any crate other than `whitaker_clones_core` plus the
  two proof scripts and the docs, stop and escalate. One budgeted intra-crate
  touch is expected there: promoting the FNV-1a constants from
  `token/fingerprint.rs` into a new `pub(crate)` `crate::hashing` module and
  updating `token` to use it (see Decision Log 🔴-E). That touch must keep
  `token`'s tests green.
- **Iterations.** If `make lint` or `make test` still fails after 4 focused
  fix attempts on the same milestone, stop and escalate.
- **Ambiguity.** The open questions in the Decision Log have proposed defaults.
  If implementation reveals that a default materially changes the 7.3.2
  interface, stop and present options before encoding it.

## Risks

- Risk: **`ra_ap_syntax` API drift across `0.0.x` snapshots.** The parser API is
  unstable and date-stamped; `SourceFile::parse` already changed from the
  one-argument form in the design sketch to the two-argument
  `parse(text, Edition::CURRENT)` form.
  Severity: medium. Likelihood: high.
  Mitigation: exact-pin the version (`=0.0.x`) with a documented reason; confine
  every `ra_ap_syntax` symbol to `ast/lowering.rs`; lower into an owned,
  parser-agnostic `NormalisedTree` so a future bump recompiles one file and
  leaves all domain logic and proofs untouched.
- Risk: **MSRV incompatibility (resolved by the Stage 0 bump).** Under the old
  `nightly-2025-09-18` pin (rustc 1.92), current `ra_ap_syntax` (MSRV 1.95) did
  not build, forcing a backwards bisect. The Stage 0 bump to `nightly-2026-05-28`
  (≈ rustc 1.9x-nightly, ≥ 1.95) removes this: a contemporaneous `ra_ap_syntax`
  snapshot now builds directly.
  Severity: low (post-bump). Likelihood: low.
  Mitigation: select the `ra_ap_syntax` snapshot dated near the new nightly and
  confirm a clean build; exact-pin and record it.
- Risk: **Lint-crate `rustc_private` breakage on the bump.** The vendored
  `rustc_*` shims are thin re-export wrappers (no edit needed), but
  `clippy_utils` and the lint crates call the nightly's internal rustc API
  directly; an ~8-month jump (rustc 1.92 → ≈ 1.9x) is likely to break some
  call sites.
  Severity: high. Likelihood: medium-high.
  Mitigation: Stage 0 treats a clean `cargo build`/`make lint` of the whole
  suite as the go/no-go; fix breakage in the affected crate (not by suppression);
  if breakage is widespread or needs API archaeology beyond the Stage 0 budget,
  stop and escalate (see Tolerances).
- Risk: **`dylint` v5 incompatible with the new nightly.** `dylint_linting` /
  `dylint_testing` v5 build a driver against the pinned toolchain; a newer
  nightly may need a newer `dylint` release.
  Severity: high. Likelihood: medium.
  Mitigation: Stage 0 verifies the UI-test harness builds and runs on the new
  channel before any AST work; if v5 cannot drive `nightly-2026-05-28`, bump
  `dylint_linting`/`dylint_testing` to a compatible release (recorded as a
  Decision), or escalate if no compatible release exists.
- Risk: **UI `.stderr` fixture drift.** ~34 `.stderr` expectation files across
  the lint crates encode rustc diagnostic text that commonly shifts between
  nightlies.
  Severity: medium. Likelihood: high.
  Mitigation: re-baseline via the Dylint/`trybuild` blessing flow as part of
  Stage 0, reviewing each diff so a genuine behaviour change is not masked by a
  cosmetic re-bless.
- Risk: **Stale toolchain-string references.** ~105 occurrences of
  `nightly-2025-09-18` exist in installer source, tests, ADR-001, and docs;
  some are load-bearing test fixtures (the installer's `ToolchainChannel`
  parsing tests), most are doc/example strings.
  Severity: medium. Likelihood: high.
  Mitigation: Stage 0 updates the load-bearing ones and the artefact/manifest
  examples; CI (`rolling-release.yml`) reads the channel dynamically from
  `rust-toolchain.toml`, so artefact naming propagates automatically — but the
  installer unit/behaviour tests that hardcode the date must be updated and
  kept green.
- Risk: **`SyntaxKind` discriminant is not a stable public contract.** There is
  no guaranteed variant-count constant, and discriminants can move between
  snapshots.
  Severity: medium. Likelihood: medium.
  Mitigation: treat `KindId(u16)` as an opaque, possibly-unstable token used
  only for equality and bucketing, never matched against named variants in the
  domain; the cache (future 7.6.x) must key on the pinned parser version.
- Risk: **Float weights defeat verification and snapshots.** The design's
  `w(depth) = 1/(1 + depth)` is a float; `f64` is hostile to Verus/Kani and to
  deterministic snapshots.
  Severity: medium. Likelihood: high.
  Mitigation: represent histogram weights as exact fixed-point scaled integers
  (see Decision Log); keep `f64` out of the stored feature vector.
- Risk: **Kani parses nothing.** Running `ra_ap_syntax` under Kani is
  intractable.
  Severity: low (by design). Likelihood: low.
  Mitigation: the lowered-IR boundary means Kani harnesses build small owned
  `NormalisedTree` values directly and never invoke the parser.
- Risk: **Dev-test dependency surface.** Correcting an earlier draft: `insta`
  and `proptest` are *already* `[workspace.dependencies]`; this item only adds
  `{ workspace = true }` dev-dep lines, not new crates. `googletest` and
  `pretty_assertions` *are* absent from the workspace.
  Severity: low. Likelihood: low.
  Mitigation: per the Decision Log, follow the established in-repo
  `assert_eq!`-with-`insta` idiom and do **not** add
  `googletest`/`pretty_assertions` for this scope-limited item. The only
  genuinely new runtime dependency is `ra_ap_syntax` and its transitives.
- Risk: **Silent cache poisoning across a parser bump.** A future
  `ra_ap_syntax` bump shifts `SyntaxKind` discriminants; without protection, a
  cache (future 7.6.x) persisting `ast_hashes` would silently match stale
  buckets that now mean something different — wrong Type-3 results, no crash.
  Severity: high. Likelihood: medium.
  Mitigation: seed `canonical_hash` with `PARSER_SCHEMA_VERSION` so every hash
  changes on a bump and cross-pin compares fail closed; snapshot the sentinel;
  do not persist `KindId` from 7.3.1 (see Constraints).

## Progress

- [x] Stage 0 — Bump `rust-toolchain.toml` to `nightly-2026-05-28` suite-wide
      (own atomic commit; go/no-go); all gates green before Stage A.
- [x] Stage A — Orientation, boundary guard (`tests/ast_boundary.rs`), and red
      skeleton (no production logic).
- [ ] Stage B — `ra_ap_syntax` version spike and dependency wiring
      (prototyping milestone; go/no-go).
- [ ] Stage C — Domain IR and pure feature math (`tree`, `features`, `hash`),
      red-green-refactor.
- [ ] Stage D — `ra_ap_syntax` adapter (`lowering.rs`) and span→node mapping.
- [ ] Stage E — Behavioural (`rstest-bdd`), snapshot (`insta`), and property
      (`proptest`) coverage.
- [ ] Stage F — Verus lemma and Kani harnesses; proof-script wiring.
- [ ] Stage G — Documentation, final gates, CodeRabbit review, roadmap tick.

Each stage records a timestamp here when complete and splits into
done/remaining if interrupted.

Stage 0 completed on 2026-06-16. Green gates:

- `make check-fmt`
- `make lint`
- `make test` (`1453` passed, `3` skipped)
- `make markdownlint`
- `coderabbit review --agent` (`0` findings)

Stage A completed on 2026-06-16. Green gates:

- `cargo test -p whitaker_clones_core ast`
- `cargo test -p whitaker_clones_core --doc`
- `make check-fmt`
- `make lint`
- `make test` (`1455` passed, `3` skipped)
- `make markdownlint`
- `coderabbit review --agent` (`0` findings)

## Surprises & discoveries

- Observation: implementation began on 2026-06-16 after explicit user
  direction to proceed from the draft plan. The branch was already
  task-specific (`7-3-1-map-candidate-spans-and-extract-ast-feature-vectors`)
  and the worktree was clean before Stage 0 edits.
- Observation: `nightly-2026-05-28` resolves locally to
  `rustc 1.98.0-nightly (57d06900f 2026-05-27)`.
- Observation: `cargo build --workspace` passed on the new nightly without
  local `rustc_private` or `clippy_utils` edits. The Dylint UI smoke failed
  with `dylint_linting` 5.0.0 because `ParseSess::env_depinfo` and
  `ParseSess::file_depinfo` no longer exist, so Stage 0 upgraded
  `dylint_linting`, `dylint_testing`, `cargo-dylint`, and `dylint-link` to
  6.0.1.
- Observation: after the Dylint 6.0.1 bump, local lint code needed the rustc
  diagnostic API migration from `span_lint` to `emit_span_lint` plus
  `DiagDecorator`. The existing `rustc_lint` shim now re-exports
  `DiagDecorator`, keeping that rustc-private surface centralised. The
  `bumpy_road_function` UI smoke passed after this migration.
- Observation: rustc 1.98 moved several APIs used by tests and lints:
  `StableHashCtxt` now lives under `rustc_data_structures::stable_hash`,
  `hir::AttrPath` segments are `Box<[Symbol]>`, and type normalisation now uses
  `ty::TypingEnv` plus `ty::Unnormalized::new_wip`.
- Observation: the full Stage 0 gates exposed two real compatibility drifts in
  `no_unwrap_or_else_panic`, not fixture churn. First, newer HIR parent
  iteration no longer walks from an expression to its owner item, so harness
  detection now checks the current owner `HirId` directly in both
  `no_unwrap_or_else_panic` and `no_expect_outside_tests`. Second,
  `rstest`/rustc generated companion modules now combine a `test` crate import
  with same-named descriptor const/function items whose expansion contexts do
  not compare equal by span; the companion matcher keeps the old strict
  same-span rule and adds this narrow generated-harness fallback.
- Observation: interpolated `panic!` expansion now constructs
  `core::fmt::Arguments::new` rather than the older `new_v1` /
  `new_v1_formatted` names. The panic classifier now accepts all three names
  after verifying the receiver type is `core::fmt::Arguments`.
- Observation: `make fmt` would currently touch unrelated Markdown files
  because of pre-existing markdownlint/formatting drift outside this Stage 0
  change. To keep the atomic commit scoped, Stage 0 used `cargo fmt --all` and
  the required `make check-fmt` Rust formatting gate; broad Markdown formatting
  remains out of scope for this compatibility commit.
- Observation: Stage A added the `ast` module skeleton, root re-exports,
  parser-boundary guard, and a neutral `#[cfg(kani)]` placeholder module. The
  placeholder exists because `rustfmt` resolves the module graph even when the
  `kani` cfg is not active.
- Observation: the new public AST skeleton carries doctest examples. The
  Makefile's `make test` target uses `cargo nextest`, so Stage A also ran
  `cargo test -p whitaker_clones_core --doc` explicitly.
- Observation: the plan's “red skeleton” step was treated as local TDD intent,
  not as a committed failing state. Repository policy requires every commit to
  pass gates, so the committed Stage A skeleton returns typed placeholder
  values/errors and includes a neutral hash regression that Stage C will replace
  with real feature logic.

## Decision log

Decisions already taken while drafting this plan:

- Decision: **Bump `rust-toolchain.toml` to `nightly-2026-05-28` as a
  prerequisite Stage 0 of this item, landing as its own atomic commit before the
  AST work.** Rationale: the bump is *both* an overdue maintenance step (the pin
  had sat on `nightly-2025-09-18` since 2025) *and* the cleanest unblock for
  Pass B — under rustc 1.92 the current `ra_ap_syntax` (MSRV 1.95) would not
  build, forcing a fragile backwards bisect; on `nightly-2026-05-28` (≈ rustc
  1.9x, ≥ 1.95) a contemporaneous `ra_ap_syntax` snapshot builds directly,
  matching rust-analyzer's own nightly cadence. Structuring it as a Stage 0
  atomic commit (rather than a separate roadmap item) keeps the enabling change
  and its consumer in one reviewable history while still isolating the bump in
  its own commit. The bump is suite-wide and its blast radius (lint-crate
  `rustc_private` API, ~34 `.stderr` fixtures, `dylint` v5 compat, ~105 string
  refs) is carried in Risks and gated by the Stage 0 go/no-go. Chosen by the
  user (2026-06-09): folded-in Stage 0, both maintenance and 7.3.1 enabler.
  Date/Author: 2026-06-09, user direction.
- Decision: **Use a lowered intermediate representation (`NormalisedTree`),
  not a `trait SyntaxTreeNode` port over live `ra_ap_syntax` nodes.**
  Rationale: a borrowed-tree port forces the domain to be generic over an
  unbounded lazily-borrowed rowan graph, which Kani cannot construct
  symbolically and proptest cannot derive `Arbitrary` for without building an
  owned mock anyway. An owned IR makes bounded Kani harnesses and proptest
  strategies trivial, makes feature functions pure `&NormalisedTree -> _`, and
  confines every parser assumption to one lowering function — the strictest
  possible drift insulation. Cost: one small allocation per candidate subtree,
  which is negligible because Pass B only touches candidate regions.
  Date/Author: 2026-06-09, planning team (synthesised).
- Decision: **Store exact integer `(KindId, depth)` counts as the canonical
  histogram substrate, and apply the depth weighting in a thin, pure, total
  seam function — not `f64`, and not a scaled-integer-at-extraction store.**
  Rationale (the count-substrate hybrid, adopted from the design-review panel,
  dissolving the unsatisfiable “exact fixed-point for all depths” trap): the
  design's `1/(1 + depth)` float makes snapshots platform-fragile and makes the
  Verus lemma and Kani bounds undecidable, so `f64` is rejected. But *scaling at
  extraction* is also wrong: `SCALE / (1 + depth)` cannot be exact for every
  depth with any finite `SCALE` (that would need `lcm` of all `1 + depth`). The
  resolution: the stored feature substrate is `KindCounts`, an exact
  `BTreeMap<(KindId, Depth), u32>` of per-(kind, depth) node counts; a separate
  pure function `weighted_histogram(&KindCounts) -> KindHistogram` applies
  `w(depth)` as a fixed-point `KindWeight`. This makes the depth-weight curve a
  *tuning knob* 7.3.2 can change without re-lowering, re-snapshotting, or
  re-proving; lets the Verus lemma fold exact `u32` counts (a clean, decidable
  permutation-invariance statement with no overflow/rounding obligation); and
  keeps the `insta` snapshot of the substrate fully exact. The fixed-point
  `KindWeight` representation and its documented `SCALE` are pinned in the
  Interfaces section, not deferred. The design document specifies the *weighted*
  histogram as the deliverable, so both `KindCounts` and the weighting seam are
  delivered; the weighted result remains the public `KindHistogram`.
  Date/Author: 2026-06-09, planning team + design-review panel.
- Decision: **Reject the borrowed-tree `trait SyntaxTreeNode` port** in favour
  of the lowered owned IR (above). Rationale recorded for posterity: a port
  trait over live rowan nodes is not symbolically constructible by Kani and not
  `Arbitrary`-derivable by proptest, so its test and proof doubles would
  recreate an owned tree anyway — without the strict drift insulation the IR
  gives. Endorsed 6/6 by the review panel.
  Date/Author: 2026-06-09.
- Decision: **`AstHash` is an opaque type; its public contract is `to_hex()
  -> String`, `Display`, and `Eq`/`Ord` only — never `AstHash(pub u64)` or a
  `get() -> u64`.** Rationale: the shipped `run0` code already overrides the
  design doc's `"astHash": u64` schema (`tokenHash` is a SHA-256 hex `String`,
  `emit.rs`), so a public `u64` both contradicts precedent and forces a breaking
  change if 7.3.2 widens the digest to `sha2`. Keeping the backing store private
  (FNV-1a `u64` for now) lets 7.3.2 swap to `sha2` without touching the surface.
  Date/Author: 2026-06-09, Telefono (review panel).
- Decision: **Seed `canonical_hash` with a compile-time `PARSER_SCHEMA_VERSION`
  constant** (derived from the pinned `ra_ap_syntax` version), absorbed at the
  hash root. Rationale: 7.6.x will cache `ast_hashes`; a future parser bump
  shifts `SyntaxKind` discriminants, so without a version seed a stale cache
  would silently match buckets that now mean something different (silent Type-3
  corruption, no crash). Seeding makes every hash change on a bump, so any
  cross-pin cache compare fails closed. An `insta` snapshot of
  `PARSER_SCHEMA_VERSION` forces any bump to be reviewed. `KindId` itself is
  **not** persisted by 7.3.1 (only `AstHash` is hashable/serialisable here);
  this is stated as a Constraint so 7.6.x inherits the rule.
  Date/Author: 2026-06-09, Doggylump (review panel).
- Decision: **Promote the FNV-1a constants and byte-mixing step from
  `token/fingerprint.rs` (currently `pub(super)`) into a `pub(crate)` shared
  hashing helper** (e.g. `crate::hashing`), and reuse it from both `token` and
  `ast/hash.rs`. Rationale: `ast/hash.rs` cannot name the constants across the
  module boundary without either widening `token`'s visibility ad hoc or
  duplicating them; a shared `pub(crate)` helper is the clean, DRY choice. This
  is a small, budgeted touch to the `token` module (recorded in Tolerances).
  Date/Author: 2026-06-09, Buzzy Bee (review panel).
- Decision: **Replace the design sketch's `.expect("parse")` with typed
  `thiserror` errors (`AstError`).** Rationale: `AGENTS.md` forbids `expect()`
  outside tests; `SourceFile::parse` is error-tolerant and returns a tree even
  for malformed input, so the real failure modes are span-out-of-bounds and
  inverted/empty spans, which deserve typed errors.
  Date/Author: 2026-06-09.
- Decision: **Canonical subtree hash uses the crate's existing FNV-1a-style
  64-bit stable mixing**, returning a `u64`, matching the design document's
  `ast_hash(node) -> u64` signature and the existing `token` module idiom.
  Rationale: the subtree hash is a fast non-cryptographic structural
  fingerprint, not content addressing; FNV-1a is a streaming byte-absorbing
  construction suited to folding `(kind, arity, ordered child hashes)`, and is
  the established stable-hash idiom in this crate. `sha2` (used by `run0` for
  SARIF partial fingerprints) is the alternative if 7.3.2 needs a wider digest;
  recorded as an open question.
  Date/Author: 2026-06-09.
- Decision: **Follow the in-repo `assert_eq!` + `insta` assertion idiom; do not
  add `googletest`/`pretty_assertions` for this item.** Rationale: those two
  crates are genuinely absent from the workspace, and adding them widens
  supply-chain surface for a scope-limited change producing pure functions over
  an owned IR, where `assert_eq!` plus `insta` is sufficient and matches the
  existing `token`/`index`/`run0` test idiom. The task brief lists them as
  available tools, not mandates. Note (correcting an earlier draft): `insta` and
  `proptest` are **already** `[workspace.dependencies]`; using them needs only
  `{ workspace = true }` dev-dep lines and is *not* a new dependency — the only
  genuinely new runtime dependency in this item is `ra_ap_syntax` and its
  transitives.
  Date/Author: 2026-06-09.
- Decision: **No `docs/users-guide.md` change for 7.3.1.** Rationale: this item
  adds no user-facing behaviour or command-line surface; the CLI (7.4.x) and the
  `clone_detected` lint (7.5.x) are separate items. Internal interface docs go
  to `docs/whitaker-clone-detector-design.md` and `docs/developers-guide.md`.
  Date/Author: 2026-06-09.

Open questions carried into implementation (each has a proposed default; encode
the default unless Stage findings contradict it, then escalate):

- OQ1: **Parse-error and `ERROR`-node policy.** `ra_ap_syntax` returns a
  recovered tree even when `parse.errors()` is non-empty. Two distinct
  questions, both decided here (per review finding 🟡-2): (a) *parse-call
  policy* — **proceed on recoverable errors** (Pass A only feeds real source),
  reserving `AstError` for span/offset problems; (b) *selected-subtree policy* —
  if the smallest covering node **is** a `SyntaxKind::ERROR` node (or its subtree
  is dominated by error nodes), return `AstError::UnparsableSpan { start, end }`
  rather than lowering garbage, and emit a `tracing::warn` (with the span and
  error count) whenever a lowered span had non-empty `parse.errors()`. This
  prevents two unrelated files with the same parse-error shape from producing a
  spurious matching hash. The Stage D malformed-source test asserts this
  *specific* behaviour, not merely “does not crash”. Revisit the error-ratio
  threshold if it proves too strict/lax.
- OQ6: **Per-node byte-span provenance in the IR.** `NormalisedNode` currently
  carries no per-node byte offset; only the root `NormalisedTree` carries the
  candidate `ByteSpan`. 7.3.2's optional bounded tree-edit-distance refinement
  may want per-node provenance, which is cheap to add at lowering time but
  expensive to retrofit through the proofs later. Proposed default: **omit
  per-node spans for 7.3.1** (the three feature outputs do not need them) and
  add them in 7.3.2 only if the edit-distance refinement is implemented; decide
  explicitly in Stage C and record. (Review finding 💡-1.)
- OQ2: **Exact pinned `ra_ap_syntax` version.** Post-bump this is the
  *contemporaneous* snapshot dated near `nightly-2026-05-28` (around v0.0.336
  per the firecrawl research), confirmed to build cleanly in Stage B and
  recorded here. Carried as the token `0.0.PINNED` until then.
- OQ3: **`Edition::CURRENT` vs per-fragment edition.** Proposed default:
  `Edition::CURRENT` (feature extraction is structural, not semantic). Revisit
  only if edition-specific syntax causes spurious parse failures.
- OQ4: **Whether token (leaf) kinds enter the histogram/productions, or only
  non-trivia `SyntaxNode`s.** Proposed default: walk `descendants()` (nodes)
  for the histogram and productions, skipping whitespace/comment trivia; map
  tokens to a `LeafClass` only for the Merkle hash. This makes
  trivia-invariance structural rather than a post-filter.
- OQ5: **64-bit FNV-1a vs `sha2` for the subtree hash.** Default FNV-1a `u64`
  (matches the design signature); flagged for 7.3.2 review if a wider digest is
  needed for the `astHash` partial fingerprint.

## Outcomes & retrospective

To be completed at milestones and at the end. Compare delivered behaviour
against the Purpose section; note what would be done differently.

## Context and orientation

This section assumes no prior knowledge of the repository.

### Where this fits

The clone detector lives in `crates/whitaker_clones_core` (pure algorithms) and
`crates/whitaker_sarif` (SARIF 2.1.0 models). The CLI and Dylint consumer are
later roadmap sections. The authoritative design is
`docs/whitaker-clone-detector-design.md`; the relevant section is **“Pass B: AST
engine (`ra_ap_syntax`)”**, covering parsing and region mapping, feature
extraction (node-kind histogram, production multiset, canonical subtree hash),
and — for 7.3.2, not here — scoring and SARIF Run 1.

The existing crate is organised by feature, each as a directory module:

- `src/token/` — `rustc_lexer` normalisation, k-shingling, Rabin-Karp hashing,
  winnowing. The stable FNV-1a-style hashing idiom this plan reuses for the
  subtree hash lives here.
- `src/index/` — MinHash sketches, LSH candidate generation, `FragmentId`,
  `CandidatePair`. Note `src/index/kani.rs` for the established
  `#[cfg(kani)]` harness style.
- `src/run0/` — token-pass acceptance and SARIF Run 0 emission.
  `src/run0/span.rs` shows the existing byte-range→SARIF-region conversion and
  its validation (`validate_range` rejects `start >= end` and non-char-boundary
  offsets); mirror that validation discipline for `ByteSpan`.
- `src/lib.rs` — re-exports the public surface; this plan adds `pub mod ast;`
  and a focused set of `ast` re-exports.

Existing public types this item interoperates with: `FragmentId` (string-backed
newtype, lexical ordering), `CandidatePair`, and the `Fingerprint { hash: u64,
range: Range<usize> }` type. 7.3.1 does not modify them.

### The `ra_ap_syntax` parser (external dependency, new to the repo)

`ra_ap_syntax` is rust-analyzer's concrete-syntax-tree library, published to
crates.io under date-stamped, **unstable** `0.0.x` versions. It builds on
ordinary crates (`rowan`, `smol_str`, `triomphe`, `itertools`, `rustc-hash`,
`ra_ap_parser`, `ra_ap_stdx`) and needs **no** `rustc-dev` sysroot coupling, so
a pure library crate can depend on it. The current API surface this plan uses
(verified against docs.rs):

```rust,ignore
use ra_ap_syntax::{SourceFile, SyntaxNode, SyntaxKind, TextRange, TextSize, Edition};

let parse = SourceFile::parse(file_text, Edition::CURRENT); // Parse<SourceFile>
let _errors = parse.errors();      // &[SyntaxError]; recovered tree still returned
let file: SourceFile = parse.tree();
let root: &SyntaxNode = file.syntax();
for node in root.descendants() {   // pre-order over SyntaxNodes
    let _r: TextRange = node.text_range();
    let _k: SyntaxKind = node.kind(); // SyntaxKind is #[repr(u16)]
}
```

`Edition` and `SyntaxKind` are re-exported from `ra_ap_syntax` directly. The
design sketch's one-argument `SourceFile::parse(text)` and `.ok().expect(...)`
are **outdated**; use the two-argument form and typed errors.

### Signposted documentation and skills

Consult these while implementing:

- `docs/whitaker-clone-detector-design.md` — §“Pass B: AST engine”
  (authoritative requirements; record new decisions back into it).
- `docs/adr-003-formal-proof-strategy-for-clone-detector-pipeline.md` — when to
  use Verus (small local pure semantic invariants, sidecar models in `verus/`)
  versus Kani (bounded checks over real production code, colocated under
  `#[cfg(kani)]`).
- `docs/rust-testing-with-rstest-fixtures.md` — fixture and parameterisation
  patterns for the unit tests.
- `docs/rstest-bdd-users-guide.md` — the behavioural test pattern; mirror the
  existing `tests/candidate_pair_behaviour.rs` + `tests/features/*.feature`
  layout.
- `docs/rust-doctest-dry-guide.md` — keep rustdoc examples runnable and
  non-duplicative.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — keep functions
  small and within the 400-line file budget.
- `docs/documentation-style-guide.md` — ADR/design-doc formatting if a new ADR
  becomes warranted.
- Skills: `hexagonal-architecture` (port/adapter boundary), `kani` (bounded
  harnesses, mutation validation), `verus` (sidecar lemma, triggers,
  `decreases`), `proptest` (strategies, shrinking, regression files),
  `rust-unit-testing` (rstest, assertions), `leta` (semantic navigation),
  `nextest` (test running). The `rust-router` skill routes any further Rust
  questions.

## Interfaces and dependencies

At the end of this milestone the following must exist in
`crates/whitaker_clones_core`.

Module layout under `src/ast/` (domain vs adapter marked):

```plaintext
ast/
  mod.rs        module root, //! docs, dependency-rule invariant, re-exports
  error.rs      DOMAIN   AstError (thiserror), AstResult
  tree.rs       DOMAIN   KindId, Depth, LeafClass, NormalisedNode/Tree, ByteSpan
  cover.rs      DOMAIN   select_smallest_covering(candidates, target) (pure index math)
  features.rs   DOMAIN   kind_counts, weighted_histogram, production_multiset (+ types)
  hash.rs       DOMAIN   canonical_hash, AstHash; uses crate::hashing (shared FNV-1a)
  lowering.rs   ADAPTER  the ONLY file importing ra_ap_syntax; lower_span; PARSER_SCHEMA_VERSION
  tests.rs      TEST     #[cfg(test)] unit + proptest over the IR
  kani.rs       PROOF    #[cfg(kani)] bounded harnesses over NormalisedTree
```

Plus, outside `ast/`, a new `crate::hashing` module (`src/hashing.rs`,
`pub(crate)`) holding the FNV-1a constants and byte-mixing step promoted from
`token/fingerprint.rs` (see Decision Log 🔴-E); `token` is updated to use it.

Split discipline (review finding 🟡-4): `lowering.rs` must stay an adapter; if
it nears 400 lines, *pure* logic (such as the smallest-covering selection in
`cover.rs`) moves **out into the domain**, never into a second adapter file, so
the “exactly one file imports `ra_ap_syntax`” invariant is never re-crossed. If
`features.rs` approaches 400 lines, split production extraction into
`ast/productions.rs` (domain).

Domain types (`tree.rs`), parser-agnostic and owned:

```rust,ignore
/// Stable, opaque node-kind id lowered from `SyntaxKind` (`#[repr(u16)]`).
/// Used only for equality and bucketing; never matched against named variants.
/// `Debug`/`Display` emit only the opaque `u16` — the domain never names a
/// `SyntaxKind` variant. The test-only `KindId -> "BIN_EXPR"` rendering used by
/// the insta snapshot lives in the adapter, behind `#[cfg(test)]`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct KindId(u16);

/// Tree depth of a node relative to the lowered subtree root (root = 0).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Depth(u16);

/// Normalised leaf token class (Type-2 erasure of identifiers and literals).
/// `#[non_exhaustive]` so 7.3.2 may add literal sub-classes (mirroring the token
/// pass's `<NUM>`/`<STR>`/… granularity) or `Lifetime` without a breaking bump.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum LeafClass { Ident, Literal, Other }

/// Owned, parser-agnostic node. The only tree the domain ever sees.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NormalisedNode {
    kind: KindId,
    leaf: Option<LeafClass>,          // Some iff this node is a normalised leaf
    children: Vec<NormalisedNode>,
}

/// A lowered candidate subtree plus its provenance span.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NormalisedTree { root: NormalisedNode, span: ByteSpan }

/// Half-open byte range [start, end) over the source text. `new` rejects
/// `start > end`, `start == end`, and offsets that are not UTF-8 char
/// boundaries (mirroring `run0/span.rs::validate_range`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ByteSpan { start: u32, end: u32 }
```

Domain feature functions (`features.rs`, `hash.rs`), pure. The histogram is a
two-step pipeline: extract exact integer counts (the canonical, snapshot-stable
substrate), then apply depth weighting in a thin total seam.

```rust,ignore
/// Exact, depth-resolved node-kind counts — the canonical histogram substrate.
/// Deterministic and exact (no rounding); this is what insta snapshots and what
/// the Verus lemma folds. Backed by `BTreeMap<(KindId, Depth), u32>`.
pub fn kind_counts(tree: &NormalisedTree) -> KindCounts;

/// Apply the depth weight `w(depth)` (fixed-point `KindWeight`) to the counts.
/// Pure and total; the depth-weight curve is a knob 7.3.2 can retune without
/// re-lowering or re-proving.
pub fn weighted_histogram(counts: &KindCounts) -> KindHistogram;

/// Convenience wrapper: `weighted_histogram(&kind_counts(tree))`.
pub fn kind_histogram(tree: &NormalisedTree) -> KindHistogram;

/// (parent->child) bigrams and (parent->child->grandchild) trigrams.
pub fn production_multiset(tree: &NormalisedTree) -> ProductionMultiset;

/// Canonical Merkle-style subtree hash of the tree's root. Leaves normalise to
/// <ID>/<LIT>/<OTHER>; internal nodes fold (kind, arity, ordered child hashes).
/// The fold is seeded with `crate::hashing::PARSER_SCHEMA_VERSION` (a neutral
/// const, not the adapter) so a parser-pin bump changes every hash (cache fails
/// closed). Backing store is private (FNV-1a u64 now).
pub fn canonical_hash(tree: &NormalisedTree) -> AstHash;
```

Public read APIs (the contract 7.3.2's cosine/Jaccard scorers consume — pinned
now so 7.3.2 adds only scoring functions, no surface churn):

```rust,ignore
/// Fixed-point depth weight. `SCALE` is documented and public; `w(depth)` is
/// chosen from a representable family (see Decision Log) so values are exact.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct KindWeight(u64);
impl KindWeight { pub const SCALE: u64 = /* documented in Stage C */ 0; }

/// Depth-weighted histogram keyed by KindId; deterministic ordered iteration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KindHistogram(/* BTreeMap<KindId, KindWeight> */);
impl KindHistogram {
    pub fn get(&self, kind: KindId) -> Option<KindWeight>;
    pub fn iter(&self) -> impl Iterator<Item = (KindId, KindWeight)> + '_; // ascending KindId
}

/// A production edge: a parent->child bigram or parent->child->grandchild trigram.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Production {
    Bigram(KindId, KindId),
    Trigram(KindId, KindId, KindId),
}

/// Multiset of production edges; deterministic ordered iteration.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ProductionMultiset(/* BTreeMap<Production, u32> */);
impl ProductionMultiset {
    pub fn count(&self, production: Production) -> u32;
    pub fn bigrams(&self) -> impl Iterator<Item = (Production, u32)> + '_;
    pub fn trigrams(&self) -> impl Iterator<Item = (Production, u32)> + '_;
    pub fn iter(&self) -> impl Iterator<Item = (Production, u32)> + '_;
}

/// Opaque canonical subtree hash. Backing width (FNV-1a u64 now; possibly sha2
/// in 7.3.2) is private; the public contract is hex/Display/ordering only.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AstHash(/* private u64 */);
impl AstHash {
    pub fn to_hex(&self) -> String;     // feeds the SARIF astHash partial fingerprint in 7.3.2
}
// plus impl std::fmt::Display for AstHash
```

`KindCounts`, `KindHistogram`, and `ProductionMultiset` use `BTreeMap`
internally for deterministic iteration. Forward-compat note: 7.3.2 cosine is
`Σ(left[k]·right[k])` over the merged `KindId` key set, served by
`KindHistogram::get`/`iter`; Jaccard over `ProductionMultiset` is served by
`count`/`iter`; `AstHash::to_hex()` feeds `properties.whitaker.astHash`.

Adapter (`lowering.rs`), the sole `ra_ap_syntax` boundary:

```rust,ignore
/// Parse `file_text`, map `span` to the smallest covering node, and lower that
/// subtree into a `NormalisedTree`. Nothing rowan-shaped crosses this boundary.
pub fn lower_span(file_text: &str, span: ByteSpan) -> Result<NormalisedTree, AstError>;
```

Errors (`error.rs`):

```rust,ignore
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AstError {
    #[error("byte span end {end} precedes start {start}")]
    InvalidSpan { start: u32, end: u32 },
    #[error("byte span is empty at offset {offset}")]
    EmptySpan { offset: u32 },
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    NonCharBoundary { offset: u32 },
    #[error("byte span {start}..{end} lies outside the parsed source of length {len}")]
    SpanOutOfBounds { start: u32, end: u32, len: usize },
    #[error("byte offset {0} exceeds the u32 TextSize range")]
    OffsetTooLarge(u32),
    #[error("byte span {start}..{end} maps to an unparsable (ERROR) subtree")]
    UnparsableSpan { start: u32, end: u32 },
}
pub type AstResult<T> = Result<T, AstError>;
```

`EmptySpan` and `NonCharBoundary` exist because the plan mirrors
`run0/span.rs::validate_range`, which rejects `start >= end` and non-char-
boundary offsets; `ByteSpan::new` rejects both *before* any `TextRange` is
built. `UnparsableSpan` realises the OQ1 selected-subtree policy.

Public re-exports added to `src/lib.rs` (minimal, forward-compatible with
7.3.2; `ra_ap_syntax` types are deliberately **not** re-exported). The list is
trimmed to what 7.3.2 and external callers provably consume: `NormalisedNode`,
`KindId`, and `LeafClass` stay `pub` *within* the `ast` module but are **not**
root re-exported (no identified cross-module consumer; `canonical_hash` now
takes `&NormalisedTree`, so `NormalisedNode` need not cross the root):

```rust,ignore
pub mod ast;
pub use ast::{
    AstError, AstResult, ByteSpan, NormalisedTree,
    KindCounts, KindHistogram, KindWeight, ProductionMultiset, Production, AstHash,
    kind_counts, weighted_histogram, kind_histogram,
    production_multiset, canonical_hash, lower_span,
};
```

Dependency declarations:

- `rust-toolchain.toml`: `channel = "nightly-2026-05-28"` (Stage 0).
- Root `Cargo.toml` `[workspace.dependencies]`: add
  `ra_ap_syntax = "=0.0.PINNED"` with a comment recording the documented reason
  for the exact pin (date-stamped unstable API; pin the snapshot contemporaneous
  with `nightly-2026-05-28`; revisit on the next toolchain bump).
- `crates/whitaker_clones_core/Cargo.toml` `[dependencies]`:
  `ra_ap_syntax = { workspace = true }`.
- `crates/whitaker_clones_core/Cargo.toml` `[dev-dependencies]`:
  `insta = { workspace = true }`, `proptest = { workspace = true }`.

Proof wiring:

- `verus/clone_detector_ast_features.rs` (new) registered in
  `scripts/run-verus.sh` under both the `clone-detector` and `all` groups.
- Kani harnesses in `src/ast/kani.rs` registered by name in
  `scripts/run-kani.sh`'s `run_clone_detector_harnesses` list.

## Plan of work

Stages end with validation; do not proceed past a failing stage.

### Stage 0 — Toolchain bump to `nightly-2026-05-28` (prerequisite; go/no-go)

A suite-wide prerequisite, landed as **its own atomic commit** before any AST
work. Edit `rust-toolchain.toml` `channel` to `nightly-2026-05-28`. Install the
channel with the pinned components (`rustfmt`, `clippy`, `rustc-dev`,
`llvm-tools-preview`, `rust-src`) and record the resolved `rustc --version` in
Surprises & Discoveries.

Verify, in order (each is a go/no-go; escalate per Tolerances rather than
suppressing): (1) the whole workspace builds — `cargo build --workspace`,
fixing any `clippy_utils`/lint-crate `rustc_private` breakage in the affected
crate; (2) `dylint_testing` can drive the new nightly — build one lint crate's
UI harness; if `dylint` v5 cannot drive it, bump `dylint_linting`/
`dylint_testing` to a compatible release and record the decision; (3)
`make check-fmt`, `make lint`, then `make test` pass — re-baseline the ~34
`.stderr` UI fixtures through the Dylint/`trybuild` blessing flow, **reviewing
each diff** so a genuine behaviour change is never masked by a cosmetic
re-bless; (4) update the load-bearing toolchain-string references (installer
`ToolchainChannel`/manifest tests and ADR-001 examples) from
`nightly-2025-09-18` to the new channel — CI's `rolling-release.yml` reads the
channel dynamically, so artefact naming propagates automatically, but the
installer unit/behaviour tests that hardcode the date must be updated and kept
green.

Commit the bump (toolchain, lockfile, re-baselined fixtures, string updates) as
one atomic commit. The Verus/Kani sidecar toolchains are pinned independently in
`scripts/` and are out of scope for this bump unless their proofs fail to run.
Validation: `make check-fmt`, `make lint`, `make test` all pass on
`nightly-2026-05-28` across the whole workspace, with the AST module not yet
present.

### Stage A — Orientation, boundary guard, and red skeleton

No production logic. Create `src/ast/mod.rs` with `//!` docs stating the
dependency-rule invariant verbatim, declare the submodules, and add `pub mod
ast;` to `src/lib.rs`. Add `error.rs`, `tree.rs`, `cover.rs`, `features.rs`,
`hash.rs`, `lowering.rs` with module docs and `todo!()`-free stubs that return
typed errors or empty values so the crate compiles.

Deliver the **boundary guard** now (review finding 🔴-A): `tests/ast_boundary.rs`
asserting that no domain file (`ast/{error,tree,cover,features,hash,mod}.rs`)
contains a `use ra_ap_*`/`use rowan` line or a bare `ra_ap_syntax::`/`rowan::`
path outside comments, and that no domain file `use`s `ast::lowering`. Keep the
forbidden-crate list as a `const`.

Add the first **red** unit test against a *pure* feature function (e.g.
`canonical_hash` over a hand-built `NormalisedTree`), which is buildable without
the parser, so the red stage does not depend on the Stage D adapter. The
smallest-covering-node red test is explicitly a Stage D artifact. Validation:
`cargo test -p whitaker_clones_core ast::` fails red for the expected reason;
`cargo build -p whitaker_clones_core` compiles the skeleton; `tests/ast_boundary.rs`
passes (the skeleton has no violations yet).

### Stage B — `ra_ap_syntax` contemporaneous pin (go/no-go)

On the `nightly-2026-05-28` channel (rustc ≥ 1.95), select the `ra_ap_syntax`
snapshot dated near the new nightly — start from the latest at planning time
(≈ v0.0.336, MSRV 1.95, per the firecrawl research) and step *down* only if a
genuine build/API error appears. Add it exact-pinned (`=0.0.x`) to
`[workspace.dependencies]` with the documented-reason comment, and
`{ workspace = true }` to `whitaker_clones_core`. Run `cargo build -p
whitaker_clones_core` under `-D warnings`, pinning at most three offending
transitive crates with `--precise` (escalate if more are needed). Replace
`0.0.PINNED` in both manifests, OQ2, and `crate::hashing::PARSER_SCHEMA_VERSION`
with the concrete version; commit `Cargo.toml` and `Cargo.lock` together; record
the resolved transitive set (including exact `rowan`/`ra_ap_parser` versions) in
Surprises & Discoveries.

Go/no-go: if no contemporaneous snapshot builds cleanly, **stop and escalate**
(Tolerances). Otherwise proceed. Validation: a throwaway `lowering.rs` line
calling `SourceFile::parse("fn f(){}", Edition::CURRENT).tree()` compiles and the
crate builds clean.

### Stage C — Domain IR and pure feature math (red-green-refactor)

First, promote the FNV-1a constants and mixing step from `token/fingerprint.rs`
into a new `pub(crate)` `src/hashing.rs`, update `token` to use it, and confirm
`token`'s tests stay green (review finding 🔴-E).

Implement `tree.rs` (the IR, `Depth`, and `ByteSpan::new` rejecting `start >=
end` and non-char-boundary offsets per `run0/span.rs`), then `cover.rs`,
`features.rs`, and `hash.rs`. The histogram follows the count-substrate hybrid
(Decision Log): `kind_counts` builds the exact `BTreeMap<(KindId, Depth), u32>`;
`weighted_histogram` applies `w(depth)` as a fixed-point `KindWeight`. Choose and
record in the Decision Log the concrete representable weight family and `SCALE`
— the unsatisfiable “exact for all depths” target is already rejected, so use
one of: depth-capped `SCALE = lcm(1..=D+1)` (with the cap proved by the Stage F
Kani structural-bound harness), `w(depth) = 2^-depth` (exact in fixed-point, no
cap), or reduced rationals. Record the max-depth assumption alongside `SCALE` in
Artifacts. Decide OQ6 (per-node spans) here and record. Keep `canonical_hash`
order-sensitive (kind + arity + ordered child hashes), leaf-normalising
(`Ident`/`Literal` erase payload → equal hashes; different kind or arity →
different hashes), and seed it with `PARSER_SCHEMA_VERSION`. To respect the
dependency rule, define `PARSER_SCHEMA_VERSION` as a plain `&str` const in the
neutral `crate::hashing` module (kept equal to the pinned `ra_ap_syntax`
version), **not** in the adapter — so the domain hash reads it without depending
on `lowering.rs`. Drive each
function with red `rstest` unit tests over hand-built `NormalisedTree` values
first. Validation: `cargo test -p whitaker_clones_core ast::` and `token::`
green; refactor within the 400-line file budget; re-run.

### Stage D — Adapter and span→node mapping

Implement `lowering.rs`: define `PARSER_SCHEMA_VERSION` sync (a test asserts the
`crate::hashing` const equals the manifest-pinned version string); parse with
`Edition::CURRENT`; validate the `ByteSpan` against the root range; build
`TextRange`; call the domain `cover::select_smallest_covering` over the
`descendants()` ranges (the pure index math lives in `cover.rs` so Kani can
verify it without parsing); lower the chosen subtree via a private recursive
`lower_node` that maps `kind() as u16 → KindId`, classifies leaf tokens via a
single private `leaf_class` function (the only place encoding which
`SyntaxKind`s are `<ID>`/`<LIT>`), recurses over children in document order
skipping trivia per OQ4, and applies the OQ1 `ERROR`-node policy (return
`AstError::UnparsableSpan` when the covering node is/contains an `ERROR` subtree;
`tracing::warn` when a lowered span had parse errors).

Drive with red `rstest` mapping tests: exact node, smallest covering inner
expression, two-sibling common ancestor, whole-file root, and the unhappy paths
the enriched `AstError` now names — out-of-bounds, inverted, **empty (`start ==
end`)**, **non-char-boundary** (a span around an identifier following a
multi-byte comment such as `// café`), and **`ERROR`-node** subtrees (assert the
*specific* `UnparsableSpan`/recovery behaviour, not merely “does not crash”).
Validation: `cargo test -p whitaker_clones_core ast::` green; the
`tests/ast_boundary.rs` guard from Stage A still passes (only `lowering.rs`
names `ra_ap_`).

### Stage E — Behavioural, snapshot, and property coverage

Add `tests/features/ast_feature_extraction.feature` and
`tests/ast_feature_extraction_behaviour.rs` (mirror `candidate_pair_behaviour.rs`
/ `SarifWorld`) with scenarios: smallest-covering-node selection;
identifier-renamed fragments share a subtree hash; structurally different
fragments differ. Add an `insta` JSON snapshot of the **exact `KindCounts`
substrate** (not weighted floats) plus the production multiset and subtree hash
for a fixed fixture (`fn add(a: i32, b: i32) -> i32 { a + b }`), and a separate
snapshot of `PARSER_SCHEMA_VERSION` so any parser bump produces a reviewable
diff that forces the bumper to confront cache invalidation. Render `KindId` as
its **named** `SyntaxKind` string so a bump yields a reviewable, not noisy, diff
— but the `KindId → "BIN_EXPR"` renderer is a `#[cfg(test)]` helper in the
adapter (`lowering.rs`), **not** a `Display` impl on the domain `KindId` (which
would re-couple the domain to parser vocabulary). Add `proptest` invariants over
an `Arbitrary` `NormalisedTree` strategy: determinism;
`kind_counts`/`production_multiset` accumulation order-independence
(sibling-visit permutation); leaf-normalisation hash equality. State explicitly
that the order-independence property excludes `canonical_hash`, which is
deliberately order-*sensitive*; proptest uses the opaque `KindId(u16)`, never
the rendered name. Keep a checked-in `proptest-regressions/` file. Validation:
`cargo test -p whitaker_clones_core` green; `cargo insta` review accepted.

### Stage F — Verus lemma and Kani harnesses

Verus (`verus/clone_detector_ast_features.rs`): prove that count accumulation is
a permutation-invariant fold over the multiset of per-node `(kind, depth)`
contributions — folding **exact `u32` counts**, not scaled rationals (the
count-substrate hybrid makes this a clean, decidable statement with no
overflow/rounding obligation). State, in one falsifiable sentence in Artifacts,
**exactly** what the trust bridge assumes versus proves. Be honest about the
division of labour (review finding 🟡-3): if the property that actually breaks
operationally — sibling visit order in the feature walk — is carried by
**proptest**, and Verus proves only the algebraic fold over a given multiset,
then say so plainly and do **not** claim Verus is “the unbounded root proptest
samples”; the two check different things. Register the file in both the
`clone-detector` and `all` groups of `scripts/run-verus.sh`. If the lemma cannot
be made substantive (not a restatement) and well-founded in two attempts,
escalate (Tolerances) — the fallback is Kani + proptest only, with a Decision
Log entry, and the bounded Kani order-independence harness must then stand alone
as the order-independence evidence (not a coverage hole).

Kani (`src/ast/kani.rs`, `#[cfg(kani)]`): harnesses over a bounded synthetic
`NormalisedTree`/candidate set, never the parser. Pin the bounded tree shape
(depth ≤ 3, ≤ 2 children) with a `const _` assertion tying the unwind bound to
it — note the recursive state space is `branching^depth`, unlike the existing
flat `LshIndex` harnesses, so confirm `--default-unwind 4` suffices or add
per-harness `#[kani::unwind(N)]`. Harnesses:
`verify_smallest_covering_node_selects_minimal_range` (over the factored
`cover::select_smallest_covering`, with `kani::assume(n >= 2)` so the minimality
postcondition has something to bite on: result covers the target and no covering
candidate is strictly smaller); a **separate**
`verify_smallest_covering_root_fallback` for the `n == 0`/no-cover path (do not
fold it into the minimality harness, where it would be vacuous);
`verify_kind_index_is_bounded`; and a cheap
`verify_count_accumulation_is_order_independent_bounded`. Register the harness
names in `run_clone_detector_harnesses` in `scripts/run-kani.sh`.
**Mutation-validate as a matrix** (review finding 🟡-3): each deliberate mutation
— `<=`→`<` in the minimality compare, *and* dropping the covering check — must
be shown to fail **at least one named harness**, recording which mutation each
harness catches in Artifacts; a single pass/fail bit is insufficient. Restore
the production code before committing. Validation: `make verus-clone-detector`
and `make kani-clone-detector` pass; the mutation matrix is recorded.

### Stage G — Documentation, gates, review, roadmap

Record the realised design decisions in
`docs/whitaker-clone-detector-design.md` under a new “Implementation decisions
(7.3.1)” subsection (mirroring the existing 7.2.x subsections), and document the
`ast` module's hexagonal boundary, the count-substrate histogram, and the
`ra_ap_syntax` pin in `docs/developers-guide.md`. Add two short runbooks to
`docs/developers-guide.md` (the repo currently documents neither): a
**“toolchain bump runbook”** capturing the Stage 0 procedure (set the channel;
rebuild the whole suite; fix `clippy_utils`/lint-crate `rustc_private` breakage;
verify `dylint` drives the new nightly; re-baseline `.stderr` fixtures with diff
review; update the load-bearing installer/ADR-001 toolchain strings), and a
**“`ra_ap_syntax` re-pinning runbook”** (review finding 🟡-7) covering the
contemporaneous-snapshot selection, the ≤ 3 transitive `--precise` pin budget,
the escalation trigger, and the note that `PARSER_SCHEMA_VERSION` and any
`ast_hashes` cache must be invalidated on a re-pin — so the next toolchain-bump
author does not re-derive Stages 0/B from scratch. Confirm `Cargo.lock` is
committed and that the CI build uses `--locked`. Assess whether the lowered-IR
boundary or the proof strategy warrants a new ADR; if so, author
`docs/adr-004-*.md` per the style guide and reference it from the design doc
(record the decision either way). Run `make check-fmt`, `make lint`, `make
test`, then the proof targets, then `make markdownlint` for the docs. Request
`coderabbit review --agent` only after all deterministic gates pass; clear every
concern. Tick roadmap item 7.3.1 to done. Commit, push, and ensure the draft PR
references this ExecPlan.

## Concrete steps

Run all commands from the worktree root
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/9fcb15ba-ebe1-4826-b124-ac54785b9705`.
Follow `AGENTS.md`: run gates sequentially (not in parallel) to benefit from the
build cache, and `tee` long outputs to a log under `/tmp`.

```bash,ignore
# Stage 0 toolchain bump: set channel then install + verify the whole suite.
# (edit rust-toolchain.toml channel -> nightly-2026-05-28 first)
rustc --version 2>&1 | tee /tmp/rustc-version.out   # record the resolved version
cargo build --workspace 2>&1 | tee /tmp/bump-build-whitaker.out
# Re-baseline UI fixtures through the Dylint/trybuild blessing flow, reviewing each diff.

# Per-gate logging template (ACTION in {fmt,lint,test,kani,verus}):
make check-fmt 2>&1 | tee /tmp/check-fmt-whitaker-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-whitaker-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-whitaker-$(git branch --show-current).out
make kani-clone-detector  2>&1 | tee /tmp/kani-whitaker-$(git branch --show-current).out
make verus-clone-detector 2>&1 | tee /tmp/verus-whitaker-$(git branch --show-current).out

# Stage B contemporaneous ra_ap_syntax pin (illustrative; replace 0.0.X):
# (toolchain is now nightly-2026-05-28, so no +toolchain override is needed)
cargo build -p whitaker_clones_core 2>&1 | tee /tmp/raap-build.out
# Focused test runs during red-green-refactor:
cargo test -p whitaker_clones_core ast:: 2>&1 | tee /tmp/ast-test.out
cargo insta test -p whitaker_clones_core 2>&1 | tee /tmp/insta.out   # then `cargo insta review`
```

Expected transcripts (illustrative): the Stage A red test fails with a message
naming the unimplemented behaviour; after Stage D, `cargo test -p
whitaker_clones_core ast::` reports all `ast::` tests passing; `make lint`
prints no warnings.

## Validation and acceptance

Acceptance is behavioural and observable.

- **Red-Green-Refactor evidence** is recorded per function: the smallest-
  covering-node test and each feature-math test must be shown failing before
  implementation (red) and passing after the minimal change (green), then the
  wider gate re-run after refactor. Capture the red failure messages in
  Artifacts.
- **Behaviour to verify by hand:** in a scratch test or doctest, call
  `lower_span("fn f() { let x = 1 + 2; }", span_of("1 + 2"))` and observe the
  returned `NormalisedTree` root maps to the binary-expression kind, not the
  block; call `canonical_hash` on the lowered subtrees of
  `fn f(){ let a = g(); }` and `fn f(){ let b = h(); }` and observe **equal**
  hashes (identifier normalisation); call it on `a + b` vs `a - b` and observe
  **different** hashes (kind sensitivity).
- **Quality criteria (definition of done):**
  - Toolchain (Stage 0): `rust-toolchain.toml` names `nightly-2026-05-28`, and
    `make check-fmt`/`make lint`/`make test` pass across the *whole workspace*
    on it (the bump commit is green on its own, before the AST module exists);
    re-baselined `.stderr` diffs were reviewed, not blindly blessed.
  - Tests: `make test` passes; new `ast::` unit tests, the
    `ast_feature_extraction` BDD scenarios, the `insta` snapshot, and the
    `proptest` invariants all pass.
  - Lint/format: `make check-fmt` and `make lint` pass with no new allows;
    `make markdownlint` passes for changed docs.
  - Verification: `make kani-clone-detector` and `make verus-clone-detector`
    pass; the Kani mutation-validation step is recorded as having failed a
    harness on a deliberate mutation.
  - Boundary: a guard test or CI grep confirms no `ast/` domain file imports
    `ra_ap_`.
  - Review: `coderabbit review --agent` run after gates are green, with all
    concerns cleared.
- **Quality method:** the `make` targets above, run sequentially, plus CodeRabbit
  after they pass.

## Idempotence and recovery

- Apart from Stage 0, all edits are additive within `whitaker_clones_core` plus
  two proof scripts and docs; re-running any `make` target is safe.
- **Stage 0 is a single atomic commit and is fully revertible** with `git revert`
  of that commit (restoring the channel, lockfile, fixtures, and string updates
  together). Because it lands first and on its own, a later AST-stage problem
  never entangles the toolchain bump, and a bump problem never entangles the AST
  work. Keep the channel install reproducible via `rust-toolchain.toml` (rustup
  auto-installs on first `cargo` invocation).
- The Stage B pin is reversible: if a candidate version fails, `git checkout
  -- Cargo.toml Cargo.lock` and retry a different pin. Commit the manifest and
  lockfile together only once a green build is achieved.
- `insta` snapshots: regenerate with `cargo insta test` and review with `cargo
  insta review`; never hand-edit `.snap` files.
- Kani mutation validation must be **reverted** before committing (restore the
  original `select_smallest_covering`); the deliberate break is a check, not a
  change.
- Each stage is committed separately so any stage can be rolled back with `git
  revert` without losing earlier stages.

## Artifacts and notes

Record here, as work proceeds: the resolved `nightly-2026-05-28` `rustc
--version`; any `dylint_linting`/`dylint_testing` version change needed to drive
it; the list of `.stderr` fixtures re-baselined (with a one-line note on each
non-cosmetic diff); the resolved `ra_ap_syntax` version and full
pinned transitive set (including exact `rowan`/`ra_ap_parser` versions); the
chosen representable weight family, `SCALE`, and the max-depth assumption; the
`PARSER_SCHEMA_VERSION` value; the red failure transcripts; the Kani harness tree
shape and unwind bounds; the **mutation matrix** (which mutation each harness
catches); and the Verus lemma's one-sentence trust-bridge statement (what it
assumes versus proves). Keep transcripts concise and focused on what proves
success.

- Stage 0 rustc: `rustc 1.98.0-nightly (57d06900f 2026-05-27)`.

## Revision note

Revision 3 (2026-06-09) — added a prerequisite **Stage 0 toolchain bump** to
`nightly-2026-05-28` at the user's direction (folded into this item as its own
atomic commit; motivated as both overdue maintenance and the cleanest unblock
for Pass B). This inverts the former “do not bump the toolchain” constraint.
Consequences threaded through the plan: the bump is suite-wide (lint crates,
`clippy_utils`, vendored shims, installer, ~34 `.stderr` fixtures, ~105
toolchain-string references) with `dylint` v5 compatibility and `rustc_private`
breakage as explicit go/no-go gates and escalation tolerances; Stage B changes
from a backwards MSRV bisect to selecting a *contemporaneous* `ra_ap_syntax`
snapshot (the MSRV risk is now largely resolved); the scope tolerance exempts
Stage 0's mechanical churn; and Stage G now also delivers a toolchain-bump
runbook in the developers' guide.

Revision 2 (2026-06-09) — folded in the community-of-experts (Logisphere)
design-review verdict (“Proceed with conditions”). Changes versus revision 1:
adopted Wafflecat's count-substrate hybrid (store exact `(KindId, depth)` counts;
weight in a pure seam) to dissolve the unsatisfiable “exact fixed-point for all
depths” trap and let the Verus lemma fold exact `u32` counts; made `AstHash`
opaque (`to_hex`) rather than `pub u64`; pinned the 7.3.2-facing read APIs
(`KindWeight`, concrete `Production` enum, `KindHistogram::get`/`iter`,
`ProductionMultiset::count`/`bigrams`/`trigrams`); made `canonical_hash` take
`&NormalisedTree` and seeded it with a neutral `PARSER_SCHEMA_VERSION` to fail
caches closed across parser bumps; enriched `AstError` (empty-span,
char-boundary, `ERROR`-node) to match the `run0/span.rs` template; promoted the
shared FNV-1a helper into `crate::hashing`; made the boundary guard a concrete
Stage A test; hardened the Verus/Kani stage against vacuity (cardinality
assumptions, separate root-fallback harness, mutation matrix, fold-algebra
honesty); marked `LeafClass` `#[non_exhaustive]`; trimmed the re-export set;
corrected the `insta`/`proptest` “new dependency” framing; and added the
`ra_ap_syntax` re-pinning runbook to the docs scope.

Revision 1 (2026-06-09) — initial draft. Produced with a planning agent team
(dependency/versioning, hexagonal boundary, feature-vector model, and
testing/verification strands) and informed by `firecrawl` research into the
current `ra_ap_syntax` API and MSRV constraints.

Pending: the Stage B empirical version pin (carried as `0.0.PINNED`). Status
remains DRAFT pending user approval per the ExecPlan approval gate.
