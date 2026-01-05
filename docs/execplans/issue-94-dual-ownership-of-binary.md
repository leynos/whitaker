# Resolve dual ownership of the installer binary

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

PLANS.md is not present in this repository.

## Purpose / Big Picture

Ensure the `whitaker-installer` binary is owned by exactly one package so
`installer/src/main.rs` is compiled once and documentation tooling sees a
single, unambiguous binary. Success is observable when `cargo install` and the
existing behavioural tests still work, and `make install-smoke` succeeds while
`Cargo.toml` no longer declares a duplicate `[[bin]]` entry.

## Constraints

- Keep the CLI entrypoint at `installer/src/main.rs` and the binary name
  `whitaker-installer` unchanged so tests in
  `installer/tests/behaviour_cli.rs` keep passing.
- Do not add new external dependencies.
- Keep en-GB spelling in documentation updates.
- Follow Makefile targets for validation and run them through `tee` with
  `set -o pipefail`.
- If a commit is made, include the issue reference `closes #94` in the commit
  message body.

## Tolerances (Exception Triggers)

- Scope: if the fix requires changing more than 6 files or 300 net lines of
  code, stop and escalate.
- Interface: if the public CLI flags or output format of `whitaker-installer`
  must change, stop and escalate.
- Dependencies: if any new crate or tool is required, stop and escalate.
- Tests: if the validation commands fail twice in a row, stop and escalate with
  logs.
- Ambiguity: if there are multiple viable ownership models (for example,
  keeping the root `[[bin]]` versus removing it) and the choice materially
  affects installation workflow, stop and confirm the preferred workflow.

## Risks

- Risk: removing the root `[[bin]]` breaks `cargo install --path .` workflows
  that assume a binary at the workspace root.
  Severity: medium
  Likelihood: medium
  Mitigation: update `make install-smoke` and any documentation to use
  `cargo install --path . --package whitaker-installer` (or `--path installer`)
  and validate with `make install-smoke`.
- Risk: documentation or CI scripts implicitly rely on the duplicate binary.
  Severity: low
  Likelihood: low
  Mitigation: search for `cargo install --path .` and `whitaker-installer`
  references and update as needed.

## Progress

- [x] (2026-01-05 00:00Z) Draft ExecPlan for issue #94.
- [ ] Inspect current workspace configuration and installation instructions.
- [ ] Remove duplicate binary ownership and adjust install workflows.
- [ ] Validate formatting, linting, tests, and install smoke check.

## Surprises & Discoveries

None yet.

## Decision Log

- Decision: plan to make the `installer` package the sole owner of the
  `whitaker-installer` binary and remove the root `[[bin]]` entry.
  Rationale: the installer crate already defines the binary needed by its
  tests; removing the root copy eliminates double compilation without changing
  CLI behaviour.
  Date/Author: 2026-01-05 / Codex

## Outcomes & Retrospective

Pending. This section will be updated after implementation.

## Context and Orientation

The workspace root `Cargo.toml` currently defines a `[[bin]]` named
`whitaker-installer` that points at `installer/src/main.rs` (around lines
52-56). The installer package in `installer/Cargo.toml` also defines a
`[[bin]]` with the same name pointing at the same source file (around lines
10-12). This causes the same binary to be compiled twice by different packages,
which creates boundary ambiguity and doc-tooling friction. The Makefile target
`install-smoke` runs `cargo install --path . --locked`, which relies on the
root package exporting a binary. The installer package already contains the
CLI tests in `installer/tests/behaviour_cli.rs` and the library surface in
`installer/src/lib.rs`.

## Plan of Work

Stage A: confirm the current ownership and install flow. Inspect
`Cargo.toml`, `installer/Cargo.toml`, `Makefile`, and any documentation that
mentions `cargo install --path .` or binary ownership. Decide whether any
user-facing docs require updates once the root `[[bin]]` is removed.
Validation: no code changes; knowledge of where to edit is captured in the
plan.

Stage B: remove duplicate ownership. Delete the root `[[bin]]` entry and its
comment in `Cargo.toml`. Update `make install-smoke` to install the installer
package explicitly (for example, `cargo install --path . --package
whitaker-installer --locked`), and update any documentation that refers to the
old install flow. Validation: `cargo metadata --no-deps` or a quick `cargo
build -p whitaker-installer` should show a single binary owner.

Stage C: harden and verify. Run formatting, linting, tests, and documentation
checks using the required Makefile targets. Finish by running
`make install-smoke` to confirm installation still works.

## Concrete Steps

1) Inspect the current ownership and install flow from the repository root:

    rg -n "\\[\\[bin\\]\\]" Cargo.toml installer/Cargo.toml
    rg -n "install-smoke|cargo install" Makefile docs README.md

2) Remove the duplicate root binary entry in `Cargo.toml` (delete the
   `[[bin]]` table and the comment that explains why it exists).

3) Update `Makefile` `install-smoke` to install the installer package
   explicitly. Prefer the workspace-root command so the path remains stable:

    cargo install --path . --package whitaker-installer --root "$$TMP_DIR" --locked

4) If any documentation mentions `cargo install --path .` for the installer,
   update it to use `--package whitaker-installer` (or `--path installer`) and
   keep paragraphs wrapped at 80 columns.

5) Run validation commands with logs (from the repository root):

    set -o pipefail; make fmt 2>&1 | tee /tmp/whitaker-fmt.log
    set -o pipefail; make markdownlint 2>&1 | tee /tmp/whitaker-markdownlint.log
    set -o pipefail; make nixie 2>&1 | tee /tmp/whitaker-nixie.log
    set -o pipefail; make check-fmt 2>&1 | tee /tmp/whitaker-check-fmt.log
    set -o pipefail; make lint 2>&1 | tee /tmp/whitaker-lint.log
    set -o pipefail; make test 2>&1 | tee /tmp/whitaker-test.log
    set -o pipefail; make install-smoke 2>&1 | tee /tmp/whitaker-install.log

   If any command fails, capture the log and stop to reassess before retrying.

## Validation and Acceptance

Quality criteria (done when all hold):

- Tests: `make test` passes.
- Lint/typecheck: `make lint` passes.
- Formatting: `make check-fmt` passes; `make fmt` has been run after
  documentation edits.
- Documentation checks: `make markdownlint` and `make nixie` pass if docs were
  touched.
- Installation: `make install-smoke` exits successfully and the temporary
  `whitaker-installer --help`/`--version` invocations succeed.

Observable behaviour confirming the fix:

- `Cargo.toml` no longer defines `[[bin]]` for `whitaker-installer`.
- `installer/Cargo.toml` remains the sole owner of the binary.
- `cargo install --path . --package whitaker-installer --locked` installs the
  CLI successfully (verified via `make install-smoke`).

## Idempotence and Recovery

Edits are safe to re-run. If a step fails, revert the file changes with Git and
re-apply. The `install-smoke` target uses a temporary directory and is safe to
repeat. If validation fails twice, stop and escalate with the captured logs.

## Artifacts and Notes

Capture key outputs in the log files listed in Concrete Steps. These logs are
sufficient evidence for validation and troubleshooting.

## Interfaces and Dependencies

- Keep the binary definition only in `installer/Cargo.toml` under `[[bin]]` with
  name `whitaker-installer` and path `src/main.rs`.
- Do not change public CLI arguments or exit codes.
- Do not add dependencies; use existing workspace tooling and Makefile targets.

