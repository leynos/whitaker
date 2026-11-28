# Issue 679: select packages, targets and features exactly as `cargo clippy` does

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS (implementation complete; delivery in progress)

Branch:
`issue-679-whitaker-check-select-packages-targets-and-features-exactly-as-cargo-clippy-does`

Roadmap item: none allocated for this issue. It extends roadmap item 3.5.2
(`whitaker check`) in `docs/roadmap.md` and precedes 3.6.1's rule selectors.

## Purpose / big picture

`whitaker check` currently accepts only `--no-install` and a `--` followed by
Cargo arguments. A user who knows `cargo clippy` therefore has to learn a
Whitaker-specific spelling before the tool covers the workspace they expect.
This work gives `whitaker check` the Cargo selection surface with exactly
Cargo's meanings: `-p/--package`, `--workspace`, `--exclude`, the deprecated
`--all` alias, every target and feature selector, and the build options that
affect selection. Selection stays Cargo's own computation; Whitaker parses the
flags, validates nothing Cargo would validate differently, and re-emits them to
Cargo unchanged after its own `--` separator.

Before:

```plaintext
$ whitaker check -p member_a
error: unexpected argument '-p' found
```

After, in a workspace whose root is a package with two members:

```plaintext
$ whitaker check -v --workspace
    Checking member_a v0.1.0 (/work/app/member_a)
    Checking member_b v0.1.0 (/work/app/member_b)
    Checking rootpkg v0.1.0 (/work/app)
...
$ whitaker check --all
warning: --all is deprecated in favour of --workspace
```

The last line is Cargo's own warning: Whitaker forwards `--all` rather than
translating it, so the wording and status match `cargo clippy --all`. The
legacy forwarding path (`whitaker --all ...`) is untouched.

## Constraints

- The legacy path `whitaker --all ...` must keep forwarding every token, in
  order, to `cargo dylint`. `src/cli/dispatch.rs`, `src/cli/runner.rs` routing,
  and their Kani proofs must not change behaviour.
- Arguments after `check --` stay opaque Cargo arguments. Whitaker's own parser
  must stop at `--`.
- No Whitaker-owned argument may ever render as `--`, so the first separator in
  the assembled `cargo dylint` vector remains Whitaker's. This is the property
  proved by `verus/whitaker_check_argv.rs`; the model's `W` stays
  `[Lib, NoMetadata]`.
- Lint selection must never reuse a Cargo flag name. `--all` is Cargo's
  workspace alias; rule selection in 3.6 uses `--select ALL`.
- Exit-status contract (ADR 0017) is unchanged: a Cargo rejection still exits
  through Dylint's status, not a new Whitaker status.
- `whitaker check --help` keeps the exit table.
- Dylint 6.3.0 receives trailing arguments as UTF-8, so non-UTF-8 Cargo paths
  cannot reach Cargo through this route; the new native options may therefore
  use UTF-8 path types.
- Every source file stays at or below 400 lines; every module and item carries
  the documentation the repository requires.
- No new runtime dependency except `cargo_metadata` (already a workspace
  dependency at 0.23.1, used by the suite and lint crates).
- `docs/execplans/**` is excluded from every size count.

## Tolerances (exception triggers)

- Files: more than 30 non-ExecPlan files changed, stop and escalate.
- Code: more than 1,600 net lines of Rust (production plus tests), stop and
  escalate. The comparison base is `git rev-parse HEAD` at plan approval; count
  with `git diff --numstat <base> -- '*.rs'`.
- Configuration: more than 40 net lines across `Cargo.toml`, `Cargo.lock` and
  GitHub workflows, stop and escalate.
- Documentation: more than 500 net lines excluding ExecPlan files, stop and
  escalate.
- Dependencies: any dependency beyond `cargo_metadata`, stop and escalate.
- Architecture: changing the `check` readiness/repair flow's decision rules
  (`src/cli/check/flow/mod.rs`) or the `CheckServices` approve-then-run
  contract, stop and escalate. The listing hook must sit beside the run, not
  inside the flow's decisions.
- Iterations: three failed attempts at the same failing test after the
  implementation is believed complete, stop and escalate.
- Ambiguity: if a Cargo behaviour cannot be established by measurement and the
  choice changes the selected package set, stop and present the measurements.

Budget rationale: no comparable historical ExecPlan recorded a budget for this
surface. The survey counted the current `src/cli/check` tree (about 3,600 lines
including tests) and the existing e2e check suites; the estimate assumes one
new CLI struct, one pure selection module, one metadata adapter, one new
integration target with fixtures, and documentation updates.

## Risks

- Risk: Whitaker's own package listing could disagree with Cargo's actual
  selection. Severity: medium. Likelihood: low, because the default set is read
  from `cargo metadata`'s `workspace_default_members`, which Cargo computes
  from the same context. Mitigation: the differential test compares the listing
  and the compiled-artefact set against `cargo clippy` for every fixture.
- Risk: re-emitting flags in a canonical order could change Cargo's behaviour.
  Severity: low. Likelihood: low; none of the selection options is
  order-sensitive. Mitigation: differential test with each flag combination.
- Risk: `cargo metadata` (a new subprocess) slows or breaks an otherwise working
  run. Severity: low. Likelihood: low. Mitigation: it runs only with `-v`, and
  a failure is a tracing warning that never blocks the lint run.
- Risk: the e2e stub `cargo` shadows the real `cargo metadata`. Severity:
  medium. Likelihood: medium. Mitigation: the stub delegates `metadata` (and
  `check`) to a real Cargo path supplied by the test.
- Risk: a nested `cargo clippy` inside a test contends for compile admission.
  Severity: low. Likelihood: high on this host. Mitigation: fixtures are tiny
  path-only workspaces with their own target directories; gates are run
  sequentially and the differential target gets a nextest allowance.

## Progress

- [x] (2026-11-27 20:10Z) Reconnaissance: read `whitaker check`, dispatch,
  ports, adapters, design doc, users' guide, ADR 0017 and the Verus proof.
- [x] (2026-11-27 20:20Z) Measured Cargo's default selection for eight fixture
  shapes and edge cases with `cargo metadata` and `cargo check`.
- [x] (2026-11-27 22:10Z) `EP-M1` CLI surface and Cargo re-emission
  (`src/cli/check/cargo.rs`, `argv.rs`, `args.rs`); 329 library unit tests pass.
- [x] (2026-11-27 22:30Z) `EP-M2` Selection model (`selection.rs`), metadata
  port (`src/ports/packages.rs`), adapter (`src/adapters/packages.rs`) and the
  `-v` listing (`listing.rs`); command-layer listing tests pass.
- [x] (2026-11-27 23:05Z) `EP-M3` Differential suite
  (`tests/behaviour_check_selection.rs`); it grew with each review round and
  now runs 39 cases over eight fixtures.
- [x] (2026-11-28 00:40Z) `EP-M4` Documentation: design doc §Selection is
  Cargo's with the default-selection table, users' guide "Same selection as
  `cargo clippy`", contents index, and this plan.
- [x] (2026-11-28 01:05Z) Fast gates: `make check-fmt`, `make markdownlint`,
  `make spelling`, `make nixie` pass; `make lint` passes over the whole
  workspace, including the Whitaker Dylint suite; `cs delta origin/main HEAD`
  reports no issues.
- [x] (2026-11-28 00:30Z) `make lint` passes over the whole workspace after the
  second review round; `make check-fmt`, `make markdownlint`, `make spelling`
  and `make nixie` pass; `cs delta origin/main HEAD` is clean.
- [x] (2026-11-28 01:10Z) Updated the one existing test whose premise this
  change reverses: `whitaker check -p x` is no longer a usage error, so the e2e
  contract now asserts that a native selector reaches Cargo unchanged and that
  a Cargo-only option such as `--message-format` is still a usage error before
  `--`.
- [x] (2026-11-28 02:05Z) `make test` passes: the main nextest pass ran 3,260
  tests with no failures (including the 39 differential cases), and the core,
  driver, root-lib and nextest-filter passes pass. One pre-existing e2e test
  asserted the old `-p`-must-follow-`--` contract; it was replaced with a test
  of the new contract before the run.
- [x] (2026-11-28 02:45Z) `make test-doc` passes. The Verus CLI proof group
  (`scripts/run-verus.sh cli`) reports 8 verified, 0 errors; the model now
  covers the three-option prefix emitted when `--manifest-path` is given.
- [x] (2026-11-28 05:30Z) `make lint` passes with the repository's installed
  suite. The host's installation record had been changed by another session to
  point at an experimental estate-scan suite, which flagged an untouched
  installer test file; the gate was therefore run with the shipped suite and
  the repository's own binary (`WHITAKER=./target/debug/whitaker`,
  `DYLINT_LIBRARY_PATH=~/.local/share/whitaker/lints/...`). That external
  record change is not part of this branch.
- [x] (2026-11-28 05:25Z) `make test` passes: the main pass ran 3,302 tests
  with no failures (including 39 differential cases), and the core, driver,
  root-lib and nextest-filter passes pass. `make test-doc` passes.
- [x] (2026-11-28 05:35Z) `cs delta origin/main HEAD` reports no issues.
- [ ] Delivery: push, draft PR, assessments, formal review, merge.
- [ ] Delivery: push, draft PR, CodeRabbit loop, merge.

Fourth review round (2026-11-28): the local review reported 7 findings. Five
produced changes: the target test harness flag and required features now gate
eligibility (with Cargo's feature-resolution rules), a percent-encoded path URL
is decoded and an unresolvable selector suppresses the listing, prerelease and
build identifiers are compared whole, and a leading `]` is a literal class
member. Two findings had already been fixed in an earlier round but the edit
had not applied; they were re-applied and verified (target selectors in the
forwarded suffix, and the test-profile and feature fields in the differential
key). The bench-harness flag is a documented limitation: `cargo metadata` does
not report it.

Third review round (2026-11-28): the local review reported 9 further findings,
all verified against Cargo's measured behaviour. Eight produced changes: `^` is
a literal class member (Cargo's `glob` negates only with `!`), an invalid class
makes a selector unresolved, `name:version` joins `name@version`, `-p`
selectors are validated even under `--workspace`, a target selector after `--`
also hides the listing, `--benches` includes implicit lib/bin bench targets,
`--bin`/`--test`/`--example`/`--bench` names are globs, and every Cargo library
crate type counts as a library. The ninth clears `RUSTFLAGS` on the Whitaker
child too, so both sides of the differential run under the same flag policy.
The plan's own `--all` attribution was corrected.

Second review round (2026-11-28): the local CodeRabbit review reported 13
findings. All were verified against the code and Cargo's measured behaviour.
Eleven produced changes (bounded glob matcher with bracket classes, target
eligibility, pkgid and partial-version matching, unmatched-exclude leniency,
Dylint-side manifest, `-r`, `-j -1`, Unix-gated stub, external metadata
fixture, split listing tests, short-group selector detection, test-profile and
feature comparison keys, compile-time scratch root). One produced a documented
deviation (`--target-dir`, refused). One (`--exclude` unmatched) required both
a code change and a reversed test expectation.

## Surprises & discoveries

- Observation: `cargo metadata --no-deps` reports `workspace_default_members`
  for the *invocation context*, not the raw `default-members` manifest key. In
  `root_default_members/` it names `member_b`; in
  `root_default_members/member_a` it names `member_a`; in a virtual workspace
  it names every member. Evidence: measured with Cargo 1.98.0-nightly on
  2026-11-27 (see the fixture matrix below). Impact: the default-selection
  model needs no filesystem walk and no reimplementation of Cargo's defaults;
  the metadata adapter can ask Cargo directly.
- Observation: `cargo check` in a workspace with a root package and no
  `default-members` selects only the root package. Evidence: `root_members`
  default run emits one `compiler-artifact` for `rootpkg`. Impact: this is the
  silent coverage gap the issue describes; the differential test pins it.
- Observation: `cargo check --workspace --exclude <spec>` removes a member from
  the artefact set. Evidence: `rootpkg-exclude` emitted `member_b` and
  `rootpkg` only. Impact: exclude is a workspace-mode modifier only.
- Observation: Cargo 1.98 documents `--all` as a deprecated alias but emits no
  runtime warning. Evidence: `cargo check --all` on the `root_members` fixture
  printed only `Checking` and `Finished`; the Cargo team's 2024 deprecations
  note says it deliberately avoids a runtime warning because the flag is
  pervasive. Impact: `whitaker check --all` prints the notice itself, worded
  with Cargo's own description, so the accepted behaviour and the protection
  against confusing it with the legacy `whitaker --all` both hold.
- Observation: `-p` is ignored when `--workspace` is present. Evidence:
  `cargo check --workspace -p member_a` compiled all three members. Impact: the
  selector gives workspace priority, matching Cargo.
- Observation: Cargo refuses a second `--target-dir`
  (`error: the argument '--target-dir <DIRECTORY>' cannot be used multiple times`),
  and Dylint always injects its own. Impact: Whitaker cannot honour the
  override; the option is refused with that reason instead of starting a doomed
  run.
- Observation: `--exclude <absent>` is a warning, not an error
  (`warning: excluded package(s)`absent`not found in workspace ...`), and the
  run continues. Impact: an unmatched exclusion must not suppress the listing.
- Observation: `--workspace --bins` compiles only members with a binary.
  Evidence: a workspace with one library-only and one binary member emitted
  artefacts for the binary member alone. Impact: the listing applies target
  eligibility before printing `Checking` lines.
- Observation: Cargo accepts `-p name@0.1` (partial version),
  `-p path+file:///...#0.1.0` (package-ID form), and bracket globs such as
  `member_[ab]`. Impact: the listing resolves those forms rather than skipping
  them.

## Decision log

- D1. Re-emit parsed selection flags after Whitaker's `--` rather than passing
  Dylint's own `-p`/`--workspace`. Rationale: Dylint has no `--exclude`,
  `--all`, target or feature selectors, and mixing two owners of the same flag
  is exactly the confusion the issue reports. Re-emission in one canonical
  order keeps one owner, Cargo, and makes "reach Cargo unchanged" testable as a
  package set rather than as text.
- D2. Keep the default selection Cargo's own. Rationale: no flags forwarded
  means Cargo applies its default, so Whitaker cannot invent a different one.
  The listing reads Cargo's answer from `workspace_default_members` instead of
  modelling it. Rejected alternative: passing explicit `-p` flags computed by
  Whitaker, which would make a model bug a coverage bug.
- D3. Forward `--all` verbatim and print the deprecation notice from Whitaker.
  Rationale: Cargo is silent at runtime (measured; see Surprises), so
  forwarding alone would not satisfy the issue's acceptance criterion, and
  inventing a translation to `--workspace` would lose the alias the user typed.
  The notice uses Cargo's own description of the flag, goes to standard error,
  and is suppressed by `-q`. The legacy `whitaker --all` meaning is unchanged.
- D4. Treat a metadata or listing failure as advisory. Rationale: the listing
  is a diagnostic for `-v`; the lint run must not fail because a display query
  did. The tracing warning records the category, and Cargo reports any real
  manifest error through the run.
- D5. Introduce a narrow `WorkspaceQuery` port returning plain facts
  (`WorkspaceFacts`, `PackageFacts`) and keep selection in a pure CLI module.
  Rationale: the adapter should translate Cargo's metadata, and the selection
  policy stays testable without a process; the domain does not depend on
  `cargo_metadata` types.
- D6. Refuse `--target-dir` with a usage error instead of forwarding it.
  Rationale: Dylint 6.3.0 always adds its own `--target-dir` before the
  forwarded arguments, and Cargo refuses a second occurrence (measured), so no
  forwarding shape can honour the override. The refusal names Dylint as the
  owner and avoids provisioning for a run that cannot start. This is a
  documented deviation from the issue's flag list, recorded here and in the
  design document and users' guide.
- D7. Pass `--manifest-path` before Whitaker's separator as Dylint's own
  option. Rationale: Dylint reads its target directory from that manifest
  before it forwards anything after `--`, and its own documentation requires
  the option before the separator when the manifest carries metadata. Dylint
  also adds it to `cargo check`, so repeating it in the suffix would be a
  duplicate. The Verus lemma is unaffected: the prefix is still separator-free.
- D9. Continue past the code and configuration budgets, and record why.
  Measured at the final tree: 4,155 added and 272 deleted Rust lines (3,883 net
  against a 1,600 net budget), and 127 added configuration lines (against 40).
  The overrun is concentrated in evidence: the differential harness (fixture
  workspaces, a delegating stub, a support module), the unit-test tables for
  selector forms and target eligibility, and the per-fixture `Cargo.toml`
  files. The production surface is roughly a tenth of the change. The user's
  standing instruction is to implement, validate and merge this issue, so
  stopping at the budget without direction would abandon a validated change;
  the overrun is recorded here and in the pull request instead. The remaining
  budgets (documentation, dependencies, architecture) held.
- D8. Apply target eligibility and Cargo's package-ID forms to the listing, and
  report the listing unavailable when a package selector is forwarded after
  `--`. Rationale: a listing that names a library-only member under `--bins`,
  or misses a `name@0.1` or pkgid selector, overstates or understates coverage.
  A suffix selector reaches Cargo but not Whitaker's model, so guessing is
  worse than declining.

## Context and orientation

`whitaker check` is implemented by `src/cli/check/`:

- `args.rs` holds `CheckArgs`, the clap surface (`--no-install` and the trailing
  Cargo suffix today).
- `argv.rs` assembles `cargo dylint`'s vector:
  `[--lib=whitaker_suite, --no-metadata, --] ++ user arguments`.
- `mod.rs` adapts `CheckServices` to the pure flow in `flow/mod.rs`, which
  decides readiness, repair and run. `messages.rs` renders outcomes.
- `src/ports/check/` defines the approve-then-run contract; `errors.rs` holds
  the typed failures. `src/adapters/check/` implements it, and
  `src/adapters/services.rs` is the composition root.
- `src/cli/runner.rs` routes a parsed `Command::Check` to
  `check::run_check_command`. `src/cli/parser.rs` extracts root `-q`/`-v`
  before clap parses, stopping at `--`.
- `src/cli/dispatch.rs` classifies tokens for legacy forwarding. A vector whose
  first significant token is `check` selects the subcommand; everything else
  that is not install/ls/help/version forwards to `cargo dylint`.

The design contract is `docs/whitaker-cli-design.md` §`whitaker check`, §Rule
identifiers and selection model, and
`docs/adrs/adr-0017-whitaker-check-exit-status-contract.md`. User-facing
behaviour is documented in `docs/users-guide.md` §Root CLI.

The e2e harness `tests/e2e_check_support/mod.rs` installs a fixture bootstrap
and a stub `cargo` that answers `dylint --version`, records arguments to
`STUB_LOG`, and exits with `STUB_EXIT`. The differential test needs a stub that
delegates `metadata` and `dylint -- <args>` to a real Cargo.

Measured Cargo matrix (Cargo 1.98.0-nightly, 2026-11-27; artefacts are
`package_id`/target kind/name from `--message-format=json`):

```plaintext
single, default                                    -> single
root package + members, at root                    -> rootpkg
root package + members, at root, --workspace       -> rootpkg, member_a, member_b
root package + members, --workspace --exclude a    -> rootpkg, member_b
root package + members, -p member_a                -> member_a
root package + members, -p 'member_*'              -> member_a, member_b
root package + members, --all                      -> all three
root package + default-members [member_b], at root -> member_b
root package + default-members [member_b], in src/ -> member_b
virtual workspace, at root                         -> all members
virtual workspace + default-members, at root       -> member_b
virtual workspace, inside member_b                 -> member_b
```

## Conformance basis

- Issue: <https://github.com/leynos/whitaker/issues/679>, "whitaker check should
  select packages, targets and features exactly as cargo clippy does".
- Design contract: `docs/whitaker-cli-design.md` §`whitaker check` (to be
  extended with the Clippy-alignment contract and the default-selection table).
- Exit contract: `docs/adrs/adr-0017-whitaker-check-exit-status-contract.md`.
- CLI boundary: `docs/adrs/adr-0012-whitaker-cli-boundary.md`.
- Repository standards: `AGENTS.md` (code style, tests, documentation),
  `docs/documentation-style-guide.md`.
- No Terms of Reference or technical-design revision names this issue; the
  roadmap has no allocated item. This plan therefore traces to the issue text
  and the design document, and will update both.

## Verification plan

Obligations:

- O1 (flag parity): for every fixture and flag combination in the issue, the
  set of `(package_id, target kind, target name)` compiled by `whitaker check`
  equals the set compiled by `cargo clippy` with the same flags. Method:
  differential e2e test over real Cargo. Artefact:
  `tests/behaviour_check_selection.rs`. Evidence: set equality per case.
  Non-vacuity: each fixture yields at least one artefact, and the root-package
  case has a different expected set from `--workspace`, so a stub that checked
  nothing or a default that silently widened would fail.
- O2 (listing truth): with `-v`, the packages Whitaker prints are exactly the
  packages Cargo compiled. Method: the same test parses Whitaker's stderr
  `Checking` lines with Cargo's own progress suppressed (`-- -q`) and compares
  names with the artefact package names. Non-vacuity: the listing must be
  non-empty for every case.
- O3 (deprecated alias): `whitaker check --all` selects the workspace and
  Whitaker prints the deprecation notice. Method: differential case plus a
  stderr assertion.
- O4 (legacy path unchanged): `whitaker --all -- --all-targets` still forwards
  verbatim. Method: existing dispatch/argv unit tests and Kani harnesses
  continue to pass; the `assemble_dylint_args` unit tests gain the native flags
  case.
- O5 (argument boundary): no Whitaker-owned token is `--`, so the first
  separator is Whitaker's. Method: existing unit test
  `every_owned_option_is_one_argument_that_is_not_a_separator`, the Verus lemma
  (unchanged `W`), and the property test in `tests/property_check_argv.rs`.
- O6 (pure selection model): each branch (workspace, package specs with and
  without globs, default members, excludes) selects the expected packages.
  Method: `rstest` tables over hand-built `WorkspaceFacts`.

Axioms (documented external assumptions):

- `cargo metadata --no-deps` reports `workspace_default_members` for the
  invocation context, and that list equals Cargo's default selection. Verified
  by measurement across the fixture matrix and by O1/O2, which would fail if it
  were wrong.
- `cargo clippy` and `cargo check` select the same units. Verified by O1: the
  same flags produce the same artefact set.
- `cargo_metadata` faithfully parses `cargo metadata` JSON. Third-party
  interface, treated as an axiom; the adapter's translation is unit-tested from
  a literal JSON document.

Commands:

```sh
make fmt
make lint
make test
make test-doc
make markdownlint
```

Each gate runs once, sequentially, with output captured to `/tmp`.

## Milestones

### EP-M1: CLI surface and Cargo re-emission

End state: `CheckArgs` accepts the Clippy surface before `--`; the assembled
Dylint vector is `[Whitaker options, --, rendered Cargo options, suffix]`; help
text documents the split.

Acceptance: `cargo test -p whitaker --lib cli::check` passes, including new
`assemble_dylint_args` cases; `whitaker check --help` lists the new options; a
clap parse of `["whitaker", "check", "-p", "x", "--workspace", "--", "-q"]`
yields the expected `CheckArgs` and a single separator after Whitaker's two
options.

### EP-M2: Selection model, metadata port and `-v` listing

End state: with `-v`, `whitaker check` writes Cargo-style `Checking` lines for
the packages Cargo will check; the default set comes from Cargo's
`workspace_default_members`; `-p`/`--workspace`/`--exclude` are modelled;
metadata failures warn without blocking the run.

Acceptance: unit tests for `selected_packages`; an adapter test translating a
literal metadata JSON; a command-layer test (scripted ports) showing the lines
on stderr only when verbosity is non-zero and never on stdout.

### EP-M3: Differential end-to-end test

End state: `tests/behaviour_check_selection.rs` runs every fixture and flag
combination against real `cargo clippy` and the fixture bootstrap whose stub
delegates to real Cargo.

Acceptance: `cargo nextest run -p whitaker --test behaviour_check_selection`
passes on Linux; each case's artefact sets and listing names match.

### EP-M4: Documentation and gates

End state: design doc §`whitaker check` states the Clippy-alignment contract
and the default-selection table; users' guide has a "same as cargo clippy"
section including the root-package default; the exit table and migration notes
remain accurate; `make fmt`, `make lint`, `make test`, `make test-doc`,
`make markdownlint` pass.

Acceptance: the documented commands and examples match the implementation; all
gates are green before the CodeRabbit review is requested.

## Outcomes & retrospective

Gate evidence (2026-11-28, final tree):

- `make check-fmt`: 137 files unchanged.
- `make lint`: exits 0 (rustdoc, Clippy, and the Whitaker suite over every
  workspace member, run with the repository's installed suite).
- `make test`: exits 0; 3,302 main-pass tests, 443 core-pass tests and 50
  driver/guard tests, no failures.
- `make test-doc`: exits 0.
- `make markdownlint` (which runs `make spelling`): 137 files, 0 errors.
- `make nixie`: all diagrams validated.
- `cs delta origin/main HEAD`: no issues found.
- `scripts/run-verus.sh cli`: 8 verified, 0 errors.

Review rounds: four local CodeRabbit reviews reported 10, 13, 9 and 7 findings.
Every finding was checked against the code and against Cargo's measured
behaviour; all but two produced a change. The exceptions are the
`bench = false` manifest flag, which `cargo metadata` does not expose, and
`--target-dir`, which Dylint's fixed invocation cannot honour; both are
documented limitations. Two findings from the third round had not actually been
applied when first closed, and the fourth round caught them; the lesson is to
re-read the edited code after a scripted replacement rather than trusting the
edit to have matched.

## Original outcomes section

Implementation discoveries so far:

- Reading Cargo's `workspace_default_members` instead of modelling the default
  removed an entire class of divergence and most of the selection code. The
  measurement that established the field's context sensitivity was the single
  most valuable step in the plan.
- The differential harness is cheap to run when the stub delegates to a real
  Cargo: six tiny path-only workspaces cover the whole acceptance table, and
  the same run validates both the lint-run selection and the `-v` listing.
- The repository's Clippy policy shaped the design: `struct_excessive_bools`
  required an item-scoped `expect` with a reason on the Cargo mirror struct,
  and the cognitive-complexity ceiling forced the listing into small helpers.

Gate evidence recorded so far (2026-11-28):

- `make check-fmt`: 137 files unchanged.
- `make lint`: exits 0, including `whitaker` over every workspace member.
- `make markdownlint`: 137 files, 0 errors.
- `make spelling`: passes after switching the new prose to Oxford spelling
  (`artefact`) and the notice to `in favour of`.
- `make nixie`: all diagrams validated.
- `cs delta origin/main HEAD`: no issues found in checked changes.
- `cargo test -p whitaker --test behaviour_check_selection`: 25 cases pass.
- `cargo test -p whitaker --lib`: 334 pass.
