# Architectural decision record (ADR) 0017: the `whitaker check` exit-status contract

## Status

Accepted (2026-11-17): pass every status `cargo dylint` reports straight
through, and reserve the small codes `2` to `9` for Whitaker's own outcomes.
`2` is a usage error, `3` a configuration error, `4` a provisioning failure,
`5` a host or execution failure, and `6` to `9` are reserved. Every operational
failure also prints a stable reason key, `whitaker: error[<key>]`, when
standard error is writable, so scripts can tell causes apart without parsing
prose.

## Date

2026-11-17.

## Context and problem statement

`whitaker check` runs the Whitaker Dylint suite over the caller's package. It
therefore sits between two existing status contracts: `cargo dylint`'s, whose
status the caller already expects to receive from a lint run, and Whitaker's
own, which has to answer a different question. A lint denial means "the code
failed"; a provisioning failure means "Whitaker could not try". A script
wrapping the command in a pre-commit hook or continuous-integration step needs
to distinguish those without reading English.

The command also owns work that can fail before Dylint is reached: it may
decide that the installation is incomplete, repair it, take an installation
lock, and start a subprocess. Each of those failures deserves a different
operator response, and each must be distinguishable from a lint denial.

ADR 0012 established the root binary's command surface and left the exit policy
for its subcommands unspecified beyond forwarding the `cargo dylint` status.
This record settles the policy for `check`, the first subcommand that runs a
lint and manages provisioning in one invocation.

The constraint that shapes the answer is that two small codes are already taken.
`1` is what Dylint and Cargo report for a lint denial or a compile error, and
`2` is what clap reports for a usage error in both Whitaker and `cargo dylint`.
Whitaker cannot claim either without making its own failures indistinguishable
from the failures it exists to report.

## Decision drivers

- Preserve the status a caller already receives from `cargo dylint`, including
  a non-zero status produced by a lint denial.
- Let automation separate "the code failed" from "Whitaker could not try"
  without parsing human-readable output.
- Offer one code per class of operator action, so the contract stays small
  enough to remember and to document in `check --help`.
- Stay disjoint from every status `cargo dylint` documents, from signal-derived
  statuses, and from Windows process statuses.
- Keep the reason keys stable, untranslated, and closed, so a script may match
  them and prose may change freely.

## Requirements

### Functional requirements

- A completed Dylint run returns that run's status unchanged.
- A Whitaker usage error, a configuration error, a provisioning failure, and a
  host or execution failure each return their own documented code.
- Every operational failure prints a reason key on standard error in a fixed
  form when standard error is writable.
- `check --help` prints the exit table, so a user who has just seen a non-zero
  status can look it up without leaving the terminal.

### Technical requirements

- Exit policy lives in the root CLI's domain layer, not in an adapter, and is
  a total function of the outcome value.
- The reason key is derived from the outcome rather than assembled from an
  error message.
- No code may collide with `cargo dylint`'s `0`, `1`, `2`, `101`, `128+n`, or
  with a negative Windows status carried as an `i32`.

## Options considered

### Option A: pass Dylint through, reserve small codes for Whitaker

Dylint's status is returned unchanged. Whitaker's own outcomes claim `2` to
`9`, of which `3`, `4` and `5` carry meaning today and `6` to `9` are held for
future operational outcomes. Each operational failure prints a stable reason
key when standard error is writable.

This is the chosen option. It answers both questions a caller asks — did the
code pass, and if not, was the fault in the code or the machine — within one
status byte for every status `cargo dylint` documents, and it costs nothing at
the call site. Pass-through is unconditional, so a child status that happens to
land in Whitaker's own range is returned too; the reason key, not the status
alone, then separates the two causes.

### Option B: return `1` for every Whitaker failure

This would keep two codes only: `0` for success and `1` for any failure. It is
simple to remember, but it throws away exactly the distinction the command
exists to make. A pre-commit hook cannot tell a lint denial from a missing
installation, and a continuous-integration job cannot decide between "fix the
code" and "run `whitaker install`".

### Option C: use the `sysexits.h` range, `64` to `78`

The BSD convention gives familiar names such as `EX_USAGE` (64) and
`EX_UNAVAILABLE` (69). It is durable on Unix, but it is unfamiliar in Rust
tooling, has no Windows convention, and makes an ordinary non-zero status look
like a distinguished failure to tools that treat `>= 64` as unusual. It also
invites granularity the command does not need: the operator actions are three,
not fifteen.

| Topic                  | A: pass through and reserve | B: two codes | C: `sysexits.h` |
| ---------------------- | --------------------------- | ------------ | --------------- |
| Lint status preserved  | Yes                         | Yes          | Yes             |
| Prose-free diagnosis   | Yes                         | No           | Yes             |
| Familiar to Rust users | Yes                         | Yes          | No              |
| Windows convention     | Yes                         | Yes          | No              |
| Codes to document      | 6                           | 2            | 15              |

_Table 1: Comparison of the exit-status options._

## Decision outcome / proposed direction

`whitaker check` returns:

- `0` when Dylint and Cargo both succeeded.
- `1` when Dylint or Cargo reported failure: a lint denial, a compile error, a
  rejected Cargo argument, or an internal Dylint error.
- `2` when Whitaker rejected its own arguments, and also when Dylint rejected
  the options Whitaker assembled, which is an internal fault. The status is
  passed through in the second case because Dylint produced it, but the cause
  is Whitaker's.
- `3` for a configuration error. Reserved now, and used from roadmap item
  3.5.4 when `check` gains its configuration flags.
- `4` for a provisioning failure: the installation is incomplete and repair
  was refused, failed, or did not converge; or, under a library path the user
  set, that path does not name exactly one Whitaker suite library, because it
  holds none or more than one.
- `5` for a host or execution failure: the data directory cannot be
  determined, an I/O error occurred, the installation lock failed,
  `cargo dylint` could not be started, or Whitaker could not write its own
  output.
- `6` to `9`, reserved binary-wide for future operational outcomes.
- `101` when `cargo dylint` panicked, and `128 + n` when it was killed by
  signal `n` on Unix, both passed through. Windows statuses pass through as the
  `i32` the process reported.

Every operational failure also writes one line to standard error in the form
`whitaker: error[<key>]: <sentence>`, optionally followed by a `help:` line,
when standard error is writable. The keys are `provision.refused`,
`provision.repair-failed`, `provision.not-converged`, `provision.user-path`,
`host.data-dir`, `host.io`, `host.lock`, `exec.spawn` and `exec.output`. A
completed lint run writes nothing of its own: Dylint has already reported its
result.

An outcome that has already been decided takes precedence over a lost
diagnostic. If the line cannot be written, the write failure is logged and the
decided status is returned unchanged, so a refusal whose key was lost still
exits `4`. The `exec.output` code covers a failure to deliver Whitaker's own
text when no other outcome has been chosen, such as a help request whose text
never reached the caller. A completed Dylint run likewise returns its status
unchanged when Whitaker's built-driver notice cannot be written: the notice is
logged rather than converted into `exec.output`.

Because pass-through is unconditional, a `cargo dylint` run that exits `3`, `4`
or `5` returns a status that overlaps Whitaker's own range. Automation that
must distinguish the two reads the reason key: an operational outcome prints
`whitaker: error[<key>]` when standard error is writable, and a pass-through
prints nothing of Whitaker's own.

The rationale for the reserved ranges is disjointness. `cargo dylint` documents
`0`, `1`, `2` and `101`; a signal-derived status is `129` to `192` and a
Windows NTSTATUS appears as a negative `i32`. `3` to `9` collides with none of
them, leaves room for the configuration and selector work that arrives with
3.5.4, and fits in the small, memorable part of the range where a user will
actually look it up.

Prior art supports the shape rather than the exact numbers. pytest assigns
small codes to operational classes, distinguishing "tests failed" from
"collection failed" and "interrupted". Ruff and ESLint use `0`, `1` and `2`
only, which Whitaker cannot copy because clap already owns `2` in the same
binary.

## Goals and non-goals

- Goals:
  - Preserve the `cargo dylint` status a caller already handles.
  - Make each operator action a distinct, documented code.
  - Make every operational cause machine-readable through a stable key.
- Non-goals:
  - Encode the specific lint that failed in the status. Dylint's own output
    reports that, and the status contract stays coarse.
  - Settle the configuration flag surface. Item 3.5.4 owns `3`.
  - Change the legacy forwarding path's contract, which returns the
    `cargo dylint` status unchanged by ADR 0012.

## Migration plan

1. Land the outcome type, the total mapping to codes, the reason keys and the
   `check --help` table with the `check` command itself, so the contract is
   documented at the moment it becomes observable.
2. Use `3` when the configuration flags arrive in 3.5.4. Until then no
   outcome maps to it.
3. Allocate `6` to `9` only when a future outcome cannot be expressed by the
   existing classes; do not reuse them for a narrower cause that the reason
   keys already distinguish.

## Known risks and limitations

- `2` means both "Whitaker rejected the arguments it was given" and "Dylint
  rejected the arguments Whitaker assembled". The remedies differ: an option
  used wrongly is corrected at the call site, for example
  `whitaker check -- -p x` rather than `whitaker check -p x`, whereas options
  Dylint rejects from Whitaker's own vector are a Whitaker defect to report.
  Advertising only the usage-error reading in `check --help` is a deliberate
  simplification.
- A reason key is a compatibility promise. Renaming one is a breaking change
  for scripts even when no code changes, so the set is closed and documented
  here rather than derived from error types.
- The table in `check --help` must be kept in step with the mapping. A
  reviewer should treat a mapping change without a table change as a defect.
- Windows statuses are opaque. A caller on Windows may receive a negative
  `i32` that means nothing in this contract; passing it through is more useful
  than flattening it to `1`.

## Architectural rationale

The status is domain policy, not adapter detail, so the mapping is a total
function over a closed outcome type in the root CLI, as ADR 0012 requires.
Producing the outcome is where the adapters contribute: a failed preflight, a
refused repair, a lock failure and a spawn failure each become one outcome
variant, and the variant alone decides the code and the key. That keeps the
contract testable without a subprocess, provable over a small closed domain,
and immune to an adapter's error text leaking into a machine-readable field.

## References

- [ADR 0012](adr-0012-whitaker-cli-boundary.md) establishes the root CLI
  boundary and its forwarding contract.
- [Whitaker CLI design](../whitaker-cli-design.md) describes the `check`
  command and its status table.
- [ExecPlan for roadmap item 3.5.2](../execplans/3-5-2-whitaker-check-as-the-default-linting-path.md)
  records the implementation and the decision log entry, D-2, that this ADR
  formalizes.
