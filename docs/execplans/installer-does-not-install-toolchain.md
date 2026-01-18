# Install pinned toolchain automatically

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

No PLANS.md file exists in the repository root, so no additional plan
constraints apply.

## Purpose / Big Picture

After this change, running `whitaker-installer` will automatically install the
pinned Rust toolchain (and its required components) when it is missing. Users
should no longer see the error about the missing toolchain; instead the
installer should invoke rustup, install the toolchain, and proceed. Success is
observable by running the installer on a machine without the pinned toolchain
and seeing it complete without manual rustup steps.

## Constraints

- Do not change the installer command-line interface unless required by the
  plan or by a documented requirement.
- Do not introduce new external dependencies or crates.
- Keep module-level (`//!`) docs at the top of each Rust module untouched.
- Maintain en-GB-oxendict spelling in new documentation and comments.
- Keep Rust files under 400 lines. If a new helper grows too large, extract
  it.

## Tolerances (Exception Triggers)

- Scope: if the implementation requires edits to more than 8 files or more
  than 250 net new lines of code, stop and escalate.
- Interface: if a public API or CLI flag must change, stop and escalate.
- Dependencies: if a new external dependency is needed, stop and escalate.
- Tests: if the tests still fail after 2 iterations, stop and escalate.
- Ambiguity: if it is unclear whether components should be installed (not just
  the toolchain), stop and ask for clarification.

## Risks

- Risk: Rustup may be unavailable or blocked by network policy.
  Severity: medium Likelihood: medium Mitigation: provide a clear error message
  from rustup and preserve the existing error context.

- Risk: Toolchain components listed in `rust-toolchain.toml` might not be
  installed by a bare `rustup toolchain install` command. Severity: high
  Likelihood: medium Mitigation: parse components and install them explicitly,
  or run `rustup toolchain install` with `--component` flags if supported.

- Risk: Adding command execution logic could make testing harder.
  Severity: low Likelihood: medium Mitigation: introduce a small internal
  command runner abstraction for unit tests without adding dependencies.

## Progress

- [x] (2026-01-18 00:20Z) Review current installer flow and toolchain detection
      in `installer/src/toolchain.rs` and `installer/src/main.rs`.
- [x] (2026-01-18 00:45Z) Add toolchain install logic (plus component handling)
      with unit tests.
- [x] (2026-01-18 00:48Z) Update user-facing output and documentation
      describing installer prerequisites.
- [x] (2026-01-18 01:12Z) Run `make fmt`, `make markdownlint`, `make nixie`,
      `make check-fmt`, `make lint`, and `make test` with captured logs.
- [x] (2026-01-18 01:30Z) Commit the change with a clear message.

## Surprises & Discoveries

None yet.

## Decision Log

- Decision: Defer implementation until plan approval.
  Rationale: ExecPlan workflow requires explicit approval. Date/Author:
  2026-01-18 (Codex)

- Decision: Install pinned toolchain components using `rustup component add`
  after ensuring the toolchain is present, and keep CLI toolchain overrides
  free of additional `rust-toolchain.toml` validation to preserve current
  override behaviour. Rationale: Component installation is idempotent and
  ensures required toolchain parts exist, while avoiding new errors for
  explicit overrides. Date/Author: 2026-01-18 (Codex)

## Outcomes & Retrospective

The installer now auto-installs the pinned toolchain and required components
via rustup, reports toolchain install success, and provides unit tests for the
new behaviour. Documentation now reflects the auto-install behaviour. The
approach kept override behaviour unchanged while ensuring components are
installed for detected toolchains.

## Context and Orientation

The installer binary is in `installer/src/main.rs`. Toolchain detection lives
in `installer/src/toolchain.rs`, which currently reads `rust-toolchain.toml`,
extracts only the `channel`, and verifies the toolchain via
`rustup run <channel> rustc --version`. Missing toolchains currently return
`InstallerError::ToolchainNotInstalled` from `installer/src/error.rs`, which
produces the error seen in the bug report. The installer does not attempt to
install the toolchain when missing, so it fails immediately before building.

Key files:

- `installer/src/main.rs` owns the CLI flow and calls `resolve_toolchain`.
- `installer/src/toolchain.rs` detects and validates the toolchain.
- `installer/src/error.rs` defines error messages for toolchain failures.
- `rust-toolchain.toml` pins the required toolchain and components.

## Plan of Work

Stage A: Understand and design. Read `installer/src/toolchain.rs` and confirm
how it checks for the toolchain. Decide whether to install components listed in
`rust-toolchain.toml`. If that decision is ambiguous, escalate before
proceeding.

Stage B: Tests and scaffolding. Add a small internal command runner abstraction
(trait and implementation) so that toolchain install behaviour can be
unit-tested without invoking rustup. Write unit tests for the new behaviour in
`installer/src/toolchain.rs`:

- When the toolchain is missing, the installer should invoke rustup install and
  succeed if rustup succeeds.
- When rustup install fails, the installer should return a meaningful error
  that includes stderr text.

Stage C: Implementation. Extend toolchain parsing to also return component
names from `rust-toolchain.toml` (optional list). Update toolchain verification
to:

- Check for toolchain availability as before.
- If missing, run `rustup toolchain install <channel>` with component flags
  derived from the toolchain file (or a follow-up `rustup component add` if
  needed by rustup behaviour).
- Re-check the toolchain after installation, returning the existing error if
  still missing.

Stage D: Messaging and documentation. Update any installer documentation in
`README.md` or `docs/` that lists manual prerequisites so it reflects the new
auto-install behaviour. Ensure messages printed by the installer clearly report
when it is installing the toolchain.

Each stage ends with validation; do not proceed if validation fails.

## Concrete Steps

Run commands from the repository root.

1. Inspect toolchain code and tests.
   Expected: identify `Toolchain::verify_installed` and how it is called.

2. Implement and test toolchain installation logic.
   Update `installer/src/toolchain.rs` and tests. If components are installed,
   parse them from `rust-toolchain.toml`.

3. Update installer output and docs.
   Update `installer/src/main.rs` to print a message when installing the
   toolchain, and update any relevant documentation.

4. Format and lint.
   - `make fmt`
   - `make markdownlint`
   - `make nixie`
   - `make check-fmt`
   - `make lint`
   - `make test`

Capture each command output with `tee` to
`/tmp/<action>-$(get-project)-$(git branch --show).out`.

## Validation and Acceptance

Success means:

- Running `whitaker-installer` on a machine without the pinned toolchain now
  installs the toolchain automatically and proceeds to build.
- The new unit tests fail before the change and pass after.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all pass.

## Idempotence and Recovery

The toolchain installation step should be idempotent: if the toolchain is
already installed, no rustup install command should run. If rustup fails, the
installer should return an error and can be re-run once the environment
recovers.

## Artifacts and Notes

Example log (expected after change):

    Toolchain nightly-2025-09-18 installed successfully.

## Interfaces and Dependencies

No new dependencies. Implement within existing modules. Expected functions or
behaviour:

- In `installer/src/toolchain.rs`, either extend `Toolchain::verify_installed`
  or introduce a new `Toolchain::ensure_installed` method that:
  - Attempts verification.
  - If missing, runs rustup install with components and retries.
- Add a small internal trait (e.g., `CommandRunner`) used only within
  `toolchain.rs` to inject command behaviour for tests.
- Keep `resolve_toolchain` in `installer/src/main.rs` as the entrypoint, calling
  the new install-aware method.

## Revision note

Initial plan created for toolchain auto-install behaviour. 2026-01-18: Updated
status, recorded decisions, and marked implementation progress to reflect
toolchain install and documentation updates. 2026-01-18: Recorded validation
completion and marked the plan complete after committing the changes.
