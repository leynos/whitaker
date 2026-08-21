# Collect brain trust diagnostics into an opt-in SARIF 2.1.0 emitter

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, `Outcomes & retrospective`, `Conformance basis`, and
`Verification plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Whitaker's brain trust analysis measures how overgrown a Rust type or trait has
become. Today those measurements can only be rendered as English sentences for
a compiler diagnostic. Nothing can feed them to a continuous integration (CI)
dashboard, a pull-request annotation, or an editor's problem list.

After this change, a Whitaker user who opts in gets a machine-readable Static
Analysis Results Interchange Format (SARIF) 2.1.0 file describing every brain
type and brain trait finding, written to a predictable path under the Cargo
target directory. SARIF is the JSON interchange format that GitHub code
scanning, Azure DevOps, and most IDE problem viewers already understand.

You can see it working like this. With the feature switched off (the default),
nothing changes and no file appears. With `WHITAKER_BRAIN_SARIF=1` set, a run
produces `target/whitaker/brain-trust/<unit>.sarif` containing a SARIF log
whose `runs[0].results` array lists one entry per warned or denied subject,
each with a rule identifier, a severity level, a source location, the measured
values, and a stable fingerprint. The same inputs always produce byte-identical
JSON, so the file can be committed as a golden fixture or diffed across builds.

The messages inside that file are always English, regardless of the locale
Whitaker uses for compiler diagnostics, because downstream tools index and
deduplicate on message text.

Scope note. This plan delivers the emitter, its collection model, its opt-in
resolution, and its file adapter. It does **not** create the `brain_type` and
`brain_trait` Dylint lint crates — those do not exist yet (see `Context and
orientation`), and they are separate roadmap work. The emitter is therefore
delivered as a library with an end-to-end, observable behaviour of its own: a
caller hands it findings, and a SARIF file appears on disk.

## Constraints

Hard invariants that must hold throughout implementation. Violating one
requires escalation, not a workaround.

- The emitter must be **off by default**. A build that sets no environment
  variable and adds no configuration must produce no SARIF file, must perform
  no filesystem access, and must not allocate per-finding storage.
- SARIF message text must be **English only**. The emitter must not read,
  import, or transitively invoke `whitaker_common::i18n`. Locale settings must
  not change a single byte of emitted JSON.
- Emitted JSON must be **deterministic**: the same set of findings must produce
  byte-identical output regardless of the order in which findings were
  recorded, the iteration order of any hash container, or the platform.
- No `std::fs` or `std::path` in the new crate. Filesystem access must go
  through `cap_std::fs_utf8` and `camino`, per `AGENTS.md` and the
  `no_std_fs_operations` lint. The new crate must **not** be added to the
  `excluded_crates` list in `dylint.toml`.
- No `.unwrap()` or `.expect()` in production code or shared fixtures. The
  workspace denies `clippy::unwrap_used` and `clippy::expect_used`
  (`Cargo.toml`, `[workspace.lints.clippy]`).
- No file in the repository may exceed 400 lines (`AGENTS.md`).
- Dependencies must use caret requirements and must be taken from
  `[workspace.dependencies]` in the root `Cargo.toml` wherever a pin already
  exists.
- The existing public behaviour of `whitaker_clones_core::run0` must keep
  working. Its BDD scenarios in
  `crates/whitaker_clones_core/tests/run0_sarif_behaviour.rs` must continue to
  pass unchanged in intent.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` must all
  succeed at every milestone boundary.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 30 files, or changing
  more than 2500 net lines of code excluding snapshots and generated fixtures,
  stop and escalate.
- Interface: if the change requires altering a public signature in
  `whitaker_common` outside the additive re-exports named in `Interfaces and
  dependencies`, stop and escalate.
- Dependencies: the plan authorizes exactly four new dependency edges, all onto
  crates already pinned in `[workspace.dependencies]`: `whitaker_sarif`,
  `serde`, `serde_json`, `cap-std`, plus `sha2` and `camino`. If any *further*
  external dependency is required, stop and escalate.
- Verification: if the Verus proof in `EP-M2` cannot be discharged after three
  modelling iterations, stop and escalate rather than weakening the lemma to a
  restatement. If the Kani harness in `EP-M5` hits CBMC state explosion after
  three bound reductions, record the blocker as roadmap 6.4.6 did and escalate
  before removing the harness.
- Iterations: if a gate still fails after four fix attempts, stop and escalate.
- Ambiguity: if the choice of output-file granularity (per compilation unit
  versus per workspace) turns out to matter to a consumer this plan has not
  identified, stop and present the options.

## Risks

- Risk: `whitaker_sarif::SarifResult::partial_fingerprints` is a
  `HashMap<String, String>`, whose serialization order is randomized per
  process. Byte-identical output is impossible without changing it.
  Severity: high. Likelihood: certain (already observed).
  Mitigation: `EP-M1` changes the field to `BTreeMap<String, String>` and
  updates the single consumer in the same milestone. This is a pre-1.0,
  `publish = false` crate with one in-repo consumer, so no compatibility layer
  is warranted.
- Risk: the `brain_type` and `brain_trait` Dylint crates do not exist, so there
  is no production call site to prove the emitter is wired correctly.
  Severity: medium. Likelihood: certain.
  Mitigation: deliver observable behaviour through an end-to-end test that
  drives the public reporter API against a real temporary directory, and record
  the wiring contract in the design document so the future lint crates have a
  single documented entry point.
- Risk: rustc invokes lints once per compilation unit, potentially in parallel
  processes. A single shared output file would need locking or append
  semantics.
  Severity: medium. Likelihood: high once lints exist.
  Mitigation: emit one file per compilation unit, named by a caller-supplied
  identifier, written atomically via temporary file plus rename. Merging is
  deferred and explicitly out of scope; `whitaker_sarif::merge_runs` already
  exists for a later consumer.
- Risk: Kani state explosion on collector harnesses, as encountered in roadmap
  6.4.6.
  Severity: medium. Likelihood: medium.
  Mitigation: model the collector over fixed-size arrays of small integer keys
  rather than `String`/`Vec`, keep the bound at three findings, and pin the
  solver if needed.
- Risk: Verus proof of encoding injectivity drifts from the shipped Rust
  implementation, because Verus compiles its own sidecar files and cannot
  `use` production modules.
  Severity: medium. Likelihood: medium.
  Mitigation: mirror the encoder as a `spec fn` with a doc comment naming the
  production function and its file, and add a Rust unit test asserting the two
  agree on a fixed vector of cases including the collision witness.
- Risk: environment-variable handling in tests violates the repository ban on
  direct environment mutation.
  Severity: low. Likelihood: medium.
  Mitigation: settings resolution is a pure function over an injected snapshot;
  only one thin adapter reads the real environment, and its test uses the
  `temp-env` guard already pinned in `[workspace.dependencies]`.

## Progress

- [ ] EP-M0 Understand and propose (no code changes). Completed: reconnaissance
  of `whitaker_sarif`, `whitaker_clones_core::run0`, and the brain trust
  modules in `common`; SARIF 2.1.0 and GitHub code-scanning research.
  Remaining: plan approval.
- [ ] EP-M1 Deterministic SARIF model foundation.
- [ ] EP-M2 Fingerprint encoding and its Verus injectivity proof.
- [ ] EP-M3 Domain finding model, ordering, and English-only rendering.
- [ ] EP-M4 Pure mapping to a SARIF run, with snapshots.
- [ ] EP-M5 Collector with dedup and ordering, plus the Kani harness.
- [ ] EP-M6 Settings resolution and the opt-in truth table.
- [ ] EP-M7 Ports, adapters, and the end-to-end file emission behaviour.
- [ ] EP-M8 Documentation, ADR, roadmap tick, and full gate run.

Timestamps are added as each milestone completes.

## Surprises & discoveries

- Observation: the `brain_type` and `brain_trait` Dylint lint crates do not
  exist. Roadmap items 6.2.2 and 6.3.2 are ticked, but what they delivered is
  the pure evaluation and formatting layer in `common/src/brain_type_metrics/`
  and `common/src/brain_trait_metrics/`.
  Evidence: `crates/` contains no `brain_type` or `brain_trait` directory; no
  `span_lint` call site in the repository references either lint; `dylint.toml`
  has no section for either.
  Impact: 6.5.1 must be scoped to the layer that exists. The emitter consumes
  `BrainTypeDiagnostic` and `BrainTraitDiagnostic` values plus a location, and
  the future lint crates become its callers.
- Observation: nothing in the repository writes a SARIF file. The clone
  detector's `emit_run0` returns an in-memory `whitaker_sarif::Run` and stops
  there.
  Evidence: `crates/whitaker_clones_core/src/run0/emit.rs:97`; no call site
  pairs `whitaker_sarif::paths::*` with a file write.
  Impact: this plan must design the sink and the opt-in mechanism from first
  principles rather than copying an existing one. The path-naming convention in
  `crates/whitaker_sarif/src/paths.rs` is the only precedent.
- Observation: `SarifResult::partial_fingerprints` is a `HashMap`, so the
  crate cannot currently produce stable bytes.
  Evidence: `crates/whitaker_sarif/src/model/result.rs:107`.
  Impact: promoted to `EP-M1`; see `Risks`.
- Observation: `whitaker_clones_core`'s `pair_fingerprint` concatenates
  components with a single zero byte
  (`crates/whitaker_clones_core/src/run0/emit.rs:285`). That encoding is not
  injective in general.
  Impact: the brain trust encoder uses length prefixes instead, and the Verus
  proof establishes why. Retrofitting the clone detector is out of scope and is
  recorded as follow-up work in the decision log.

## Decision log

- Decision: deliver the emitter as a new crate, `crates/whitaker_brain_sarif`,
  rather than as a module inside `whitaker-common`.
  Rationale: `whitaker-common` is the domain layer. SARIF is an outbound
  interchange format, so under the dependency rule the domain must not depend
  on it. Keeping the emitter separate also keeps `serde`, `serde_json`,
  `cap-std`, and `sha2` out of `whitaker-common`, and means the new crate is
  subject to the `no_std_fs_operations` capability policy rather than being
  covered by the `whitaker_common` exclusion in `dylint.toml`. This mirrors the
  existing split between `whitaker_sarif` (format) and `whitaker_clones_core`
  (analysis).
  Date/Author: 2026-08-21, planning agent.
- Decision: emit one SARIF log per compilation unit into a directory, rather
  than appending to one shared file.
  Rationale: rustc lints run per crate and may run concurrently. Per-unit files
  need no locking, no append semantics, and no partial-write recovery, and they
  align with GitHub's July 2025 change that stopped combining multiple runs
  from a single uploaded file. Merging remains available later through
  `whitaker_sarif::merge_runs`.
  Date/Author: 2026-08-21, planning agent.
- Decision: change `SarifResult::partial_fingerprints` from `HashMap` to
  `BTreeMap` and update the clone detector in the same milestone, with no
  compatibility shim.
  Rationale: the crate is `publish = false`, pre-1.0, and has exactly one
  in-repo consumer. Under the ExecPlan compatibility rules, no shim is
  warranted; the question "compatible with whom?" has no answer. Determinism is
  a stated constraint that cannot otherwise be met.
  Date/Author: 2026-08-21, planning agent.
- Decision: add two optional, additive fields to `whitaker_sarif` —
  `ReportingDescriptor::help` and `Run::automation_details` — each with
  `skip_serializing_if = "Option::is_none"`.
  Rationale: rule help text is the field GitHub code scanning renders next to
  an alert, and `automationDetails.id` is the category key that stops per-crate
  uploads from clobbering one another. Both are `None` for existing clone
  detector output, so no existing serialized form changes.
  Date/Author: 2026-08-21, planning agent.
- Decision: use length-prefixed component encoding for fingerprint pre-images,
  not delimiter separation.
  Rationale: delimiter separation is only injective when the delimiter cannot
  occur in a component, which is an assumption about all future component types
  rather than a property of the encoding. Length prefixing is unconditionally
  injective and the property is provable.
  Date/Author: 2026-08-21, planning agent.
- Decision: reserve rule identifiers `WHK101` for `brain_type` and `WHK102` for
  `brain_trait`, leaving `WHK001`–`WHK003` to the clone detector and `WHK1xx`
  to structural design lints.
  Rationale: SARIF rule identifiers are a global namespace across a tool; a
  disjoint block avoids future collisions.
  Date/Author: 2026-08-21, planning agent.
- Decision: notes, help text, and decomposition advice go into the result's
  `properties` bag and the rule's `help` field, not into `message.text`.
  Rationale: GitHub renders only the first sentence of `message.text` as an
  alert title and indexes on the whole string. Appending multi-paragraph advice
  would degrade the title and destabilize deduplication.
  Date/Author: 2026-08-21, planning agent.
- Follow-up recorded, not actioned here: `whitaker_clones_core`'s
  `pair_fingerprint` and `token_hash` use zero-delimited concatenation. A
  future task should migrate them to the proven encoder. Out of scope for
  6.5.1 because it changes clone detector fingerprints, which is a persisted
  format change for anyone holding existing SARIF output.

## Outcomes & retrospective

To be completed at `EP-M8`. Before setting the plan to `COMPLETE`, reconcile
every discovery against `docs/brain-trust-lints-design.md` §SARIF output, the
new ADR, `docs/users-guide.md`, `docs/developers-guide.md`, and
`docs/roadmap.md` item 6.5.1.

## Context and orientation

Assume you have only this repository and this file.

### What Whitaker is

Whitaker is a suite of Rust lints built on **Dylint**, a tool that loads
out-of-tree lints into the Rust compiler. Lints live in `crates/<lint_name>/`.
Shared, compiler-independent helper logic lives in the `whitaker-common`
package at `common/`. The workspace members are declared in the root
`Cargo.toml` as `["common", "crates/*", "installer", "suite"]`.

### What "brain trust" means here

Two planned lints share an analysis:

- `brain_type` flags a type that has grown too complex, using Weighted Method
  Count (WMC, the sum of per-method cognitive complexity), LCOM4 (Lack of
  Cohesion of Methods, version 4 — the number of connected components in the
  method/field graph, where a higher number means the type is really several
  types), foreign reach (how many other types' data the methods touch), and
  "brain methods" (individual methods that are both very complex and very
  long).
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
  extractions and renders them as a note.
- `common/src/span.rs` — `SourceLocation` (one-based line and column) and
  `SourceSpan` (a start and an end location, validated so the start never
  follows the end).

**Important:** the Dylint lint crates `crates/brain_type/` and
`crates/brain_trait/` do **not** exist. Nothing in the repository currently
calls `evaluate_brain_type` from a compiler pass. This plan therefore treats
`BrainTypeDiagnostic` and `BrainTraitDiagnostic` as the inputs to the emitter,
and the future lint crates as its callers.

### What SARIF is

SARIF 2.1.0 is an OASIS standard JSON format for static analysis output. A
SARIF *log* has a `$schema`, a `version`, and an array of *runs*. Each run
names the *tool* that produced it and carries an array of *results*. A result
has a `ruleId`, a `level` (`none`, `note`, `warning`, or `error`), a
`message.text`, one or more `locations`, and optionally `partialFingerprints`
— a small map of strings that consumers use to recognize "the same finding" as
code moves between commits.

`crates/whitaker_sarif/` already models the subset Whitaker needs:

- `model/log.rs` — `SarifLog { schema, version, runs }`, with `SARIF_SCHEMA`
  and `SARIF_VERSION = "2.1.0"`.
- `model/run.rs` — `Run { tool, invocations, results, artefacts }`, `Tool`,
  `ToolComponent { name, version, information_uri, rules }`, `Invocation`,
  `Artefact`.
- `model/result.rs` — `SarifResult`, `Level`, `Message`.
- `model/location.rs` — `Location`, `PhysicalLocation`, `ArtefactLocation`,
  `Region`, `RelatedLocation`.
- `model/descriptor.rs` — `ReportingDescriptor` (the SARIF term for a rule) and
  `MultiformatMessageString`.
- `builders/` — `SarifLogBuilder`, `RunBuilder`, `ResultBuilder`,
  `LocationBuilder`, `RegionBuilder`.
- `merge.rs` — `merge_runs` and `deduplicate_results`.
- `paths.rs` — `WHITAKER_DIR = "whitaker"` plus clone-detector filenames.
- `rules.rs` — `WHK001`–`WHK003` for the clone detector.

The crate has no compiler dependency, uses `serde` for serialization, and
performs no input or output of its own.

### The one existing producer

`crates/whitaker_clones_core/src/run0/emit.rs` builds a `Run` from accepted
clone pairs. Read `emit_run0` (line 97) and `build_result` (line 121) before
starting: they are the house style for turning analysis output into SARIF.
Note that they stop at an in-memory `Run` — nothing writes it to disk.

### Where things will go

This plan adds one crate, `crates/whitaker_brain_sarif`, and makes small
additive changes to `crates/whitaker_sarif`.

## Conformance basis

There is no Terms of Reference document in this repository. The upstream
artefacts are:

- `docs/roadmap.md` §6.5, item 6.5.1 (revision: the tree at branch
  `harden-lint-config`, commit `02e6c1c`).
- `docs/brain-trust-lints-design.md` §SARIF output (lines 629–641) and
  §Configuration, localization, and testing (lines 643–668).
- `docs/whitaker-clone-detector-design.md` §Rules and §Runs, as the precedent
  for rule identifiers, result mapping, and file layout.
- `docs/whitaker-dylint-suite-design.md` for suite-wide lint conventions.
- `AGENTS.md` for code style, dependency policy, error handling, testing, and
  observability rules.
- `docs/documentation-style-guide.md` for the ADR shape and Markdown rules.
- OASIS SARIF 2.1.0 (Errata 01) as the format specification, and GitHub's
  "SARIF support for code scanning" reference for the ingestion subset.

Stable identifiers introduced by this plan:

- `BTS-REQ-01` — brain trust diagnostics are collected into a SARIF 2.1.0
  document. (From roadmap 6.5.1.)
- `BTS-REQ-02` — emission is opt-in and imposes no cost when disabled. (From
  design §SARIF output, "Avoid overhead when SARIF output is disabled".)
- `BTS-REQ-03` — messages are English only. (From design §SARIF output, "Keep
  messages in English for consistent tool ingestion".)
- `BTS-REQ-04` — results carry rule metadata, locations, and messages,
  serialized with `serde`. (From design §SARIF output.)
- `BTS-REQ-05` — output is deterministic and stable enough for tool
  deduplication. (Derived from BTS-REQ-01's purpose; recorded in the ADR.)

Trace links:

```plaintext
roadmap-6.5.1 -> BTS-REQ-01 -> EP-M3, EP-M4 -> tests::mapping::snapshot_brain_type_deny
roadmap-6.5.1 -> BTS-REQ-02 -> EP-M6, EP-M7 -> tests::reporter::disabled_writes_nothing
roadmap-6.5.1 -> BTS-REQ-03 -> EP-M3       -> tests::render::locale_does_not_affect_output
roadmap-6.5.1 -> BTS-REQ-04 -> EP-M4       -> tests::mapping::round_trip_is_byte_identical
roadmap-6.5.1 -> BTS-REQ-05 -> EP-M1, EP-M2, EP-M5 -> verus::brain_trust_fingerprint,
                                                       verus::brain_trust_ordering,
                                                       kani::verify_collector_dedup_bounded
```

## Verification plan

### Invariants and lemmas introduced

**VP-1 — Fingerprint pre-image encoding is injective.**

The emitter derives each result's stable identity by hashing an encoding of a
component tuple: the rule identifier, the file uniform resource identifier
(URI), the subject kind, and the subject name. If two different tuples can
encode to the same byte string, two unrelated findings can collapse into one
alert. The encoding is therefore required to be injective.

- Obligation: for component sequences `a` and `b`,
  `encode(a) == encode(b) ==> a == b`, where `encode` writes each component as
  an eight-byte big-endian length prefix followed by the component's bytes.
- Method: formal proof in Verus.
- Rationale: the property quantifies over all byte sequences of all lengths.
  Bounded model checking cannot cover it and property tests can only sample it.
  It is a small, self-contained lemma about a pure function — exactly the shape
  Verus handles well, and the repository already runs a Verus sidecar
  (`make verus`, `scripts/run-verus.sh`).
- Domain: `Seq<Seq<u8>>` of arbitrary length, components of arbitrary length.
- Artefact: `verus/brain_trust_fingerprint.rs`, registered in
  `scripts/run-verus.sh` under a new `brain-trust` group and a new
  `make verus-brain-trust` target.
- Evidence: `make verus-brain-trust`. Before the proof is written, the file
  contains the lemma statement with its body left as an open goal, and Verus
  reports an unproven assertion. After, Verus reports `0 errors`.
- Proof shape: define `spec fn encode(components: Seq<Seq<u8>>) -> Seq<u8>`
  recursively with `decreases components.len()`, and a matching
  `spec fn decode(bytes: Seq<u8>) -> Option<Seq<Seq<u8>>>`. Prove
  `lemma_decode_encode_round_trip(components)` by induction, then derive
  injectivity as a corollary: `decode(encode(a)) == Some(a)` and
  `decode(encode(b)) == Some(b)` with `encode(a) == encode(b)` forces
  `Some(a) == Some(b)`. This is a genuine argument, not a restatement: the work
  is in the round-trip lemma, and the concatenation reasoning needs
  `broadcast use vstd::seq::group_seq_axioms;`.
- Non-vacuity: the antecedent is inhabited — `encode(seq![seq![]])` is a
  satisfying witness with an empty component, and the lemma is exercised on
  non-empty components too. The negative control is a Rust unit test,
  `encoding_separates_components_a_delimiter_would_merge`, asserting that the
  tuples `("ab", "c")` and `("a", "bc")` — which produce identical bytes under
  single-byte-delimiter concatenation — produce different bytes and different
  fingerprints under the length-prefixed encoder. A second control mutates the
  Verus `encode` to drop the length prefix and confirms the round-trip lemma
  then fails; this mutation is performed once, observed, and reverted, with the
  transcript recorded in `Artefacts and notes`.
- Residual gap: SHA-256 collision resistance is assumed, not proved (see
  `Axioms`). Injectivity of the pre-image encoding is what this repository
  owns; the hash's behaviour is not.

**VP-2 — The result ordering is a total order.**

Results are sorted before emission so that output is deterministic. If the
comparator is not a total order, `sort_by` may produce different permutations
for equal inputs presented in different orders, breaking determinism.

- Obligation: the relation `finding_leq(a, b)` induced by lexicographic
  comparison of `(rule_id, file_uri, start_line, start_column, subject_name)`
  is reflexive, antisymmetric, transitive, and strongly connected.
- Method: formal proof in Verus, composing four sub-lemmas into
  `vstd`'s `total_ordering`.
- Rationale: this is a contractual property of introduced business logic that
  must hold for all inputs; the repository already has precedent for exactly
  this proof shape in the decomposition work (roadmap 6.4.3–6.4.4).
- Domain: all tuples of two byte sequences, two naturals, and one byte
  sequence.
- Artefact: `verus/brain_trust_ordering.rs`, same `brain-trust` group.
- Evidence: `make verus-brain-trust` reports `0 errors`; the sub-lemmas fail
  individually before their bodies are supplied.
- Non-vacuity: each sub-lemma is instantiated on witnesses that differ at each
  tie-break level in turn, so no level is vacuously satisfied. The negative
  control drops `subject_name` from the key and shows antisymmetry failing for
  two distinct subjects declared on the same line and column — a real
  configuration, since a type and its inherent `impl` can start at the same
  position in generated code.

**VP-3 — The collector deduplicates without losing distinct subjects, and
returns findings in sorted order.**

- Obligation: for any recorded sequence of findings, `finish()` returns a
  vector that (a) is sorted under `finding_leq`, (b) contains no two entries
  with equal fingerprints, and (c) contains one entry for every distinct
  fingerprint recorded.
- Method: bounded model checking with Kani.
- Rationale: this is a transition property over a small state machine
  (a sequence of `record` calls followed by one `finish`). Exhaustive
  exploration within a bound of three findings covers every interleaving of the
  duplicate, tie, and distinct cases, which is more than a property test can
  guarantee and cheaper than a full proof.
- Domain: at most three findings; symbolic fingerprint key in `0..3`; symbolic
  start line in `1..3`; symbolic disposition. The model uses fixed-size arrays
  of `u8` keys rather than `String`s and `Vec`s, following the lesson recorded
  in roadmap 6.4.6 that heap collections hit a sharp CBMC cliff.
- Artefact: `crates/whitaker_brain_sarif/src/domain/collector_kani.rs`, gated
  behind `#[cfg(kani)]`, with `check-cfg = ['cfg(kani)']` declared in the
  crate's `[lints.rust]`. Registered in `scripts/run-kani.sh` under a new
  `brain-trust` group and a new `make kani-brain-trust` target.
- Evidence: `make kani-brain-trust`. Expect `VERIFICATION:- SUCCESSFUL` for
  each harness.
- Non-vacuity: a `kani::cover` assertion proves the duplicate-key case is
  reachable within the bound, and a second proves the all-distinct case is
  reachable; a bound that admitted neither would be a zero-work bound. The
  negative control mutates `finish()` to truncate its last element and confirms
  clause (c) fails with a counter-example trace; the mutation is observed once
  and reverted.
- Residual gap: bounded at three findings. Behaviour at larger sizes is covered
  by VP-4's property test, not by exhaustive search.

**VP-4 — Emission is invariant under input permutation and survives a JSON
round trip.**

- Obligation: for any multiset of findings and any two orders of recording
  them, the serialized SARIF bytes are equal; and
  `to_json(from_json(to_json(log))) == to_json(log)`.
- Method: property test with `proptest`.
- Rationale: the invariant ranges over generated inputs and orderings at sizes
  beyond Kani's practical bound, and it is the property that actually protects
  the user-visible guarantee of stable diffs.
- Domain: 0–12 findings; file URIs drawn from a four-element alphabet so
  collisions and ties actually occur; subject names from a five-element
  alphabet; lines in `1..=6`; both subject kinds; `Warn` and `Deny`
  dispositions.
- Artefact: `crates/whitaker_brain_sarif/tests/emission_properties.rs`.
- Evidence: `cargo nextest run -p whitaker_brain_sarif`. Regression seeds are
  committed under `crates/whitaker_brain_sarif/proptest-regressions/`.
- Non-vacuity: the test asserts, over the whole run, that at least one
  generated case contained two findings sharing a file URI and at least one
  contained two findings sharing a line, using counters accumulated in a
  `std::sync::atomic` pair and checked in a final `#[test]`. A generator that
  produced only distinct findings would fail this check rather than passing
  vacuously. The negative control removes the sort from the mapping function
  and confirms the permutation property fails.

**VP-5 — Opt-in resolution is total and correct across its finite partition.**

- Obligation: `resolve_settings(env_snapshot, file_settings)` returns exactly
  the documented result for every combination of inputs, and rejects malformed
  boolean values with a typed error rather than silently defaulting.
- Method: exhaustive parameterized tests with `rstest`, using `googletest`
  matchers and `pretty_assertions`.
- Rationale: the input space is a small finite product; enumeration is
  practical and clearer than generation.
- Domain: the enable variable in {unset, `"1"`, `"0"`, `"true"`, `"false"`,
  `"TRUE"`, `"  1  "`, `""`, `"maybe"`} × the directory variable in {unset,
  set, set-to-empty} × file settings in {absent, `enabled = true`,
  `enabled = false`, with and without `output_dir`}.
- Artefact: `crates/whitaker_brain_sarif/src/settings_tests.rs`.
- Evidence: every cell is an explicit `#[case]`; the count is asserted in the
  test module's doc comment and reviewed at `EP-M6`.
- Non-vacuity: each cell asserts a distinct expected outcome, including the two
  error cells (`"maybe"` and set-to-empty directory). The negative control
  changes precedence so file settings win over the environment and confirms
  four cells fail.

**VP-6 — Disabled emission performs no filesystem access and no per-finding
work.**

- Obligation: when settings resolve to disabled, `record()` stores nothing and
  `finish()` makes no sink call.
- Method: parameterized test with a `mockall` sink asserting `times(0)`, plus
  an assertion that the collector's length stays zero after 1024 `record`
  calls.
- Rationale: this is BTS-REQ-02 stated as observable behaviour; a mock with a
  zero-call expectation fails loudly if the implementation regresses.
- Artefact: `crates/whitaker_brain_sarif/src/reporter_tests.rs`.
- Non-vacuity: a paired positive case with `times(1)` proves the mock would
  register a call if one occurred, so the zero-call expectation is not passing
  because the harness is inert.

**VP-7 — Locale does not affect emitted bytes.**

- Obligation: emitted SARIF bytes are identical when the Whitaker locale is
  `en-GB`, `cy`, or `gd`.
- Method: behavioural test (`rstest-bdd`) driving the reporter under each
  locale, plus a structural fitness test asserting the crate's own sources
  never mention `i18n`, `Localizer`, or `fluent`.
- Rationale: the behavioural test catches accidental localisation at runtime;
  the fitness test catches it at authoring time, before anyone can wire it in.
- Artefact: `crates/whitaker_brain_sarif/tests/english_only_behaviour.rs` and
  `crates/whitaker_brain_sarif/tests/features/english_only.feature`.
- Non-vacuity: the fitness test is first run against a deliberately added
  `// Localizer` comment to confirm it fails, then the comment is removed.

**VP-8 — The emitted document is a valid SARIF 2.1.0 subset with the shape
downstream tools expect.**

- Obligation: `$schema` and `version` are present and correct; every result's
  `ruleId` appears in `runs[0].tool.driver.rules`; every result has at least
  one location with a one-based `startLine`; `Pass` dispositions produce no
  result.
- Method: `insta` snapshots across the multivariant matrix, plus explicit
  structural assertions.
- Rationale: output format consistency across variants is exactly what snapshot
  testing is for, and the structural assertions state the contract in a form a
  reviewer can read.
- Domain: brain type warn, brain type deny with two brain methods and a
  decomposition note, brain trait warn, brain trait deny, a mixed file with
  both kinds, and an empty run.
- Artefact: `crates/whitaker_brain_sarif/src/mapping_tests.rs` with snapshots
  under `crates/whitaker_brain_sarif/src/snapshots/`.
- Non-vacuity: the empty-run snapshot proves the mapping does not fabricate
  results; the two-brain-method case proves the properties bag is populated
  rather than being an empty object in every variant.

### Axioms

These are assumed, not verified. Verifying third-party internals is out of
scope.

- SHA-256 as implemented by `sha2` is collision resistant, and its output for a
  given input is stable across platforms and versions within the pinned caret
  range.
- `serde_json` serializes a `BTreeMap` in ascending key order, and serializes
  `serde_json::Value::Object` (backed by `BTreeMap` when the `preserve_order`
  feature is off) in ascending key order. This repository must therefore never
  enable `serde_json/preserve_order`; a comment in the new crate's
  `Cargo.toml` records that.
- `cap_std::fs_utf8::Dir::rename` within a single directory is atomic on the
  platforms Whitaker supports, so a temporary-file-then-rename write is never
  observed half-written.
- `dylint_linting::config` reads and deserializes the correct table from
  `dylint.toml`. Repository-owned logic around it — namespace selection,
  defaults, and precedence against the environment — is verified by VP-5
  against a `mockall` double of the reader trait, following the pattern already
  used in `crates/no_std_fs_operations/src/config.rs`.
- Verus and Kani, as pinned by `scripts/install-verus.sh` and
  `scripts/install-kani.sh`, are sound.

### What is deliberately not verified

The English text of the diagnostic messages is produced by
`whitaker_common`'s existing `format_*` functions, which roadmap 6.2.2 and
6.3.2 already covered with unit and behavioural tests. This plan asserts that
the emitter uses them unmodified, not that their wording is correct.

## Plan of work

### Stage A — understand and propose (no code changes)

Read, in order: `docs/brain-trust-lints-design.md` §SARIF output;
`crates/whitaker_sarif/src/lib.rs` and `model/`;
`crates/whitaker_clones_core/src/run0/emit.rs`;
`common/src/brain_type_metrics/diagnostic.rs` and `evaluation.rs`;
`common/src/brain_trait_metrics/diagnostic.rs`; `common/src/span.rs`;
`crates/no_std_fs_operations/src/config.rs` for the configuration-reader trait
pattern. Then obtain approval for this plan.

Stage A ends when the plan is approved.

### Stage B — red tests and open proof obligations

For each milestone below, write the failing test or the open proof goal first
and observe it fail for the stated reason, then implement. The milestone
descriptions name the red artefact explicitly.

### Stage C — implementation and verification together

Milestones `EP-M1` through `EP-M7`, in order. Each ends at a coherent,
validated repository state.

### Stage D — documentation, proof cleanup, and wider validation

Milestone `EP-M8`.

## Milestones and plateaus

### EP-M1 — deterministic SARIF model foundation

- Identifier and outcome: `whitaker_sarif` produces byte-stable JSON, and gains
  the two optional fields the brain trust emitter needs.
- Requirements and gaps: `BTS-REQ-05`, partially `BTS-REQ-04`.
- Changes:
  - `crates/whitaker_sarif/src/model/result.rs`: change
    `partial_fingerprints` from `HashMap<String, String>` to
    `BTreeMap<String, String>`; update the `skip_serializing_if` to
    `BTreeMap::is_empty`; update the doc example.
  - `crates/whitaker_sarif/src/builders/result_builder.rs`: same container
    change in the builder's field.
  - `crates/whitaker_sarif/src/merge.rs`: adjust the `ResultKey` construction
    if it depends on `HashMap` methods.
  - `crates/whitaker_sarif/src/model/descriptor.rs`: add
    `pub help: Option<MultiformatMessageString>` with
    `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  - `crates/whitaker_sarif/src/model/run.rs`: add
    `pub automation_details: Option<RunAutomationDetails>` with the same serde
    attributes, and define
    `pub struct RunAutomationDetails { pub id: String }`.
  - `crates/whitaker_sarif/src/builders/run_builder.rs`: add
    `with_automation_details(self, id: impl Into<String>) -> Self`.
  - `crates/whitaker_clones_core/src/run0/emit.rs` and any other consumer:
    update to the new container type. No behaviour change is expected.
- Red artefact: a new test in
  `crates/whitaker_sarif/tests/sarif_behaviour.rs` named
  `partial_fingerprints_serialize_in_key_order`, asserting that a result with
  fingerprint keys inserted as `zeta`, `alpha`, `mu` serializes with `alpha`
  first. Under `HashMap` this fails intermittently; run it with
  `cargo nextest run -p whitaker_sarif --no-capture` a few times, or construct
  the map with enough keys that the failure is reliable, and record the
  observed failure.
- Acceptance evidence: `partial_fingerprints_serialize_in_key_order` passes;
  the existing eight BDD scenarios in `crates/whitaker_sarif/tests/` still
  pass; `crates/whitaker_clones_core` tests still pass; no existing serialized
  form gained a field, verified by a test asserting that a clone-detector
  result serializes without `help` or `automationDetails` keys.
- Conformance check: the two added fields are optional and default to `None`,
  so `docs/whitaker-clone-detector-design.md`'s documented result shape is
  unchanged. No new dependency, no trust boundary change.
- Recovery: the milestone is a mechanical container swap plus two additive
  fields; revert with `git revert` if a consumer proves harder than expected.
- Remaining gaps: nothing brain-trust-specific yet.
- Compatibility decision: none required. `whitaker_sarif` is `publish = false`,
  pre-1.0, with one in-repo consumer.

### EP-M2 — fingerprint encoding and its Verus injectivity proof

- Identifier and outcome: a proven-injective component encoder and a stable
  fingerprint function exist in the new crate.
- Requirements and gaps: `BTS-REQ-05`; discharges `VP-1`.
- Changes:
  - Create `crates/whitaker_brain_sarif/` with `Cargo.toml` (edition 2024,
    `publish = false`, version `0.2.7`, `[lints] workspace = true`, and
    `[lints.rust] unexpected_cfgs = { level = "warn", check-cfg =
    ['cfg(kani)'] }`), and register the crate in `[workspace.dependencies]`.
  - `src/lib.rs` with a module-level `//!` doc comment.
  - `src/domain/fingerprint.rs`: `encode_components(components: &[&str]) ->
    Vec<u8>` (eight-byte big-endian length prefix per component, written with
    explicit shifts because `clippy::host_endian_bytes` is denied) and
    `subject_fingerprint(rule_id, file_uri, subject_kind, subject_name) ->
    String` returning lowercase hexadecimal SHA-256.
  - `verus/brain_trust_fingerprint.rs` with the round-trip lemma and the
    injectivity corollary.
  - `scripts/run-verus.sh`: add a `brain-trust` group and include the new file
    in `all`.
  - `Makefile`: add `verus-brain-trust`.
- Red artefact: the Verus file with `lemma_decode_encode_round_trip`'s body
  left as `assume(false);` replaced by nothing — Verus reports the
  postcondition unproven. Also
  `encoding_separates_components_a_delimiter_would_merge` in
  `src/domain/fingerprint_tests.rs`, written before the encoder exists.
- Acceptance evidence: `make verus-brain-trust` reports `0 errors`;
  `cargo nextest run -p whitaker_brain_sarif` passes; the delimiter-collision
  control test passes.
- Conformance check: no public interface outside the new crate; one new
  dependency edge onto `sha2`, already workspace-pinned, within tolerance.
- Recovery: the crate is additive; delete the directory and the script entry to
  revert.
- Remaining gaps: no findings, mapping, or emission yet.
- Compatibility decision: none required; the crate is new.

### EP-M3 — domain finding model, ordering, and English-only rendering

- Identifier and outcome: brain type and brain trait diagnostics can be
  converted into a common, ordered, English-rendered finding type.
- Requirements and gaps: `BTS-REQ-01`, `BTS-REQ-03`; discharges `VP-2`,
  `VP-7`.
- Changes:
  - `src/domain/finding.rs`: `SubjectKind` (`Type` | `Trait`),
    `FindingSeverity` (`Warning` | `Error`), `SubjectRef { kind, name,
    file_uri, span }`, and `BrainTrustFinding` carrying the subject, the
    severity, the rule identifier, the rendered English message, the measured
    values as an ordered property map, and the optional notes.
  - `src/domain/finding_from.rs`: `from_brain_type(diagnostic, subject) ->
    Option<BrainTrustFinding>` and `from_brain_trait(...)`, returning `None`
    for `Pass`.
  - `src/domain/ordering.rs`: `finding_key` and `finding_order`.
  - `src/domain/render.rs`: thin wrappers over `whitaker_common`'s
    `format_primary_message`, `format_note`, `format_help`, and
    `format_decomposition_note`. This module and the crate as a whole must
    not reference `i18n`.
  - `verus/brain_trust_ordering.rs` and its `run-verus.sh` entry.
  - `tests/features/english_only.feature` and
    `tests/english_only_behaviour.rs`.
- Red artefact: `finding_order_is_a_total_order` sub-lemmas open in Verus; the
  BDD scenario `Locale does not change SARIF message text`; and the fitness
  test `crate_sources_never_reference_localisation`, first observed failing
  against a deliberately inserted `Localizer` mention.
- Acceptance evidence: `make verus-brain-trust` reports `0 errors` for both
  proof files; the three English-only scenarios pass;
  `Pass` dispositions produce `None`, asserted by a parameterized test over all
  three disposition variants for each of the two subject kinds.
- Conformance check: `BTS-REQ-03` is now structurally enforced (no
  `fluent-templates` dependency in the new crate) as well as behaviourally
  tested.
- Recovery: additive within the new crate.
- Remaining gaps: nothing is serialized yet.
- Compatibility decision: none required.

Feature specification, `crates/whitaker_brain_sarif/tests/features/english_only.feature`:

```gherkin
Feature: SARIF messages are English regardless of locale

  Scenario: Welsh locale does not change SARIF output
    Given a denied brain type finding for "OrderProcessor"
    When the SARIF log is emitted with the locale set to "cy"
    Then the emitted bytes are identical to the "en-GB" emission

  Scenario: Scottish Gaelic locale does not change SARIF output
    Given a denied brain trait finding for "Repository"
    When the SARIF log is emitted with the locale set to "gd"
    Then the emitted bytes are identical to the "en-GB" emission

  Scenario: The message text is the English primary message
    Given a warned brain type finding for "Ledger"
    When the SARIF log is emitted
    Then the first result message equals the English primary message
```

### EP-M4 — pure mapping to a SARIF run, with snapshots

- Identifier and outcome: an ordered slice of findings maps to a complete,
  deterministic `whitaker_sarif::SarifLog`.
- Requirements and gaps: `BTS-REQ-01`, `BTS-REQ-04`; discharges `VP-4`,
  `VP-8`.
- Changes:
  - `src/domain/rules.rs`: `WHK101_ID`, `WHK102_ID`, `whk101_rule()`,
    `whk102_rule()`, `all_brain_trust_rules()`. Each descriptor sets `name`,
    `short_description`, `help` (the English guidance text), and `help_uri`
    pointing at `docs/brain-trust-lints-design.md`.
  - `src/mapping.rs`: `to_run(findings, tool) -> Result<Run, BrainSarifError>`
    and `to_log(findings, tool, automation_id) -> Result<SarifLog, _>`. Sorts
    with `finding_order`, builds one `SarifResult` per finding via
    `ResultBuilder`, attaches `partialFingerprints` under
    `whitakerBrainSubject`, attaches the measured values under
    `properties.whitakerBrainTrust`, and sets `automationDetails.id`.
  - `src/error.rs`: `BrainSarifError` via `thiserror`, wrapping
    `whitaker_sarif::SarifError` and adding typed variants for invalid subject
    identifiers and invalid output targets.
  - `src/mapping_tests.rs` with the six `insta` snapshots.
  - `tests/emission_properties.rs` with the `proptest` properties.
- Red artefact: the six snapshot tests, written before `to_run` exists, plus
  `emission_is_permutation_invariant`.
- Acceptance evidence: `cargo nextest run -p whitaker_brain_sarif` passes;
  `cargo insta test --check` reports no pending snapshots; the permutation and
  round-trip properties pass with the non-vacuity counters satisfied.
- Conformance check: rule identifiers do not collide with `WHK001`–`WHK003`,
  asserted by a test that intersects `whitaker_sarif::all_rules()` identifiers
  with `all_brain_trust_rules()` identifiers and expects an empty set.
- Recovery: additive.
- Remaining gaps: findings must still be collected and written.
- Compatibility decision: none required.

### EP-M5 — collector with dedup and ordering, plus the Kani harness

- Identifier and outcome: a collector accumulates findings, deduplicates by
  fingerprint, and returns them ordered.
- Requirements and gaps: `BTS-REQ-05`; discharges `VP-3`.
- Changes:
  - `src/domain/collector.rs`: `BrainTrustCollector` with `record`, `len`,
    `is_empty`, and `finish(self) -> Vec<BrainTrustFinding>`. Backed by a
    `BTreeMap` keyed on the ordering key so insertion order cannot leak.
  - `src/domain/collector_kani.rs` behind `#[cfg(kani)]`, with the bounded
    model over three findings, `kani::cover` reachability assertions, and
    `#[kani::unwind(5)]`.
  - `scripts/run-kani.sh`: add a `brain-trust` group listing the harnesses.
  - `Makefile`: add `kani-brain-trust`.
- Red artefact: `collector_keeps_first_of_duplicate_subjects` and
  `collector_returns_sorted_findings`, both written first; then the Kani
  harnesses, with the deliberate truncation mutation observed failing.
- Acceptance evidence: `make kani-brain-trust` reports
  `VERIFICATION:- SUCCESSFUL` for every harness including the cover checks;
  unit tests pass.
- Conformance check: the collector holds no I/O and no configuration, keeping
  the domain pure.
- Recovery: additive; if Kani proves intractable, follow the escalation rule in
  `Tolerances` rather than deleting the harness.
- Remaining gaps: no opt-in, no file output.
- Compatibility decision: none required.

### EP-M6 — settings resolution and the opt-in truth table

- Identifier and outcome: the emitter's on/off state and output directory are
  resolved by a pure function from an injected environment snapshot and
  optional file configuration.
- Requirements and gaps: `BTS-REQ-02`; discharges `VP-5`.
- Changes:
  - `src/settings.rs`: `SarifSettings { enabled, output_dir, tool_version }`,
    `EnvSnapshot { enable: Option<String>, dir: Option<String> }`,
    `FileSettings { enabled: Option<bool>, output_dir: Option<String> }`, and
    `resolve_settings(&EnvSnapshot, Option<&FileSettings>) ->
    Result<SarifSettings, SettingsError>`.
  - Precedence, documented and tested: the environment wins over file
    configuration, which wins over the default of disabled. Setting
    `WHITAKER_BRAIN_SARIF_DIR` to a non-empty path implies enabled unless
    `WHITAKER_BRAIN_SARIF` explicitly says otherwise. Booleans accept `1`,
    `true`, `0`, `false`, case-insensitively, after trimming; an empty value is
    treated as unset; anything else is a typed error.
  - `src/ports.rs`: `trait FileSettingsSource { fn load(&self) ->
    Result<Option<FileSettings>, SettingsError>; }` with a `mockall`
    double, and a `DylintFileSettingsSource` adapter that calls
    `dylint_linting::config` for the `brain_trust_sarif` namespace. The adapter
    is feature-gated so the crate builds without a compiler dependency.
  - `src/settings_tests.rs` with the exhaustive truth table.
- Red artefact: the truth table, written before `resolve_settings` exists.
- Acceptance evidence: every cell passes; the two error cells produce
  `SettingsError` values asserted with `googletest` matchers.
- Conformance check: no test mutates the process environment directly; the
  single adapter test that must read real environment variables uses
  `temp-env`'s guard.
- Recovery: additive.
- Remaining gaps: nothing is written to disk yet.
- Compatibility decision: none required.

### EP-M7 — ports, adapters, and end-to-end file emission

- Identifier and outcome: a caller records findings and, when enabled, a SARIF
  file appears at a predictable path; when disabled, nothing happens.
- Requirements and gaps: `BTS-REQ-01`, `BTS-REQ-02`; discharges `VP-6`.
- Changes:
  - `src/ports.rs`: `trait SarifSink { fn write(&self, unit: &UnitId, log:
    &SarifLog) -> Result<(), BrainSarifError>; }` with a `mockall` double.
  - `src/domain/unit.rs`: `UnitId`, a newtype over a non-empty string
    validated to contain no path separator, no `..`, and no character outside
    `[A-Za-z0-9_.-]`, with `TryFrom<&str>` and `AsRef<str>`.
  - `src/adapters/cap_std_sink.rs`: `CapStdSarifSink::new(dir: Dir)` writing
    `<unit>.sarif` by creating `<unit>.sarif.tmp`, writing pretty JSON with a
    trailing newline, syncing, and renaming. Uses `cap_std::fs_utf8` only.
  - `src/reporter.rs`: `BrainTrustSarifReporter`, an application service with
    `disabled()`, `new(settings, sink)`, `record_brain_type`,
    `record_brain_trait`, and `finish(self, unit) -> Result<Option<Utf8PathBuf>,
    BrainSarifError>` returning `Ok(None)` when disabled or when there are no
    findings.
  - `src/paths.rs` or an addition to `whitaker_sarif::paths`: the default
    output directory `target/whitaker/brain-trust` and a
    `brain_trust_dir(target_dir)` helper. Prefer extending
    `whitaker_sarif::paths` so the layout stays in one place.
  - `src/test_support.rs` behind `#[cfg(any(test, feature = "test-support"))]`:
    an in-memory sink and finding builders.
  - `tests/features/brain_trust_sarif.feature` and
    `tests/brain_trust_sarif_behaviour.rs` for the end-to-end scenarios.
- Red artefact: the end-to-end scenario `An enabled reporter writes a SARIF
  file`, written before the sink exists.
- Acceptance evidence: running the behavioural suite creates and then reads
  back a real file under a `tempfile` directory whose contents parse as a
  `SarifLog` with one run and the expected results; the disabled scenario
  asserts the directory is still empty; the mock sink asserts `times(0)` when
  disabled and `times(1)` when enabled with at least one finding.
- Conformance check: `make lint` runs the Whitaker suite over the new crate;
  `no_std_fs_operations` must pass without adding the crate to
  `excluded_crates`. If it does not, that is a design failure, not a
  configuration problem — fix the code.
- Recovery: additive; the sink writes to a temporary file first, so a failed
  write leaves no partial output.
- Remaining gaps: documentation.
- Compatibility decision: none required.

Feature specification,
`crates/whitaker_brain_sarif/tests/features/brain_trust_sarif.feature`:

```gherkin
Feature: Opt-in SARIF emission for brain trust findings

  Scenario: An enabled reporter writes a SARIF file
    Given SARIF emission is enabled for a temporary output directory
    And a denied brain type finding for "OrderProcessor" in "src/orders.rs"
    When the reporter finishes for compilation unit "my_crate"
    Then a file named "my_crate.sarif" exists in the output directory
    And the file parses as a SARIF 2.1.0 log with one run
    And the run contains one result with rule identifier "WHK101"
    And the result level is "error"

  Scenario: A disabled reporter writes nothing
    Given SARIF emission is disabled
    And a denied brain type finding for "OrderProcessor" in "src/orders.rs"
    When the reporter finishes for compilation unit "my_crate"
    Then the output directory is empty

  Scenario: A passing subject produces no result
    Given SARIF emission is enabled for a temporary output directory
    And a passing brain trait evaluation for "Repository"
    When the reporter finishes for compilation unit "my_crate"
    Then the output directory is empty

  Scenario: The same findings recorded in either order produce the same file
    Given SARIF emission is enabled for a temporary output directory
    And two warned brain trait findings recorded in ascending name order
    When the reporter finishes for compilation unit "first_unit"
    And the same findings are recorded in descending name order
    And the reporter finishes for compilation unit "second_unit"
    Then the two files differ only in their automation identifier

  Scenario: An invalid compilation unit identifier is rejected
    Given SARIF emission is enabled for a temporary output directory
    And a warned brain type finding for "Ledger" in "src/ledger.rs"
    When the reporter finishes for compilation unit "../escape"
    Then the reporter reports an invalid unit identifier error
    And the output directory is empty
```

### EP-M8 — documentation, ADR, roadmap, and full gate run

- Identifier and outcome: the change is documented everywhere the repository
  requires, and every gate passes.
- Requirements and gaps: closes out all five `BTS-REQ` items.
- Changes:
  - `docs/adr-005-brain-trust-sarif-emission.md`, following the required ADR
    sections. It records: the separate-crate decision; per-unit files versus a
    shared file; the `BTreeMap` determinism fix; length-prefixed encoding and
    why delimiter separation was rejected; the rule identifier block; and the
    English-only constraint.
  - `docs/brain-trust-lints-design.md` §SARIF output: replace the four-line
    "planned approach" with the delivered design, and reference the ADR.
  - `docs/users-guide.md`: a new section describing the environment variables,
    the `dylint.toml` section, the output location, the rule identifiers, and
    the English-only guarantee.
  - `docs/developers-guide.md`: the internal conventions — the ports and
    adapters layout, how to add a new brain trust rule, the determinism rules
    (no `HashMap` in serialized models, no `serde_json/preserve_order`), and
    how to run the new proof targets.
  - `docs/repository-layout.md` and `docs/contents.md`: register the new crate
    and the new ADR.
  - `docs/roadmap.md`: mark 6.5.1 as done.
  - `typos.local.toml` if any new term trips the spelling gate, followed by
    `make spelling-config-write`.
- Acceptance evidence: `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, `make verus-brain-trust`, and
  `make kani-brain-trust` all succeed, with output captured to
  `/tmp/<action>-whitaker-6-5-1-collect-brain-trust-diagnostics-into-sarif-emitter.out`.
- Conformance check: every `BTS-REQ` identifier maps to a passing test named in
  `Conformance basis`; the design document no longer describes the work as
  planned; no upstream assumption is left falsified and unrecorded.
- Recovery: documentation-only; safe to iterate.
- Remaining gaps: the `brain_type` and `brain_trait` Dylint crates remain
  unimplemented, as does merging per-unit SARIF files. Both are recorded in
  `Outcomes & retrospective` and belong to later roadmap items.
- Compatibility decision: none required.

## Concrete steps

Run everything from the repository root,
`/home/leynos/.lody/repos/github---leynos---whitaker/worktrees/b09a23bd-30c3-4848-9f03-29d31d2244b2`,
on branch `6-5-1-collect-brain-trust-diagnostics-into-sarif-emitter`.

Tee long output to a log so nothing is truncated:

```bash
ACTION=test
LOG="/tmp/${ACTION}-whitaker-$(git branch --show-current).out"
make "${ACTION}" 2>&1 | tee "${LOG}"
```

Focused test runs during development:

```bash
cargo nextest run -p whitaker_brain_sarif 2>&1 | tee /tmp/nextest-brain-sarif.out
cargo nextest run -p whitaker_sarif -p whitaker_clones_core
```

Expected transcript shape for a green focused run:

```plaintext
    Starting 41 tests across 4 binaries
        PASS [   0.004s] whitaker_brain_sarif domain::fingerprint_tests::encoding_separates_components_a_delimiter_would_merge
        PASS [   0.006s] whitaker_brain_sarif mapping_tests::snapshot_brain_type_deny
...
     Summary [   0.412s] 41 tests run: 41 passed, 0 skipped
```

Proof sidecars:

```bash
make verus-brain-trust 2>&1 | tee /tmp/verus-brain-trust.out
make kani-brain-trust  2>&1 | tee /tmp/kani-brain-trust.out
```

Expected Verus tail:

```plaintext
verification results:: 7 verified, 0 errors
```

Expected Kani tail per harness:

```plaintext
VERIFICATION:- SUCCESSFUL
```

Snapshot review:

```bash
cargo insta test --package whitaker_brain_sarif
cargo insta review
```

Full gate sequence before each commit. Run these **sequentially**, never in
parallel, so the build cache is used:

```bash
make check-fmt && make typecheck && make lint && make test
```

Manual demonstration of the user-visible behaviour, using the end-to-end test
binary as the driver (there is no lint crate to run yet):

```bash
WHITAKER_BRAIN_SARIF=1 cargo nextest run -p whitaker_brain_sarif \
    -E 'test(/brain_trust_sarif_behaviour/)' --no-capture
```

## Validation and acceptance

Acceptance is phrased as behaviour.

1. With no environment variable set and no `dylint.toml` section, a caller that
   records ten denied findings and calls `finish` receives `Ok(None)` and the
   output directory contains zero files. Test:
   `disabled_reporter_writes_nothing`.
2. With `WHITAKER_BRAIN_SARIF=1` and an output directory, the same caller
   receives `Ok(Some(path))`, and reading `path` yields JSON whose `$schema` is
   the SARIF 2.1.0 schema URI, whose `version` is `"2.1.0"`, and whose
   `runs[0].results` has ten entries sorted by file then line. Test:
   `enabled_reporter_writes_sorted_results`.
3. Recording the same findings in reverse order produces byte-identical file
   contents apart from the automation identifier. Test:
   `emission_is_permutation_invariant` and the fourth BDD scenario.
4. Setting the Whitaker locale to `cy` changes no byte. Tests: the three
   English-only scenarios.
5. A `Pass` disposition never appears in the output. Test:
   `passing_subject_produces_no_result`.
6. A compilation unit identifier containing `..` or a path separator is
   rejected before any file is created. Test:
   `invalid_unit_identifier_is_rejected`.

Red-Green-Refactor evidence to record for each milestone:

- Red: the named test or open proof goal, the exact command, and the observed
  failure message. For proofs, the Verus error or the Kani counter-example.
- Green: the same command passing after the minimal implementation.
- Refactor: `make check-fmt && make typecheck && make lint && make test`
  passing after cleanup.

Quality criteria (what "done" means):

- Tests: `make test` passes with no new ignored or skipped tests. Every new
  public item has a Rustdoc example that runs as a doctest.
- Verification: `VP-1` and `VP-2` discharged by Verus with `0 errors`; `VP-3`
  discharged by Kani with `VERIFICATION:- SUCCESSFUL` including both cover
  checks; `VP-4` through `VP-8` discharged by their named test artefacts, each
  with its non-vacuity check recorded.
- Lint and typecheck: `make lint` and `make typecheck` pass with warnings
  denied. The new crate is not added to `dylint.toml`'s `excluded_crates`.
- Documentation: `make markdownlint` and `make nixie` pass.
- Performance: no benchmark is required. The disabled path must perform zero
  allocations per recorded finding, asserted structurally by the collector
  being absent from the disabled reporter variant rather than by measurement.
- Security: no new network access, no new process spawning. The sink writes
  only inside the capability-scoped directory it is handed.

Quality method (how we check): the gate sequence above, run sequentially, with
output captured to `/tmp` for review. Delegate full gate runs to the
`scrutineer` sub-agent rather than running them in the planning context.

## Idempotence and recovery

Every step is re-runnable. The sink writes to `<unit>.sarif.tmp` and renames,
so a repeated run overwrites cleanly and an interrupted run leaves no partial
`.sarif` file. Deleting the output directory between runs is always safe;
nothing reads it back except the tests.

The proof sidecars install their toolchains into a cache directory and are safe
to re-run. If `make kani-brain-trust` is interrupted, re-run it; the install
step is idempotent (note the warm-cache trap fixed during roadmap 6.4.6).

To abandon the work entirely: `git checkout main -- docs/roadmap.md` and delete
`crates/whitaker_brain_sarif/`, `verus/brain_trust_*.rs`, and the new Makefile
targets. `EP-M1`'s changes to `whitaker_sarif` are independently valuable and
can be kept.

## Artefacts and notes

Illustrative shape of the emitted document, for one denied brain type finding:

```json
{
  "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "whitaker-brain-trust",
          "version": "0.2.7",
          "informationUri": "https://github.com/leynos/whitaker",
          "rules": [
            {
              "id": "WHK101",
              "name": "BrainType",
              "shortDescription": { "text": "Type has grown into a brain class" },
              "help": { "text": "Split the type along the suggested method clusters." },
              "helpUri": "https://github.com/leynos/whitaker/blob/main/docs/brain-trust-lints-design.md#brain_type-signals"
            }
          ]
        }
      },
      "automationDetails": { "id": "whitaker/brain-trust/my_crate" },
      "results": [
        {
          "ruleId": "WHK101",
          "level": "error",
          "message": {
            "text": "`OrderProcessor` has WMC=118 and LCOM4=4, with 2 brain methods."
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": { "uri": "src/orders.rs" },
                "region": { "startLine": 42, "startColumn": 1, "endLine": 310, "endColumn": 2 }
              }
            }
          ],
          "partialFingerprints": {
            "whitakerBrainSubject": "6f1c…"
          },
          "properties": {
            "whitakerBrainTrust": {
              "brainMethods": [
                { "cognitiveComplexity": 41, "linesOfCode": 96, "name": "reconcile" },
                { "cognitiveComplexity": 33, "linesOfCode": 88, "name": "settle" }
              ],
              "disposition": "deny",
              "foreignReach": 14,
              "lcom4": 4,
              "notes": ["Consider extracting `reconcile` and `settle` into a helper struct."],
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

The `properties.whitakerBrainTrust` keys are alphabetically ordered because
`serde_json::Value::Object` is a `BTreeMap`. That ordering is load-bearing for
determinism; do not enable `serde_json/preserve_order` anywhere in the
workspace.

## Interfaces and dependencies

### New crate

`crates/whitaker_brain_sarif/Cargo.toml`:

```toml
[package]
name = "whitaker_brain_sarif"
version = "0.2.7"
edition = "2024"
publish = false
description = "Opt-in SARIF 2.1.0 emission for Whitaker brain trust findings"
license.workspace = true
repository.workspace = true

[features]
default = []
test-support = []
# Enables the `dylint.toml` configuration adapter, which requires the Dylint
# driver environment. Off by default so the crate builds and tests standalone.
dylint-config = ["dep:dylint_linting"]

[dependencies]
camino = { workspace = true }
cap-std = { workspace = true }
serde = { workspace = true }
# Never enable `serde_json/preserve_order`: object key order is load-bearing
# for byte-identical output.
serde_json = { workspace = true }
sha2 = { workspace = true }
thiserror = { workspace = true }
whitaker-common = { workspace = true }
whitaker_sarif = { workspace = true }
dylint_linting = { workspace = true, optional = true }

[dev-dependencies]
googletest = "0.15"
insta = { workspace = true }
mockall = { workspace = true }
pretty_assertions = "1"
proptest = { workspace = true }
rstest = { workspace = true }
rstest-bdd = { workspace = true }
rstest-bdd-macros = { workspace = true }
temp-env = { workspace = true }
tempfile = { workspace = true }
whitaker_test_macros = { workspace = true }

[lints]
workspace = true

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }
```

Note: `googletest` and `pretty_assertions` are not yet in
`[workspace.dependencies]`. Add them there at `EP-M4` with caret requirements
and reference them as `{ workspace = true }`, matching the repository's
dependency policy. Confirm `insta`, `mockall`, `proptest`, `tempfile`, and
`temp-env` pins already exist before use.

### Required signatures at the end of the work

In `crates/whitaker_brain_sarif/src/domain/fingerprint.rs`:

```rust
/// Encodes components with an injective, length-prefixed framing.
#[must_use]
pub fn encode_components(components: &[&str]) -> Vec<u8>;

/// Returns the lowercase hexadecimal SHA-256 fingerprint of a subject.
#[must_use]
pub fn subject_fingerprint(
    rule_id: &str,
    file_uri: &str,
    subject_kind: SubjectKind,
    subject_name: &str,
) -> String;
```

In `crates/whitaker_brain_sarif/src/domain/finding.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubjectKind { Type, Trait }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectRef {
    kind: SubjectKind,
    name: String,
    file_uri: String,
    span: whitaker_common::span::SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrainTrustFinding { /* private fields */ }

impl BrainTrustFinding {
    #[must_use]
    pub fn from_brain_type(
        diagnostic: &whitaker_common::BrainTypeDiagnostic,
        subject: SubjectRef,
    ) -> Option<Self>;

    #[must_use]
    pub fn from_brain_trait(
        diagnostic: &whitaker_common::BrainTraitDiagnostic,
        subject: SubjectRef,
    ) -> Option<Self>;
}
```

In `crates/whitaker_brain_sarif/src/ports.rs`:

```rust
/// Writes a completed SARIF log for one compilation unit.
pub trait SarifSink {
    /// # Errors
    ///
    /// Returns an error when the log cannot be serialized or written.
    fn write(&self, unit: &UnitId, log: &SarifLog) -> Result<(), BrainSarifError>;
}

/// Loads optional file-based SARIF settings.
pub trait FileSettingsSource {
    /// # Errors
    ///
    /// Returns an error when the configuration exists but cannot be parsed.
    fn load(&self) -> Result<Option<FileSettings>, SettingsError>;
}
```

In `crates/whitaker_brain_sarif/src/reporter.rs`:

```rust
pub struct BrainTrustSarifReporter<S: SarifSink> { /* private fields */ }

impl<S: SarifSink> BrainTrustSarifReporter<S> {
    #[must_use]
    pub fn new(settings: SarifSettings, sink: S) -> Self;

    pub fn record_brain_type(
        &mut self,
        diagnostic: &whitaker_common::BrainTypeDiagnostic,
        subject: SubjectRef,
    );

    pub fn record_brain_trait(
        &mut self,
        diagnostic: &whitaker_common::BrainTraitDiagnostic,
        subject: SubjectRef,
    );

    /// # Errors
    ///
    /// Returns an error when the unit identifier is invalid or the sink fails.
    pub fn finish(self, unit: &UnitId) -> Result<Option<Utf8PathBuf>, BrainSarifError>;
}
```

### Changes to `whitaker_sarif`

```rust
// crates/whitaker_sarif/src/model/result.rs
pub struct SarifResult {
    // ...
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub partial_fingerprints: BTreeMap<String, String>,
    // ...
}

// crates/whitaker_sarif/src/model/descriptor.rs
pub struct ReportingDescriptor {
    // ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<MultiformatMessageString>,
}

// crates/whitaker_sarif/src/model/run.rs
pub struct RunAutomationDetails { pub id: String }

pub struct Run {
    // ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation_details: Option<RunAutomationDetails>,
}
```

### Signposted documentation and skills

Read before or during the work:

- `AGENTS.md` — code style, dependency, error-handling, and testing policy.
- `docs/brain-trust-lints-design.md` — the upstream design, especially §SARIF
  output and §Diagnostic output.
- `docs/whitaker-clone-detector-design.md` §Rules, §Result mapping, §Runs — the
  precedent this plan follows and deviates from.
- `docs/whitaker-dylint-suite-design.md` — suite-wide lint conventions.
- `docs/rust-testing-with-rstest-fixtures.md` — fixture design and
  parameterization.
- `docs/rstest-bdd-users-guide.md` — the Gherkin runner used for the feature
  files above.
- `docs/rust-doctest-dry-guide.md` — keeping Rustdoc examples non-repetitive
  while still exercising the API.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` — the
  vocabulary the brain trust lints are built on.
- `docs/reliable-testing-in-rust-via-dependency-injection.md` — the port and
  double pattern used for the sink and the settings source.
- `docs/documentation-style-guide.md` — ADR shape and Markdown rules.
- `docs/repository-layout.md` and `docs/contents.md` — where new files are
  registered.

Skills to load:

- `leta` for symbol navigation; prefer `leta show`, `leta refs`, and
  `leta calls` over reading files or grepping for symbols.
- `hexagonal-architecture` for the port and adapter boundaries; the point is to
  keep the domain free of SARIF, configuration, and filesystem concerns, not to
  add layers for their own sake.
- `kani` for the collector harness, particularly the unwind off-by-one rule and
  the heap-collection cliff.
- `verus` for the two proofs, particularly triggers, `assert ... by { }`
  scoping, and `broadcast use vstd::seq::group_seq_axioms;`.
- `rust-unit-testing` for `rstest`, `googletest`, `pretty_assertions`, and
  `insta` conventions.
- `proptest` for generator design and shrinking discipline.
- `rust-errors` for the `thiserror` enum shape at the crate boundary.
- `arch-crate-design` for the new crate's feature flags and public surface.
- `arch-decision-records` for the Y-statement ADR content.
- `execplans` for keeping this document current.

### External references

- OASIS, *Static Analysis Results Interchange Format (SARIF) Version 2.1.0
  Errata 01*,
  <https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/sarif-v2.1.0-errata01-os-complete.html>.
  Relevant sections: §3.27.17 `partialFingerprints`, §3.17 `runAutomationDetails`,
  §3.49 `reportingDescriptor`.
- GitHub, *SARIF support for code scanning*,
  <https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support>.
  Defines the ingested subset, the `automationDetails.id` category convention,
  and confirms that only `primaryLocationLineHash` is read from
  `partialFingerprints`.
- GitHub, *SARIF results exceed one or more limits*,
  <https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/troubleshoot-sarif-uploads/results-exceed-limit>.
  Limits: 20 runs per file, 25,000 results per run (top 5,000 retained),
  25,000 rules per run, 20 tags per rule. Per-unit files keep Whitaker well
  inside these.
- GitHub Changelog, *Code scanning will stop combining multiple SARIF runs
  uploaded in the same SARIF file* (21 July 2025),
  <https://github.blog/changelog/2025-07-21-code-scanning-will-stop-combining-multiple-sarif-runs-uploaded-in-the-same-sarif-file/>.
  The reason this plan emits one run per file.
