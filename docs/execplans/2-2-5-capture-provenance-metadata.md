# Capture provenance metadata automatically in TEI headers (roadmap 2.2.5)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

Roadmap item 2.2.5 requires automatic provenance capture in TEI headers so each
canonical content artefact records where content came from, when it was
ingested/generated, which source priorities were applied, and who reviewed it.
The same provenance policy must apply to both source-ingestion flows and script
generation flows.

After implementation, users and reviewers can inspect a TEI header and answer
four questions without external logs:

- Which sources contributed and in what priority order.
- Which ingestion or generation operation produced this artefact.
- When that operation happened.
- Which reviewer identity was assigned to the review state.

## Constraints

- Follow hexagonal architecture boundaries.
  - Domain defines provenance policy and data structures.
  - Application layer orchestrates provenance assembly.
  - Adapters provide time, identity lookup, persistence, and TEI serialisation.
- Domain and application code must not depend on TEI XML libraries directly.
  TEI formatting lives in outbound adapters only.
- Behaviour must be consistent across both flows:
  - source ingestion
  - script generation (including future script generation if not yet shipped)
- Reviewer identities must be captured via a stable identifier in TEI metadata,
  with optional display name where available.
- Existing TEI consumers must remain compatible; required existing header fields
  cannot be removed.
- Tests must include:
  - unit tests with `pytest`
  - behavioural tests with `pytest-bdd`
- Required quality gates must pass before completion:
  - `make check-fmt`
  - `make typecheck`
  - `make lint`
  - `make test`
- Documentation updates are mandatory:
  - design document decision record update
  - `docs/users-guide.md`
  - `docs/developers-guide.md`
- On feature completion, mark roadmap entry 2.2.5 as done.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 15 files or 700 net lines, stop and
  escalate with a narrower phased rollout.
- Data model: if TEI schema changes require introducing new required fields for
  all historical artefacts, stop and escalate with migration options.
- Interface: if public APIs for ingestion or script generation require breaking
  signature changes, stop and escalate with compatibility options.
- Dependencies: if a new external dependency is needed for TEI handling,
  identity, or timestamp logic, stop and escalate before adding it.
- Test stability: if quality gates fail after 3 repair iterations, stop and
  escalate with logs and failure analysis.
- Ambiguity: if roadmap wording and repository state conflict materially (for
  example, missing referenced docs or missing roadmap section), stop and
  escalate before implementation edits.

## Risks

- Risk: The repository roadmap and the requested roadmap section may diverge,
  causing uncertain acceptance criteria.
  - Severity: high
  - Likelihood: medium
  - Mitigation: resolve canonical roadmap source in Stage A before coding.

- Risk: Ingestion and script generation may currently build TEI headers through
  separate code paths, increasing drift risk.
  - Severity: high
  - Likelihood: high
  - Mitigation: introduce a shared provenance assembly application service used
    by both flows.

- Risk: Reviewer identity availability may vary by execution context
  (automated, human-reviewed, or missing auth context).
  - Severity: medium
  - Likelihood: high
  - Mitigation: define explicit fallback semantics in the domain model
    (`unknown`, `system`, or configured service account) and test each case.

- Risk: Timestamp consistency may vary by timezone or clock source.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: define UTC ISO-8601 output contract and source timestamps from
    a single clock port.

- Risk: Existing TEI fixtures may not include new provenance fields, causing
  broad test fixture churn.
  - Severity: medium
  - Likelihood: high
  - Mitigation: add fixture builders and shared assertions to centralise
    expected provenance blocks.

## Progress

- [x] (2026-02-18 00:00Z) Draft ExecPlan authored for roadmap item 2.2.5.
- [ ] Stage A: reconcile roadmap/doc references and baseline current TEI header
  flow.
- [ ] Stage B: design provenance domain model and ports.
- [ ] Stage C: implement shared provenance assembly service in application
  layer.
- [ ] Stage D: wire ingestion adapter path.
- [ ] Stage E: wire script generation adapter path.
- [ ] Stage F: add unit tests (`pytest`) and behavioural tests (`pytest-bdd`).
- [ ] Stage G: update design doc, users guide, developers guide, and roadmap
  status.
- [ ] Stage H: run full quality gates and record evidence.

## Surprises & discoveries

- Observation: Referenced documents from the request were not found in the
  current repository snapshot.
  - Evidence: lookup for
    `docs/episodic-podcast-generation-system-design.md`,
    `docs/async-sqlalchemy-with-pg-and-falcon.md`,
    `docs/testing-async-falcon-endpoints.md`,
    `docs/testing-sqlalchemy-with-pytest-and-py-pglite.md`, and
    `docs/agentic-systems-with-langgraph-and-celery.md` returned no matches.
  - Impact: Stage A must confirm authoritative paths before implementation.

## Decision Log

- Decision: Use a single shared provenance assembly service invoked by both
  ingestion and script-generation use cases.
  - Rationale: Prevents divergence and enforces one policy for provenance
    construction while preserving adapter isolation.
  - Date/Author: 2026-02-18 / Codex.

- Decision: Treat roadmap/doc mismatch as a blocking ambiguity to resolve at
  Stage A before implementation changes.
  - Rationale: Prevents delivery against the wrong acceptance target.
  - Date/Author: 2026-02-18 / Codex.

## Outcomes & retrospective

Not started. This section will be completed after implementation and validation
finish.

## Context and orientation

This plan introduces provenance metadata capture into TEI header creation while
preserving hexagonal architecture.

Conceptual module responsibilities:

- Domain:
  - `ProvenanceRecord`, `SourcePriority`, `ReviewerIdentity`,
    `IngestionTimestamp` value objects.
  - validation rules (non-empty source identifiers, monotonic priority order,
    UTC timestamp contract).
- Application:
  - `CaptureProvenance` use-case service assembling provenance for a TEI
    artefact from request context and driven ports.
- Driven ports:
  - `ClockPort` (current UTC timestamp)
  - `ReviewerIdentityPort` (resolve reviewer principal)
  - `SourcePriorityPort` (resolve source priority map)
- Adapters:
  - ingestion adapter maps source payload metadata into domain request.
  - script generation adapter maps generation context into same domain request.
  - TEI adapter serialises `ProvenanceRecord` into TEI header elements.

The implementation should keep TEI element naming and placement under one
adapter path so both workflows emit identical provenance sections.

## Plan of work

Stage A: alignment and baseline (no behaviour changes)

- Confirm the canonical roadmap entry location and wording for item 2.2.5.
- Confirm the canonical design document path; if the referenced design document
  is absent, agree the replacement path before coding.
- Locate all current TEI header construction entry points for:
  - ingestion
  - script generation
- Record current behaviour and existing tests to preserve backwards
  compatibility.

Go/no-go: proceed only when acceptance scope is unambiguous.

Stage B: domain and port design

- Define a provenance domain model representing:
  - source priorities (ordered list, highest first)
  - ingestion or generation timestamp (UTC)
  - reviewer identity (stable ID, optional display name)
  - operation type (`ingestion` or `script_generation`) for provenance context
- Define driven ports for clock, reviewer identity resolution, and source
  priority resolution.
- Add design decision notes to the design document describing:
  - why a shared provenance service is required
  - why ports are owned by the domain/application boundary
  - how fallback reviewer identity is handled

Validation for Stage B:

- Domain unit tests (`pytest`) cover value-object validation and normalisation.

Stage C: shared application service

- Implement a single `CaptureProvenance` application service to assemble
  provenance metadata from use-case input and driven ports.
- Ensure output is adapter-neutral (plain domain/application structures, no XML
  dependencies).
- Add unit tests (`pytest`) for orchestration:
  - default timestamp usage
  - explicit source-priority ordering
  - missing reviewer identity fallback
  - operation-type differentiation between ingestion and script generation

Validation for Stage C:

- `pytest` unit suite passes for service behaviour.

Stage D: ingestion flow integration

- Update ingestion use-case orchestration to call `CaptureProvenance`.
- Pass resulting provenance structure into TEI header rendering adapter.
- Ensure persisted/generated TEI from ingestion now includes provenance block.

Validation for Stage D:

- Behaviour scenario (`pytest-bdd`): ingest from multiple sources and verify
  TEI header contains ordered source priority, UTC timestamp, and reviewer ID.

Stage E: script generation flow integration

- Integrate the same `CaptureProvenance` service into script generation path.
- Ensure script generation TEI headers encode equivalent provenance fields with
  operation type `script_generation`.

Validation for Stage E:

- Behaviour scenario (`pytest-bdd`): generate script and verify TEI header has
  source priority, generation timestamp, reviewer identity, and operation type.

Stage F: behavioural and regression hardening

- Add or update shared TEI header assertions to avoid duplicate fixture logic.
- Add negative-path tests:
  - unknown reviewer context
  - missing priority configuration
  - single-source ingestion
- Ensure old TEI fixtures either migrate or are versioned with clear expected
  differences.

Validation for Stage F:

- `pytest` and `pytest-bdd` suites cover both positive and fallback paths.

Stage G: documentation and roadmap updates

- Update design document with final decision record and architecture rationale.
- Update `docs/users-guide.md` with user-facing behaviour:
  - where provenance appears in TEI headers
  - what reviewer identity semantics users should expect
- Update `docs/developers-guide.md` with internal contracts:
  - provenance service responsibilities
  - port interfaces and adapter obligations
  - test strategy for keeping ingestion and script-generation parity
- Mark roadmap item 2.2.5 as done after all quality gates pass.

Validation for Stage G:

- Docs accurately describe delivered behaviour and interfaces.

Stage H: full validation and evidence capture

- Run required gates with log capture via `tee` and `set -o pipefail`.
- Record command outputs in plan notes for future traceability.

Go/no-go for completion: only complete when all gates pass and roadmap status
is updated.

## Concrete steps

Run from repository root:

1. Baseline discovery and scope confirmation

    rg -n "2\.2\.5|Canonical content foundation|provenance|TEI" docs/
    rg -n "TEI|header|ingest|script" src tests docs/

2. Implement domain/application/adapter changes per stages B-E.

3. Add tests:

    pytest tests/unit
    pytest tests/behaviour

4. Run required quality gates with captured logs:

    set -o pipefail; make check-fmt 2>&1 | tee /tmp/2-2-5-check-fmt.log
    set -o pipefail; make typecheck 2>&1 | tee /tmp/2-2-5-typecheck.log
    set -o pipefail; make lint 2>&1 | tee /tmp/2-2-5-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/2-2-5-test.log

5. Update documentation and roadmap completion status:

    - update design doc decision section
    - update `docs/users-guide.md`
    - update `docs/developers-guide.md`
    - mark roadmap item 2.2.5 as done

## Validation and acceptance

Feature acceptance (behavioural):

- Ingestion-generated TEI headers include provenance metadata for source
  priority, timestamp, and reviewer identity.
- Script-generation TEI headers include the same provenance metadata and apply
  the same ordering/identity rules.
- Timestamps are UTC and deterministic in tests via injected clock port.
- Reviewer fallback semantics are explicit and test-covered.

Quality gates:

- Unit tests: `pytest tests/unit` passes.
- Behavioural tests: `pytest tests/behaviour` (pytest-bdd scenarios) passes.
- Formatting: `make check-fmt` passes.
- Type checking: `make typecheck` passes.
- Linting: `make lint` passes.
- Full tests: `make test` passes.

Documentation acceptance:

- Design decisions are recorded in the design document.
- End-user behaviour is reflected in `docs/users-guide.md`.
- Internal interfaces/practices are reflected in `docs/developers-guide.md`.
- Roadmap item 2.2.5 is marked done after implementation completion.

## Idempotence and recovery

- Staged implementation is idempotent: each stage can be re-run safely.
- If a stage fails validation, revert only that stage's changes and rerun its
  tests before proceeding.
- Keep ingestion and script-generation updates in separate commits when
  possible, then merge with a final documentation/roadmap commit after all
  checks pass.

## Artifacts and notes

Capture and retain these artefacts during implementation:

- `/tmp/2-2-5-check-fmt.log`
- `/tmp/2-2-5-typecheck.log`
- `/tmp/2-2-5-lint.log`
- `/tmp/2-2-5-test.log`
- sample TEI outputs from ingestion and script generation showing provenance
  blocks

## Interfaces and dependencies

Required interfaces (names may vary by existing code conventions, but
responsibilities are fixed):

- `CaptureProvenance` application service:
  - input: operation context (ingestion/script generation), source list,
    optional reviewer context
  - output: normalised provenance structure independent of TEI formatting
- `ClockPort`:
  - provides current UTC timestamp for deterministic substitution in tests
- `ReviewerIdentityPort`:
  - resolves stable reviewer identity for TEI metadata
- `SourcePriorityPort`:
  - returns ordered source-priority mapping used by provenance assembly
- TEI header adapter:
  - translates provenance structure into canonical TEI header fields

No new dependency should be introduced unless required by an escalation decision
under `Tolerances`.

## Revision note

- 2026-02-18: Initial draft created for roadmap item 2.2.5 using execplans
  format and hexagonal architecture constraints; added explicit handling for
  missing referenced docs and roadmap ambiguity.
