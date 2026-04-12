# Add an explicit manual force switch for rolling dependency-binary rebuilds

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

The rolling-release workflow already rebuilds dependency binaries on any manual
`workflow_dispatch` run, but it does so implicitly. That makes it impossible to
tell from the dispatch form or the workflow contract whether a manual run is
intended to rebuild third-party dependency binaries or merely to republish the
current rolling assets. It also means the workflow cannot express the real
operator intent for the recovery case: "force a rebuild now because a previous
dependency-binary build failed".

After this change, the GitHub Actions UI for
`.github/workflows/rolling-release.yml` will expose a boolean manual input such
as `force_dependency_binary_rebuild`. A push to `main` will continue to rebuild
dependency binaries automatically when `installer/dependency-binaries.toml`
changes. A manual run will rebuild those dependency binaries only when the
operator explicitly sets that input to `true`; otherwise the workflow will skip
the rebuild job and continue to reuse or restore the existing dependency
archives. The behaviour will be protected by workflow contract tests and
documented for contributors in `docs/developers-guide.md`.

Success is observable by:

1. Opening the `Rolling Release` workflow in GitHub and seeing a manual
   `workflow_dispatch` input that clearly states whether dependency binaries
   should be forcibly rebuilt.
2. Running the workflow tests and seeing assertions that the manual input
   exists and that the change-detection job reads
   `github.event.inputs.force_dependency_binary_rebuild` instead of rebuilding
   on every manual run.
3. Running the full local quality gates and seeing them all pass after the
   workflow, tests, and docs are updated.

## Constraints

- Keep the change narrowly focused on
  `.github/workflows/rolling-release.yml`,
  `tests/workflows/test_rolling_release_workflow.py`, the supporting workflow
  fixtures if needed, and the relevant contributor documentation.
- Preserve the existing push behaviour on `main`: a change to
  `installer/dependency-binaries.toml` must still trigger dependency-binary
  rebuilds automatically.
- Preserve the current recovery path for missing dependency archives:
  when dependency binaries are not rebuilt, the publish job must still be able
  to restore matching archives from the existing `rolling` release.
- Do not change the tagged `release.yml` workflow in this task. The user asked
  specifically for the rolling-release recovery path.
- Prefer contract tests in `tests/workflows/test_rolling_release_workflow.py`
  over a new heavy smoke test. The existing `act` smoke coverage is optional
  and already gated behind `ACT_WORKFLOW_TESTS=1`.
- Keep documentation in en-GB-oxendict spelling and wrap Markdown prose at
  80 columns.
- Use `apply_patch` for all file edits.
- Implementation must follow this approved plan unless a tolerance is reached.

## Tolerances

- If the clean implementation requires modifying more than 5 existing files or
  adding more than 180 net new lines, stop and check whether the plan is
  drifting into broader workflow refactoring.
- If a realistic contract test cannot assert the manual-dispatch input and the
  change-detection shell logic without introducing a new parsing helper module,
  stop and justify that helper before adding it.
- If the workflow change would require altering the publish job's dependency
  restore semantics beyond the `should_build` gate, stop and review the
  release-behaviour impact before proceeding.
- If `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`, or
  `make test` fail after three focused repair attempts per gate, stop and
  escalate with the saved log paths.

## Risks

- Risk: the current workflow already treats every manual dispatch as
  `should_build=true`, so changing to an explicit force switch could surprise
  anyone who relies on manual runs always rebuilding dependency binaries.
  Mitigation: make the new input name and description explicit in the workflow
  UI, and document in the developer guide that manual rebuilds now require the
  checkbox or boolean input to be enabled.

- Risk: a manual run without the force input will now fall into the existing
  "restore dependency archives from previous release" path, which means the
  rolling release must already contain usable dependency assets for the current
  manifest versions. Mitigation: keep the restore step unchanged and document
  that the force input is the intended recovery path when prior dependency
  builds failed or assets are missing.

- Risk: the workflow tests currently focus on packaging helpers and restore
  guards; adding dispatch-input assertions could become brittle if the YAML
  structure is parsed inconsistently. Mitigation: reuse the existing
  `_load_workflow_mapping()` helper and keep the new tests as narrow contract
  checks on stable keys.

## Progress

- [x] (2026-04-12) Reviewed the current
  `.github/workflows/rolling-release.yml` behaviour and confirmed that
  `workflow_dispatch` currently forces dependency-binary rebuilds implicitly by
  writing `should_build=true` unconditionally.
- [x] (2026-04-12) Reviewed the existing workflow contract tests in
  `tests/workflows/test_rolling_release_workflow.py` and helper fixtures in
  `tests/workflows/conftest.py` and `tests/workflows/workflow_test_helpers.py`.
- [x] (2026-04-12) Confirmed that
  `docs/whitaker-dylint-suite-design.md` and
  `docs/execplans/install-dependency-binaries.md` already describe a manual
  force/recovery concept, but the workflow has not exposed that concept as an
  explicit operator input.
- [x] (2026-04-12) Drafted this ExecPlan at
  `docs/execplans/forceable-rebuild-of-binary-dependencies.md`.
- [x] (2026-04-12) User approved implementation against this ExecPlan.
- [x] (2026-04-12) Established the current red baseline by confirming the
  workflow still lacked `workflow_dispatch.inputs` and still treated every
  manual dispatch as `should_build=true`.
- [x] (2026-04-12) Added workflow contract coverage for the new
  `force_dependency_binary_rebuild` dispatch input and for the tighter
  change-detection shell contract.
- [x] (2026-04-12) Updated `rolling-release.yml` to make dependency-binary
  rebuilds opt-in on manual dispatch via `force_dependency_binary_rebuild`.
- [x] (2026-04-12) Updated `docs/developers-guide.md` to explain when
  contributors should use the manual force input versus the existing restore
  path.
- [x] (2026-04-12) Updated `docs/whitaker-dylint-suite-design.md` so the
  design record matches the new manual-force workflow contract.
- [x] (2026-04-12) Ran formatting, Markdown validation, lint, focused workflow
  contract tests, and the full Rust test suite through the required `tee` +
  `pipefail` gates.

## Surprises & Discoveries

- The current workflow already has a `workflow_dispatch` trigger, but it has no
  `inputs:` block. The manual rebuild behaviour is therefore hidden inside the
  shell script in the `dependency-manifest-changes` job rather than being part
  of the workflow's visible interface.

- The existing dependency-binary design documentation says rolling dependency
  binaries are rebuilt "when the workflow is run manually", which is broader
  than the user request. The implementation should narrow that wording to "when
  the workflow is run manually with the force input enabled" anywhere the docs
  would otherwise drift from the actual workflow contract.

- The `act` fixture file
  `tests/workflows/fixtures/workflow_dispatch.rolling-release.event.json`
  currently contains only `ref`, `repository`, and `sender`. That is enough for
  the existing `build-lints` smoke test, but an implementation may choose to
  add an `inputs` object for realism if it helps future workflow coverage.

- Follow-up discovery (same implementation thread): `make workflow-test-deps`
  originally tried to install directly into the system Python, which is brittle
  under PEP 668 and contrary to the repository's `uv`-based local-validation
  guidance. The target should create a virtual environment via `uv` and install
  workflow-test dependencies into that environment instead.

## Decision Log

- Decision: treat this as a workflow-interface clarification rather than a
  recovery-only script tweak. Rationale: the real gap is that the dispatch form
  does not expose the operator intent, so the fix should start at
  `on.workflow_dispatch.inputs` and let the shell logic consume that explicit
  input. Date/Author: 2026-04-12 / plan author.

- Decision: manual dispatch should no longer imply a dependency-binary rebuild
  by default. Rationale: the user asked for "explicit forcing", and the most
  literal interpretation is that rebuilds remain automatic on manifest changes
  but become opt-in on manual runs. That preserves cheap manual republishes
  while still allowing targeted recovery from previous dependency-binary
  failures. Date/Author: 2026-04-12 / plan author.

- Decision: protect the change with workflow contract tests, not a new `act`
  integration job. Rationale: the behaviour being changed is mostly static YAML
  structure plus shell-guard logic, and the repository already treats `act`
  smoke coverage as optional. Contract tests are the smallest reliable guard.
  Date/Author: 2026-04-12 / plan author.

- Decision: keep `release.yml` out of scope unless implementation uncovers an
  unavoidable shared helper or documentation dependency. Rationale: the request
  names only the rolling-release workflow and the recovery case for previously
  failed dependency-binary builds on rolling. Date/Author: 2026-04-12 / plan
  author.

- Decision: a manual dispatch without the force input should set
  `should_build=false` immediately instead of falling through to the push diff
  logic. Rationale: `workflow_dispatch` events do not have a meaningful
  `${{ github.event.before }}` diff base, and the desired contract is an
  explicit operator choice between "force rebuild now" and "reuse/restore the
  current dependency archives". Date/Author: 2026-04-12 / implementation author.

- Decision: do not modify the `act` workflow-dispatch fixture in this patch.
  Rationale: the current smoke job exercises `build-lints`, which does not read
  `github.event.inputs.force_dependency_binary_rebuild`, so the fixture can
  stay minimal and the file budget remains inside the plan tolerance.
  Date/Author: 2026-04-12 / implementation author.

## Context and orientation

The repository root is `/home/user/project`.

The current workflow behaviour is split across three places:

1. `.github/workflows/rolling-release.yml` defines the manual trigger,
   calculates the `should_build` output in the `dependency-manifest-changes`
   job, gates the `build-dependency-binaries` matrix job on that output, and
   restores dependency archives from the existing rolling release when
   `should_build == 'false'`.
2. `tests/workflows/test_rolling_release_workflow.py` contains workflow
   contract tests that parse the real YAML and assert release invariants such
   as packaging-bin contracts, publish-job `always()` semantics, and restore
   step error handling.
3. `docs/developers-guide.md` documents release-helper binaries and dependency
   binary packaging, but it does not yet explain how contributors should use a
   manual rolling-release dispatch to force a rebuild after prior dependency
   binary publication failures.

The current shell logic in `dependency-manifest-changes` is the key starting
point. It does this today:

1. If `github.event_name == "workflow_dispatch"`, it writes
   `should_build=true` and exits.
2. Otherwise, for push events on `main`, it compares
   `${{ github.event.before }}` to `${{ github.sha }}` for
   `installer/dependency-binaries.toml`.
3. That `should_build` output drives both whether dependency binaries are
   rebuilt and whether the publish job restores prior dependency archives.

That means the implementation does not need a new job. It needs a clearer
manual interface and a tighter condition inside the existing job.

## Plan of work

### Milestone 1: Lock the intended contract in tests first

Start in `tests/workflows/test_rolling_release_workflow.py`. Add red tests that
fail against the current workflow for the exact behaviour the user wants.

The first test should parse the workflow mapping and assert that
`on.workflow_dispatch.inputs.force_dependency_binary_rebuild` exists with a
boolean-like contract that is explicit enough for a maintainer to understand.
At minimum, assert:

1. the input exists;
2. it is marked `type: boolean`;
3. it is not required;
4. it defaults to `false`; and
5. its description makes the recovery intent clear, for example by mentioning
   dependency binaries or rebuild recovery.

The second test should inspect the run script for the
`Check whether dependency manifest changed` step and assert that:

1. the script reads
   `${{ github.event.inputs.force_dependency_binary_rebuild }}` or an
   equivalent expression;
2. the script sets `should_build=true` only when that manual input is true;
3. the script no longer treats every `workflow_dispatch` event as an automatic
   rebuild trigger; and
4. the push-path diff against
   `installer/dependency-binaries.toml` remains present.

If keeping these assertions readable requires a tiny local helper function
inside the test module, add it there rather than expanding
`workflow_test_helpers.py` prematurely.

Acceptance for Milestone 1:

1. Running the targeted workflow test command before the YAML change produces
   a red result with failures describing the missing manual input and/or the
   still-implicit dispatch behaviour.

Recommended command:

```sh
set -o pipefail; python3 -m pytest tests/workflows/test_rolling_release_workflow.py 2>&1 | tee /tmp/forceable-rebuild-workflow-pytest.log
```

After the follow-up correction, the intended local command is:

```sh
make workflow-test-deps
set -o pipefail; .venv/bin/python -m pytest tests/workflows/test_rolling_release_workflow.py 2>&1 | tee /tmp/forceable-rebuild-workflow-pytest.log
```

### Milestone 2: Make manual dependency-binary rebuilds explicitly forceable

Update `.github/workflows/rolling-release.yml`.

First, add a `workflow_dispatch.inputs` block beneath the existing trigger. Use
one boolean input named `force_dependency_binary_rebuild` with a description
that makes the operator intent obvious. The description should say that it
forces rebuilding dependency binaries even when
`installer/dependency-binaries.toml` has not changed, and that the input is
useful for recovery after previous dependency-binary build failures.

Second, rewrite only the early manual-dispatch branch in the
`dependency-manifest-changes` job:

1. If the workflow is running under `workflow_dispatch` and the input is
   `true`, write `should_build=true` and exit.
2. If the workflow is running under `workflow_dispatch` and the input is not
   `true`, write `should_build=false` and exit.
3. Leave the push-event diff logic unchanged for non-dispatch runs.

Do not change the `build-dependency-binaries` job gate; it should continue to
use `needs.dependency-manifest-changes.outputs.should_build == 'true'`. Do not
change the publish job's restore-step condition either; the new manual `false`
path should deliberately fall through to the existing restore logic.

Acceptance for Milestone 2:

1. The targeted workflow tests from Milestone 1 now pass.
2. Reading the YAML makes it obvious that manual rebuilds are opt-in rather
   than implicit.
3. A manual dispatch with the input left at `false` skips dependency-binary
   rebuilding and reuses the existing publish path.
4. A manual dispatch with the input set to `true` rebuilds dependency binaries
   even if the manifest did not change.

### Milestone 3: Document the operator-facing recovery path

Update `docs/developers-guide.md` in the dependency-binary packaging section.

Add a short subsection that explains:

1. push behaviour on `main`: dependency binaries rebuild automatically only
   when `installer/dependency-binaries.toml` changes;
2. manual behaviour: the rolling-release workflow exposes
   `force_dependency_binary_rebuild`, which must be enabled when a maintainer
   wants to recover from a previously failed dependency-binary build; and
3. non-forced manual runs reuse or restore the existing dependency-binary
   assets instead of rebuilding third-party tools unnecessarily.

If the implementation changes the workflow contract described in
`docs/whitaker-dylint-suite-design.md`, update the relevant sentence there in
the same patch so the design document remains accurate. The developer guide is
mandatory for this task; the design doc update is a consistency follow-up that
should happen if the wording would otherwise remain misleading.

Acceptance for Milestone 3:

1. A contributor reading `docs/developers-guide.md` can tell when the rolling
   workflow rebuilds dependency binaries automatically and when they must force
   it manually.
2. Any remaining design-doc wording matches the implemented behaviour.

### Milestone 4: Run the full validation gates

After the workflow, tests, and docs are updated, run the required project gates
with `tee` and `set -o pipefail`.

Run these commands from `/home/user/project`:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/forceable-rebuild-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/forceable-rebuild-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/forceable-rebuild-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/forceable-rebuild-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/forceable-rebuild-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/forceable-rebuild-test.log
```

Expected success signals:

1. `.venv/bin/python -m pytest tests/workflows/test_rolling_release_workflow.py`
   passes with the new contract tests included.
2. `make markdownlint` and `make nixie` pass after the doc update.
3. `make check-fmt`, `make lint`, and `make test` all complete successfully.

## Outcomes & Retrospective

Implemented the planned workflow-interface change without exceeding the file or
scope tolerances. `.github/workflows/rolling-release.yml` now exposes a boolean
`workflow_dispatch` input named `force_dependency_binary_rebuild`, and the
`dependency-manifest-changes` job now sets `should_build=true` only when that
manual input is `true`. Manual dispatches with the default `false` value now
skip the dependency-binary rebuild and fall through to the existing restore
path.

Workflow contract coverage now protects both halves of the change. The Python
tests assert that the new manual input exists with the expected boolean/default
contract, and that the change-detection shell step reads
`github.event.inputs.force_dependency_binary_rebuild` instead of rebuilding on
every `workflow_dispatch`. While touching that file, the existing `publish.if`
assertion was also hardened to accept the expression form returned by the
current `ruamel.yaml` parser.

Contributor-facing documentation now matches the shipped behaviour.
`docs/developers-guide.md` explains when maintainers should force a rebuild
manually versus reuse existing dependency archives, and
`docs/whitaker-dylint-suite-design.md` no longer implies that every manual run
rebuilds dependency binaries.

Follow-up correction: the repository's workflow-test dependency setup now uses
`uv` with a local `.venv` instead of attempting to install into the system
Python. `Makefile` now creates that environment in `workflow-test-deps`, and
the local workflow-validation guide documents the same path.

Validation results:

1. Focused workflow contract tests passed:
   `.venv/bin/python -m pytest tests/workflows/test_rolling_release_workflow.py`
    → `9 passed, 1 skipped`.
2. Documentation gates passed:
   `make fmt`, `make markdownlint`, and `make nixie`.
3. Repository code gates passed:
   `make check-fmt`, `make lint`, and `make test` →
   `1293 tests run: 1293 passed, 2 skipped`.

Lesson learned: workflow-test infrastructure should follow the same `uv` + venv
discipline as the rest of the repository's local validation paths instead of
assuming that mutating the system Python is acceptable.
