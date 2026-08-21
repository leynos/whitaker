# Collect brain trust diagnostics into an opt-in SARIF 2.1.0 emitter

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Whitaker's brain trust analysis measures how overgrown a Rust type or trait has
become. Today those measurements can only be rendered as English sentences.
Nothing can feed them to a continuous integration (CI) dashboard, a
pull-request annotation, or an editor's problem list.

After this change, brain trust findings can be turned into a Static Analysis
Results Interchange Format (SARIF) 2.1.0 run — the JSON interchange format that
GitHub code scanning, Azure DevOps, and most editor problem viewers understand
— using the **same infrastructure and the same shape as Whitaker's clone
detector**. A caller hands the emitter evaluated diagnostics and gets back a
`whitaker_sarif::Run` whose serialized form is deterministic, English-only, and
carries rule metadata, source locations, measured values, and stable
fingerprints.

You can see it working by serializing that run. The plan commits golden
snapshots of the JSON for six variants, and a behavioural suite that builds a
run from realistic inputs and asserts its contents. Recording the same findings
in a different order produces byte-identical JSON.

Getting there means finishing some shared plumbing the clone detector started
and left crate-local. Four small refactors move fingerprint hashing, file-URI
normalization, span-to-region conversion, and the Whitaker property bag out of
`whitaker_clones_core` and into `whitaker_sarif`, where both producers use one
implementation. Each of those refactors also fixes a real defect in the current
clone detector code.

### What this item deliberately does not do

It does not write SARIF to disk. That mirrors the clone detector exactly:
roadmap 7.2.3 shipped `emit_run0`, which returns an in-memory
`whitaker_sarif::Run` and stops there, because
`docs/whitaker-clone-detector-design.md` §CLI surface makes file emission the
CLI's job (roadmap 7.4.1, not yet done). Brain trust follows the same
boundary. Everything about writing files — output paths, atomic writes,
concurrent compilation units, incremental-build staleness, stale-file cleanup —
is out of scope and belongs with whichever CLI or driver item takes it on.

It does not define a user-facing configuration surface. Roadmap 6.6.1 owns
brain trust configuration and explicitly requires 3.6.3, which adopts
`ortho_config` with `whitaker.toml` as the canonical file. This item models
opt-in as a value the caller supplies, so 6.6.1 can wire whatever surface
3.6.3 lands on without rework.

It does not create the `brain_type` and `brain_trait` Dylint lint crates. They
do not exist, and no roadmap item creates them — see `Risks`.

## Constraints

Hard invariants. Violating one requires escalation, not a workaround.

- Reuse before invention. Any capability the clone detector already has —
  builders, rules, merge, dedup, property bags, region conversion, fingerprint
  hashing — must be reused, and refactored into `whitaker_sarif` for shared use
  where it is currently crate-local. Adding a parallel implementation of
  something `whitaker_sarif` or `whitaker_clones_core` already does is a design
  failure.
- No filesystem access, no environment reads, no process spawning, and no
  network access anywhere in this item. Every function added is pure.
- Emission must be **opt-in**: when the supplied mode is disabled, the
  collection entry point returns without building a run and without allocating
  per-finding storage.
- SARIF message text must be **English only**. The emitter must not consult
  `whitaker_common::i18n`.
- Serialized output must be **deterministic**: the same set of findings must
  produce byte-identical JSON regardless of recording order, hash-container
  iteration order, or platform.
- No `.unwrap()` or `.expect()` in production code or shared fixtures; the
  workspace denies `clippy::unwrap_used` and `clippy::expect_used`.
- No file may exceed 400 lines (`AGENTS.md`).
- Dependencies must use caret requirements and come from
  `[workspace.dependencies]`.
- The clone detector's observable behaviour must keep working. Its unit tests
  and the six scenarios in
  `crates/whitaker_clones_core/tests/run0_sarif_behaviour.rs` must still pass.
  Where a refactor changes the clone detector's emitted JSON, the change must be
  deliberate, recorded in `Decision log`, and reflected in that crate's tests.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` must succeed
  at every milestone boundary.

## Tolerances (exception triggers)

Thresholds are stated per milestone, because the shared refactors and the brain
trust emitter have different blast radii.

- Scope, shared refactors (`EP-M1`–`EP-M4`): if any one milestone requires
  touching more than 15 files or changing more than 600 net lines, stop and
  escalate.
- Scope, emitter (`EP-M5`): if it requires more than 15 files or more than 900
  net lines including snapshots, stop and escalate.
- Interface: `EP-M1`–`EP-M4` deliberately change public signatures inside
  `whitaker_sarif`. That is authorized. Changing a public signature in
  `whitaker-common` beyond the additive module and re-exports named in
  `Interfaces and dependencies` is not — stop and escalate.
- Dependencies: the only authorized additions are (a) `sha2` to
  `whitaker_sarif`, already workspace-pinned; (b) `whitaker_sarif`,
  `serde`, and `serde_json` to `whitaker-common`, all already workspace-pinned;
  and (c) `googletest` and `pretty_assertions` added to
  `[workspace.dependencies]` at `EP-M1`. Any further external dependency —
  including `cap-std`, `metrics`, or any SARIF crate from crates.io — means
  stop and escalate.
- Verification: if the Verus proof in `EP-M2` is not discharged after three
  modelling iterations, stop and escalate rather than weakening the lemma to a
  restatement. If the Kani harness in `EP-M3` hits CBMC state explosion after
  three bound reductions, record the blocker as roadmap 6.4.6 did and escalate.
- Iterations: if a gate still fails after four fix attempts, stop and escalate.
- Ambiguity: if a refactor to `whitaker_sarif` turns out to change the clone
  detector's emitted JSON in a way not anticipated in `Decision log`, stop and
  present the options.

## Risks

- Risk: `whitaker_sarif::SarifResult::partial_fingerprints` is a
  `HashMap<String, String>`, whose serialization order is randomized per
  process, so byte-identical output is currently impossible.
  Severity: high. Likelihood: certain (observed).
  Mitigation: `EP-M1` changes it to `BTreeMap`. Verified by review that every
  consumer uses only `get`, `insert`, `new`, `contains_key`, and moves — all
  available on `BTreeMap`.
- Risk: the `brain_type` and `brain_trait` Dylint crates do not exist, **and no
  roadmap item creates them**. Roadmap 6.2.2 and 6.3.2 are ticked but delivered
  only the evaluation layer in `whitaker-common`; 6.6.3 presupposes
  `crates/brain_type/ui/` that nothing produces. The emitter is therefore
  designed without its production caller in the room.
  Severity: medium. Likelihood: certain.
  Mitigation: keeping this item pure and I/O-free means the caller only has to
  supply data it already holds. The open question this leaves — how a
  `LateLintPass` turns a `rustc_span::Span` into a normalized file URI and a
  `SourceSpan` — is answered concretely in `Interfaces and dependencies` and
  recorded in the ADR so the lint crates inherit a decision rather than a gap.
  The missing roadmap item is raised in `Outcomes & retrospective` as follow-up
  work; fixing the roadmap is not in this item's scope.
- Risk: promoting fingerprint hashing changes the clone detector's emitted
  fingerprint values.
  Severity: low. Likelihood: certain.
  Mitigation: nothing consumes those values. There is no CLI (7.4.1) and no
  `clone_detected` lint (7.5.x), and no SARIF file is written anywhere in the
  tree. This is the cheapest moment this change will ever be available;
  recorded as a decision.
- Risk: Verus proof drifts from the shipped Rust encoder, because Verus
  compiles its own sidecar files and cannot `use` production modules.
  Severity: medium. Likelihood: medium.
  Mitigation: mirror the encoder as a `spec fn` whose doc comment names the
  production function and file, and add a Rust unit test asserting the two
  agree on a fixed vector of cases including the collision witness.
- Risk: `whitaker_common`'s `format_primary_message` and friends are English
  today, but roadmap 6.6.2 plans Fluent entries for both lints. If those
  functions become localized, the emitter silently starts emitting non-English
  text with no change to its own source.
  Severity: medium. Likelihood: medium.
  Mitigation: the emitter owns its message rendering by calling the `format_*`
  functions through a single, named seam, and `EP-M6` records in the design
  document that 6.6.2 must localize the *diagnostic* path without localizing
  these functions — or must fork them. A test asserts the SARIF text equals the
  English primary message verbatim, so a divergence fails loudly.
- Risk: `brain_methods` is an uncapped `Vec<MethodMetrics>`, and
  `format_primary_message` already joins every entry into one sentence. A god
  class with 30 brain methods produces a ~900-byte alert title and an unbounded
  property bag.
  Severity: medium. Likelihood: medium.
  Mitigation: `EP-M5` caps the property bag at three entries with an omitted
  count, matching the `MAX_SUGGESTIONS`/`MAX_METHODS_PER_SUGGESTION` precedent
  in `common/src/decomposition_advice/note.rs:14`, and truncates the SARIF
  message at the first sentence.
- Risk: `googletest` and `pretty_assertions` are absent from this repository,
  and execplan 7-3-1 explicitly declined to add them.
  Severity: low. Likelihood: certain.
  Mitigation: the task brief for this item authorizes both, so `EP-M1` adds
  them to `[workspace.dependencies]` properly. The precedent and the contrary
  argument are recorded in `Decision log` so the choice is visible.

## Progress

- [~] (2026-08-21) EP-M0 Understand and propose. Completed: reconnaissance of
  `whitaker_sarif`, `whitaker_clones_core::run0`, and the brain trust modules
  in `whitaker-common`; SARIF 2.1.0 and GitHub code-scanning research; a
  six-lens design review whose findings are folded into this revision.
  Remaining: plan approval.
- [ ] EP-M1 Deterministic SARIF results and workspace test dependencies.
- [ ] EP-M2 Shared fingerprint encoding, with the Verus injectivity proof.
- [ ] EP-M3 Shared file-URI and region conversion, with the Kani harness.
- [ ] EP-M4 Shared rule registry and an extensible Whitaker property bag.
- [ ] EP-M5 Brain trust SARIF emitter.
- [ ] EP-M6 Documentation, ADR, and roadmap.

Add a timestamp to each entry as it completes.

## Surprises & discoveries

- Observation: the `brain_type` and `brain_trait` Dylint lint crates do not
  exist, and no roadmap item creates them.
  Evidence: `crates/` contains no such directory; no `span_lint` call site
  references either lint; `dylint.toml` has no section for either; roadmap
  6.6.3 presupposes `crates/brain_type/ui/`.
  Impact: 6.5.1 is scoped to the layer that exists. Recorded as follow-up.
- Observation: nothing in the repository writes a SARIF file. `emit_run0`
  returns an in-memory `Run` and stops.
  Evidence: `crates/whitaker_clones_core/src/run0/emit.rs:97`; no call site
  pairs `whitaker_sarif::paths::*` with a file write.
  Impact: file emission is out of scope here, exactly as it was for 7.2.3.
- Observation: `SarifResult::partial_fingerprints` is a `HashMap`, so the crate
  cannot currently produce stable bytes.
  Evidence: `crates/whitaker_sarif/src/model/result.rs:107`.
  Impact: `EP-M1`.
- Observation: `pair_fingerprint` and `token_hash` concatenate components with a
  single zero byte.
  Evidence: `crates/whitaker_clones_core/src/run0/emit.rs:285`.
  Impact: that encoding is not injective in general, and it is crate-local
  rather than shared. `EP-M2` promotes and fixes it.
- Observation: `TokenFragment::file_uri` is an unvalidated caller-supplied
  `String`.
  Evidence: `crates/whitaker_clones_core/src/run0/types.rs:31`.
  Impact: an absolute or backslash-separated path would make
  `artifactLocation.uri` unresolvable against a repository root and would make
  fingerprints vary by working directory and platform. `EP-M3` introduces a
  shared validated newtype for both producers.
- Observation: `region_for_range` already counts UTF-16 code units.
  Evidence: `crates/whitaker_clones_core/src/run0/span.rs:78`.
  Impact: the clone detector already matches SARIF's default `columnKind`. The
  brain trust emitter must use the same convention, and `EP-M3` makes that
  structural by sharing the code.
- Observation: `all_rules()` is clone-specific and its test asserts it returns
  exactly three.
  Evidence: `crates/whitaker_sarif/src/rules.rs:120` and `:128`.
  Impact: the name becomes a lie once a second rule family exists. `EP-M4`
  renames it.
- Observation: `WhitakerProperties` already owns the `properties.whitaker`
  extension point, with `try_to_value` and a `TryFrom<&Value>` inverse, but its
  fields are clone-specific.
  Evidence: `crates/whitaker_sarif/src/whitaker_properties.rs:37`.
  Impact: `EP-M4` generalizes it into a discriminated family rather than
  inventing a second namespace.
- Observation: `whitaker-common` has no `serde` dependency, and
  `BrainTypeThresholds` deliberately does not derive `Deserialize` for that
  reason.
  Evidence: `common/Cargo.toml`; the decision recorded in execplan 6-2-2.
  Impact: `EP-M5` adds `serde` and `serde_json` to `whitaker-common`. Both are
  workspace-pinned and neither pulls compiler or filesystem capability.

## Decision log

- Decision: follow the clone detector's architecture exactly — a pure emitter
  returning an in-memory `whitaker_sarif::Run`, with file writing deferred to a
  CLI or driver item.
  Rationale: it is the house pattern (`emit_run0`), it keeps the item free of
  every operational hazard that running inside rustc introduces (incremental
  compilation replaying cached lint results, stale files, concurrent
  compilation units colliding on a filename, ambient filesystem authority in a
  crate the capability policy governs), and it matches
  `docs/whitaker-clone-detector-design.md` §CLI surface. Note this is a
  deliberate narrowing of `docs/brain-trust-lints-design.md`'s §SARIF output
  wording, which says only "collect diagnostics in a shared module when SARIF
  output is enabled" and does not name a writer. `EP-M6` updates that section.
  Date/Author: 2026-08-21, planning agent, on the maintainer's direction.
- Decision: promote fingerprint hashing, file-URI normalization, region
  conversion, and the property bag from `whitaker_clones_core` into
  `whitaker_sarif`, and migrate the clone detector onto the shared code in the
  same milestones.
  Rationale: the maintainer's direction is to reuse SARIF infrastructure and
  refactor to enable reuse rather than reinvent. Each promotion also fixes a
  real defect: a non-injective encoding, an unvalidated URI, a duplicated
  column convention, and a single-purpose property namespace.
  Date/Author: 2026-08-21, planning agent.
- Decision: put the brain trust emitter in `common/src/brain_trust_sarif/`
  rather than in a new crate.
  Rationale: the clone detector puts its emitter in the analysis crate
  (`whitaker_clones_core::run0`), and `whitaker-common` is the analysis crate
  for brain trust. With file I/O out of scope the emitter needs no `cap-std`
  and no ambient authority, so the objection that drove the earlier
  separate-crate proposal disappears. `whitaker_sarif` has no compiler
  dependency, so nothing about the dependency direction is disturbed, and
  `whitaker-common` is already covered by `make lint`'s package list.
  Date/Author: 2026-08-21, planning agent, revising an earlier draft after
  review.
- Decision: change `SarifResult::partial_fingerprints` from `HashMap` to
  `BTreeMap`, with no compatibility shim.
  Rationale: the crate is `publish = false`, pre-1.0, with one in-repo
  consumer. "Compatible with whom?" has no answer. Determinism is a stated
  constraint that cannot otherwise be met. Reviewed: `merge.rs:62` uses `get`,
  `result_builder.rs` uses `new`/`insert`, `emit.rs:351` uses `get`,
  `tests_emit.rs` uses `get`/`contains_key` — all available on `BTreeMap`.
  `clippy::implicit_hasher` does not interact, because it fires on public
  parameters and impls, not public fields; `BTreeMap` makes it permanently
  inapplicable.
  Date/Author: 2026-08-21, planning agent.
- Decision: leave the two `HashSet` uses in `merge.rs:105` and `merge.rs:178`
  alone.
  Rationale: both are membership sets whose output order follows a `Vec`, so
  neither is a determinism hazard. Recorded so a later reader does not "fix"
  them.
  Date/Author: 2026-08-21, Telefono (review panel).
- Decision: use length-prefixed component encoding for fingerprint pre-images,
  not delimiter separation, and migrate the clone detector's `pair_fingerprint`
  and `token_hash` onto it.
  Rationale: delimiter separation is injective only while no component can
  contain the delimiter, which is an assumption about every future component
  type rather than a property of the encoding. Length prefixing is
  unconditionally injective and the property is provable. Changing the clone
  detector's fingerprint values is free right now because nothing reads them.
  Date/Author: 2026-08-21, planning agent.
- Decision: rename `whitaker_sarif::all_rules()` to `clone_detection_rules()`
  and add `brain_trust_rules()`.
  Rationale: `all_rules()` returns only clone rules and its test asserts an
  arity of three. Keeping the name once a second family exists would be
  actively misleading. One consumer (`emit.rs:112`) needs updating.
  Date/Author: 2026-08-21, Pandalump (review panel).
- Decision: generalize `WhitakerProperties` into an internally tagged family
  under the existing `properties.whitaker` key rather than adding a
  `whitakerBrainTrust` key.
  Rationale: `whitaker_properties.rs` is the established extension point, with
  a serializer and an inverse and round-trip tests. A second namespace would
  duplicate it, and `AGENTS.md` requires sweeping for an existing equivalent
  before adding an abstraction. An internally tagged enum also makes "add a new
  metric family" a compile-time event and preserves the `TryFrom<&Value>`
  inverse that an untyped map would have thrown away. The added `kind`
  discriminant changes clone-detector JSON; nothing consumes it.
  Date/Author: 2026-08-21, Telefono and Pandalump (review panel).
- Decision: reuse `whitaker_common::decomposition_advice::SubjectKind` rather
  than defining a second `SubjectKind { Type, Trait }`.
  Rationale: it already exists, is already re-exported from the crate root, is
  already used by `brain_type_metrics::diagnostic` at line 260, and already has
  `FromStr` and `Ord`. Two identically named, identically shaped types in one
  dependency graph is a naming accident, not a boundary.
  Date/Author: 2026-08-21, Pandalump (review panel).
- Decision: map disposition to level as `Warn` → `warning`, `Deny` → `error`,
  `Pass` → no result, and do **not** add `defaultConfiguration.level` to
  `ReportingDescriptor`.
  Rationale: every emitted result carries an explicit `level`, and consumers
  fall back to `defaultConfiguration.level` only when `result.level` is absent.
  An always-`None` field for symmetry is gold-plating. Recorded explicitly
  because silence would read as oversight. `Pass` genuinely has no result to
  report; SARIF's `kind: "pass"` is for tools reporting proof obligations.
  Date/Author: 2026-08-21, Telefono (review panel).
- Decision: omit `ruleIndex`.
  Rationale: it disambiguates result-to-descriptor binding when rules come from
  multiple tool components. There is one driver and rule identifiers are
  unique — an invariant the plan already asserts.
  Date/Author: 2026-08-21, Telefono (review panel).
- Decision: add `help` to `ReportingDescriptor` but not `automationDetails` to
  `Run`.
  Rationale: `help` is the field consumers render beside an alert, and the
  brain trust rules have genuine standing guidance to put there. By contrast
  `automationDetails.id` is a category key whose semantics only matter to an
  upload step that does not exist, and whose format is easy to get wrong — a
  trailing-slash mistake makes every unit share a category and clobber the
  others. Deciding it without a real upload path is guessing. Deferred.
  Date/Author: 2026-08-21, Telefono and Wafflecat (review panel).
- Decision: model opt-in as a value the caller supplies
  (`BrainTrustSarifMode`), and defer the configuration surface to roadmap
  6.6.1.
  Rationale: `docs/whitaker-cli-design.md` §Configuration model makes
  `whitaker.toml` via `ortho_config` the canonical surface with
  `WHITAKER_<SECTION>_<KEY>` variables parsed case-sensitively, and roadmap
  6.6.1 (brain trust configuration) explicitly requires 3.6.3 (which adopts
  `ortho_config`). Inventing a `dylint.toml` table and a case-insensitive
  boolean grammar here would build on the surface the CLI design deprecates and
  guarantee a migration.
  Date/Author: 2026-08-21, Wafflecat, Doggylump, and Dinolump (review panel).
- Decision: cut the planned Verus proof that the result ordering is a total
  order.
  Rationale: the ordering key is a tuple of two strings, two integers, and a
  string. `#[derive(Ord)]` gives lexicographic total order by construction, so
  a proof would be proving the compiler, and it could not fail when the
  implementation is wrong. A parameterized test exercising each tie-break level
  carries the real risk, which is choosing the wrong key.
  Date/Author: 2026-08-21, Wafflecat, Buzzy Bee, and Dinolump (review panel).
- Decision: cut the planned Kani harness over the collector, and spend the
  bounded-model-checking budget on span-to-region conversion instead.
  Rationale: a collector backed by a `BTreeMap` keyed on the ordering key makes
  dedup and sortedness type-level facts, so the harness would verify a standard
  library guarantee — and would verify it against a handwritten array model
  rather than the shipped code. Span-to-region conversion is a genuine
  arithmetic invariant over integers, is Kani-friendly, and has a real failure
  mode: `SourceLocation::new(0, 0)` is constructible but SARIF requires 1-based
  positions.
  Date/Author: 2026-08-21, Buzzy Bee and Wafflecat (review panel), accepted by
  the planning agent.
- Decision: add `googletest` and `pretty_assertions` to
  `[workspace.dependencies]`.
  Rationale: the task brief for this item authorizes both and asks for them.
  Recorded dissent: execplan 7-3-1 declined to add them, and the review panel
  argued that 40-odd crates already use `rstest` plus plain assertions and
  `insta`, so two new assertion frameworks add cognitive load and supply-chain
  surface for little gain. They are introduced workspace-wide rather than with
  literal versions in one manifest, so the repository gets one pinned decision
  rather than a local exception.
  Date/Author: 2026-08-21, planning agent, over Dinolump's objection.
- Follow-up recorded, not actioned here: no roadmap item creates
  `crates/brain_type/` or `crates/brain_trait/`, yet 6.6.3 presupposes their
  `ui/` directories. Raise a roadmap addendum.

## Outcomes & retrospective

To be completed at `EP-M6`. Before setting the plan to `COMPLETE`, reconcile
every discovery against `docs/brain-trust-lints-design.md` §SARIF output,
`docs/whitaker-clone-detector-design.md` §SARIF schema and mapping, the new
ADR, `docs/users-guide.md`, `docs/developers-guide.md`, and `docs/roadmap.md`
item 6.5.1.

## Context and orientation

Assume you have only this repository and this file.

### What Whitaker is

Whitaker is a suite of Rust lints built on **Dylint**, a tool that loads
out-of-tree lints into the Rust compiler. Lints live in `crates/<lint_name>/`.
Shared, compiler-independent helper logic lives in the `whitaker-common`
package at `common/`. Workspace members are declared in the root `Cargo.toml`
as `["common", "crates/*", "installer", "suite"]`.

### What "brain trust" means here

Two planned lints share an analysis:

- `brain_type` flags a type that has grown too complex, using Weighted Method
  Count (WMC — the sum of per-method cognitive complexity), LCOM4 (Lack of
  Cohesion of Methods version 4 — the number of connected components in the
  method-and-field graph, where a higher number means the type is really
  several types), foreign reach (how many other types' data the methods touch),
  and "brain methods" (methods that are both very complex and very long).
- `brain_trait` flags a trait that imposes too much on implementors, using the
  required-method count, the default-method count, the summed cognitive
  complexity of default methods, and the total item count.

Both are described in `docs/brain-trust-lints-design.md`.

The analysis is implemented and tested. The relevant modules are:

- `common/src/brain_type_metrics/` — `TypeMetrics`, `MethodMetrics`,
  `evaluate_brain_type`, `BrainTypeDisposition` (`Pass` | `Warn` | `Deny`),
  `BrainTypeDiagnostic`, and the English formatters `format_primary_message`,
  `format_note`, `format_help`, `format_decomposition_note`.
- `common/src/brain_trait_metrics/` — the trait analogue, with
  `BrainTraitDiagnostic` and `BrainTraitDisposition`.
- `common/src/decomposition_advice/` — clusters methods into suggested
  extractions (`DecompositionSuggestion`), renders them as a note
  (`format_diagnostic_note`), and defines `SubjectKind { Type, Trait }`.
- `common/src/span.rs` — `SourceLocation` (line and column) and `SourceSpan`
  (a start and an end location, validated so the start never follows the end).

**Important:** the Dylint lint crates `crates/brain_type/` and
`crates/brain_trait/` do **not** exist. Nothing currently calls
`evaluate_brain_type` from a compiler pass. This item therefore treats
`BrainTypeDiagnostic` and `BrainTraitDiagnostic` as the emitter's inputs and
the future lint crates as its callers.

### What SARIF is

SARIF 2.1.0 is an OASIS standard JSON format for static analysis output. A
SARIF *log* has a `$schema`, a `version`, and an array of *runs*. Each run
names the *tool* that produced it and carries an array of *results*. A result
has a `ruleId`, a `level` (`none`, `note`, `warning`, or `error`), a
`message.text`, one or more `locations`, and optionally `partialFingerprints`
— a small map of strings consumers use to recognize "the same finding" as code
moves between commits.

### What `whitaker_sarif` already provides

`crates/whitaker_sarif/` models the subset Whitaker needs, with no compiler
dependency and no input or output of its own:

- `model/log.rs` — `SarifLog { schema, version, runs }`, plus `SARIF_SCHEMA`
  (the OASIS schema URL) and `SARIF_VERSION`.
- `model/run.rs` — `Run { tool, invocations, results, artefacts }`, `Tool`,
  `ToolComponent`, `Invocation`, `Artefact`.
- `model/result.rs` — `SarifResult`, `Level`, `Message`.
- `model/location.rs` — `Location`, `PhysicalLocation`, `ArtefactLocation`,
  `Region`, `RelatedLocation`.
- `model/descriptor.rs` — `ReportingDescriptor` (SARIF's word for a rule) and
  `MultiformatMessageString`.
- `builders/` — `SarifLogBuilder`, `RunBuilder`, `ResultBuilder`,
  `LocationBuilder`, `RegionBuilder`.
- `merge.rs` — `merge_runs`, `deduplicate_results`, and
  `WHITAKER_FRAGMENT_KEY`.
- `rules.rs` — `WHK001`–`WHK003` descriptors and `all_rules()`.
- `whitaker_properties.rs` — the `properties.whitaker` extension point.
- `paths.rs` — the `target/whitaker/` layout constants.

### The existing producer, and the pattern to copy

`crates/whitaker_clones_core/src/run0/emit.rs` builds a `Run` from accepted
clone pairs. **Read `emit_run0` (line 97) and `build_result` (line 121) before
starting.** They are the template for this item: resolve inputs, build one
`SarifResult` per finding through `ResultBuilder`, attach a location and a
region, attach fingerprints, attach a Whitaker property bag, sort the results,
deduplicate them, and assemble a `Run` through `RunBuilder` with the rule
descriptors attached. The function returns the `Run`; nothing writes it.

Supporting it are `run0/span.rs` (`region_for_range`, byte range to SARIF
`Region`, counting UTF-16 code units) and the private hashing helpers at the
bottom of `emit.rs` (`pair_fingerprint`, `token_hash`, `digest_hex`,
`hex_digit`, `u64_big_endian_bytes`). Those are generic SARIF plumbing living
in the clone detector; this item promotes them.

## Conformance basis

There is no Terms of Reference document. The upstream artefacts are:

- `docs/roadmap.md` §6.5, item 6.5.1, at the tree of branch
  `harden-lint-config` (`origin/main` at `02e6c1c`).
- `docs/brain-trust-lints-design.md` §SARIF output and §Configuration,
  localization, and testing.
- `docs/whitaker-clone-detector-design.md` §SARIF schema and mapping, §Runs,
  and §CLI surface — the architecture this item mirrors.
- `docs/whitaker-cli-design.md` §Rule identifiers and selection model and
  §Configuration model — the reason configuration is deferred.
- `docs/whitaker-dylint-suite-design.md`, `AGENTS.md`, and
  `docs/documentation-style-guide.md`.
- OASIS SARIF 2.1.0 Errata 01, and GitHub's "SARIF support for code scanning".

Stable identifiers introduced here:

- `BTS-REQ-01` — brain trust diagnostics are collected into a SARIF 2.1.0 run.
- `BTS-REQ-02` — collection is opt-in and costs nothing when disabled.
- `BTS-REQ-03` — messages are English only.
- `BTS-REQ-04` — results carry rule metadata, locations, and messages,
  serialized with `serde`.
- `BTS-REQ-05` — output is deterministic and stable enough for tool
  deduplication.
- `BTS-REQ-06` — SARIF construction reuses shared `whitaker_sarif`
  infrastructure; no capability is implemented twice.

Trace links:

```plaintext
roadmap-6.5.1 -> BTS-REQ-01 -> EP-M5 -> brain_trust_sarif::tests::emits_one_result_per_finding
roadmap-6.5.1 -> BTS-REQ-02 -> EP-M5 -> brain_trust_sarif::tests::disabled_mode_builds_no_run
roadmap-6.5.1 -> BTS-REQ-03 -> EP-M5 -> features/brain_trust_sarif.feature: English primary message
roadmap-6.5.1 -> BTS-REQ-04 -> EP-M4, EP-M5 -> brain_trust_sarif::tests::snapshot_brain_type_deny
roadmap-6.5.1 -> BTS-REQ-05 -> EP-M1, EP-M2, EP-M3 -> verus::brain_trust_fingerprint,
                                                       kani::verify_span_to_region_bounds,
                                                       tests::emission_is_permutation_invariant
roadmap-6.5.1 -> BTS-REQ-06 -> EP-M2, EP-M3, EP-M4 -> run0 migrated onto shared helpers
```

## Verification plan

### VP-1 — the fingerprint pre-image encoding is injective

Both producers derive a result's stable identity by hashing an encoding of a
component tuple. If two different tuples can encode to the same bytes, two
unrelated findings collapse into one alert, and `deduplicate_results` — which
keys on a fingerprint — silently drops one of them.

- Obligation: for component sequences `a` and `b`, `encode(a) == encode(b)`
  implies `a == b`, where `encode` writes each component as an eight-byte
  big-endian length prefix followed by its bytes.
- Method: formal proof in Verus.
- Rationale: the property quantifies over all byte sequences of all lengths, so
  bounded model checking cannot cover it and property tests can only sample it.
  It is a small, self-contained lemma about a pure function, and after `EP-M2`
  it protects *two* producers and replaces a shipped encoding that does not
  have the property.
- Domain: `Seq<Seq<u8>>` of arbitrary length, components of arbitrary length.
- Artefact: `verus/brain_trust_fingerprint.rs`, registered in
  `scripts/run-verus.sh` under a new `brain-trust` group, in that script's
  argument allow-list, in its `all` arm, and behind a new
  `make verus-brain-trust` target listed in `.PHONY`.
- Evidence: `make verus-brain-trust`. Before the proof body exists, Verus
  reports the postcondition unproven; after, it reports `0 errors`.
- Proof shape: define `spec fn encode(components: Seq<Seq<u8>>) -> Seq<u8>`
  recursively with `decreases components.len()`, and a matching
  `spec fn decode(bytes: Seq<u8>) -> Option<Seq<Seq<u8>>>`. Prove
  `lemma_decode_encode_round_trip` by induction, then derive injectivity: from
  `encode(a) == encode(b)`, `decode` of both sides gives `Some(a) == Some(b)`.
  The work is in the round-trip lemma; concatenation reasoning needs
  `broadcast use vstd::seq::group_seq_axioms;`.
- Non-vacuity: the antecedent is inhabited — `encode(seq![seq![]])` is a
  witness with an empty component, and the lemma is exercised on non-empty
  components too. The negative control is a Rust test,
  `encoding_separates_components_a_delimiter_would_merge`, asserting that
  `("ab", "c")` and `("a", "bc")` — identical under single-byte-delimiter
  concatenation, which is what `emit.rs:285` ships today — produce different
  bytes and different fingerprints under the length-prefixed encoder. A second
  control mutates the Verus `encode` to drop the length prefix and confirms the
  round-trip lemma then fails; performed once, observed, reverted, transcript
  recorded in `Artefacts and notes`.
- Residual gap: SHA-256 collision resistance is assumed (see `Axioms`).
  Injectivity of the pre-image is the part this repository owns.

### VP-2 — span-to-region conversion always yields a valid SARIF region

- Obligation: for every `SourceSpan`, `span_to_region` returns a `Region` with
  `start_line >= 1`, `start_column >= 1`, `end_line >= start_line`, and, when
  the lines are equal, `end_column >= start_column` — that is, it never
  produces a value `RegionBuilder::build` would reject.
- Method: bounded model checking with Kani.
- Rationale: this is arithmetic over small integers with a real failure mode.
  `SourceSpan::new` only enforces that the start does not follow the end; it
  permits `SourceLocation::new(0, 0)`, whereas SARIF positions are one-based.
  Exhaustive exploration within a small bound settles whether the conversion's
  normalization policy is correct for every reachable input, which examples
  cannot. Integers only — no `String`, no `Vec` — so it avoids the CBMC
  heap-collection cliff recorded in roadmap 6.4.6.
- Domain: symbolic `line` and `column` in `0..=3` for both endpoints,
  constrained by `kani::assume` to the invariant `SourceSpan::new` enforces.
- Artefact: `crates/whitaker_sarif/src/location_kani.rs`, gated behind
  `#[cfg(kani)]`; a `brain-trust` group in `scripts/run-kani.sh` that is **not**
  added to the no-argument path (which currently runs the decomposition group,
  still blocked by the CBMC issue recorded in 6.4.6); and a
  `make kani-brain-trust` target listed in `.PHONY`.
- Evidence: `make kani-brain-trust` reports `VERIFICATION:- SUCCESSFUL`.
- Non-vacuity: a `kani::cover` assertion proves the zero-valued-position case
  is reachable within the bound, and a second proves the multi-line case is.
  The negative control removes the one-based normalization and confirms the
  harness reports a counter-example with a concrete trace; performed once,
  observed, reverted.
- Residual gap: bounded at coordinates `0..=3`. Larger coordinates are covered
  by VP-3's generated cases.

### VP-3 — emission is permutation invariant and survives a JSON round trip

- Obligation: for any multiset of findings and any two recording orders, the
  serialized JSON is equal; and re-serializing a deserialized log reproduces
  the original bytes.
- Method: property test with `proptest`.
- Rationale: the invariant ranges over generated inputs and orderings beyond
  Kani's practical bound, and it protects the user-visible guarantee of stable
  diffs. It is also the test that would catch a `HashMap` sneaking back in.
- Domain: 0–12 findings; file URIs from a four-element alphabet so collisions
  and ties actually occur; subject names from a five-element alphabet; lines in
  `1..=6`; both subject kinds; `Warn` and `Deny`.
- Artefact: `common/tests/brain_trust_sarif_properties.rs`, with regression
  seeds committed under `common/proptest-regressions/`.
- Non-vacuity: counters accumulated across the run assert that at least one
  generated case contained two findings sharing a file URI and at least one
  contained two sharing a line; a generator producing only distinct findings
  fails this check rather than passing vacuously. The negative control removes
  the sort from the emitter and confirms the permutation property fails.

### VP-4 — the disposition-to-level mapping is total and correct

- Obligation: every `BrainTypeDisposition` and `BrainTraitDisposition` variant
  maps to exactly one outcome; `Pass` yields no result.
- Method: exhaustive parameterized tests with `rstest`, using `googletest`
  matchers and `pretty_assertions`.
- Rationale: the input space is a finite enumeration; enumeration is clearer
  than generation.
- Artefact: `common/src/brain_trust_sarif/mapping_tests.rs`.
- Non-vacuity: each variant asserts a distinct outcome, so collapsing two arms
  fails at least one case.

### VP-5 — the ordering key breaks ties at every level, and caps are honoured

- Obligation: results are ordered by `(rule_id, file_uri, start_line,
  start_column, subject_name)`, with each level actually consulted; and the
  property bag carries at most three brain methods plus an omitted count.
- Method: parameterized tests with `rstest`.
- Rationale: the ordering itself is a derived `Ord` on a tuple, so the risk is
  not that comparison is wrong but that the *key* is wrong — which only a test
  that differs at exactly one level can detect. The cap follows the
  `MAX_SUGGESTIONS` precedent in `decomposition_advice/note.rs:14`.
- Artefact: `common/src/brain_trust_sarif/ordering_tests.rs`.
- Non-vacuity: one case per tie-break level, each differing only at that level;
  dropping any level from the key fails exactly one case. The cap test supplies
  five brain methods and asserts three plus `brainMethodsOmitted: 2`.

### VP-6 — the emitted document has the shape consumers expect

- Obligation: `$schema` and `version` are correct; every result's `ruleId`
  appears in `runs[0].tool.driver.rules`; every result has a location with a
  one-based `startLine`; the property bag round-trips through
  `TryFrom<&Value>`.
- Method: `insta` snapshots across the variant matrix, plus explicit structural
  assertions.
- Domain: brain type warn; brain type deny with brain methods and a
  decomposition note; brain type deny with more brain methods than the cap;
  brain trait warn; brain trait deny; a mixed file with both kinds; and an
  empty run.
- Artefact: `common/src/brain_trust_sarif/mapping_tests.rs` with snapshots
  under `common/src/brain_trust_sarif/snapshots/`.
- Non-vacuity: the empty-run snapshot proves the mapper does not fabricate
  results; the over-cap snapshot proves elision is exercised rather than
  every variant coincidentally sitting under the cap.

### VP-7 — behaviour: opt-in, English-only, and clone-detector parity

- Obligation: a disabled mode builds no run; an enabled mode builds one result
  per warned or denied subject; the message equals the English primary message
  verbatim; and the clone detector still emits equivalent runs after the shared
  refactors.
- Method: `rstest-bdd` scenarios.
- Artefact: `common/tests/brain_trust_sarif_behaviour.rs` and
  `common/tests/features/brain_trust_sarif.feature`; the existing
  `crates/whitaker_clones_core/tests/run0_sarif_behaviour.rs` serves as the
  parity guard for the refactors.
- Non-vacuity: the message-equality scenario compares against
  `format_primary_message` computed independently in the test, so a change to
  either side fails. Note that the locale-invariance question is *not* tested
  here: `format_*` reads no locale today, so such a test could not fail. The
  obligation is instead stated as message equality with the English formatter,
  which does fail if 6.6.2 later localizes it — see `Risks`.

### What is deliberately not verified, and why

- **No proof that the result ordering is a total order.** The key is a tuple
  with `#[derive(Ord)]`; lexicographic total order holds by construction, and a
  proof could not fail when the implementation is wrong. VP-5 covers the real
  risk, which is key selection.
- **No bounded model check of the collector.** It is a `BTreeMap` keyed on the
  ordering key, so dedup and sortedness are type-level facts; a harness would
  verify a standard library guarantee against a handwritten model rather than
  the shipped code. VP-3 covers the observable consequence.
- **No verification of the English wording** of the diagnostic messages.
  Roadmap 6.2.2 and 6.3.2 already covered `format_*` with unit and behavioural
  tests. This item asserts that the emitter uses them unmodified.

### Axioms

- SHA-256 as implemented by `sha2` is collision resistant and stable across
  platforms within the pinned caret range.
- `serde_json` serializes `BTreeMap` and `Value::Object` (backed by `BTreeMap`
  when `preserve_order` is off) in ascending key order. The workspace must
  therefore never enable `serde_json/preserve_order`; a comment records this.
- `#[derive(Ord)]` on a tuple of `Ord` fields yields a lexicographic total
  order.
- Verus and Kani, as pinned by `scripts/install-verus.sh` and
  `scripts/install-kani.sh`, are sound.

## Plan of work

### Stage A — understand and propose (no code changes)

Read, in order: `docs/brain-trust-lints-design.md` §SARIF output;
`docs/whitaker-clone-detector-design.md` §SARIF schema and mapping and §CLI
surface; `crates/whitaker_sarif/src/` in full;
`crates/whitaker_clones_core/src/run0/emit.rs` and `span.rs`;
`common/src/brain_type_metrics/diagnostic.rs` and `evaluation.rs`;
`common/src/brain_trait_metrics/diagnostic.rs`;
`common/src/decomposition_advice/note.rs` and `profile.rs`;
`common/src/span.rs`. Then obtain approval.

### Stage B — red tests and open proof obligations

For each milestone, write the failing test or the open proof goal first and
observe it fail for the stated reason.

### Stage C — implementation and verification together

`EP-M1` through `EP-M5`, in order. `EP-M1` through `EP-M4` are shared-
infrastructure refactors, each independently valuable and each ending at a
coherent state where the clone detector still works.

### Stage D — documentation and wider validation

`EP-M6`.

## Milestones and plateaus

### EP-M1 — deterministic SARIF results and workspace test dependencies

- Outcome: `whitaker_sarif` produces byte-stable JSON, and the two authorized
  test crates are pinned workspace-wide.
- Requirements: `BTS-REQ-05`.
- Changes: in `crates/whitaker_sarif/src/model/result.rs`, change
  `partial_fingerprints` to `BTreeMap<String, String>` and update the
  `skip_serializing_if` to `BTreeMap::is_empty`; make the same container change
  in `builders/result_builder.rs`; update the doc examples in both. Add
  `googletest` and `pretty_assertions` to `[workspace.dependencies]` in the
  root `Cargo.toml` with caret requirements.
- Red artefact: `partial_fingerprints_serialize_in_key_order` in
  `crates/whitaker_sarif/tests/sarif_behaviour.rs`, inserting keys as `zeta`,
  `alpha`, `mu` and asserting `alpha` serializes first. Use enough keys that
  the `HashMap` failure is reliable rather than intermittent, and record the
  observed failure.
- Acceptance evidence: the new test passes; the eight existing scenarios in
  `crates/whitaker_sarif/tests/` pass; `whitaker_clones_core` tests pass.
- Conformance check: no wire-format change beyond key ordering; no new runtime
  dependency; `merge.rs` needs no edit (verified: it uses only `get`).
- Recovery: mechanical; revert with `git revert`.
- Remaining gaps: nothing brain-trust-specific.
- Compatibility decision: none required — `publish = false`, pre-1.0, one
  in-repo consumer.

### EP-M2 — shared fingerprint encoding, with the Verus proof

- Outcome: one injective fingerprint implementation, used by both producers.
- Requirements: `BTS-REQ-05`, `BTS-REQ-06`; discharges `VP-1`.
- Changes: add `crates/whitaker_sarif/src/fingerprint.rs` with
  `encode_components(&[&str]) -> Vec<u8>` (eight-byte big-endian length prefix
  per component, written with explicit shifts because the workspace denies
  `clippy::host_endian_bytes`, `little_endian_bytes`, **and**
  `big_endian_bytes`), `fingerprint_hex(&[&str]) -> String` returning lowercase
  hexadecimal SHA-256, and `digest_hex`. Add `sha2` to `whitaker_sarif`'s
  dependencies. Delete `pair_fingerprint`, `token_hash`, `digest_hex`,
  `hex_digit`, and `u64_big_endian_bytes` from
  `crates/whitaker_clones_core/src/run0/emit.rs` and call the shared functions
  instead. Add `verus/brain_trust_fingerprint.rs`, extend
  `scripts/run-verus.sh` in all three places (group function, argument
  allow-list at line 53, and the `all` arm), and add `verus-brain-trust` to the
  `Makefile` including `.PHONY`.
- Red artefact: the Verus round-trip lemma with no body, reporting an unproven
  postcondition; and
  `encoding_separates_components_a_delimiter_would_merge`.
- Acceptance evidence: `make verus-brain-trust` reports `0 errors`;
  `cargo nextest run -p whitaker_sarif -p whitaker_clones_core` passes with
  clone-detector fingerprint expectations updated to the new values.
- Conformance check: the clone detector's fingerprint values change
  deliberately; nothing consumes them; recorded in `Decision log`.
- Recovery: the shared module is additive; the `run0` migration is a
  call-site swap.
- Remaining gaps: URIs and regions still crate-local.
- Compatibility decision: none required; no consumer of the old values exists.

### EP-M3 — shared file-URI and region conversion, with the Kani harness

- Outcome: one normalized URI type and one region-conversion implementation,
  used by both producers.
- Requirements: `BTS-REQ-05`, `BTS-REQ-06`; discharges `VP-2`.
- Changes: add `FileUri` to `crates/whitaker_sarif/src/model/location.rs` (or a
  sibling module) — a newtype validated as non-empty, repository-root-relative,
  forward-slashed, with no drive letter and no `.` or `..` component, with
  `TryFrom<&str>`, `AsRef<str>`, and a typed error. Move `region_for_range`
  from `crates/whitaker_clones_core/src/run0/span.rs` into `whitaker_sarif`,
  preserving its UTF-16 code-unit column convention, and add a sibling
  `span_to_region(SourceSpan) -> Region` for line-and-column inputs with a
  documented one-based normalization policy. Migrate `run0` onto both. Add
  `crates/whitaker_sarif/src/location_kani.rs`, a `brain-trust` group in
  `scripts/run-kani.sh` (not on the no-argument path), and
  `kani-brain-trust` to the `Makefile` including `.PHONY`.
- Red artefact: `file_uri_rejects_absolute_and_backslash_paths`, and the Kani
  harness before the normalization policy is implemented, which should report a
  counter-example at line zero.
- Acceptance evidence: `make kani-brain-trust` reports
  `VERIFICATION:- SUCCESSFUL` for the harnesses and the two cover checks; both
  crates' tests pass.
- Conformance check: `columnKind` semantics are now shared rather than
  duplicated; `crates/whitaker_sarif/src/paths.rs:24` already documents the
  forward-slash requirement, and `FileUri` now enforces it.
- Recovery: additive plus a call-site migration.
- Remaining gaps: rules and property bag.
- Compatibility decision: none required.

### EP-M4 — shared rule registry and an extensible property bag

- Outcome: `whitaker_sarif` hosts both rule families and one discriminated
  property bag.
- Requirements: `BTS-REQ-04`, `BTS-REQ-06`.
- Changes: in `crates/whitaker_sarif/src/rules.rs`, rename `all_rules()` to
  `clone_detection_rules()`, update its arity test and the one consumer at
  `emit.rs:112`, and add `WHK101_ID`/`WHK102_ID`, `whk101_rule()`,
  `whk102_rule()`, and `brain_trust_rules()`. Add
  `help: Option<MultiformatMessageString>` to `ReportingDescriptor` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. In
  `whitaker_properties.rs`, convert `WhitakerProperties` into an internally
  tagged enum — `#[serde(tag = "kind", rename_all = "camelCase")]` with a
  `Clone` variant holding today's fields and a `BrainTrust` variant holding the
  brain trust metrics — preserving `try_to_value`, `TryFrom<&Value>`, and the
  `"whitaker"` wrapper key. Rename the existing builder to match the clone
  variant and update `emit.rs`.
- Red artefact: `whitaker_properties_round_trip_preserves_brain_trust_variant`,
  and `rule_ids_are_disjoint_across_families` asserting the intersection of
  clone and brain trust rule identifiers is empty.
- Acceptance evidence: both crates' tests pass; the clone detector's property
  bag gains a `kind` discriminant, asserted in an updated test.
- Conformance check: rule identifiers `WHK101`/`WHK102` extend the clone
  detector's SARIF `ruleId` namespace, which is distinct from the selector
  codes (`DOC001`, `MOD001`) that roadmap 3.6.1 owns. Record that distinction
  in the ADR so 3.6.1 remains free to assign selector codes.
- Recovery: additive plus mechanical renames.
- Remaining gaps: the emitter itself.
- Compatibility decision: none required.

### EP-M5 — brain trust SARIF emitter

- Outcome: brain trust diagnostics become a deterministic
  `whitaker_sarif::Run`, opt-in and English-only.
- Requirements: `BTS-REQ-01`–`BTS-REQ-05`; discharges `VP-3`–`VP-7`.
- Changes: add `whitaker_sarif`, `serde`, and `serde_json` to
  `common/Cargo.toml`. Add `common/src/brain_trust_sarif/` with:
  - `mod.rs` — module documentation and re-exports.
  - `finding.rs` — `BrainTrustSubject { kind: SubjectKind, name, file_uri:
    FileUri, span: SourceSpan }` (reusing `decomposition_advice::SubjectKind`)
    and `BrainTrustFinding`, built from a diagnostic plus a subject plus an
    optional `&[DecompositionSuggestion]`, returning `None` for `Pass`.
  - `mapping.rs` — `emit_brain_trust_run(findings, tool) -> Result<Run, _>`,
    mirroring `emit_run0`: sort, `deduplicate_results`, `RunBuilder` with
    `brain_trust_rules()`.
  - `collector.rs` — a `BTreeMap`-backed collector keyed on the ordering key.
  - `mode.rs` — `BrainTrustSarifMode { Disabled, Enabled }`, with the
    collection entry point returning early when disabled.
  - `ordering.rs`, plus the colocated `*_tests.rs` modules the repository's
    convention uses.
- Red artefacts: `disabled_mode_builds_no_run`;
  `emits_one_result_per_finding`; the six `insta` snapshots; and the BDD
  scenarios below.
- Acceptance evidence: `cargo nextest run -p whitaker-common` passes;
  `cargo insta test --package whitaker-common --check` reports no pending
  snapshots; VP-3's non-vacuity counters are satisfied.
- Conformance check: `make lint` already covers `whitaker-common`
  (`Makefile:81` lists `-p whitaker-common`), so no Makefile change is needed
  for the lint gate. No file exceeds 400 lines. No `HashMap` appears in any
  serialized position.
- Recovery: additive within `whitaker-common`.
- Remaining gaps: documentation; and, out of scope, the lint crates and file
  emission.
- Compatibility decision: none required.

Feature specification,
`common/tests/features/brain_trust_sarif.feature`:

```gherkin
Feature: Opt-in SARIF emission for brain trust findings

  Scenario: A denied brain type becomes an error-level result
    Given SARIF collection is enabled
    And a denied brain type evaluation for "OrderProcessor" in "src/orders.rs"
    When the brain trust run is emitted
    Then the run contains one result with rule identifier "WHK101"
    And the result level is "error"
    And the result location file is "src/orders.rs"

  Scenario: A warned brain trait becomes a warning-level result
    Given SARIF collection is enabled
    And a warned brain trait evaluation for "Repository" in "src/repo.rs"
    When the brain trust run is emitted
    Then the run contains one result with rule identifier "WHK102"
    And the result level is "warning"

  Scenario: Disabled collection builds no run
    Given SARIF collection is disabled
    And a denied brain type evaluation for "OrderProcessor" in "src/orders.rs"
    When the brain trust run is emitted
    Then no run is produced

  Scenario: A passing subject produces no result
    Given SARIF collection is enabled
    And a passing brain trait evaluation for "Repository" in "src/repo.rs"
    When the brain trust run is emitted
    Then the run contains no results

  Scenario: The message text is the English primary message
    Given SARIF collection is enabled
    And a warned brain type evaluation for "Ledger" in "src/ledger.rs"
    When the brain trust run is emitted
    Then the first result message equals the English primary message

  Scenario: Recording order does not change the emitted run
    Given SARIF collection is enabled
    And two warned brain type evaluations recorded in ascending name order
    When the brain trust run is emitted
    And the same evaluations are recorded in descending name order
    Then both runs serialize to identical JSON
```

### EP-M6 — documentation, ADR, and roadmap

- Outcome: the change is documented everywhere the repository requires.
- Changes:
  - `docs/adr-005-brain-trust-sarif-emission.md`, using the required ADR
    sections. It records: mirroring the clone detector's pure-emitter
    architecture and why file writing is deferred; promoting four helpers into
    `whitaker_sarif` and the defects each promotion fixed; the `BTreeMap`
    determinism fix and why `implicit_hasher` does not apply; length-prefixed
    encoding versus delimiter separation; the discriminated property bag; the
    `WHK1xx` SARIF rule block and its distinction from the roadmap-3.6.1
    selector codes; deferring configuration to 6.6.1; omitting
    `defaultConfiguration.level`, `ruleIndex`, and `automationDetails`; and the
    span-to-URI contract the future lint crates must satisfy.
  - `docs/brain-trust-lints-design.md` §SARIF output: replace the planned
    approach with the delivered design, note that 6.6.2 must not localize the
    `format_*` functions the emitter depends on without forking them, and
    reference the ADR.
  - `docs/whitaker-clone-detector-design.md`: record the shared-helper
    promotions and the fingerprint-value change.
  - `docs/users-guide.md`: describe the rule identifiers, the SARIF shape, and
    that emission is a library capability with no user-facing switch yet,
    pointing at 6.6.1.
  - `docs/developers-guide.md`: the internal conventions — where shared SARIF
    helpers live, how to add a rule family, the determinism rules (no `HashMap`
    in serialized positions, never enable `serde_json/preserve_order`), and how
    to run the new proof targets.
  - `docs/contents.md` and `docs/repository-layout.md`: register the ADR and
    the new module.
  - `docs/roadmap.md`: mark 6.5.1 done.
  - `typos.local.toml` then `make spelling-config-write`, if a new term trips
    the gate.
- Acceptance evidence: `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, `make verus-brain-trust`, and
  `make kani-brain-trust` all succeed; `mbake validate Makefile` passes.
- Conformance check: every `BTS-REQ` maps to a passing test named in
  `Conformance basis`; no upstream deviation is left unrecorded.
- Recovery: documentation only.
- Remaining gaps: the lint crates, file emission, and the configuration
  surface, all out of scope and all recorded.

## Concrete steps

Run everything from the repository root of your checkout, on branch
`6-5-1-collect-brain-trust-diagnostics-into-sarif-emitter`.

Tee long output so nothing is truncated:

```bash
ACTION=test
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make "${ACTION}" 2>&1 | tee "${LOG}"
```

Focused runs during development:

```bash
cargo nextest run -p whitaker_sarif -p whitaker_clones_core
cargo nextest run -p whitaker-common 2>&1 | tee /tmp/nextest-brain-sarif.out
```

Expected shape of a green focused run:

```plaintext
    Starting 38 tests across 4 binaries
        PASS [   0.004s] whitaker_sarif fingerprint::tests::encoding_separates_components_a_delimiter_would_merge
        PASS [   0.006s] whitaker-common brain_trust_sarif::mapping_tests::snapshot_brain_type_deny
     Summary [   0.389s] 38 tests run: 38 passed, 0 skipped
```

Proof sidecars:

```bash
make verus-brain-trust 2>&1 | tee /tmp/verus-brain-trust.out
make kani-brain-trust  2>&1 | tee /tmp/kani-brain-trust.out
```

Expected tails:

```plaintext
verification results:: 4 verified, 0 errors
VERIFICATION:- SUCCESSFUL
```

Snapshot review:

```bash
cargo insta test --package whitaker-common
cargo insta review
```

Full gate sequence before each commit, run **sequentially** so the build cache
is used:

```bash
make check-fmt && make typecheck && make lint && make test
```

Delegate full gate runs to the `scrutineer` sub-agent rather than running them
in the planning context.

## Validation and acceptance

Acceptance is phrased as behaviour.

1. Given a disabled mode and ten denied findings, the entry point produces no
   run and the collector holds nothing. Test: `disabled_mode_builds_no_run`.
2. Given an enabled mode and the same findings, the entry point produces a
   `Run` whose `results` has ten entries ordered by rule, file, line, column,
   and subject name, each with a location and a one-based `startLine`. Test:
   `emits_one_result_per_finding`.
3. Serializing that run yields JSON whose `$schema` is
   `whitaker_sarif::SARIF_SCHEMA` and whose `version` is `"2.1.0"`. Test:
   `snapshot_brain_type_deny`.
4. Recording the same findings in reverse order yields byte-identical JSON.
   Tests: `emission_is_permutation_invariant` and the sixth BDD scenario.
5. A `Pass` disposition never appears. Test:
   `passing_subject_produces_no_result`.
6. A subject with five brain methods yields three in the property bag plus
   `brainMethodsOmitted: 2`. Test: `brain_methods_are_capped`.
7. After the shared refactors, the clone detector's six BDD scenarios still
   pass, with fingerprint expectations updated once and deliberately.

Record Red-Green-Refactor evidence per milestone: the red command and its
observed failure (for proofs, the Verus error or Kani counter-example); the
green command passing after the minimal implementation; and
`make check-fmt && make typecheck && make lint && make test` passing after
cleanup.

Quality criteria (what "done" means):

- Tests: `make test` passes with no new ignored or skipped tests; every new
  public item has a Rustdoc example that runs as a doctest.
- Verification: `VP-1` discharged by Verus with `0 errors`; `VP-2` by Kani with
  `VERIFICATION:- SUCCESSFUL` including both cover checks; `VP-3`–`VP-7` by
  their named artefacts, each with its non-vacuity check recorded.
- Lint and typecheck: `make lint` and `make typecheck` pass with warnings
  denied.
- Documentation: `make markdownlint` and `make nixie` pass; `mbake validate
  Makefile` passes.
- Performance: no benchmark required. The disabled path builds no run and
  allocates no per-finding storage; note honestly that a caller still pays to
  construct a `BrainTrustSubject` before calling, so the emitter exposes
  `BrainTrustSarifMode::is_enabled` as the documented call-site gate.
- Security: no new network access, no filesystem access, no process spawning.

## Idempotence and recovery

Every step is re-runnable; nothing in this item touches the filesystem at
runtime, so there is no partial-write or cleanup story to manage. The proof
sidecars cache their toolchains and are safe to re-run; if `make
kani-brain-trust` is interrupted, re-run it (the install step is idempotent —
note the warm-cache trap fixed during roadmap 6.4.6).

To abandon: `git checkout main -- docs/roadmap.md`, delete
`common/src/brain_trust_sarif/` and `verus/brain_trust_fingerprint.rs`, and
revert the script and `Makefile` entries. `EP-M1` through `EP-M4` are
independently valuable shared-infrastructure improvements and can be kept.

## Artefacts and notes

Illustrative shape of one emitted result, serialized:

```json
{
  "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/schemas/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "whitaker_brain_trust",
          "version": "0.2.7",
          "rules": [
            {
              "id": "WHK101",
              "name": "BrainType",
              "shortDescription": { "text": "Type has grown into a brain class" },
              "help": { "text": "Split the type along the suggested method clusters." },
              "helpUri": "https://github.com/leynos/whitaker/blob/main/docs/brain-trust-lints-design.md"
            }
          ]
        }
      },
      "results": [
        {
          "ruleId": "WHK101",
          "level": "error",
          "message": { "text": "`OrderProcessor` has WMC=118 and LCOM4=4." },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": { "uri": "src/orders.rs" },
                "region": { "startLine": 42, "startColumn": 1, "endLine": 310, "endColumn": 2 }
              }
            }
          ],
          "partialFingerprints": { "whitakerBrainSubject/v1": "6f1c" },
          "properties": {
            "whitaker": {
              "brainMethods": [
                { "cognitiveComplexity": 41, "linesOfCode": 96, "name": "reconcile" }
              ],
              "brainMethodsOmitted": 0,
              "foreignReach": 14,
              "kind": "brainTrust",
              "lcom4": 4,
              "subjectKind": "type",
              "wmc": 118
            }
          }
        }
      ]
    }
  ]
}
```

Object keys are alphabetical because `serde_json::Value::Object` is a
`BTreeMap`. That ordering is load-bearing; never enable
`serde_json/preserve_order`.

## Interfaces and dependencies

### Promoted into `whitaker_sarif`

```rust
// crates/whitaker_sarif/src/fingerprint.rs

/// Encodes components with an injective, length-prefixed framing.
#[must_use]
pub fn encode_components(components: &[&str]) -> Vec<u8>;

/// Returns the lowercase hexadecimal SHA-256 fingerprint of the components.
#[must_use]
pub fn fingerprint_hex(components: &[&str]) -> String;
```

```rust
// crates/whitaker_sarif/src/model/location.rs

/// A repository-root-relative, forward-slashed SARIF artefact URI.
pub struct FileUri(String);

impl TryFrom<&str> for FileUri { type Error = SarifError; /* ... */ }

/// Converts a line-and-column span into a one-based SARIF region.
#[must_use]
pub fn span_to_region(span: SourceSpan) -> Region;

/// Converts a byte range into a SARIF region, counting UTF-16 code units.
///
/// Moved here from `whitaker_clones_core::run0::span`.
pub fn region_for_range(
    subject_id: &str,
    source_text: &str,
    range: std::ops::Range<usize>,
) -> Result<Region, SarifError>;
```

```rust
// crates/whitaker_sarif/src/rules.rs

pub const WHK101_ID: &str = "WHK101";
pub const WHK102_ID: &str = "WHK102";

#[must_use] pub fn whk101_rule() -> ReportingDescriptor;
#[must_use] pub fn whk102_rule() -> ReportingDescriptor;
#[must_use] pub fn brain_trust_rules() -> Vec<ReportingDescriptor>;

/// Renamed from `all_rules`, which no longer described its contents.
#[must_use] pub fn clone_detection_rules() -> Vec<ReportingDescriptor>;
```

```rust
// crates/whitaker_sarif/src/whitaker_properties.rs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WhitakerProperties {
    Clone(CloneProperties),
    BrainTrust(BrainTrustProperties),
}

impl WhitakerProperties {
    /// Wraps these properties under the `"whitaker"` key.
    ///
    /// # Errors
    ///
    /// Returns [`SarifError::Serialization`] when serialization fails.
    pub fn try_to_value(&self) -> crate::error::Result<Value>;
}
```

### Added to `whitaker-common`

```rust
// common/src/brain_trust_sarif/finding.rs

pub struct BrainTrustSubject {
    kind: whitaker_common::decomposition_advice::SubjectKind,
    name: String,
    file_uri: whitaker_sarif::FileUri,
    span: whitaker_common::span::SourceSpan,
}

pub struct BrainTrustFinding { /* private fields */ }

impl BrainTrustFinding {
    /// Builds a finding, or `None` when the disposition is `Pass`.
    #[must_use]
    pub fn from_brain_type(
        diagnostic: &crate::BrainTypeDiagnostic,
        subject: BrainTrustSubject,
        suggestions: &[crate::DecompositionSuggestion],
    ) -> Option<Self>;

    #[must_use]
    pub fn from_brain_trait(
        diagnostic: &crate::BrainTraitDiagnostic,
        subject: BrainTrustSubject,
        suggestions: &[crate::DecompositionSuggestion],
    ) -> Option<Self>;
}
```

```rust
// common/src/brain_trust_sarif/mode.rs

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrainTrustSarifMode {
    #[default]
    Disabled,
    Enabled,
}

impl BrainTrustSarifMode {
    #[must_use]
    pub const fn is_enabled(self) -> bool { matches!(self, Self::Enabled) }
}
```

```rust
// common/src/brain_trust_sarif/mapping.rs

/// Builds a SARIF run from brain trust findings, mirroring `emit_run0`.
///
/// Returns `Ok(None)` when the mode is disabled.
///
/// # Errors
///
/// Returns an error when a result cannot be constructed.
pub fn emit_brain_trust_run(
    mode: BrainTrustSarifMode,
    findings: &[BrainTrustFinding],
    tool_name: &str,
    tool_version: &str,
) -> Result<Option<whitaker_sarif::Run>, BrainTrustSarifError>;
```

Note the `Option` here carries exactly one meaning — disabled. An enabled run
with no findings returns `Ok(Some(run))` with an empty `results` array, so a
future consumer can tell "clean" from "not asked".

### The contract the future lint crates must satisfy

A `LateLintPass` holds a `rustc_span::Span` and a `TyCtxt`. To build a
`BrainTrustSubject` it must produce a repository-root-relative, forward-slashed
path for `FileUri::try_from`, and a `SourceSpan` from the span's start and end
line and column. Record this in the ADR; it is the seam roadmap 6.6.x inherits.

### Signposted documentation and skills

Read before or during the work: `AGENTS.md`;
`docs/brain-trust-lints-design.md`;
`docs/whitaker-clone-detector-design.md` §SARIF schema and mapping, §Runs, and
§CLI surface; `docs/whitaker-dylint-suite-design.md`;
`docs/whitaker-cli-design.md` §Rule identifiers and §Configuration model;
`docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/documentation-style-guide.md`; `docs/repository-layout.md`;
`docs/contents.md`.

Skills to load: `leta` for symbol navigation (prefer `leta show`, `leta refs`,
and `leta calls` over reading files or grepping for symbols);
`hexagonal-architecture` for keeping the emitter's pure core free of
configuration and I/O concerns — noting that with no adapters in this item the
relevant guidance is boundary discipline, not a ports-and-adapters transplant;
`verus` for VP-1, particularly triggers, `assert ... by { }` scoping, and
`broadcast use vstd::seq::group_seq_axioms;`; `kani` for VP-2, particularly the
unwind off-by-one rule and the heap-collection cliff; `rust-unit-testing` for
`rstest`, `googletest`, `pretty_assertions`, and `insta`; `proptest` for
generator design and shrinking; `rust-errors` for the `thiserror` boundary;
`arch-crate-design` for the shared-crate surface; `arch-decision-records` for
the ADR; and `execplans` for keeping this document current.

### External references

- OASIS, *Static Analysis Results Interchange Format (SARIF) Version 2.1.0
  Errata 01*. Relevant sections: §3.27.16 `partialFingerprints` (the `/v<n>`
  key-suffix convention), §3.49 `reportingDescriptor`, §3.14.6 `columnKind`.
- GitHub, *SARIF support for code scanning* — the ingested subset, and
  confirmation that only `primaryLocationLineHash` is read from
  `partialFingerprints`. Whitaker's subject fingerprint is for its own
  `deduplicate_results` and for future merge, not for that consumer.
- GitHub, *SARIF results exceed one or more limits* — 20 runs per file, 25,000
  results per run, 25,000 rules per run. A single brain trust run is well
  inside these; whichever item takes on file emission must revisit them.
