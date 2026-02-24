# Restore rolling manifest publication for installer prebuilt downloads

This execution plan (ExecPlan) is a living document. The sections Constraints,
Tolerances, Risks, Progress, Surprises & Discoveries, Decision Log, and
Outcomes & Retrospective must be kept up to date as work proceeds.

Status: IN PROGRESS

This document must be maintained in accordance with `AGENTS.md`.

## Purpose / big picture

`whitaker-installer` currently attempts to download `manifest-<target>.json`
from the `rolling` GitHub Release, before falling back to local compilation.
Right now that URL returns 404, so every install on otherwise supported targets
takes the slow fallback path.

After this work, a successful rolling-release run will always publish a
`rolling` release that includes `manifest-<target>.json` and matching
`.tar.zst` archives for built targets. Running `whitaker-installer` on a built
target (for example `x86_64-unknown-linux-gnu`) will download the manifest and
archive instead of immediately falling back to local build.

## Constraints

- Restrict code changes to `.github/workflows/rolling-release.yml` and
  workflow tests under `tests/workflows/` unless new evidence proves another
  file is required.
- Do not change installer fallback semantics in `installer/src/prebuilt.rs`.
  Any prebuilt error must remain non-fatal.
- Keep release asset naming unchanged:
  `manifest-<target>.json` and `whitaker-lints-...tar.zst`.
- Do not add new dependencies.
- Preserve Makefile and AGENTS quality-gate requirements.

## Tolerances (exception triggers)

- Scope: if the fix requires edits outside workflow + workflow tests + this
  plan, stop and escalate.
- Behaviour: if changing publication reliability requires changing installer
  URL format or release tag (`rolling`), stop and escalate.
- Dependencies: if any new crate or Python package is required, stop and
  escalate.
- Validation: if reproduction evidence cannot be obtained from GitHub run logs
  and API responses, stop and escalate.

## Risks

- Risk: `rustup component add` behaviour can vary by runner image.
  Mitigation: validate with at least one Linux matrix run and one Darwin run in
  GitHub Actions logs.
- Risk: publishing stays blocked when any non-critical matrix leg fails.
  Mitigation: include publish-job gating hardening if needed after the primary
  toolchain conflict fix is validated.
- Risk: local `act` tests cannot cover macOS or Windows matrix legs.
  Mitigation: treat GitHub Actions run evidence as the source of truth.

## Progress

- [x] 2026-02-24 17:05 UTC: Reproduced symptom from installer logs
  (`artefact not found` at `releases/download/rolling/manifest-...json`).
- [x] 2026-02-24 17:05 UTC: Confirmed installer URL construction and fallback
  path in `installer/src/artefact/download.rs` and `installer/src/prebuilt.rs`.
- [x] 2026-02-24 17:05 UTC: Analysed workflow run `22319537097`; two matrix
  jobs failed in `Install pinned toolchain components`, and `publish` was
  skipped.
- [x] 2026-02-24 17:05 UTC: Confirmed `GET /releases/tags/rolling` returns
  `{"message":"Not Found"}`.
- [x] 2026-02-24 18:05 UTC: Implemented workflow fix to install
  `rustc-dev`/`llvm-tools-preview` for `matrix.target` only.
- [x] 2026-02-24 18:05 UTC: Added workflow regression test enforcing the
  matrix-target-only `rustc-dev` install contract.
- [x] 2026-02-24 18:27 UTC: Local gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie`.
- [x] 2026-02-24 18:27 UTC: Workflow contract tests passed via
  `python3 -m pytest tests/workflows/test_rolling_release_workflow.py` (3
  passed, 1 skipped).
- [ ] Validate with GitHub Actions run evidence that `publish` executes.
- [ ] Validate manifest URL is downloadable after publish.

## Surprises & Discoveries

- The `x86_64-unknown-linux-gnu` leg succeeded and uploaded an Actions
  artefact, but that does not make assets available at
  `releases/download/rolling/...`; only the `publish` job creates that release.
- The failing step installs `rustc-dev` twice for different targets in the
  same toolchain invocation path, and the second installation conflicts on
  `lib/rustlib/rustc-src/rust/Cargo.lock`.
- `publish` is currently blocked by `needs: build-lints`, so a single matrix
  failure removes prebuilt availability for all targets.

## Decision Log

- 2026-02-24: Root cause accepted as continuous integration (CI) publication
  failure, not installer URL construction bug. Evidence: installer code points
  at `rolling`; release tag is absent; run `22319537097` failed before publish.
- 2026-02-24: Initial remediation focus is the workflow component-install step.
  The installer code already handles missing artefacts correctly by falling
  back.
- 2026-02-24: Keep release tag and manifest naming unchanged to avoid widening
  blast radius.
- 2026-02-24: Use `matrix.target` as the sole `rustc-dev` installation target
  per matrix leg. This removes the second `rustc-dev` install call that
  triggered rustup file conflicts.

## Outcomes & Retrospective

Implementation work is complete in-repo: workflow install logic now avoids the
dual-target `rustc-dev` conflict, and tests enforce the updated contract. Local
Rust/Python/docs gates all pass. Remaining work is external verification on
GitHub Actions that `publish` executes and that the `rolling` manifest URL
returns HTTP 200.

## Technical orientation

Relevant files and why they matter:

- `.github/workflows/rolling-release.yml`
  - Defines matrix build, artefact upload, and `publish` release creation.
  - Current failure is in step `Install pinned toolchain components`.
- `installer/src/artefact/download.rs`
  - Constructs release URL:
    `https://github.com/leynos/whitaker/releases/download/rolling/{filename}`.
- `installer/src/prebuilt.rs`
  - Converts download/parsing/verification failures into fallback messages.
- `tests/workflows/test_rolling_release_workflow.py`
  - Contract/smoke tests for workflow configuration and build-lints execution.

External evidence captured during investigation:

- GitHub Actions run `22319537097` concluded `failure`.
- Failing jobs:
  - `build-lints (x86_64-apple-darwin, macos-latest)`
  - `build-lints (aarch64-unknown-linux-gnu, ubuntu-latest, true)`
- Failure message in both jobs:
  - `failed to install component: rustc-dev-… detected conflict:
    lib/rustlib/rustc-src/rust/Cargo.lock`
- API probe:
  - `GET https://api.github.com/repos/leynos/whitaker/releases/tags/rolling`
    returned `{"message":"Not Found"}`.

## Plan of work

### Phase 1: Fix component installation conflict

Update the `Install pinned toolchain components` script in
`.github/workflows/rolling-release.yml` so each matrix leg installs only the
required `rustc-dev`/`llvm-tools-preview` target set once, avoiding conflicting
multi-target `rustc-dev` installation in a single job.

Implementation intent:

- Keep `rustup target add --toolchain "$TOOLCHAIN" "${{ matrix.target }}"`.
- Install `rust-src` once if still required by lint crate compilation.
- Install `rustc-dev llvm-tools-preview` for `matrix.target` only.
- Remove the current host-target + conditional second-target dual install path.

### Phase 2: Keep workflow contract coverage aligned

Adjust `tests/workflows/test_rolling_release_workflow.py` so tests enforce the
revised component-install policy and prevent regression to the conflicting
installation sequence.

Minimum assertions to add:

- Workflow still installs the pinned toolchain.
- `Install pinned toolchain components` does not attempt two separate
  `rustc-dev` target installs in one matrix job.
- Existing crate-list contract tests remain unchanged and passing.

### Phase 3: Validate publication end-to-end

Run validation in this order:

- [x] 3.1 Run local workflow checks with:
  `python3 -m pytest tests/workflows/test_rolling_release_workflow.py`
- [ ] 3.2 Trigger/observe a GitHub rolling-release run on this branch (or an
  equivalent repository test run).
- [ ] 3.3 Confirm run completion and publish execution.
- [ ] 3.4 Verify release manifest endpoint exists with:
  `curl -fL https://github.com/leynos/whitaker/releases/download/rolling/manifest-x86_64-unknown-linux-gnu.json`
- [ ] 3.5 Verify installer prebuilt path no longer immediately falls back on
  Linux x86_64.

### Phase 4: Hardening (only if needed)

If publication is still brittle when non-critical matrix legs fail, introduce
an explicit policy for partial release publication versus strict all-target
blocking, document the decision, and add tests accordingly.

## Validation commands and expected signals

Use `tee` logs for long output as required by AGENTS guidance.

- Local workflow test command:

  ```plaintext
  python3 -m pytest tests/workflows/test_rolling_release_workflow.py \
    | tee /tmp/test-workflow-whitaker-$(git branch --show).out
  ```

  Expected: tests pass, including new contract assertions.
- GitHub run inspection:
  - `gh run view <run-id> --repo leynos/whitaker`
  - Expected: `build-lints` legs succeed and `publish` is not skipped.
- Release availability check:

  ```plaintext
  curl -fL \
    https://github.com/leynos/whitaker/releases/download/rolling/manifest-x86_64-unknown-linux-gnu.json
  ```

  - Expected: HTTP 200 and JSON body.

## Rollback / recovery

If the workflow fix regresses other targets, revert only the component-install
step change, keep investigation notes in this plan, and re-open with per-target
strategy options (host-only, matrix-only, or split jobs).
