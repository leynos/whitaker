# Implement `whitaker check` as the default linting path

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT

Roadmap item: 3.5.2 in `docs/roadmap.md`.

## Purpose / big picture

Today a Whitaker user must provision first and lint second:

```console
cargo install whitaker
whitaker install
whitaker --all -- -p my-crate --all-targets
```

The last line is not a Whitaker command at all. The root `whitaker` binary does
not recognize `--all`, so it forwards every argument verbatim to `cargo dylint`
(requirement `CLI-REQ-FWD`, recorded in
`docs/adrs/adr-0012-whitaker-cli-boundary.md`). If nothing has been installed,
the user gets Dylint's own error about missing libraries rather than any
Whitaker guidance.

After this change, the documented path is:

```console
cargo install whitaker
whitaker check
whitaker check -- -p my-crate --all-targets
```

`whitaker check` notices a missing or incomplete installation, repairs it, and
then runs the Whitaker core lints. It passes every argument after `--`
unchanged to Cargo. Continuous integration can opt out of the repair with
`whitaker check --no-install`. That turns a missing installation into a
distinct, documented failure instead of a network download in the middle of a
build.

The exit status becomes a contract that automation can rely on:

- When Dylint ran, `whitaker check` exits with Dylint's status unchanged, so
  lint failures look exactly as they do with `cargo dylint` today.
- When Whitaker could not get as far as running Dylint, it exits with a
  reserved operational code. That code is never `0` or `1`, so an installation
  failure can never be mistaken for a clean run or for lint findings.

Success is observable. In a fresh home directory, `whitaker check` in a small
crate installs the suite, runs the lints, and returns the same status as
`cargo dylint`. Run a second time, it performs no installation work. Run with
`--no-install` against an empty home directory, it exits with the provisioning
code and prints advice to run `whitaker install`. With
`-- --message-format=json`, standard output contains only Cargo's JSON stream,
even when a repair ran first.

## Definitions

These terms are used throughout. They are defined here so that no prior
knowledge is assumed.

**Dylint.** A tool that runs custom Rust lints compiled as dynamic libraries.
`cargo-dylint` is its Cargo subcommand. `dylint-link` is the linker wrapper it
needs to build lint libraries. Whitaker's lints are Dylint lints.

**Lint bundle.** A compiled Whitaker lint library, such as
`libwhitaker_suite@nightly-2026-05-28.so`, staged in a directory that Dylint
reads through `DYLINT_LIBRARY_PATH`. The file name encodes the nightly
toolchain that built it.

**Bootstrap.** The installed launcher recorded at
`<whitaker_data>/whitaker-bootstrap/bootstrap.json`. It runs `cargo dylint`
with Whitaker's verified Clippy snapshot, its managed driver slot, and a lock
that serializes the first ("cold") driver build. Forwarding already prefers it
(`src/adapters/mod.rs`, `forward_source`).

**Readiness.** Whether everything `check` needs to run Dylint is present: the
Dylint tools at their pinned versions, the bundle's toolchain, and a lint
bundle for that toolchain and the host target.

**Lazy repair.** Running the installation flow from inside `check` only when
readiness reports something missing, then checking readiness again before
linting.

**Operational error.** A failure that stops Whitaker before Dylint produces a
lint result. Examples are an invalid configuration, a missing installation under
`--no-install`, a failed repair, or a failure to start the Dylint process.

**Forwarded Cargo arguments.** The arguments after the first `--` on the
`whitaker check` command line. Dylint passes them to `cargo check`.

**Port / adapter.** A port is a trait owned by the command-policy layer
(`src/ports.rs`) that describes something the policy needs from the outside
world. An adapter (`src/adapters/`) implements it against the real system.
Tests substitute fakes.

**Kani.** A bounded model checker for Rust. A `#[kani::proof]` harness explores
every execution of real Rust code within stated bounds.

**Verus.** A deductive verifier for Rust-like specifications. In this
repository, files under `verus/` model the implementation and prove properties
for all inputs. Kani then checks the real code against the same property within
bounds (see `docs/developers-guide.md`, "Verus scope and trust boundary").

## Constraints

These are hard invariants. Violating one requires escalation, not a workaround.

1. `CLI-REQ-FWD` remains in force. Every invocation that is not `install`,
   `ls`, `check`, root help, or root version is still forwarded verbatim to
   `cargo dylint`, with its status preserved. `whitaker --all ...` must keep
   working, because `make lint` and CI use it. Forwarding is removed only in
   roadmap item 3.9.3.
2. The exit-code policy for `install`, `ls`, display requests, usage errors and
   legacy forwarding does not change (`EP-INV-EXIT` from the 3.5.1 plan). The
   new codes apply only to `check`. Unifying the other commands is a separate
   decision (see `Decision log`, D-5).
3. The command layer decides whether and when to repair. The installer library
   decides how (ADR 0012). `check` must not reimplement installation steps. It
   calls the existing `InstallService` port.
4. During `check`, Whitaker writes nothing to standard output except what the
   Dylint child writes. All Whitaker progress, repair output and diagnostics go
   to standard error. This includes the installer's
   `whitaker-installer: suite-source=...` marker, which goes to standard output
   during `whitaker install`.
5. A non-empty, user-provided `DYLINT_LIBRARY_PATH` stays authoritative, as it
   is for forwarding. `check` never repairs or replaces a user-managed library
   path.
6. Domain and reusable library crates do not depend on `ortho_config`
   (ADR 0008). Only the root application boundary may.
7. No new external dependencies. The root package already has `rstest`,
   `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, `proptest`,
   `tempfile` and `cap-std` as development dependencies.
8. No file may exceed 400 lines. `src/adapters/mod.rs` (360 lines),
   `src/adapters/library_path/mod.rs` (399) and `src/adapters/telemetry/mod.rs`
   (388) are already near the limit. New `check` adapter code goes in new
   modules.
9. Tests never mutate the process environment directly (`AGENTS.md`). End-to-
   end tests set the environment on the spawned child `Command` only.
10. Rule codes, selectors, exit codes and JSON keys are stable and never
    translated. Human-facing prose goes through the existing localization path
    with an `en-GB` fallback.
11. Proof files contain no `assume`, `admit`, `external_body` or equivalent
    trusted shortcut in their final form.

## Tolerances (exception triggers)

ExecPlan files are excluded from every count below, both file counts and line
counts, including when this plan is edited in the same commit.

- Scope: if milestones `EP-M0` to `EP-M3` together need more than 30
  non-ExecPlan files or more than 2,500 net lines of code and documentation,
  stop and escalate. `EP-M4` and `EP-M5` have a separate budget of 20 files and
  1,500 net lines.
- Installer interface: adding public query functions to `whitaker-installer`
  is in scope. Changing the signature of an existing public installer function,
  `InstallArgs`, or `run_install_for_whitaker` requires escalation.
- Packaging: if `EP-M4` needs the root package to depend on the
  `publish = false` `whitaker-cli` package, stop. That is an ADR 0008 decision
  (see `Risks`).
- Prerequisites: if a prerequisite item (3.6.2, 3.6.4 or 3.7.1) has merged with
  an interface that contradicts this plan's consumed-interface assumptions,
  stop and revise the plan before writing code against it.
- Dylint semantics: if the `EP-M0` prototype falsifies hypothesis H-1, H-2 or
  H-3 (see `Plan of work`), stop and revise the invocation design before
  `EP-M1`.
- Iterations: if a focused test still fails after three fix attempts, stop and
  record the failure in `Surprises & discoveries`.
- Verification: if a Verus obligation fails to verify after two hours of
  proof work, or `./scripts/run-verus.sh cli` exceeds 600 seconds on two
  consecutive runs, stop and escalate. Do not weaken the theorem.
- Ambiguity: if the design document and roadmap disagree on behaviour that a
  test would observe, stop and present the options.

## Risks

- Risk: the roadmap prerequisites are not merged. Item 3.5.2 requires 3.6.2
  (selector precedence), 3.6.4 (shared presentation flags) and 3.7.1
  (`whitaker install` with `--offline`, `--build-from-source` and
  `--toolchain`). None is ticked in `docs/roadmap.md`, 3.6.1 (rule codes) is
  also open, and no open pull request covers any of them as of 2026-11-11.
  Severity: high. Likelihood: certain. Mitigation: this plan separates work
  that depends on those items (`EP-M4`, `EP-M5`) from work that does not
  (`EP-M0` to `EP-M3`). Each dependent milestone has an explicit entry gate.
  Starting the independent milestones before the prerequisites merge is a
  deviation from the roadmap's ordering and needs explicit approval
  (`Decision log`, D-1).

- Risk: `cargo dylint` reports lint denials and its own failures, such as a
  driver build failure or a library that cannot load, with the same exit status,
  `1`. Its `main` returns `anyhow::Result`, and a clap usage error exits `2`.
  Severity: medium. Likelihood: certain. Mitigation: `check` reduces
  Dylint-internal failures by establishing readiness first. ADR 0014 documents
  status `1` as "Dylint reported failure", not "lint findings only". Whitaker's
  own operational codes are chosen outside Dylint's documented set
  `{0, 1, 2, 101}`.

- Risk: multiple installed toolchains make library selection ambiguous. The
  forwarding library path joins every recorded staging directory. `--lib NAME`
  errors when one name matches more than one toolchain, while `--all` loads
  every discovered library, including libraries from the user's
  `[workspace.metadata.dylint]`. Severity: medium. Likelihood: medium.
  Mitigation: prototype `EP-M0` settles the invocation shape against the pinned
  `cargo-dylint` before any production code (H-1, H-2).

- Risk: wiring `whitaker.toml` into the published root package is blocked
  because the configuration package `whitaker-cli` is `publish = false`. A
  published crate cannot have a non-development dependency on an unpublished
  one. Severity: medium. Likelihood: high if 3.6.4 has not already resolved it.
  Mitigation: `EP-M4`'s entry gate checks whether a prerequisite has wired
  configuration into the root. If none has, escalate with options rather than
  choosing a packaging change silently.

- Risk: two concurrent `whitaker check` runs both repair lazily. Severity:
  medium. Likelihood: low on developer machines, plausible in CI matrices
  sharing a home directory. Mitigation: `EP-M0` records whether the installer
  serializes concurrent installs. The bootstrap already serializes cold driver
  builds. If installs are not serialized, the repair step takes the installer's
  lock, if one exists, or the risk is documented in the users' guide and
  escalated.

- Risk: the readiness probe is too slow for an everyday command. It may run
  `cargo dylint --version`, `cargo install --list` and `rustup run`, each a
  subprocess. Severity: low. Likelihood: medium. Mitigation: measure in `EP-M0`
  with `hyperfine`. If the probe adds more than 500 ms, escalate with a
  proposal to probe cheaply first and verify versions only when the cheap probe
  passes.

- Risk: claiming the word `check` changes behaviour for anyone who runs
  `whitaker check ...` today expecting forwarding. Today that becomes
  `cargo dylint check`. Severity: low. Likelihood: low, because `check` is not a
  `cargo dylint` option or operation, so that invocation fails today.
  Mitigation: record the change in the users' guide and the design document.

## Progress

- [x] (2026-11-11) Branch renamed to
  `3-5-2-whitaker-check-as-the-default-linting-path`.
- [x] (2026-11-11) Reconnaissance of the root CLI, the installer, the
  configuration package and the verification infrastructure.
- [x] (2026-11-11) External semantics of `cargo dylint` exit statuses and
  library selection researched (see `Surprises & discoveries`).
- [x] (2026-11-11) Draft ExecPlan written.
- [ ] Expert design review and revision.
- [ ] Plan approved by the user, including decision D-1.
- [ ] `EP-M0` prototype: Dylint invocation semantics settled.
- [ ] `EP-M1` provisioned-path `check` with the exit contract.
- [ ] `EP-M2` lazy repair and `--no-install`.
- [ ] `EP-M3` documentation plateau.
- [ ] `EP-M4` configuration and presentation (gated on 3.6.4).
- [ ] `EP-M5` rule selection and install behaviour flags (gated on 3.6.2 and
  3.7.1), and roadmap item 3.5.2 marked done.

## Surprises & discoveries

- Observation: none of the roadmap prerequisites for 3.5.2 is complete.
  Evidence: `docs/roadmap.md` lines 245 to 273 show 3.6.1, 3.6.2, 3.6.4 and
  3.7.1 unticked. `gh pr list --state open` shows no pull request for them.
  Impact: drives decision D-1 and the milestone split.

- Observation: the root CLI's exit policy maps every operational failure to
  `1` (`src/cli/exit.rs`, `exit_code_for`). That is the same status
  `cargo dylint` returns for lint failures. Evidence:
  `Outcome::OperationalFailure => 1`. The `cargo-dylint` `main` returns
  `dylint::ColorizedResult<()>`, so any `Err` exits `1` (`trailofbits/dylint`,
  `cargo-dylint/src/main.rs`). Impact: `check` needs its own exit contract (ADR
  0014, decision D-2).

- Observation: `cargo dylint` passes everything after `--` to `cargo check`
  (`#[clap(last = true)] args`). Its help also warns that `--manifest-path`
  must appear before `--` when the manifest uses metadata. Impact: `check`
  places all of its own Dylint options before a single inserted `--` (see
  `CHK-LEM-ARGV`).

- Observation: there is no compiled-in Whitaker toolchain. The toolchain for a
  bundle comes from the workspace or managed clone at install time. It is
  recorded in the bootstrap metadata and in the library file name. Impact:
  readiness takes the toolchain from the installation's own records, not from
  the user's project.

- Observation: the root package does not depend on `whitaker-cli`, which is
  `publish = false`. Impact: configuration integration is not free (see
  `Risks`).

- Observation: no installer function reports what is missing without also
  installing it at the orchestration level. `check_dylint_tools` is a pure
  query. Toolchain presence is reachable through `Toolchain::verify_installed`.
  Bundle presence has no query, but the root `active_library_path` comes close.
  Impact: `EP-M1` adds a readiness adapter composed from these.

- Observation: ADR number 0013 is taken on the unmerged branch
  `6-1-3-adr-formalizing-the-brain-trust-lint-driver-interfaces`. Impact: the
  new exit-contract ADR is numbered 0014.

## Decision log

- Decision D-1 (awaiting approval): deliver 3.5.2 in two phases inside one
  roadmap item. Phase one (`EP-M0` to `EP-M3`) delivers routing, readiness,
  lazy repair, `--no-install`, forwarded Cargo arguments and the exit contract
  against the installer that exists today. Phase two (`EP-M4`, `EP-M5`)
  consumes 3.6.2, 3.6.4 and 3.7.1 once they merge. The roadmap entry is ticked
  only after phase two. Rationale: the item's own four named behaviours do not
  depend on rule selection, presentation flags or the new install flags.
  Waiting would keep users on the forwarding workaround for the whole of 3.6
  and 3.7, and the exit contract is the riskiest part, so it should be settled
  early. The alternative is strict ordering, which starts nothing until all
  four prerequisite items merge. It is equally coherent and changes no roadmap
  sequencing; it only delays delivery. This is a deviation from the roadmap's
  `Requires` ordering. Affected upstream identifiers: roadmap 3.5.2 and its
  `Requires` clause. If accepted, a one-line note is added to the 3.5.2 roadmap
  entry stating that the prerequisites gate `EP-M4` and `EP-M5`. Approving
  authority: the user. Date/Author: 2026-11-11, planning agent.

- Decision D-2: `check` adopts a dedicated exit contract, recorded in a new
  ADR, `docs/adrs/adr-0014-whitaker-check-exit-status-contract.md`:

  - When Dylint ran, its status is passed through unchanged.
  - `2` is a usage error, as for every command.
  - `3` is a configuration error.
  - `4` is a provisioning failure: the installation is incomplete and repair
    is not permitted, the repair failed or did not converge, or the
    installation state could not be read.
  - `5` is an execution failure: Dylint could not be started, or Whitaker
    could not write its own output.

  Codes `6` to `9` are reserved for future Whitaker operational outcomes.
  Rationale: the design requires install and configuration failures to be
  distinct from lint failures. Status `1` is taken by Dylint, and `2` is shared
  with clap usage errors in both Whitaker and `cargo dylint`. Small codes above
  `2` sit outside every status `cargo dylint` documents. One code per class of
  operator action — fix the configuration, provision, or investigate the host —
  lets CI branch without parsing text. `sysexits.h` codes (`64` to `78`) were
  considered and rejected: they are unfamiliar in the Rust tool ecosystem, and
  Windows has no equivalent convention. Date/Author: 2026-11-11, planning agent.

- Decision D-3: readiness is a query port, `ReadinessProbe`, separate from the
  `InstallService` command port. The repair decision is a pure function over
  the probe's result and a policy. The flow probes, decides, optionally repairs
  once, probes again, and decides again with repair forbidden. Rationale: this
  follows command–query separation and the ownership split in ADR 0012. Reusing
  one decision function for the second pass makes "at most one repair" a
  structural property rather than a counter to maintain. Date/Author:
  2026-11-11, planning agent.

- Decision D-4: a user-managed library path, meaning a non-empty
  `DYLINT_LIBRARY_PATH`, makes the bundle readiness `UserManaged`. The probe
  still checks the Dylint tools. It never reports a user-managed bundle as
  missing, so `check` never installs over it. Rationale: Constraint 5, and
  consistency with forwarding. Date/Author: 2026-11-11, planning agent.

- Decision D-5: the other commands keep their 3.5.1 exit policy. Whether
  `install` and `ls` adopt ADR 0014 codes is left to roadmap item 3.7.1 and the
  3.9 compatibility release. ADR 0014 records this as an outstanding decision.
  Rationale: Constraint 2, and changing them would alter a released contract
  outside this item's scope. Date/Author: 2026-11-11, planning agent.

## Outcomes & retrospective

Not started. Record outcomes at each milestone boundary.

## Context and orientation

The repository is a Cargo workspace. The parts this plan touches are described
below.

### The root `whitaker` package

`src/` is the root package, published as `whitaker`. Roadmap item 3.5.1 added
it.

- `src/main.rs` collects arguments, configures diagnostics, calls
  `cli::run_from`, and exits with `exit_code_for(&outcome)`.
- `src/cli/dispatch.rs` holds the pure router.
  `TokenTag` has eight tags: `Install`, `Ls`, `Help`, `Version`, `DoubleDash`,
  `GlobalFlag`, `DylintFlag` and `Other`. `Subcommand` is `Install` or `Ls`, and
  `Class` is `Subcommand(_)`, `Forward` or `Display`.
  `classify(&[TokenTag]) -> Class` skips global flags and lets the first other
  token decide. A `#[cfg(kani)] mod verification` module proves totality and
  disjointness, plus one reachability harness per class. Its `tag_for` maps
  `value & 7` onto the eight tags.
- `src/cli/parser.rs` holds the clap model. `Cli` has `quiet`, `verbosity` and
  `command`, and `Command` is `Install(InstallArgs)` or `Ls(ListArgs)`. Parsing
  is localized through `ortho_config`'s `LocalizedParse` with `NoOpLocalizer`.
- `src/cli/runner.rs` holds `run_from` (tokenize, classify, dispatch),
  `run_command`, `run_install`, `run_list`, `print_cli_error` (every `CliError`
  becomes `Outcome::OperationalFailure`) and `forward_arguments`.
- `src/cli/exit.rs` holds `Outcome` (`Success`, `Display`, `UsageError`,
  `OperationalFailure`, `Forwarded(i32)`) and `exit_code_for`.
- `src/ports.rs` holds `CliError` and the ports `InstallService`,
  `LintInventory` and `LintRunner`.
  `LintRunner::forward(args, stderr) -> Result<i32, CliError>` is the one
  method that runs Dylint. The `Services` struct groups the ports.
- `src/adapters/mod.rs` holds `SystemServices`, which implements the ports.
  Forwarding goes through the installed bootstrap when
  `bootstrap::is_installed` is true. Otherwise it runs `cargo dylint` directly.
  `child_exit_code` maps signals to `128 + signal`.
- `src/adapters/library_path/mod.rs` holds
  `active_library_path(dirs) -> Result<LibraryPath, CliError>`. It returns
  `Explicit`, `Resolved` or `NoCandidate`.
- `tests/` holds the root integration tests:
  - `e2e_exit_codes.rs` drives the real binary against a fake `cargo` script
    on a replaced `PATH`.
  - `behaviour_whitaker.rs` with `tests/features/whitaker_cli.feature` is the
    rstest-bdd suite. `BootstrapFixture` fakes an installed bootstrap.
  - The rest are `root_install_e2e.rs`, `cli_list_snapshot.rs` (insta) and
    `property_arg_parity.rs` (proptest).

### The installer library

`installer/` is published as `whitaker-installer`. It owns how installation is
done.

- `orchestration::run_install_for_whitaker(&InstallArgs, stdout, stderr)`
  in `whitaker_installer` is the full installation flow without wrapper
  scripts. The flow is: dependency tools, Clippy snapshot, workspace,
  toolchain, prebuilt download, then source build as a fallback.
- `deps::check_dylint_tools(&dyn CommandExecutor) -> Result<DylintToolStatus>`
  reports `cargo_dylint` and `dylint_link` presence at their pinned versions.
  It is a pure query.
- `toolchain::Toolchain::verify_installed` checks a toolchain through
  `rustup run`.
- `bootstrap::is_installed(&dyn BaseDirs) -> bool` and the bootstrap metadata
  (`toolchain`, `library_dir`) describe the installed launcher.
- `installation_record::read_installation_record` reads
  `installed_lint_paths.json`, which maps each toolchain to its staging
  directory.
- `InstallArgs` (`installer/src/cli/mod.rs`) has `skip_deps`, `quiet`,
  `toolchain`, `no_source_fallback` and `is_build_only`. It has no `offline`
  and no `build_from_source`; those arrive with 3.7.1.
- The `test-support` feature exposes `StubExecutor` for scripted subprocess
  calls.

### The configuration package

`cli/` is `whitaker-cli`, which is `publish = false`. It provides
`load_with_sources(&Sources) -> OrthoResult<LoadedConfiguration>` and
`ValidatedSettings` with `lint`, `install` (`lazy`, `prefer_prebuilt`,
`allow_build_from_source`), `diagnostics` and `output`. It already registers a
`check` command namespace (`[cmds.check]`, `WHITAKER_CMDS_CHECK_*`). Nothing in
the root package uses it yet.

### Verification tooling

- `scripts/run-kani.sh whitaker-cli` runs the root CLI harnesses. Its `*)`
  fallback silently routes unknown group names to another group, so always
  confirm that the harness names appear in the output.
- `./scripts/run-verus.sh cli` verifies `verus/whitaker_cli_dispatch.rs`.
  `make verus` runs every group.
- `make kani` runs every Kani group.

### The gates

`make check-fmt`, `make lint` and `make test` must pass at the end of every
milestone. Documentation changes also need `make markdownlint`, which runs
`make spelling`, and `make nixie` for Mermaid diagrams. Run every gate through
`tee` into
`/tmp/<action>-whitaker-3-5-2-whitaker-check-as-the-default-linting-path.out`.

## Conformance basis

Upstream artefacts, at the revisions in the working tree on 2026-11-11
(`origin/main` at `d9eb8a09`):

- Design: `docs/whitaker-cli-design.md`, §Public CLI surface, §Accessibility
  and localization requirements, §`whitaker check`, §Rule identifiers and
  selection model, §Configuration model and §`whitaker install`.
- ADR 0008 (`docs/adrs/adr-0008-ortho-config-cli-boundary.md`): configuration
  boundary.
- ADR 0012 (`docs/adrs/adr-0012-whitaker-cli-boundary.md`): root CLI boundary,
  `CLI-REQ-FWD`, and the lazy-repair ownership split.
- The 3.5.1 plan (`docs/execplans/3-5-1-root-whitaker-binary.md`): obligations
  `EP-INV-DISPATCH`, `EP-LEM-DISPATCH` and `EP-INV-EXIT`, which this plan
  amends.
- Roadmap: `docs/roadmap.md`, item 3.5.2.
- Standards: `AGENTS.md` and `docs/documentation-style-guide.md`.

No Terms of Reference document exists for the CLI. The design document is the
highest upstream artefact.

Traced requirements. The identifiers are introduced here because the design
document does not number its requirements:

- `CHK-REQ-ROUTE`: `whitaker check` is a Whitaker subcommand, not a forwarded
  invocation (§Public CLI surface).
- `CHK-REQ-LAZY`: missing Dylint dependencies or lint bundles are installed
  lazily, and then linting runs (§`whitaker check`, first paragraph; ADR 0012
  ownership split).
- `CHK-REQ-NOINSTALL`: `--no-install` disables lazy repair (§`whitaker check`
  help surface).
- `CHK-REQ-FWD`: arguments after `--` are forwarded to the underlying Cargo
  invocation (§`whitaker check`).
- `CHK-REQ-EXIT`: exit codes preserve lint-failure semantics, and install and
  configuration failures are distinct operational errors (§`whitaker check`).
- `CHK-REQ-STREAMS`: progress and diagnostics go to standard error, and
  machine-readable output is not polluted (§`whitaker install`; §Accessibility
  and localization requirements).
- `CHK-REQ-CONFIG`: `--config`, `install.lazy` and `[cmds.check]` take part in
  configuration merging (§Configuration model).
- `CHK-REQ-PRESENT`: `--locale`, `--colour`, `--progress`, `-q` and `-v` are
  accepted by `check` (§Accessibility and localization requirements).
- `CHK-REQ-SELECT`: `--select`, `--ignore`, `--extend-select` and
  `--experimental` determine which rules run (§Rule identifiers and selection
  model).
- `CHK-REQ-INSTALLFLAGS`: `--offline` and `--build-from-source` shape lazy
  repair (§`whitaker check` help surface; §`whitaker install`).

Trace chains:

```plaintext
CHK-REQ-ROUTE      -> EP-M1 -> EP-INV-DISPATCH', EP-LEM-DISPATCH' -> whitaker_check.feature "check is not forwarded"
CHK-REQ-FWD        -> EP-M1 -> CHK-LEM-ARGV, CHK-INV-ARGV -> property_check_argv, feature "forwarded cargo arguments"
CHK-REQ-EXIT       -> EP-M1 -> CHK-INV-EXIT, ADR 0014 -> e2e_check_exit_codes
CHK-REQ-LAZY       -> EP-M2 -> CHK-INV-REPAIR -> whitaker_check.feature "lazy repair", Kani verify_check_flow_*
CHK-REQ-NOINSTALL  -> EP-M2 -> CHK-INV-REPAIR -> whitaker_check.feature "--no-install refuses"
CHK-REQ-STREAMS    -> EP-M2 -> CHK-INV-STDOUT -> e2e_check_streams
CHK-REQ-CONFIG     -> EP-M4 -> CHK-INV-CONFIG -> whitaker_check.feature "install.lazy = false"
CHK-REQ-PRESENT    -> EP-M4 -> (consumed from 3.6.4) -> check help snapshot
CHK-REQ-SELECT     -> EP-M5 -> CHK-INV-SELECT-MAP -> property_check_selection
CHK-REQ-INSTALLFLAGS -> EP-M5 -> CHK-INV-REPAIR-REQUEST -> unit tests over repair request mapping
```

## Verification plan

This work introduces four new decision procedures. Each one fails silently when
wrong, which is why each needs more than examples:

1. Routing must recognize `check` without disturbing legacy forwarding.
2. The repair flow decides whether to install, at most once, and only when
   permitted.
3. The exit composition maps flow outcomes to statuses.
4. The argument assembly must preserve the user's Cargo arguments exactly.

Everything else is integration plumbing, verified by unit, behavioural and
end-to-end tests: adapters, help text, rendering of advice, and wiring of the
installer call.

### Axioms (assumed, not verified)

- `AX-DYLINT-EXIT`: `cargo dylint` exits `0` on success and `1` for any error,
  whether lint denial or internal failure. It exits `2` for its own clap usage
  errors and `101` on panic. Source: `trailofbits/dylint`,
  `cargo-dylint/src/main.rs`, whose `main` returns
  `dylint::ColorizedResult<()>`. `EP-M0` re-confirms this against the pinned
  version.
- `AX-DYLINT-LAST`: `cargo dylint` passes every argument after its first `--`
  to `cargo check`, in order (`#[clap(last = true)]`).
- `AX-CLAP-LAST`: clap 4.5 collects every token after the first `--` into a
  `last = true` or `trailing_var_arg` positional, in order, including later
  `--` tokens. Repository logic built on this is tested against real clap.
- `AX-OS-STATUS`: the operating system reports a child's status as an `i32`
  code, or as a signal on Unix. The existing `child_exit_code` mapping is
  retained.
- `AX-INSTALL-IDEMPOTENT`: `run_install_for_whitaker` run on a complete
  installation leaves it complete. It is relied on so that a repair cannot
  break a component that was already present.

### `EP-INV-DISPATCH'`: routing stays total and disjoint with `check` (amended, bounded)

- Obligation: for every token sequence, `classify` returns exactly one class.
  A sequence whose first non-global token is `Check` yields
  `Subcommand(Check)`. Every sequence that does not contain `Check` classifies
  exactly as before this change. That second clause is the regression guarantee
  for `CLI-REQ-FWD`.
- Method: Kani over the real `classify`, plus `rstest` cases.
- Rationale: the existing harness already establishes the pattern. Adding a
  tag must extend it, not fork it.
- Domain: nine tags with `tag_for` widened to `value % 9`, which keeps the
  mapping total, and sequences of length 0 to 6. That is
  `sum(9^k for k in 0..=6) = 597,871` sequences.
- Artefact: `src/cli/dispatch.rs`, `#[cfg(kani)] mod verification`. Add
  `verify_check_is_reachable`, and a `classify_before_check` oracle (a copy of
  the eight-tag rule) for the regression clause.
- Evidence: `./scripts/run-kani.sh whitaker-cli 2>&1 | tee
  /tmp/kani-whitaker-3-5-2-whitaker-check-as-the-default-linting-path.out`
  must show `VERIFICATION:- SUCCESSFUL` for
  `verify_classify_is_total_and_disjoint`, and a counterexample for every
  reachability harness, including `verify_check_is_reachable`.
- Non-vacuity: the reachability harnesses assert `false` and must fail. As a
  negative control, route `Check` to `Forward`. The totality harness must then
  produce a counterexample whose first significant tag is `Check`. Record the
  transcript, then restore.

### `EP-LEM-DISPATCH'`: unbounded classification with `check` (amended Verus proof)

- Obligation: for every finite token sequence of any length, exactly one of
  `is_subcommand`, `is_display` and `is_forward` holds, and the subcommand
  class now includes `Check`.
- Method: extend `verus/whitaker_cli_dispatch.rs`. Add `Check` to the
  `TokenTag` model and to the subcommand guard, and add a `Check` witness to
  `lemma_classes_are_inhabited`.
- Rationale: argv has no length bound, so Kani cannot establish this. The
  proof already exists, and leaving it stale would make it prove a property of
  code that no longer exists.
- Artefact: `verus/whitaker_cli_dispatch.rs`.
- Evidence: `timeout 600 ./scripts/run-verus.sh cli 2>&1 | tee
  /tmp/verus-whitaker-3-5-2-whitaker-check-as-the-default-linting-path.out`
  must end with `verification results:: N verified, 0 errors`.
- Non-vacuity: the witness lemma exhibits `[Check]` as a subcommand sequence
  before the disjointness lemma is stated. As a negative control, add `Check`
  to the forward guard as well. The disjointness ensures clause must fail.

### `CHK-INV-REPAIR`: the repair flow (new, exhaustive by Kani)

The abstract model comes first, independently of adapters.

```plaintext
Presence     = Ready | Missing | UserManaged          (UserManaged only for the bundle)
Readiness    = { tools: Presence, toolchain: Presence, bundle: Presence }
Policy       = { repair_allowed: bool }
Decision     = Run | Repair(MissingSet) | Refuse(MissingSet)

decide(r, p) =
  let m = { c | r.c = Missing }
  if m = {}               then Run
  else if p.repair_allowed then Repair(m)
  else                         Refuse(m)

flow(probe, repair, p) =
  r1 <- probe()                              -- Err => Failed(ProbeFailed)
  match decide(r1, p)
    Run        => Linted
    Refuse(m)  => Failed(Refused m)
    Repair(m)  => repair(m)                  -- Err => Failed(RepairFailed)
                  r2 <- probe()              -- Err => Failed(ProbeFailed)
                  match decide(r2, { repair_allowed: false })
                    Run       => Linted
                    Refuse(m') => Failed(NotConverged m')
```

Preconditions: the probe and repair results are arbitrary, so the model is
adversarial over adapter behaviour. Postconditions and invariants:

- `R1` (refusal honoured): if `repair_allowed` is false, `repair` is never
  invoked.
- `R2` (idempotence): if the first probe reports no `Missing` component, then
  `repair` is never invoked and the flow reaches `Linted`.
- `R3` (bounded repair): `repair` is invoked at most once per flow.
- `R4` (safety): the flow reaches `Linted` only if the most recent successful
  probe reported no `Missing` component. Dylint never runs against a readiness
  the flow has not observed as complete.
- `R5` (minimal request): when `repair` is invoked, its argument is exactly
  the set of `Missing` components from the first probe.
- `R6` (errors are not absence): a probe error ends the flow in
  `Failed(ProbeFailed)`. It is never treated as `Missing`, and it never leads to
  `repair` or `Linted`.
- `R7` (user-managed is untouchable): `UserManaged` never appears in a repair
  request.

The method is Kani over the real `decide` and the real `CheckFlow` driver, with
probe and repair as nondeterministic closures. The domain is finite: three
presences per component, the probe error flag on each probe, the repair error
flag, and both policies. Kani therefore explores it exhaustively; this is a
complete check, not a sampled one.

Verus is not used here. The state space is finite and small, so Kani's
exhaustive exploration of the real code is a stronger guarantee than a Verus
proof of a model, which would add a model–code gap for no gain in coverage.
This is the selective-use judgement the brief asks for, and it is recorded so
that reviewers can challenge it.

The supporting tests are:

- An `rstest` truth table over all 3 × 3 × 2 presence combinations (the
  bundle may also be `UserManaged`) and both policies for `decide`.
- A state-machine test of `CheckFlow` with handwritten fake ports that count
  calls.
- rstest-bdd scenarios for the user-visible paths.

The artefacts are `src/cli/check/repair.rs` (`decide`,
`#[cfg(kani)] mod verification`), `src/cli/check/flow.rs` (`CheckFlow`) and
their unit tests.

For evidence, the `scripts/run-kani.sh whitaker-cli` output must show the
harnesses `verify_check_flow_refusal_is_honoured`,
`verify_check_flow_repairs_at_most_once`,
`verify_check_flow_runs_only_when_ready` and
`verify_probe_errors_are_not_absence` all `SUCCESSFUL`.

Non-vacuity rests on three reachability harnesses, which must each produce a
counterexample: `Linted` after a repair, `Failed(NotConverged)`, and `Refuse`.
These prove the model reaches each material state, so the safety harnesses do
not pass vacuously. There are also two negative controls:

- Make the second `decide` use the caller's policy. `R3` must fail with a
  two-repair trace.
- Map a probe error to `Missing`. `R6` must fail.

### `CHK-INV-EXIT`: exit composition (new, exhaustive by Kani)

The obligation is stated over `check_exit_code(CheckOutcome) -> i32`:

- If the outcome is `Linted(code)`, meaning Dylint ran and returned `code`,
  the exit is `code`, for every `i32`.
- Otherwise the exit lies in the operational set `{3, 4, 5}`, or is `2` for a
  usage error.
- The operational set is disjoint from `{0, 1}`.
- Each failure class maps to exactly one code: configuration to `3`,
  provisioning (`Refused`, `RepairFailed`, `NotConverged`, `ProbeFailed`) to
  `4`, and execution to `5`.

The method combines Kani with a symbolic `i32`, an `rstest` partition, and an
end-to-end test against the real binary. The partition is finite apart from the
passed-through code, and Kani treats that `i32` symbolically, so the check is
exhaustive over the whole domain. The end-to-end test supplies what the unit
mapping cannot: evidence that `main` does not discard the mapping.

The artefacts are `src/cli/check/exit.rs` (or an extension of `src/cli/exit.rs`
with a `Check` outcome family), its Kani harness, and
`tests/e2e_check_exit_codes.rs`.

The end-to-end test uses a fake `cargo` script that exits with `0`, `1`, `23`,
`101`, and `300` (Windows only), and one that kills itself with `SIGTERM` (Unix
only). The `whitaker check` exit code must equal the expected value in each
case. A provisioning refusal must exit `4`, and a spawn failure (a `PATH` with
no `cargo`) must exit `5`.

For non-vacuity, a reachability harness must produce a counterexample for each
outcome class. As a negative control, map `ProbeFailed` to `1`. The
disjointness assertion must then fail.

### `CHK-LEM-ARGV` and `CHK-INV-ARGV`: forwarded Cargo arguments are preserved (new)

The abstract model:

```plaintext
assemble(W, U) = W ++ ["--"] ++ U       where W is Whitaker's Dylint options
first_dd(s)    = least i such that s[i] = "--"
cargo_args(s)  = s[first_dd(s) + 1 ..]   (AX-DYLINT-LAST)
```

The lemma `CHK-LEM-ARGV`: for every `W` that does not contain `"--"` and every
`U` of any length, including `U`s that themselves contain `"--"`:

- `first_dd(assemble(W, U)) = |W|`, and
- `cargo_args(assemble(W, U)) = U`.

The design pressure this exerts is that `W` must never contain `"--"`. The
implementation therefore builds `W` from a closed enum of Whitaker-owned Dylint
options, never from user text. That makes the precondition true by
construction, and a unit test asserts it for every variant.

The executable invariant `CHK-INV-ARGV` checks the real path. For every argv
`whitaker check [opts] -- U`, the argument vector handed to the `LintRunner`
port has `cargo_args(...) = U` byte for byte (`OsString`). The real path
consists of the clap parse (`AX-CLAP-LAST`), `CheckRequest`, and
`assemble_dylint_args`.

These need different methods:

- `CHK-LEM-ARGV` is a Verus proof over `Seq<Token>`. `U` is unbounded, and the
  "first occurrence" reasoning is exactly where an off-by-one or a stray
  inserted `--` would hide.
- `CHK-INV-ARGV` is a `proptest` over the real parser and assembler. That
  closes the model–code gap.

The `proptest` domain is generated `U` of length 0 to 16. Tokens are drawn from:
`--`, `-p`, `--all-targets`, `--message-format=json`, the empty string,
strings with spaces, non-ASCII strings, strings that start with `-`, and, on
Unix, non-UTF-8 `OsString`s. Generated Whitaker options come from the `check`
flag set.

The artefacts are `verus/whitaker_check_argv.rs` (added to the `cli` group in
`scripts/run-verus.sh` and to its `case` allowlist), `src/cli/check/argv.rs`,
and `tests/property_check_argv.rs`.

Evidence:

- `./scripts/run-verus.sh cli` verifies with 0 errors.
- `cargo nextest run -p whitaker --test property_check_argv` passes at 4,096
  cases.

Non-vacuity:

- The Verus witness lemma exhibits a `U` containing `"--"` and proves
  `cargo_args` returns it whole. This is the interesting case: a model that
  splits at the last `--` passes for `U`s without `--` and fails here.
- The `proptest` classifies cases and fails the run unless at least 10% of
  them contain an inner `--`, at least 10% have an empty `U`, and at least 10%
  start with a dash.
- As a negative control, insert `--` into `W` when `--no-install` is given.
  Both the proof precondition check and the property must fail.

### `CHK-INV-STDOUT`: standard output purity (new)

The obligation: during `check`, every byte on Whitaker's standard output came
from the Dylint child. That covers repair output, the `suite-source` marker,
advice and progress.

The method is two layers of tests:

- A unit test of the repair adapter that hands it a stdout writer which fails
  on any write.
- An end-to-end test that runs `whitaker check -- --message-format=json`
  against a fake `cargo` emitting one JSON line and a fake installer state that
  forces a repair. It asserts that standard output parses as JSON lines and
  contains nothing else.

These are integration properties of real streams, so tests are the right tool.
There is no algebra to prove.

The artefacts are `tests/e2e_check_streams.rs` and the unit test beside the
repair adapter.

For non-vacuity, the fake state must actually trigger a repair. The test
asserts that standard error contains the repair notice. As a negative control,
pass the real stdout to `InstallService::install`. The end-to-end test must
then fail on the marker line.

### `CHK-INV-CONFIG`, `CHK-INV-SELECT-MAP` and `CHK-INV-REPAIR-REQUEST` (phase two)

These obligations are stated now so that the phase-two interfaces are designed
against them. Their methods are fixed once the prerequisite interfaces exist.

- `CHK-INV-CONFIG`: the effective `repair_allowed` is
  `!no_install && install.lazy`, after `ortho_config` merging. `--no-install`
  always wins over configuration. A configuration error maps to exit `3` and
  never runs Dylint. The method is an `rstest` truth table plus the BDD
  scenario.
- `CHK-INV-SELECT-MAP`: given the installed lint catalogue `C` and the
  resolved rule set `S ⊆ C` from 3.6.2, the Dylint invocation enables exactly
  `S` and disables exactly `C \ S`. Any `DYLINT_RUSTFLAGS` the user set are
  preserved as a prefix. The method is a `proptest` over generated subsets of a
  generated catalogue, with Kani over the real eight-lint catalogue (256
  subsets, exhaustive). Verus is not warranted: the mapping is a filter, and
  set complement over a finite catalogue gains nothing from an unbounded proof.
- `CHK-INV-REPAIR-REQUEST`: `--offline` and `--build-from-source` map one to
  one onto the 3.7.1 install request. A repair under `--offline` never performs
  network access. That last part is verified by the installer's own 3.7.1
  tests, and this plan only tests the mapping.

### Not verified, deliberately

- The internals of `cargo-dylint`, clap, `ortho_config`, rustup and the
  installer orchestration. These are covered by axioms or by their own suites.
- Wording of help and advice text. That is snapshot-tested only, because
  wording is not an invariant.
- Localization catalogues, which arrive with 3.6.4.

### Relationship between proofs and tests

The Verus proofs establish two semantic facts about models:

- Routing is total and disjoint for argv of any length.
- The assembly of Dylint arguments recovers the user's Cargo arguments for
  argv of any length.

Kani establishes that the real `classify`, `decide`, `CheckFlow` and exit
mapping satisfy their invariants exhaustively over their finite domains.

Property tests close the gap between the argv model and the real parser.
Behavioural and end-to-end tests establish externally observable behaviour that
no proof covers: real process exit statuses, real streams, and the real
installer call. None of the proofs replaces a test, and none of the tests is
presented as exhaustive.

## Plan of work

### Stage A: prototype and settle Dylint invocation (`EP-M0`)

This stage is prototyping. It changes no production code. Its output is
evidence recorded in `Surprises & discoveries` and a decision recorded in
`Decision log`.

Run each experiment against the `cargo-dylint` version pinned in the
installer's dependency manifest, using a scratch crate under `/tmp` and a real
installed Whitaker suite. The hypotheses are:

- H-1: with `DYLINT_LIBRARY_PATH` narrowed to a single staging directory,
  `cargo dylint --lib whitaker_suite -- <cargo args>` loads exactly the
  Whitaker suite. It ignores user workspace metadata libraries only when
  `--no-metadata` is also given.
- H-2: with two toolchains' `whitaker_suite` libraries on the library path,
  `--lib whitaker_suite` fails with an ambiguity error. That is why `check`
  must narrow the path to one bundle.
- H-3: `cargo dylint` exits `1` for a deny-level lint and `1` for a missing
  library. With `--fail-on-no-libraries`, an empty library path also exits `1`.
- H-4: lint levels can be set per lint through `DYLINT_RUSTFLAGS`
  (`-A module_max_lines`) without an `unknown_lints` warning when the suite is
  loaded. This is needed only for `EP-M5`, but it is cheap to settle now.
- H-5: the installer serializes, or does not serialize, two concurrent
  `run_install_for_whitaker` calls in one home directory.
- H-6: the readiness probe costs less than 500 ms on a warm machine, measured
  with `hyperfine 'whitaker-probe-prototype'`. The prototype probe is a
  throwaway test binary under `/tmp`, not a repository file.

There is a go/no-go point. If H-1 or H-2 is falsified, revise the invocation
design and the `assemble` model before `EP-M1`. If H-6 is falsified, escalate
as described in `Risks`.

### Stage B: red tests and specifications

Write these before any production code, and confirm that each fails for the
expected reason:

1. `tests/features/whitaker_check.feature`, reproduced in full under
   `Artefacts and notes`, with step bindings in
   `tests/behaviour_whitaker_check.rs` and support in
   `tests/behaviour_whitaker_check/`. Red: every scenario fails, because
   `check` is forwarded to the fake `cargo dylint` as `check`, or because the
   step reports an unexpected exit code.
2. `tests/e2e_check_exit_codes.rs`. Red: `whitaker check` with a fake cargo
   exiting `23` exits `23` only by accident of forwarding, so the provisioning
   case (exit `4`) fails.
3. `tests/property_check_argv.rs`. Red: it does not compile until
   `assemble_dylint_args` exists. To make the red stage observable, write the
   test against a `todo!()`-bodied function. It then fails at run time with
   `not yet implemented`, which is the expected reason.
4. The Kani harness additions in `src/cli/dispatch.rs`. Red: the regression
   oracle passes and `verify_check_is_reachable` reports no counterexample for
   `Subcommand(Check)`, which does not exist yet. Record the transcript.
5. Verus model updates. Red: adding the `Check` witness before the guard
   change fails to verify.

### Stage C: implementation

The modules are new unless noted. Each stays well under 400 lines.

1. `src/cli/dispatch.rs`: add `TokenTag::Check` and `Subcommand::Check`, and
   let `classify` route `Check`. Update the Kani `tag_for` to `value % 9` and
   add the regression oracle. `src/cli/runner.rs`: map the literal `check` in
   `token_tag`.
2. `src/cli/parser.rs`: add `Command::Check(CheckArgs)`. Define `CheckArgs` in
   a new `src/cli/check/args.rs` with `no_install: bool` and
   `cargo_args: Vec<OsString>` (`last = true`). In phase one, `CheckArgs`
   carries only these fields.
3. `src/cli/check/argv.rs`: `enum DylintOption`, the closed set of
   Whitaker-owned Dylint options settled in `EP-M0` (for example
   `Lib(&'static str)` and `NoMetadata`), and
   `assemble_dylint_args(&[DylintOption], &[OsString]) -> Vec<OsString>`.
4. `src/cli/check/repair.rs`: `Component`, `Presence`, `Readiness`, `Policy`,
   `Decision`, and `decide`. These are pure and `Copy`, with a `MissingSet`
   bitset newtype. Add the Kani harnesses.
5. `src/cli/check/flow.rs`: `CheckFlow`, which drives probe, decide, repair,
   probe, decide, run over the ports and returns `CheckOutcome`.
6. `src/cli/check/exit.rs`: `CheckOutcome` and `check_exit_code`, with Kani
   harnesses. Extend `src/cli/exit.rs` `Outcome` with one variant,
   `Check(CheckOutcome)`, so that `main` keeps a single exit path.
7. `src/ports.rs`: add the following.
   - `trait ReadinessProbe { fn probe(&self, request: &ReadinessRequest) ->
     Result<Readiness, CliError>; }`
   - `LintRunner::run_check(&self, invocation: &CheckInvocation, stderr: &mut
     dyn Write) -> Result<i32, CliError>`. `CheckInvocation` carries the
     assembled args and the narrowed library path.
   - New `CliError` variants: `Probe { source }`, `RepairRefused { missing }`
     and `RepairNotConverged { missing }`.
   - A `readiness` field on `Services`.
8. `src/adapters/readiness/mod.rs`: `SystemServices` implements
   `ReadinessProbe` by composing `deps::check_dylint_tools`, the bootstrap
   metadata and library-path resolution (reusing
   `library_path::active_library_path`), and `Toolchain::verify_installed`. If
   the installer lacks a needed pure query, add one as a new public function.
   This is in tolerance.
9. `src/adapters/check_runner/mod.rs`: implements `run_check` through the
   bootstrap when it is installed, and directly otherwise, mirroring `forward`.
   It reuses `child_exit_code`.
10. `src/cli/check/mod.rs`: `run_check_command(&CheckArgs, &Services,
    &mut CommandStreams) -> Outcome`. It wires parsing to the flow, routes
    repair output to standard error (Constraint 4), and renders advice for
    refusals.

`EP-M2` adds the repair call. `CheckFlow` gets a `repair` step that builds an
`InstallRequest` from the missing set (`skip_deps = !missing.contains(Tools)`,
`quiet` from verbosity) and calls
`InstallService::install(request, &mut stderr_as_stdout, stderr)`.

### Stage D: refactor, documentation and wider validation

- Re-read the touched modules against
  `docs/complexity-antipatterns-and-refactoring-strategies.md`. The suite lints
  this repository. Keep `decide` flat, using a predicate per branch, and keep
  `CheckFlow` free of nested conditionals by matching on `Decision`.
- Write ADR 0014 and update the design document, the users' guide, the
  developers' guide, `docs/contents.md` and `docs/repository-layout.md` (see
  `EP-M3`).

## Milestones and plateaus

### `EP-M0`: Dylint invocation semantics settled (prototype)

- Outcome: hypotheses H-1 to H-6 are each recorded as falsified or not
  falsified, with transcripts. The `DylintOption` set and the library-path
  narrowing rule are decided. No repository files change apart from this plan.
- Requirements: de-risks `CHK-REQ-FWD`, `CHK-REQ-EXIT` and `CHK-REQ-LAZY`.
- Acceptance evidence: a `Surprises & discoveries` entry per hypothesis and a
  `Decision log` entry for the invocation shape.
- Conformance check:
  - No requirement is discharged yet.
  - The design is unchanged.
  - Axiom `AX-DYLINT-EXIT` is re-confirmed or corrected.
  - There is no interface or dependency change.
- Recovery: the prototype is throwaway. Delete the `/tmp` scratch crate.
- Remaining gaps: everything else.
- Compatibility decision: none.

### `EP-M1`: `whitaker check` on a provisioned installation

- Outcome: `whitaker check [-- args]` runs the Whitaker suite through Dylint
  when readiness is complete, passes Dylint's status through, and forwards
  Cargo arguments exactly. When anything is missing, it refuses with exit `4`
  and advice to run `whitaker install`. That refusal is the behaviour
  `--no-install` will name explicitly in `EP-M2`. ADR 0014 is written.
- Requirements: `CHK-REQ-ROUTE`, `CHK-REQ-FWD` and `CHK-REQ-EXIT` are
  discharged. `CHK-REQ-NOINSTALL` is partly discharged: the refusal path exists.
- Acceptance evidence:
  - The feature scenarios "check is not forwarded", "forwarded cargo
    arguments", "Dylint status passes through" and "an unprovisioned machine
    is refused" pass.
  - `e2e_check_exit_codes` passes.
  - `property_check_argv` passes.
  - Kani for `whitaker-cli` and `./scripts/run-verus.sh cli` pass.
  - `make check-fmt`, `make lint` and `make test` pass.
- Conformance check:
  - The routing, argument and exit requirements are satisfied.
  - The ADR 0012 ownership split is preserved: no installation happens.
  - `CLI-REQ-FWD` is preserved by the Kani regression oracle.
  - The new public items in the root library (`CheckArgs`, `ReadinessProbe`)
    are application-internal and pre-1.0.
  - ADR 0014 records the new public exit contract.
  - The trace links are current.
- Recovery: each step is a separate commit. Reverting the milestone's commits
  restores forwarding of `check`.
- Remaining gaps: lazy repair, `--no-install` as a flag, and phase two.
- Compatibility decision: none. `whitaker check` previously forwarded to an
  invocation that `cargo dylint` rejects, so no working behaviour is lost.

### `EP-M2`: lazy repair and `--no-install`

- Outcome:
  - On an unprovisioned machine, `whitaker check` installs, re-probes, and
    lints. A second run installs nothing.
  - `--no-install` refuses with exit `4`.
  - A failed repair exits `4` with the installer's error and advice.
  - Standard output stays pure.
- Requirements: `CHK-REQ-LAZY`, `CHK-REQ-NOINSTALL` and `CHK-REQ-STREAMS` are
  discharged.
- Acceptance evidence:
  - The feature scenarios "lazy repair", "repair is idempotent",
    "--no-install refuses", "a failed repair is an operational error" and
    "repair output stays off stdout" pass.
  - The Kani `CHK-INV-REPAIR` harnesses and their negative controls are
    recorded.
  - `e2e_check_streams` passes.
  - All gates pass.
- Conformance check:
  - The repair goes through `InstallService` only (Constraint 3).
  - A user-managed path is never repaired (Constraint 5).
  - There is no new dependency.
  - No persisted format changes: only the installer writes records, as before.
- Recovery: revert the milestone's commits. `EP-M1` behaviour remains.
- Remaining gaps: phase two.
- Compatibility decision: none.

### `EP-M3`: documentation plateau

- Outcome: users and maintainers can discover and reason about `check`.
- Changes:
  - `docs/users-guide.md`: update "Root CLI" with `check`, `--no-install`,
    forwarded arguments and the exit-code table. Make `whitaker check` the
    first command in "Quick Setup".
  - `docs/whitaker-cli-design.md`: update the 3.5.1 status table. Under
    `whitaker check`, add the exit contract with a reference to ADR 0014 and
    the readiness and lazy-repair design.
  - `docs/developers-guide.md`, "Root `whitaker` CLI boundary": add the
    `ReadinessProbe` port and its re-use policy (only `check` and, later,
    `doctor` consume it; adapters compose installer queries and never
    install), the `CheckFlow` driver, and the Kani and Verus artefacts.
  - `docs/adrs/adr-0012-whitaker-cli-boundary.md`: add a note that lazy repair
    is now implemented, linking ADR 0014.
  - `docs/contents.md`: add entries for ADR 0014 and this plan.
  - `docs/repository-layout.md`: add `src/cli/check/` and the new `verus/`
    file.
- Acceptance evidence: `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint` and `make test` pass.
- Conformance check: the documentation matches the behaviour, and the
  staged-status table says what is and is not yet delivered.
- Recovery: documentation-only commits.
- Remaining gaps: phase two. The roadmap item stays unticked.
- Compatibility decision: none.

### `EP-M4`: configuration and presentation (phase two, gated)

- Entry gate: roadmap 3.6.4 has merged. It delivers the shared `--locale`,
  `--colour` and `--progress` flags and, if it does so, configuration loading
  in the root package. If configuration is not yet wired into the root package,
  stop and escalate (see `Tolerances`, Packaging).
- Outcome:
  - `--config`, `install.lazy`, `[cmds.check]` and the shared presentation
    flags apply to `check`.
  - `install.lazy = false` behaves like `--no-install`.
  - A configuration error exits `3`.
- Requirements: `CHK-REQ-CONFIG` and `CHK-REQ-PRESENT` are discharged.
- Acceptance evidence: the `CHK-INV-CONFIG` truth table, the feature scenario
  "install.lazy = false refuses", the `check --help` insta snapshot, and all
  gates.
- Conformance check: ADR 0008 precedence holds, with CLI over environment over
  file.
- Recovery: revert the milestone's commits.
- Remaining gaps: selection and install flags.
- Compatibility decision: none.

### `EP-M5`: rule selection and install behaviour flags (phase two, gated)

- Entry gate: roadmap items 3.6.2 (and therefore 3.6.1) and 3.7.1 have merged.
- Outcome:
  - `--select`, `--ignore`, `--extend-select` and `--experimental` determine
    the lints Dylint enables.
  - The readiness bundle set includes the experimental bundle when an
    experimental rule is selected.
  - `--offline` and `--build-from-source` shape the repair request.
  - The full `whitaker check --help` surface from the design is present.
  - Roadmap item 3.5.2 is marked done.
- Requirements: `CHK-REQ-SELECT` and `CHK-REQ-INSTALLFLAGS` are discharged,
  which completes 3.5.2.
- Acceptance evidence: `CHK-INV-SELECT-MAP` and `CHK-INV-REPAIR-REQUEST`
  evidence, the updated help snapshot, and all gates plus Kani and Verus.
- Conformance check:
  - Every `CHK-REQ-*` item is discharged.
  - `docs/roadmap.md` 3.5.2 is ticked.
  - Every upstream discovery is reconciled.
- Recovery: revert the milestone's commits.
- Remaining gaps: none for 3.5.2. Failure recording for `doctor` is roadmap
  3.8.3.
- Compatibility decision: none.

## Interfaces and dependencies

There are no new dependencies. The required phase-one signatures, in the root
package, are as follows. Names may be refined during implementation, provided
the `Decision log` records it.

```rust
// src/cli/check/args.rs
#[derive(Debug, Clone, PartialEq, Eq, clap::Args)]
pub struct CheckArgs {
    /// Refuse to install missing dependencies or lint bundles.
    #[arg(long)]
    pub no_install: bool,
    /// Arguments passed unchanged to `cargo check`.
    #[arg(last = true, value_name = "CARGO_ARGS")]
    pub cargo_args: Vec<std::ffi::OsString>,
}

// src/cli/check/repair.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component { Tools, Toolchain, Bundle }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence { Ready, Missing, UserManaged }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Readiness { pub tools: Presence, pub toolchain: Presence, pub bundle: Presence }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MissingSet(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy { pub repair_allowed: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision { Run, Repair(MissingSet), Refuse(MissingSet) }

#[must_use]
pub fn decide(readiness: Readiness, policy: Policy) -> Decision;

// src/cli/check/exit.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Linted(i32),
    ConfigurationFailed,
    Refused(MissingSet),
    RepairFailed,
    NotConverged(MissingSet),
    ProbeFailed,
    ExecutionFailed,
}

#[must_use]
pub const fn check_exit_code(outcome: &CheckOutcome) -> i32;

// src/ports.rs (additions)
pub trait ReadinessProbe {
    /// Reports which components `check` needs and whether each is present.
    ///
    /// # Errors
    ///
    /// Returns an error when presence cannot be established; absence is
    /// reported as `Presence::Missing`, never as an error.
    fn probe(&self, request: &ReadinessRequest) -> Result<Readiness, CliError>;
}
```

`ReadinessRequest` carries the narrowed library-path policy decided in `EP-M0`.
`CheckInvocation` carries the assembled `Vec<OsString>` and the library path.
Both are plain data structs in `src/ports.rs`.

## Concrete steps

Run every command from the worktree root. Use the shared Cargo cache. Expect
compile admission queueing (`[build-limits] ... waiting for a compile slot`),
which is normal and must not be interrupted.

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-whitaker-${BRANCH}.out"
make lint      2>&1 | tee "/tmp/lint-whitaker-${BRANCH}.out"
make test      2>&1 | tee "/tmp/test-whitaker-${BRANCH}.out"
./scripts/run-kani.sh whitaker-cli 2>&1 | tee "/tmp/kani-whitaker-${BRANCH}.out"
timeout 600 ./scripts/run-verus.sh cli 2>&1 | tee "/tmp/verus-whitaker-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-whitaker-${BRANCH}.out"
make nixie        2>&1 | tee "/tmp/nixie-whitaker-${BRANCH}.out"
```

Focused loops during Red-Green-Refactor:

```bash
cargo nextest run -p whitaker --test behaviour_whitaker_check
cargo nextest run -p whitaker --test e2e_check_exit_codes
cargo nextest run -p whitaker --test property_check_argv
cargo nextest run -p whitaker --lib check
```

Expected Kani tail for the `whitaker-cli` group (abridged):

```plaintext
Checking harness verification::verify_classify_is_total_and_disjoint...
VERIFICATION:- SUCCESSFUL
Checking harness verification::verify_check_is_reachable...
VERIFICATION:- FAILED   (expected: reachability witness)
```

Expected Verus tail:

```plaintext
verification results:: N verified, 0 errors
```

Gate each commit with `make check-fmt`, `make lint` and `make test`, plus Kani
and Verus for commits touching `dispatch.rs`, `src/cli/check/` or `verus/`.
Commit after each green step. Delegate full gate runs to the `scrutineer` agent
where available.

## Validation and acceptance

### Behaviour to observe (phase one)

Run these in a scratch crate with an isolated `HOME`, `XDG_DATA_HOME` and
`CARGO_HOME` copy:

```console
$ whitaker check --no-install; echo "status=$?"
error: Whitaker is not installed for this machine: missing dylint tools, lint bundle
help: run `whitaker install`, or rerun without `--no-install`
status=4

$ whitaker check; echo "status=$?"
[stderr] Whitaker lint bundle missing; installing (this happens once)...
[stderr] ...installer progress...
[cargo dylint output]
status=0          # or Dylint's own status, unchanged

$ whitaker check -- --message-format=json | jq -c .reason | sort -u
"build-finished"
"compiler-message"
...
```

The exact wording is settled during implementation and snapshot-tested. The
status values and the stream separation are the contract.

### Red-Green-Refactor evidence to record

For each of `EP-M1` and `EP-M2`, record the red transcript of the feature and
end-to-end tests, the green transcript, and the Kani and Verus negative-control
transcripts in `Outcomes & retrospective`.

### Quality criteria

- `make check-fmt`, `make lint` and `make test` all succeed at each milestone.
- `./scripts/run-kani.sh whitaker-cli` and `./scripts/run-verus.sh cli` succeed
  at `EP-M1`, `EP-M2` and `EP-M5`.
- `make markdownlint` and `make nixie` succeed at `EP-M3`.

## Idempotence and recovery

Every step is re-runnable. Tests use temporary directories and spawned-child
environments only. If a gate fails part-way, fix the cause and re-run that
gate. Do not re-run unrelated gates. Milestones are separate commits, or series
of commits, on this branch, so `git revert` of a milestone's range is the
rollback. The prototype in `EP-M0` writes only under `/tmp`.

## Artefacts and notes

### Feature specification (`tests/features/whitaker_check.feature`)

```gherkin
Feature: Running Whitaker lints with whitaker check

  Scenario: check is a Whitaker command, not a forwarded invocation
    Given the whitaker binary is available
    When I run whitaker with "check --help"
    Then the command succeeds
    And the output describes the check subcommand

  Scenario: Dylint status passes through unchanged
    Given a provisioned Whitaker installation
    And a fake cargo dylint command returning exit code 23
    When I run whitaker with "check"
    Then the exit code is 23

  Scenario: Forwarded cargo arguments reach cargo check unchanged
    Given a provisioned Whitaker installation
    And a fake cargo dylint command that records its arguments
    When I run whitaker with "check -- -p my-crate -- --nested"
    Then cargo check receives exactly "-p my-crate -- --nested"

  Scenario: An unprovisioned machine is refused under --no-install
    Given an empty Whitaker data directory
    When I run whitaker with "check --no-install"
    Then the exit code is 4
    And stderr advises running "whitaker install"
    And no installation was attempted

  Scenario: Missing components are repaired lazily before linting
    Given an empty Whitaker data directory
    And a fake installer that provisions successfully
    And a fake cargo dylint command returning exit code 0
    When I run whitaker with "check"
    Then an installation was attempted once
    And the invocation reaches cargo dylint
    And the exit code is 0

  Scenario: A complete installation is not repaired again
    Given a provisioned Whitaker installation
    And a fake cargo dylint command returning exit code 0
    When I run whitaker with "check"
    Then no installation was attempted

  Scenario: A failed repair is an operational error
    Given an empty Whitaker data directory
    And a fake installer that fails
    When I run whitaker with "check"
    Then the exit code is 4
    And cargo dylint was not invoked

  Scenario: Repair output stays off standard output
    Given an empty Whitaker data directory
    And a fake installer that provisions successfully
    And a fake cargo dylint command that prints one JSON line
    When I run whitaker with "check -- --message-format=json"
    Then stdout contains only the JSON line
    And stderr reports the repair

  Scenario: A user-managed library path is never repaired
    Given a user-provided DYLINT_LIBRARY_PATH
    And a fake cargo dylint command returning exit code 0
    When I run whitaker with "check"
    Then no installation was attempted
    And the exit code is 0

  Scenario: Legacy forwarding is unchanged
    Given the whitaker binary is available
    And a fake cargo dylint command returning exit code 23
    When I run whitaker with "--all -- -p whitaker-common"
    Then the invocation reaches cargo dylint
    And the exit code is 23
```

Phase two adds "install.lazy = false refuses", "a configuration error exits 3"
and selection scenarios. Those are written at `EP-M4` and `EP-M5` against the
delivered prerequisite interfaces.

The "fake installer" is not a fake `whitaker-installer` binary. It is a
pre-staged prebuilt archive served from a local `file://` fixture, or the
existing `root_install_fixture` stub-cargo approach. The choice is made in
`EP-M2`, reusing `tests/root_install_fixture/` where possible.

### Verus proof skeleton (`verus/whitaker_check_argv.rs`)

The witness comes first. Statements only; the proof bodies are written at
`EP-M1`.

```rust
use vstd::prelude::*;

verus! {

pub enum Token { DoubleDash, Word(int) }

pub open spec fn has_no_dd(s: Seq<Token>) -> bool {
    forall|i: int| 0 <= i < s.len() ==> s[i] != Token::DoubleDash
}

pub open spec fn assemble(w: Seq<Token>, u: Seq<Token>) -> Seq<Token> {
    w + seq![Token::DoubleDash] + u
}

/// Index of the first `--`; the caller guarantees one exists.
pub open spec fn first_dd(s: Seq<Token>) -> int;

pub open spec fn cargo_args(s: Seq<Token>) -> Seq<Token> {
    s.subrange(first_dd(s) + 1, s.len() as int)
}

/// Witness: a user suffix that itself contains `--` survives intact.
proof fn lemma_inner_double_dash_is_preserved()
    ensures
        cargo_args(assemble(seq![Token::Word(1)],
                            seq![Token::Word(2), Token::DoubleDash, Token::Word(3)]))
            == seq![Token::Word(2), Token::DoubleDash, Token::Word(3)];

/// Whitaker's options never contain `--`, so cargo receives the suffix whole.
proof fn lemma_cargo_args_recover_user_suffix(w: Seq<Token>, u: Seq<Token>)
    requires has_no_dd(w),
    ensures
        first_dd(assemble(w, u)) == w.len(),
        cargo_args(assemble(w, u)) == u;

} // verus!
```

`first_dd` is defined recursively at `EP-M1`. The proof of
`lemma_cargo_args_recover_user_suffix` is expected to need an auxiliary lemma
relating `first_dd` of a concatenation to its prefix. That is a hypothesis
about the proof structure, deferred until the verification conditions are
visible.

## Signposts

Read these before starting:

- `docs/whitaker-cli-design.md`: the specification. §Public CLI surface and
  §`whitaker check` are normative.
- `docs/roadmap.md`: items 3.5.2, 3.6.x, 3.7.x and 3.8.3, which set what
  belongs here and what does not.
- `docs/adrs/adr-0012-whitaker-cli-boundary.md`: `CLI-REQ-FWD` and the
  lazy-repair ownership split.
- `docs/adrs/adr-0008-ortho-config-cli-boundary.md`: the configuration
  boundary for `EP-M4`.
- `docs/execplans/3-5-1-root-whitaker-binary.md`: the obligations amended
  here, and lessons about Kani group routing and Verus run time.
- `docs/developers-guide.md`: root CLI boundary, Kani conventions, and the
  Verus trust boundary.
- `docs/users-guide.md`: "Root CLI" and "Planned Whitaker CLI configuration".
- `docs/whitaker-dylint-suite-design.md`: the suite's lint registry and
  experimental features, which matter for `EP-M5` bundle selection.
- `docs/whitaker-clone-detector-design.md`: the naming conventions for new
  crates and modules, so new names do not collide.
- `docs/rust-testing-with-rstest-fixtures.md`: fixture patterns for the
  `CheckWorld` BDD fixture.
- `docs/rstest-bdd-users-guide.md`: `#[scenario]` bindings and step
  functions.
- `docs/rust-doctest-dry-guide.md`: doctests on the new public items.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`: keeping
  `decide` and `CheckFlow` flat. The suite lints this repository.
- `docs/documentation-style-guide.md`: the ADR template, en-GB-oxendict
  spelling, and wrapping.
- `AGENTS.md`: gates, commit rules, the 400-line limit, and the
  test-environment rules.

Skills:

- `leta` for symbol navigation.
- `hexagonal-architecture` for the new port.
- `rust-router`, then `rust-errors` for `CliError` variants.
- `domain-cli-and-daemons` for process exit and stream behaviour.
- `rust-unit-testing` for the `rstest`, `googletest`, `pretty_assertions` and
  `insta` style.
- `kani` for the harnesses, `verus` for the proofs, and `proptest` for
  `CHK-INV-ARGV`.
- `arch-decision-records` for ADR 0014.
- `execplans` for keeping this document current.

## Revision note

- 2026-11-11: initial draft written from reconnaissance and external research.
  It awaits expert review and user approval of decision D-1.
