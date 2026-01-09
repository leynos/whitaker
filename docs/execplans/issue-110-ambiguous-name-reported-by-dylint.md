# Clarify Whitaker suite name in `dylint` listing

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / Big Picture

When running `cargo dylint list` with Whitaker libraries staged, Dylint reports
the lint suite as `suite`, which is ambiguous. This plan updates the Whitaker
Dylint suite naming so the listing shows a descriptive identifier that clearly
ties the suite to the Whitaker project. Success is observable when
`cargo dylint list` shows the new identifier in place of `suite` and the
installer can still build, stage, and list the suite without regressions.

## Constraints

- Do not introduce a new `whitaker` list subcommand; rely on `cargo dylint list`
  for listing output.
- Preserve the column/line structure of `cargo dylint list`; only change the
  suite name that appears in the output.
- Do not add new external dependencies.
- Update documentation using en-GB spelling and wrap paragraphs at 80 columns.
- Follow Makefile validation targets and run them through `tee` with
  `set -o pipefail`.
- If a commit is made, include the issue reference `closes #110` in the commit
  message body.

## Tolerances (Exception Triggers)

- Scope: if the fix requires changing more than 10 files or 350 net lines of
  code, stop and escalate.
- Interface: if the public CLI flags or output format of `whitaker` or
  `whitaker-installer` must change, stop and escalate.
- Dependencies: if any new crate or tool is required, stop and escalate.
- Tests: if validation commands fail twice in a row, stop and escalate with
  logs.
- Ambiguity: if multiple naming options are viable (for example,
  `whitaker_suite` vs `whitaker-lints`) and the choice materially affects user
  expectations, stop and confirm the preferred identifier.

## Risks

- Risk: renaming the suite crate will require updates across installer logic,
  tests, and documentation.
  - Severity: medium
  - Likelihood: high
  - Mitigation: catalogue all references with `rg` and update them in one
    atomic change; keep tests aligned with the new name.
- Risk: existing users may have old `libsuite@<toolchain>` libraries staged,
  causing `cargo dylint list` to show both old and new names after reinstall.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: decide whether to add a cleanup step in the installer or note
    the reinstallation requirement in documentation.
- Risk: changing the suite name could break assumptions in Dylint tooling.
  - Severity: low
  - Likelihood: low
  - Mitigation: verify with `cargo dylint list` and run the full test suite.

## Progress

- [x] (2026-01-09 00:00Z) Draft ExecPlan for issue #110.
- [x] (2026-01-09 00:20Z) Update plan to avoid `whitaker list` references.
- [ ] Confirm where `cargo dylint list` output originates and which name is
  used.
- [ ] Decide the new suite identifier and document the rationale.
- [ ] Update suite naming across build, staging, and resolution logic.
- [ ] Update tests and documentation, then validate all checks.

## Surprises & Discoveries

None yet.

## Decision Log

- Decision: treat `cargo dylint list` as the only listing command and avoid
  references to `whitaker list`. Rationale: the `whitaker` subcommand is
  unnecessary, and the output source is Dylint's built-in listing. Date/Author:
  2026-01-09 / Codex.

## Outcomes & Retrospective

Pending.

## Context and Orientation

`cargo dylint list` displays the crate name embedded in the staged library
filename. The suite crate currently lives in `suite/` with `suite/Cargo.toml`
declaring `name = "suite"`. The installer stages libraries using
`installer/src/stager.rs`, which applies the Dylint naming convention
`{prefix}{crate_name}@{toolchain}{extension}` based on `CrateName` values from
`installer/src/resolution.rs`. The suite crate name is also referenced by
constants and tests in the installer (`SUITE_CRATE` in
`installer/src/resolution.rs`, `installer/src/scanner.rs`,
`installer/src/list_output.rs`, and installer behavioural tests). The design
notes in `docs/whitaker-dylint-suite-design.md` discuss the aggregated suite
crate by name and must remain accurate.

Issue context: the ambiguity is reported in issue #110 and a linked PR
comment.[^1][^2]

## Plan of Work

Stage A: confirm the source of the listing and decide the new identifier. Use
`rg` to list all references to `suite` in code and docs. Confirm that the name
comes from library filenames shown by `cargo dylint list`. Choose a new
identifier that is descriptive and consistent with existing naming conventions
(for example, `whitaker_suite`). Validation: produce a short summary in the
plan and update `Decision Log` once the name is chosen.

Stage B: update crate naming and resolution logic. Change the suite crate name
in `suite/Cargo.toml` and update `SUITE_CRATE` plus any helper logic that uses
it. Ensure build, staging, and scan logic use the new name, and that the
installer still selects and recognises the suite. Validation: unit tests for
name parsing and list output should reflect the new identifier.

Stage C: update tests and documentation. Adjust installer tests that hardcode
`suite` and update docs (notably `docs/whitaker-dylint-suite-design.md` and any
user-facing references). If the change affects staged library naming, consider
whether installer documentation should mention removing old `libsuite` files
and add guidance if required. Validation: tests and docs updated with correct
wrap width and en-GB spelling.

Stage D: validate and document outcomes. Run format, lint, and test suites via
Makefile targets and capture logs. Run `cargo dylint list` against a staged
suite and confirm the new identifier appears. Validation: commands pass and
listing output shows the descriptive name.

## Concrete Steps

1) Inventory naming references and confirm list origin (from repository root):

    rg -n "\\bsuite\\b" suite installer docs Cargo.toml
    rg -n "cargo dylint list|DYLINT_LIBRARY_PATH" installer

2) Decide the new suite identifier. If uncertain, stop and confirm the
   preferred name before editing any files. Record the decision in the
   `Decision Log`.

3) Update the suite crate name and installer constants. Typical edits include:

    - suite/Cargo.toml
    - installer/src/resolution.rs
    - installer/src/scanner.rs
    - installer/src/list_output.rs
    - installer/src/stager.rs (if library filename assumptions change)

4) Update tests that reference the old name, including:

    - installer/src/list.rs
    - installer/src/list_output.rs
    - installer/src/scanner.rs
    - installer/tests/behaviour_core.rs

5) Update documentation references to the suite name, primarily:

    - docs/whitaker-dylint-suite-design.md
    - README.md or docs/users-guide.md if they mention the suite name

6) Run validation commands with logs (from the repository root):

    set -o pipefail; make fmt 2>&1 | tee /tmp/whitaker-fmt.log
    set -o pipefail; make markdownlint 2>&1 | tee /tmp/whitaker-markdownlint.log
    set -o pipefail; make nixie 2>&1 | tee /tmp/whitaker-nixie.log
    set -o pipefail; make check-fmt 2>&1 | tee /tmp/whitaker-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/whitaker-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/whitaker-test.log

7) Validate the runtime behaviour by rebuilding and listing the suite with the
   staged library path set:

    DYLINT_LIBRARY_PATH="${HOME}/.local/share/dylint/lib" cargo dylint list

   Expected output should show the new suite identifier instead of `suite`.

If any command fails, capture the log and stop to reassess before retrying.

## Validation and Acceptance

Quality criteria (done when all hold):

- Tests: `make test` passes.
- Lint/typecheck: `make lint` passes.
- Formatting: `make check-fmt` passes; `make fmt` has been run after
  documentation edits.
- Documentation checks: `make markdownlint` and `make nixie` pass if docs were
  touched.

Observable behaviour confirming the fix:

- `cargo dylint list` shows the new suite identifier in place of `suite` for
  the aggregated lint library.
- The installer can still build and stage the suite without errors.
- No tests or lint checks regress.

## Idempotence and Recovery

Edits are safe to re-run. If a step fails, revert file changes with Git and
re-apply. Validation commands are safe to repeat. If validation fails twice,
stop and escalate with the captured logs.

## Artifacts and Notes

Keep the validation logs from `/tmp/whitaker-*.log` as evidence for each
quality gate. Record any output from `cargo dylint list` that shows the updated
identifier.

## Interfaces and Dependencies

- Preferred suite identifier: decide and record in `Decision Log` before
  coding. If the chosen identifier is `whitaker_suite`, ensure:
  - `suite/Cargo.toml` uses `name = "whitaker_suite"`.
  - `installer/src/resolution.rs` updates `SUITE_CRATE` to the same value.
  - All installer logic and tests compare against the updated name.
- Do not add new dependencies; use existing installer modules and constants.

## Revision note

2026-01-09: Removed `whitaker list` references and clarified that listing
validation uses `cargo dylint list` directly; no behavioural change to the
implementation steps beyond the listing command.

[^1]: <https://github.com/leynos/whitaker/issues/110>
[^2]: <https://github.com/leynos/whitaker/pull/109#discussion_r1903475000>
